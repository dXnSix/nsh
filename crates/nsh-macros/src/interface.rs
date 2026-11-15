//! `#[interface]`: a Rust trait TypeScript objects implement. It declares a
//! TS `export interface` of the trait's name (a method per trait method,
//! optional where the trait has a default body) and makes the trait's
//! stand-in, `Js<Trait>`: a struct over a `JsHandle` that implements the
//! trait by calling the JS object's members.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    FnArg, Ident, ItemTrait, Pat, ReturnType, TraitItem, TraitItemFn, Type, spanned::Spanned,
};

use crate::{camel, decl, docs};

/// A method of the trait.
struct Method<'a> {
    f: &'a TraitItemFn,
    /// `#[getter]`: a property JS reads.
    getter: bool,
    /// The parameters after `&self`: their names and types.
    params: Vec<(&'a Ident, &'a Type)>,
    /// What it returns (`()` when nothing).
    ret: Type,
}

pub(crate) fn interface(item: ItemTrait) -> syn::Result<TokenStream2> {
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            item.generics.span(),
            "an interface has no generic parameters",
        ));
    }
    if !item.supertraits.is_empty() {
        return Err(syn::Error::new(
            item.supertraits.span(),
            "an interface has no supertraits: its stand-in implements this trait only (declare \
             the members here)",
        ));
    }
    if let Some(unsafety) = &item.unsafety {
        return Err(syn::Error::new(
            unsafety.span(),
            "an interface is not an `unsafe trait`",
        ));
    }
    if let Some(auto) = &item.auto_token {
        return Err(syn::Error::new(
            auto.span(),
            "an interface is no auto trait",
        ));
    }
    let mut methods = Vec::new();
    for member in &item.items {
        let TraitItem::Fn(f) = member else {
            return Err(syn::Error::new(
                member.span(),
                "an interface has methods only (no types, constants or macros)",
            ));
        };
        methods.push(method(f)?);
    }
    let nsh_core = crate::root::nsh_core();
    let ui = quote!(#nsh_core::element);
    let private = quote!(#ui::__private);
    let name = &item.ident;
    let ts_name = name.to_string();
    let vis = &item.vis;
    let stand_in = format_ident!("Js{}", name);
    let doc = decl(&docs(&item.attrs));
    let stand_in_doc = format!(
        " The JS object standing in for a [`{name}`]: a TS object implementing the interface \
         `{name}`, whose members its methods call (`#[interface]`)."
    );

    let mut trait_items = Vec::new();
    let mut members = Vec::new();
    let mut impls = Vec::new();
    for (index, m) in methods.iter().enumerate() {
        let f = m.f;
        let ident = &f.sig.ident;
        let optional = f.default.is_some();
        let is_async = f.sig.asyncness.is_some();
        let cfgs: Vec<_> = f
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("cfg"))
            .collect();
        let attrs: Vec<_> = f
            .attrs
            .iter()
            .filter(|a| !a.path().is_ident("getter"))
            .collect();
        let names: Vec<&Ident> = m.params.iter().map(|(n, _)| *n).collect();
        let tys: Vec<&Type> = m.params.iter().map(|(_, t)| *t).collect();
        let ret = &m.ret;
        // The arguments as one value: nothing, the one, or a tuple.
        let (args_ty, args, args_pat) = match tys.len() {
            0 => (quote!(()), quote!(()), quote!(_)),
            1 => (quote!(#(#tys)*), quote!(#(#names)*), quote!(#(#names)*)),
            _ => (
                quote!((#(#tys,)*)),
                quote!((#(#names,)*)),
                quote!((#(#names,)*)),
            ),
        };
        // The signature with plain parameter names (no `mut`), as the
        // forwarding default and the stand-in write it.
        let output = &f.sig.output;
        let asyncness = &f.sig.asyncness;
        let plain = quote! {
            #asyncness fn #ident(&self, #(#names: #tys),*) #output
        };
        let hidden = format_ident!("__nsh_default_{}", ident);
        if let Some(body) = &f.default {
            // The default body moves to a hidden method the stand-in calls
            // when the JS object has no such member; the method's own
            // default forwards to it, so Rust implementors see no change.
            let mut moved = f.sig.clone();
            moved.ident = hidden.clone();
            let forward = if is_async {
                quote!(self.#hidden(#(#names),*).await)
            } else {
                quote!(self.#hidden(#(#names),*))
            };
            let default_doc = format!(
                " The default of [`{name}::{ident}`]: what the stand-in runs when the JS object \
                 has no `{}`.",
                camel(&ident.to_string())
            );
            trait_items.push(quote! {
                #(#attrs)*
                #plain {
                    #forward
                }

                #(#cfgs)*
                #[doc = #default_doc]
                #[doc(hidden)]
                #[allow(async_fn_in_trait)]
                #moved #body
            });
        } else {
            trait_items.push(quote! {
                #(#attrs)*
                #plain;
            });
        }

        // How the stand-in calls the JS object.
        let def = quote!(<Self as #ui::Interface>::DEF);
        let body = if is_async {
            if optional {
                quote! {
                    match #private::interface_async_optional::<#args_ty, #ret>(
                        &self.object, #def, #index, #args,
                    ).await {
                        ::core::result::Result::Ok(__nsh_result) => __nsh_result,
                        ::core::result::Result::Err(#args_pat) => self.#hidden(#(#names),*).await,
                    }
                }
            } else {
                quote! {
                    #private::interface_async::<#args_ty, #ret>(&self.object, #def, #index, #args).await
                }
            }
        } else {
            let default = if optional {
                quote! {
                    ::core::option::Option::Some({
                        let __nsh_this = ::core::clone::Clone::clone(self);
                        move |#args_pat: #args_ty| __nsh_this.#hidden(#(#names),*)
                    })
                }
            } else {
                quote!(::core::option::Option::<fn(#args_ty) -> #ret>::None)
            };
            quote! {
                #private::interface_sync::<#args_ty, #ret, _>(&self.object, #def, #index, #args, #default)
            }
        };
        impls.push(quote! {
            #(#cfgs)*
            #plain {
                #body
            }
        });

        let js = camel(&ident.to_string());
        let member_doc = decl(&docs(&f.attrs));
        let kind = if m.getter {
            quote!(Getter)
        } else {
            quote!(Method)
        };
        let param_names: Vec<String> = names.iter().map(|n| camel(&n.to_string())).collect();
        members.push(quote! {
            #(#cfgs)*
            #ui::InterfaceMember {
                name: #js,
                doc: #member_doc,
                kind: #ui::MemberKind::#kind,
                params: &[#((#param_names, #private::ret_ts::<#tys>(), false)),*],
                returns: #private::interface_ret_ts::<#ret>(),
                is_async: #is_async,
                optional: #optional,
                refs: |__out| {
                    let _ = &__out;
                    #(#private::ret_classes::<#tys>(__out);)*
                    #private::interface_ret_classes::<#ret>(__out);
                },
            }
        });
    }

    let trait_attrs = &item.attrs;
    let trait_token = &item.trait_token;
    Ok(quote! {
        #(#trait_attrs)*
        #vis #trait_token #name {
            #(#trait_items)*
        }

        #[doc = #stand_in_doc]
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #vis struct #stand_in {
            object: #ui::JsHandle,
        }

        // A private interface's stand-in may be made only from JS.
        #[allow(dead_code)]
        impl #stand_in {
            /// The stand-in calling `object`'s members.
            #vis fn new(object: #ui::JsHandle) -> Self {
                Self { object }
            }

            /// The JS object it calls.
            #vis fn object(&self) -> &#ui::JsHandle {
                &self.object
            }
        }

        impl ::core::convert::From<#ui::JsHandle> for #stand_in {
            fn from(object: #ui::JsHandle) -> Self {
                Self { object }
            }
        }

        // SAFETY: the one value it keeps, through its own `Trace`.
        unsafe impl #ui::Trace for #stand_in {
            fn trace(&self, tracer: &mut #ui::Tracer<'_>) {
                #ui::Trace::trace(&self.object, tracer);
            }
        }

        impl #ui::Interface for #stand_in {
            const DEF: &'static #ui::InterfaceDef = &#ui::InterfaceDef {
                name: #ts_name,
                doc: #doc,
                type_id: #private::type_id::<#stand_in>,
                type_name: #private::type_name::<#stand_in>,
                members: &[#(#members),*],
            };
        }

        /// A JS object with the interface's required methods (TS: the
        /// interface).
        impl #ui::JsValue for #stand_in {
            fn ts_type(types: &mut #ui::TsTypes) -> ::std::string::String {
                #private::interface_ts(<Self as #ui::Interface>::DEF, types)
            }

            fn from_js(
                value: #ui::JsIn<'_>,
            ) -> ::core::result::Result<Self, #ui::PropError> {
                #private::interface_from_js(value, <Self as #ui::Interface>::DEF).map(Self::new)
            }

            fn to_js<'a>(
                &self,
                host: &'a dyn #ui::JsHost,
            ) -> ::core::result::Result<#ui::JsOwned<'a>, #ui::PropError> {
                #ui::JsValue::to_js(&self.object, host)
            }

            fn classes(out: &mut ::std::vec::Vec<&'static #ui::ClassDef>) {
                #private::interface_classes(<Self as #ui::Interface>::DEF, out);
            }
        }

        impl #name for #stand_in {
            #(#impls)*
        }
    })
}

/// The method `f` of an interface, checked.
fn method(f: &TraitItemFn) -> syn::Result<Method<'_>> {
    let sig = &f.sig;
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            sig.generics.span(),
            "an interface's method has no generic parameters",
        ));
    }
    if let Some(token) = &sig.constness {
        return Err(syn::Error::new(
            token.span(),
            "an interface's method is not `const`",
        ));
    }
    if let Some(token) = &sig.unsafety {
        return Err(syn::Error::new(
            token.span(),
            "an interface's method is not `unsafe`",
        ));
    }
    if let Some(abi) = &sig.abi {
        return Err(syn::Error::new(
            abi.span(),
            "an interface's method has no ABI",
        ));
    }
    if let Some(variadic) = &sig.variadic {
        return Err(syn::Error::new(
            variadic.span(),
            "an interface's method is not variadic",
        ));
    }
    let receiver_error = || {
        syn::Error::new(
            sig.span(),
            "an interface's method takes `&self` (the stand-in calls the JS object it keeps)",
        )
    };
    match sig.inputs.first() {
        Some(FnArg::Receiver(r))
            if r.reference.is_some() && r.mutability.is_none() && r.colon_token.is_none() => {}
        _ => return Err(receiver_error()),
    }
    let mut params = Vec::new();
    for arg in sig.inputs.iter().skip(1) {
        let FnArg::Typed(t) = arg else {
            return Err(receiver_error());
        };
        let Pat::Ident(p) = &*t.pat else {
            return Err(syn::Error::new(
                t.pat.span(),
                "an interface's parameters are named",
            ));
        };
        if p.by_ref.is_some() || p.subpat.is_some() {
            return Err(syn::Error::new(
                t.pat.span(),
                "an interface's parameters are named",
            ));
        }
        if let Some(attr) = t.attrs.first() {
            return Err(syn::Error::new(
                attr.span(),
                "an interface's parameters take no attributes",
            ));
        }
        match &*t.ty {
            Type::Reference(r) => {
                return Err(syn::Error::new(
                    r.span(),
                    "an interface's parameters are owned values (`String`, `Instance<T>`), \
                     which the call hands to JS",
                ));
            }
            Type::ImplTrait(i) => {
                return Err(syn::Error::new(
                    i.span(),
                    "an interface's parameters have named types",
                ));
            }
            _ => {}
        }
        params.push((&p.ident, &*t.ty));
    }
    let getter = f.attrs.iter().any(|a| a.path().is_ident("getter"));
    if getter && !params.is_empty() {
        return Err(syn::Error::new(
            sig.span(),
            "a getter takes only `&self` and returns the property's value",
        ));
    }
    if getter && matches!(sig.output, ReturnType::Default) {
        return Err(syn::Error::new(
            sig.span(),
            "a getter returns the property's value",
        ));
    }
    let name = camel(&sig.ident.to_string());
    if name == "constructor" {
        return Err(syn::Error::new(
            sig.ident.span(),
            "rename `constructor`: every JS object has one (its class)",
        ));
    }
    let ret = match &sig.output {
        ReturnType::Default => syn::parse_quote!(()),
        ReturnType::Type(_, ty) => {
            if let Type::ImplTrait(i) = &**ty {
                return Err(syn::Error::new(
                    i.span(),
                    "an interface's method returns a named type (write `async fn` for a promise)",
                ));
            }
            (**ty).clone()
        }
    };
    Ok(Method {
        f,
        getter,
        params,
        ret,
    })
}

#[cfg(test)]
mod tests {
    use quote::quote;

    /// The error `#[interface]` gives `item`.
    fn error(item: proc_macro2::TokenStream) -> String {
        match super::interface(syn::parse2(item).unwrap()) {
            Ok(_) => "no error".to_owned(),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn methods_take_shared_self_and_owned_values() {
        assert_eq!(
            error(quote!(
                trait T {
                    fn a(&self, n: u32) -> u32;
                    async fn b(&self) {}
                }
            )),
            "no error"
        );
        for item in [
            quote!(
                trait T {
                    fn a(&mut self);
                }
            ),
            quote!(
                trait T {
                    fn a(self);
                }
            ),
            quote!(
                trait T {
                    fn a();
                }
            ),
            quote!(
                trait T {
                    fn a(self: Box<Self>);
                }
            ),
        ] {
            assert!(error(item).contains("takes `&self`"));
        }
        assert!(
            error(quote!(
                trait T {
                    fn a(&self, s: &str);
                }
            ))
            .contains("owned values")
        );
        assert!(
            error(quote!(
                trait T {
                    fn a(&self, (x, y): (u32, u32));
                }
            ))
            .contains("named")
        );
        assert!(
            error(quote!(
                trait T {
                    fn a(&self) -> impl Sized;
                }
            ))
            .contains("returns a named type")
        );
    }

    #[test]
    fn what_an_interface_cannot_have_is_refused() {
        assert!(
            error(quote!(
                trait T<A> {}
            ))
            .contains("no generic parameters")
        );
        assert!(
            error(quote!(
                trait T: Clone {}
            ))
            .contains("no supertraits")
        );
        assert!(
            error(quote!(
                unsafe trait T {}
            ))
            .contains("not an `unsafe trait`")
        );
        assert!(
            error(quote!(
                trait T {
                    type A;
                }
            ))
            .contains("methods only")
        );
        assert!(
            error(quote!(
                trait T {
                    const A: u32;
                }
            ))
            .contains("methods only")
        );
        assert!(
            error(quote!(
                trait T {
                    fn a<X>(&self);
                }
            ))
            .contains("no generic parameters")
        );
        assert!(
            error(quote!(
                trait T {
                    unsafe fn a(&self);
                }
            ))
            .contains("not `unsafe`")
        );
        assert!(
            error(quote!(
                trait T {
                    fn constructor(&self);
                }
            ))
            .contains("rename `constructor`")
        );
    }

    #[test]
    fn getters_read_a_value() {
        assert_eq!(
            error(quote!(
                trait T {
                    #[getter]
                    fn a(&self) -> u32;
                }
            )),
            "no error"
        );
        assert!(
            error(quote!(
                trait T {
                    #[getter]
                    fn a(&self, n: u32) -> u32;
                }
            ))
            .contains("only `&self`")
        );
        assert!(
            error(quote!(
                trait T {
                    #[getter]
                    fn a(&self);
                }
            ))
            .contains("returns the property's value")
        );
    }

    #[test]
    fn a_default_body_moves_to_a_hidden_method() {
        let item = syn::parse2(quote!(
            trait T {
                fn a(&self, mut n: u32) -> u32 {
                    n += 1;
                    n
                }
            }
        ))
        .unwrap();
        let made = super::interface(item).unwrap().to_string();
        assert!(
            made.contains("fn __nsh_default_a (& self , mut n : u32) -> u32"),
            "{made}"
        );
        assert!(made.contains("self . __nsh_default_a (n)"), "{made}");
        assert!(made.contains("struct JsT"), "{made}");
    }
}
