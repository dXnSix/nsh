//! `#[class_extension(C)]`: instance methods a crate adds to class `C`, a
//! `#[class]` of another crate, on an `impl` block of a type of its own
//! (the holder). The methods' entries are made by `#[class]`'s member code
//! (`crate::class::member_def`), with `this` an instance of `C`.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{FnArg, ImplItem, ImplItemFn, ItemImpl, Pat, spanned::Spanned};

use crate::{exported, last_segment, strip_args};

pub(crate) fn class_extension(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let class = class_path(attr)?;
    let class_name = class
        .segments
        .last()
        .expect("a path has a segment")
        .ident
        .to_string();
    let mut item: ItemImpl = syn::parse2(item).map_err(|e| {
        syn::Error::new(
            e.span(),
            "`#[class_extension(C)]` goes on an `impl` block of a type of this crate (`impl \
             I18nBuilder { .. }`), whose functions become methods of class `C`",
        )
    })?;
    if !item.generics.params.is_empty() || item.trait_.is_some() {
        return Err(syn::Error::new(
            item.span(),
            "`#[class_extension(C)]` goes on an inherent `impl` block without generics \
             (`impl I18nBuilder { .. }`)",
        ));
    }
    if let Some(doc) = item.attrs.iter().find(|a| a.path().is_ident("doc")) {
        return Err(syn::Error::new(
            doc.span(),
            "document each method of the extension, not its `impl` block: TS shows the methods' \
             doc comments with the class (a `//` comment here stays in Rust)",
        ));
    }
    let nsh_core = crate::root::nsh_core();
    let ui = quote!(#nsh_core::element);
    let this_ty = quote!(#class);
    let mut members = Vec::new();
    for member in &item.items {
        let ImplItem::Fn(f) = member else { continue };
        if let Some(attr) = f
            .attrs
            .iter()
            .find(|a| a.path().is_ident("getter") || a.path().is_ident("setter"))
        {
            return Err(syn::Error::new(
                attr.span(),
                format!(
                    "a class extension adds instance methods only, no getters or setters: make \
                     it a method taking `this: Instance<{class_name}>`"
                ),
            ));
        }
        if !exported(&f.vis) {
            // A helper: Rust's only.
            continue;
        }
        check_method(f, &class_name)?;
        members.push(crate::class::member_def(f, &class_name, &this_ty)?);
    }
    // The callbacks' and signatures' attributes are the macro's.
    for member in &mut item.items {
        if let ImplItem::Fn(f) = member {
            f.attrs
                .retain(|a| !a.path().is_ident("overload") && !a.path().is_ident("ts"));
            strip_args(&mut f.sig);
        }
    }
    let self_ty = &item.self_ty;
    Ok(quote! {
        #item

        impl #ui::ClassExtension for #self_ty {
            const DEF: &'static #ui::ClassExtensionDef = &#ui::ClassExtensionDef {
                class: <#class as #ui::Class>::DEF,
                type_id: #ui::__private::type_id::<Self>,
                type_name: #ui::__private::type_name::<Self>,
                members: &[#(#members),*],
            };
        }

        // The messages name the class as JS does: by its own name, which an
        // alias (`use app::AppBuilder as Builder`) is not.
        const _: () = ::core::assert!(
            #ui::__private::same_name(<#class as #ui::Class>::DEF.name, #class_name),
            "`#[class_extension(C)]` names the class by its own name (the struct's, which JS knows \
             it by), not by an alias: write `#[class_extension(path::to::AppBuilder)]`",
        );
    })
}

/// The class `attr` names: one type path, without generic arguments.
fn class_path(attr: TokenStream2) -> syn::Result<syn::Path> {
    let usage = "`#[class_extension(C)]` names the `#[class]` it adds methods to: one type path \
                 (`#[class_extension(AppBuilder)]`, `#[class_extension(app::AppBuilder)]`)";
    if attr.is_empty() {
        return Err(syn::Error::new(proc_macro2::Span::call_site(), usage));
    }
    let span = attr.span();
    let path: syn::Path = syn::parse2(attr).map_err(|_| syn::Error::new(span, usage))?;
    if path
        .segments
        .iter()
        .any(|s| !matches!(s.arguments, syn::PathArguments::None))
    {
        return Err(syn::Error::new(
            path.span(),
            "a class has no generic arguments: name it alone (`#[class_extension(Token)]`)",
        ));
    }
    Ok(path)
}

/// Refuses what is not an instance method of the class: a receiver, no
/// `this: Instance<C>` first, `new`.
fn check_method(f: &ImplItemFn, class: &str) -> syn::Result<()> {
    let this = format!("this: Instance<{class}>");
    if f.sig.ident == "new" {
        return Err(syn::Error::new(
            f.sig.ident.span(),
            format!(
                "a class extension adds no constructor: rename `new` (an instance method takes \
                 `{this}` first)"
            ),
        ));
    }
    match f.sig.inputs.first() {
        Some(FnArg::Receiver(r)) => Err(syn::Error::new(
            r.span(),
            format!(
                "a class extension's method takes `{this}`, not `self`: the extension's type is \
                 not the class (borrow the value with `this.with(..)` / `this.with_mut(..)`)"
            ),
        )),
        Some(FnArg::Typed(t)) if matches!(&*t.pat, Pat::Ident(p) if p.ident == "this") => {
            match last_segment(&t.ty) {
                Some((id, Some(of))) if id == "Instance" => match last_segment(of) {
                    Some((name, _)) if name == class => Ok(()),
                    _ => Err(syn::Error::new(
                        t.ty.span(),
                        format!(
                            "`this` is an instance of the class the extension adds to: `{this}` \
                             (`Instance<Self>` would be the extension's own type)"
                        ),
                    )),
                },
                _ => Err(syn::Error::new(
                    t.ty.span(),
                    format!("`this` is the class's instance: `{this}`"),
                )),
            }
        }
        _ => Err(syn::Error::new(
            f.sig.span(),
            format!(
                "a class extension adds instance methods only: take `{this}` first (no statics)"
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;

    /// The error `#[class_extension(attr)]` gives `item`.
    fn error(attr: proc_macro2::TokenStream, item: proc_macro2::TokenStream) -> String {
        match super::class_extension(attr, item) {
            Ok(_) => "no error".to_owned(),
            Err(e) => e.to_string(),
        }
    }

    /// The error on a block of `Builder`'s extension.
    fn block_error(item: proc_macro2::TokenStream) -> String {
        error(quote!(Builder), item)
    }

    #[test]
    fn a_receiver_is_refused() {
        for item in [
            quote!(impl Ext { pub fn add(&self) {} }),
            quote!(impl Ext { pub fn add(&mut self) {} }),
            quote!(impl Ext { pub fn add(self) {} }),
            quote!(impl Ext { pub async fn add(&self) {} }),
        ] {
            let error = block_error(item.clone());
            assert!(
                error.contains("takes `this: Instance<Builder>`, not `self`"),
                "{item}: {error}"
            );
        }
    }

    #[test]
    fn statics_and_constructors_are_refused() {
        let error = block_error(quote!(impl Ext { pub fn parse(text: String) -> u32 { 0 } }));
        assert!(
            error.contains("instance methods only: take `this: Instance<Builder>` first"),
            "{error}"
        );
        let error = block_error(quote!(impl Ext { pub async fn load() -> u32 { 0 } }));
        assert!(error.contains("(no statics)"), "{error}");
        let error = block_error(quote!(impl Ext { pub fn new(this: Instance<Builder>) {} }));
        assert!(
            error.contains("adds no constructor: rename `new`"),
            "{error}"
        );
    }

    #[test]
    fn getters_and_setters_are_refused() {
        for item in [
            quote!(impl Ext { #[getter] pub fn count(this: Instance<Builder>) -> u32 { 0 } }),
            quote!(impl Ext { #[setter] pub fn set_count(this: Instance<Builder>, v: u32) {} }),
            // A helper may not carry one either: it would be lost.
            quote!(impl Ext { #[getter] fn count(this: Instance<Builder>) -> u32 { 0 } }),
        ] {
            let error = block_error(item.clone());
            assert!(error.contains("no getters or setters"), "{item}: {error}");
        }
    }

    #[test]
    fn this_of_another_type_is_refused() {
        for item in [
            quote!(impl Ext { pub fn add(this: Instance<Self>) {} }),
            quote!(impl Ext { pub fn add(this: Instance<Other>) {} }),
            quote!(impl Ext { pub async fn add(this: Instance<Other>) {} }),
        ] {
            let error = block_error(item.clone());
            assert!(
                error.contains(
                    "`this` is an instance of the class the extension adds to: `this: Instance<Builder>`"
                ),
                "{item}: {error}"
            );
        }
        let error = block_error(quote!(impl Ext { pub fn add(this: u32) {} }));
        assert!(error.contains("`this` is the class's instance"), "{error}");
        // A path naming the class is the class.
        assert_eq!(
            block_error(quote!(impl Ext { pub fn add(this: Instance<app::Builder>) {} })),
            "no error"
        );
    }

    #[test]
    fn names_every_class_has_are_refused() {
        let error = block_error(quote!(impl Ext { pub fn dispose(this: Instance<Builder>) {} }));
        assert!(error.contains("`dispose` is every class's own"), "{error}");
        let error =
            block_error(quote!(impl Ext { pub fn constructor(this: Instance<Builder>) {} }));
        assert!(
            error.contains("`constructor` is the prototype's own"),
            "{error}"
        );
    }

    #[test]
    fn a_module_context_is_refused() {
        let error = block_error(quote!(
            impl Ext {
                pub fn add(this: Instance<Builder>, cx: &mut ModuleCx) {}
            }
        ));
        assert!(error.contains("takes no `ModuleCx`"), "{error}");
    }

    #[test]
    fn generics_and_trait_impls_are_refused() {
        for item in [
            quote!(
                impl<T> Ext<T> {
                    pub fn add(this: Instance<Builder>) {}
                }
            ),
            quote!(impl Trait for Ext { fn add(this: Instance<Builder>) {} }),
        ] {
            let error = block_error(item.clone());
            assert!(
                error.contains("an inherent `impl` block without generics"),
                "{item}: {error}"
            );
        }
        let error = block_error(quote!(
            struct Ext;
        ));
        assert!(error.contains("goes on an `impl` block"), "{error}");
    }

    #[test]
    fn a_doc_comment_on_the_block_is_refused() {
        let error = block_error(quote!(
            #[doc = " Builder methods."]
            impl Ext {
                pub fn add(this: Instance<Builder>) {}
            }
        ));
        assert!(error.contains("document each method"), "{error}");
    }

    #[test]
    fn the_attribute_is_one_type_path() {
        let item = quote!(impl Ext { pub fn add(this: Instance<Builder>) {} });
        for attr in [
            quote!(),
            quote!(Builder, Other),
            quote!("Builder"),
            quote!(class = Builder),
        ] {
            let error = error(attr.clone(), item.clone());
            assert!(error.contains("one type path"), "{attr}: {error}");
        }
        let error = error(
            quote!(Token<T>),
            quote!(impl Ext { pub fn add(this: Instance<Token>) {} }),
        );
        assert!(error.contains("no generic arguments"), "{error}");
        assert_eq!(error_of_path(quote!(app::ts::Builder)), "no error");
    }

    fn error_of_path(attr: proc_macro2::TokenStream) -> String {
        error(
            attr,
            quote!(impl Ext { pub fn add(this: Instance<Builder>) {} }),
        )
    }

    #[test]
    fn a_valid_block_expands_to_the_class_s_methods() {
        let made = super::class_extension(
            quote!(app::Builder),
            quote!(
                impl I18nBuilder {
                    /// Adds i18n.
                    pub fn add_i18n(
                        this: Instance<app::Builder>,
                        #[args(options)] configure: Option<Callback<Instance<Options>>>,
                    ) -> Result<Instance<app::Builder>, String> {
                        helper();
                        Ok(this)
                    }

                    /// Loads later.
                    pub async fn load(this: Instance<app::Builder>) {}

                    /// Whether it has the service `token` names.
                    #[ts("(token: object): boolean")]
                    pub fn has(this: Instance<app::Builder>, token: JsHandle) -> bool {
                        false
                    }

                    fn helper() {}
                }
            ),
        )
        .unwrap()
        .to_string();
        for part in [
            "impl :: nsh_core :: element :: ClassExtension for I18nBuilder",
            "class : < app :: Builder as :: nsh_core :: element :: Class > :: DEF",
            "name : \"addI18n\"",
            "\"Builder.addI18n\"",
            "this :: < app :: Builder >",
            "Self :: add_i18n",
            "same_name (< app :: Builder as :: nsh_core :: element :: Class > :: DEF . name , \"Builder\")",
            "name : \"load\"",
            "MemberCall :: Async",
        ] {
            assert!(made.contains(part), "{part}: {made}");
        }
        // The macro's attributes are gone from the block; helpers stay.
        assert!(!made.contains("# [args"), "{made}");
        assert!(!made.contains("# [ts"), "{made}");
        assert!(made.contains("fn helper ()"), "{made}");
        assert!(!made.contains("name : \"helper\""), "{made}");
    }
}
