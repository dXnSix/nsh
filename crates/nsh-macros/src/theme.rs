//! `#[derive(AppTheme)]`: a theme type of a struct's fields
//! (`nsh_core::element::AppTheme`).

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitStr, Type, spanned::Spanned};

/// The roles a field may play (`nsh_core::element::Role`'s names), each
/// with the kind of token it reads.
const ROLES: &[(&str, RoleKind)] = &[
    ("background", RoleKind::Color),
    ("foreground", RoleKind::Color),
    ("card", RoleKind::Color),
    ("card-foreground", RoleKind::Color),
    ("popover", RoleKind::Color),
    ("popover-foreground", RoleKind::Color),
    ("primary", RoleKind::Color),
    ("primary-foreground", RoleKind::Color),
    ("secondary", RoleKind::Color),
    ("secondary-foreground", RoleKind::Color),
    ("muted", RoleKind::Color),
    ("muted-foreground", RoleKind::Color),
    ("accent", RoleKind::Color),
    ("accent-foreground", RoleKind::Color),
    ("destructive", RoleKind::Color),
    ("border", RoleKind::Color),
    ("input", RoleKind::Color),
    ("ring", RoleKind::Color),
    ("chart-1", RoleKind::Color),
    ("chart-2", RoleKind::Color),
    ("chart-3", RoleKind::Color),
    ("chart-4", RoleKind::Color),
    ("chart-5", RoleKind::Color),
    ("sidebar", RoleKind::Color),
    ("sidebar-foreground", RoleKind::Color),
    ("sidebar-primary", RoleKind::Color),
    ("sidebar-primary-foreground", RoleKind::Color),
    ("sidebar-accent", RoleKind::Color),
    ("sidebar-accent-foreground", RoleKind::Color),
    ("sidebar-border", RoleKind::Color),
    ("sidebar-ring", RoleKind::Color),
    ("selection", RoleKind::Color),
    ("radius", RoleKind::Number),
    ("font-sans", RoleKind::Font),
    ("font-mono", RoleKind::Font),
];

/// What a role reads: a color, a number or a font family.
#[derive(Clone, Copy)]
enum RoleKind {
    Color,
    Number,
    Font,
}

impl RoleKind {
    fn what(self) -> &'static str {
        match self {
            RoleKind::Color => "a color",
            RoleKind::Number => "a number",
            RoleKind::Font => "a font family",
        }
    }
}

/// A kind of token: its name, `TokenKind`'s variant, the namespace its
/// names take, and the field types it fits.
struct Kind {
    name: &'static str,
    variant: &'static str,
    prefix: &'static str,
    types: &'static [&'static str],
}

const NUMBERS: &[&str] = &["f32", "Points"];

const KINDS: &[Kind] = &[
    Kind {
        name: "color",
        variant: "Color",
        prefix: "",
        types: &["AdaptiveColor"],
    },
    Kind {
        name: "number",
        variant: "Number",
        prefix: "",
        types: NUMBERS,
    },
    Kind {
        name: "radius",
        variant: "Radius",
        prefix: "radius-",
        types: NUMBERS,
    },
    Kind {
        name: "spacing",
        variant: "Spacing",
        prefix: "spacing-",
        types: NUMBERS,
    },
    Kind {
        name: "text",
        variant: "Text",
        prefix: "text-",
        types: NUMBERS,
    },
    Kind {
        name: "weight",
        variant: "Weight",
        prefix: "font-weight-",
        types: NUMBERS,
    },
    Kind {
        name: "tracking",
        variant: "Tracking",
        prefix: "tracking-",
        types: NUMBERS,
    },
    Kind {
        name: "leading",
        variant: "Leading",
        prefix: "leading-",
        types: NUMBERS,
    },
    Kind {
        name: "blur",
        variant: "Blur",
        prefix: "blur-",
        types: NUMBERS,
    },
    Kind {
        name: "font",
        variant: "Font",
        prefix: "font-",
        types: &["SharedString", "String"],
    },
    Kind {
        name: "shadow",
        variant: "Shadow",
        prefix: "shadow-",
        types: &["AdaptiveShadow"],
    },
];

/// What a field is to the theme.
enum Role {
    /// One token.
    Token {
        kind: &'static Kind,
        name: String,
        /// The roles it plays, as written.
        roles: Vec<String>,
    },
    /// An inner theme type, its names under `prefix`.
    Nested { prefix: String },
    /// Left out (it keeps `Default`'s value).
    Skip,
}

struct Field {
    ident: syn::Ident,
    ty: Type,
    doc: String,
    role: Role,
}

/// What a field's `#[token(..)]` says, each part with where it is written.
#[derive(Default)]
struct Attr {
    kind: Option<(&'static Kind, Span)>,
    /// `nested`, `flatten` or `skip`.
    mode: Option<(&'static str, Span)>,
    name: Option<LitStr>,
    /// Each `role = "…"`: a token may play several.
    roles: Vec<LitStr>,
}

fn parse_attr(attr: &syn::Attribute) -> syn::Result<Attr> {
    let mut out = Attr::default();
    attr.parse_nested_meta(|meta| {
        let key = meta
            .path
            .get_ident()
            .map(ToString::to_string)
            .unwrap_or_default();
        let span = meta.path.span();
        let twice = |what: &str| syn::Error::new(span, format!("{what} is given twice"));
        match key.as_str() {
            "nested" | "flatten" | "skip" => {
                if let Some((first, _)) = out.mode {
                    return Err(syn::Error::new(
                        span,
                        format!("a field is one of `nested`, `flatten` and `skip`; it is already `{first}`"),
                    ));
                }
                let mode = match key.as_str() {
                    "nested" => "nested",
                    "flatten" => "flatten",
                    _ => "skip",
                };
                out.mode = Some((mode, span));
            }
            "name" => {
                if out.name.is_some() {
                    return Err(twice("`name`"));
                }
                out.name = Some(meta.value()?.parse::<LitStr>()?);
            }
            "role" => out.roles.push(meta.value()?.parse::<LitStr>()?),
            other => match KINDS.iter().find(|k| k.name == other) {
                Some(k) => {
                    if let Some((first, _)) = out.kind {
                        return Err(syn::Error::new(
                            span,
                            format!("a token has one kind; it is already `{}`", first.name),
                        ));
                    }
                    out.kind = Some((k, span));
                }
                None => {
                    return Err(meta.error(format!(
                        "unknown token kind `{other}`: one of {}, or `nested`, `flatten`, `skip`",
                        KINDS.iter().map(|k| k.name).collect::<Vec<_>>().join(", ")
                    )));
                }
            },
        }
        Ok(())
    })?;
    if let Some((mode, span)) = out.mode {
        if out.kind.is_some() {
            return Err(syn::Error::new(
                span,
                format!("a `{mode}` field has no kind of its own"),
            ));
        }
        if let Some(lit) = out.name.as_ref().or(out.roles.first()) {
            return Err(syn::Error::new(
                lit.span(),
                format!("a `{mode}` field is no token: it takes no `name` or `role`"),
            ));
        }
    }
    Ok(out)
}

/// Why the core would not read `name` as a `kind` token, if it would not:
/// a theme file's value is read by the name's form
/// (`ThemeSet::try_add`'s rule).
fn unreadable(kind: &Kind, name: &str) -> Option<String> {
    let starts = |p: &str| name.starts_with(p);
    let readable = match kind.name {
        "font" => starts("font-") && !starts("font-weight-"),
        "shadow" => starts("shadow-"),
        "color" => !starts("font-"),
        _ => !starts("font-") || starts("font-weight-"),
    };
    (!readable).then(|| {
        format!(
            "the {} token `{name}` is named as no {} is (a font is `font-*`, a shadow `shadow-*`, a weight \
             `font-weight-*`, and nothing else starts `font-`)",
            kind.name, kind.name
        )
    })
}

/// The classes that read token `name` of `kind`, for the patch's docs.
fn classes(kind: &Kind, name: &str) -> String {
    let Some(rest) = name
        .strip_prefix(kind.prefix)
        .filter(|_| !kind.prefix.is_empty())
    else {
        return match kind.name {
            "color" => format!(" (`bg-{name}`, `text-{name}`, `border-{name}`, …)"),
            _ => String::new(),
        };
    };
    let class = match kind.name {
        "radius" => format!("`rounded-{rest}`"),
        "spacing" => format!("`p-{rest}`, `gap-{rest}`, …"),
        "text" => format!("`text-{rest}`"),
        "weight" | "font" => format!("`font-{rest}`"),
        "tracking" => format!("`tracking-{rest}`"),
        "leading" => format!("`leading-{rest}`"),
        "blur" => format!("`blur-{rest}`"),
        "shadow" => format!("`shadow-{rest}`"),
        _ => return String::new(),
    };
    format!(" ({class})")
}

pub(crate) fn derive(input: DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            input.generics.span(),
            "`AppTheme` is derived for a struct with no generics: a theme type is one set of names",
        ));
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "`AppTheme` is derived for structs",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new(
            input.span(),
            "`AppTheme` takes a struct with named fields",
        ));
    };
    let mut fields = Vec::new();
    let mut seen: Vec<(String, Span)> = Vec::new();
    let mut roles: Vec<(String, Span)> = Vec::new();
    for field in &named.named {
        let ident = field.ident.clone().expect("named");
        let mut attrs = field.attrs.iter().filter(|a| a.path().is_ident("token"));
        let Some(attr) = attrs.next() else {
            return Err(syn::Error::new(
                ident.span(),
                "each field of an `AppTheme` needs `#[token(..)]`: its kind, `nested`, `flatten` or `skip`",
            ));
        };
        if let Some(second) = attrs.next() {
            return Err(syn::Error::new(
                second.span(),
                "a field takes one `#[token(..)]`",
            ));
        }
        let field_name = kebab(&ident.to_string());
        let parsed = parse_attr(attr)?;
        let doc = doc_of(&field.attrs);
        let role_of_field = match parsed.mode {
            Some(("skip", _)) => Role::Skip,
            Some((mode, span)) => {
                let prefix = if mode == "nested" {
                    format!("{field_name}-")
                } else {
                    String::new()
                };
                let clash = if prefix.starts_with("font-") {
                    Some("its colors and numbers would be named `font-…`, as fonts are")
                } else if prefix.starts_with("weight-") {
                    Some("its fonts would be named `font-weight-…`, as weights are")
                } else {
                    None
                };
                if let Some(clash) = clash {
                    return Err(syn::Error::new(
                        span,
                        format!("under the prefix `{prefix}`, {clash}: rename the field"),
                    ));
                }
                Role::Nested { prefix }
            }
            None => {
                let Some((kind, kind_span)) = parsed.kind else {
                    return Err(syn::Error::new(
                        attr.span(),
                        "`#[token(..)]` needs a kind (`color`, `radius`, …), `nested`, `flatten` or `skip`",
                    ));
                };
                let ty = last_segment(&field.ty);
                if !kind.types.contains(&ty.as_str()) {
                    return Err(syn::Error::new(
                        kind_span,
                        format!(
                            "a `{}` token is a field of type {}, not `{ty}`",
                            kind.name,
                            kind.types
                                .iter()
                                .map(|t| format!("`{t}`"))
                                .collect::<Vec<_>>()
                                .join(" or ")
                        ),
                    ));
                }
                let (name, name_span) = match &parsed.name {
                    Some(lit) => (lit.value(), lit.span()),
                    None => (format!("{}{field_name}", kind.prefix), ident.span()),
                };
                if name.is_empty() {
                    return Err(syn::Error::new(name_span, "a token's name cannot be empty"));
                }
                if !kind.prefix.is_empty() && !name.starts_with(kind.prefix) {
                    return Err(syn::Error::new(
                        name_span,
                        format!(
                            "a `{}` token's name starts `{}`, as its classes read it and so that a nested \
                             type's names agree: `{}{name}`",
                            kind.name, kind.prefix, kind.prefix
                        ),
                    ));
                }
                if let Some(why) = unreadable(kind, &name) {
                    return Err(syn::Error::new(name_span, why));
                }
                if let Some((_, first)) = seen.iter().find(|(n, _)| *n == name) {
                    let mut err =
                        syn::Error::new(name_span, format!("the token `{name}` is declared twice"));
                    err.combine(syn::Error::new(
                        *first,
                        format!("`{name}` is first declared here"),
                    ));
                    return Err(err);
                }
                seen.push((name.clone(), name_span));
                let mut plays = Vec::new();
                for lit in &parsed.roles {
                    let r = lit.value();
                    let Some((_, fits)) = ROLES.iter().find(|(n, _)| *n == r) else {
                        return Err(syn::Error::new(
                            lit.span(),
                            format!("no role is named `{r}`"),
                        ));
                    };
                    let kind_fits = match fits {
                        RoleKind::Color => kind.name == "color",
                        RoleKind::Font => kind.name == "font",
                        RoleKind::Number => kind.types == NUMBERS,
                    };
                    if !kind_fits {
                        return Err(syn::Error::new(
                            lit.span(),
                            format!(
                                "the role `{r}` is {}, not a `{}` token",
                                fits.what(),
                                kind.name
                            ),
                        ));
                    }
                    // A token may play several roles; a role is played by
                    // one token, and named once.
                    if let Some((_, first)) = roles.iter().find(|(n, _)| *n == r) {
                        let mut err =
                            syn::Error::new(lit.span(), format!("the role `{r}` is mapped twice"));
                        err.combine(syn::Error::new(
                            *first,
                            format!("`{r}` is first mapped here"),
                        ));
                        return Err(err);
                    }
                    roles.push((r.clone(), lit.span()));
                    plays.push(r);
                }
                Role::Token {
                    kind,
                    name,
                    roles: plays,
                }
            }
        };
        fields.push(Field {
            ident,
            ty: field.ty.clone(),
            doc,
            role: role_of_field,
        });
    }

    let ui = quote!(::nsh_core::element);
    let private = quote!(::nsh_core::element::__private);
    let patch = format_ident!("{name}Patch");
    let vis = &input.vis;

    let mut specs = Vec::new();
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    let mut patch_fields = Vec::new();
    let mut patch_writes = Vec::new();
    for f in &fields {
        let ident = &f.ident;
        let ty = &f.ty;
        match &f.role {
            Role::Token { kind, name, roles } => {
                let variant = format_ident!("{}", kind.variant);
                let doc = &f.doc;
                let role = roles.iter().map(|r| {
                    let variant = format_ident!("{}", camel(r));
                    quote!(.role(#ui::Role::#variant))
                });
                let full = quote!(&#ui::TokenKind::#variant.prefixed(prefix, #name));
                specs.push(quote! {
                    specs.push(#ui::TokenSpec::new(#name, #ui::TokenKind::#variant).doc(#doc) #(#role)*);
                });
                writes.push(quote! {
                    #private::ThemeValue::write(&self.#ident, #full, out);
                });
                reads.push(quote! {
                    #ident: #private::ThemeValue::read(tokens, #full).unwrap_or(default.#ident),
                });
                let lead = if doc.is_empty() {
                    String::new()
                } else {
                    format!(" {doc}")
                };
                let patch_doc = format!(
                    "{lead} Token `{name}`{}; `None`: as it is.",
                    classes(kind, name)
                );
                patch_fields.push(quote! {
                    #[doc = #patch_doc]
                    #vis #ident: ::core::option::Option<#ty>,
                });
                patch_writes.push(quote! {
                    if let ::core::option::Option::Some(value) = &patch.#ident {
                        #private::ThemeValue::write(value, #full, out);
                    }
                });
            }
            Role::Nested { prefix: inner } => {
                specs.push(quote! {
                    for spec in <#ty as #ui::AppTheme>::token_specs() {
                        let mut spec = ::core::clone::Clone::clone(spec);
                        spec.name = ::std::borrow::Cow::Owned(spec.kind.prefixed(#inner, &spec.name));
                        specs.push(spec);
                    }
                });
                let full = quote!(&::std::format!("{}{}", prefix, #inner));
                writes.push(quote! {
                    <#ty as #ui::AppTheme>::write_tokens(&self.#ident, #full, out);
                });
                reads.push(quote! {
                    #ident: <#ty as #ui::AppTheme>::read_tokens(tokens, #full),
                });
                let under = if inner.is_empty() {
                    String::new()
                } else {
                    format!(" under `{inner}`")
                };
                let patch_doc = format!(
                    " The tokens of `{ident}`{under} (`TokenKind::prefixed`); `None`: as they are."
                );
                patch_fields.push(quote! {
                    #[doc = #patch_doc]
                    #vis #ident: ::core::option::Option<<#ty as #ui::AppTheme>::Patch>,
                });
                patch_writes.push(quote! {
                    if let ::core::option::Option::Some(inner) = &patch.#ident {
                        <#ty as #ui::AppTheme>::write_patch(inner, #full, out);
                    }
                });
            }
            Role::Skip => {
                reads.push(quote! { #ident: default.#ident, });
            }
        }
    }
    let patch_doc = format!(
        " Some of the fields of [`{name}`], each `None` for \"as it is\" (`update_theme::<{name}>`)."
    );
    Ok(quote! {
        #[doc = #patch_doc]
        #[derive(Clone, Default)]
        #vis struct #patch {
            #(#patch_fields)*
        }

        impl #ui::AppTheme for #name {
            type Patch = #patch;

            fn token_specs() -> &'static [#ui::TokenSpec] {
                static SPECS: ::std::sync::OnceLock<::std::vec::Vec<#ui::TokenSpec>> =
                    ::std::sync::OnceLock::new();
                SPECS.get_or_init(|| {
                    let mut specs = ::std::vec::Vec::new();
                    #(#specs)*
                    specs
                })
            }

            fn write_tokens(&self, prefix: &str, out: &mut #ui::Tokens) {
                let _ = (prefix, &out);
                #(#writes)*
            }

            fn read_tokens(tokens: &#ui::Tokens, prefix: &str) -> Self {
                let _ = (tokens, prefix);
                let default = <Self as ::core::default::Default>::default();
                let _ = &default;
                Self {
                    #(#reads)*
                }
            }

            fn write_patch(patch: &#patch, prefix: &str, out: &mut #ui::Tokens) {
                let _ = (patch, prefix, &out);
                #(#patch_writes)*
            }
        }
    })
}

/// The doc comment of a field, its lines joined.
fn doc_of(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => Some(s.value().trim().to_owned()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The last segment of a type's path (`nsh_core::geometry::Points`:
/// `Points`).
fn last_segment(ty: &Type) -> String {
    match ty {
        Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// `surface_raised` as `surface-raised`.
fn kebab(name: &str) -> String {
    name.trim_start_matches("r#").replace('_', "-")
}

/// `popover-foreground` as `PopoverForeground`, `chart-1` as `Chart1`.
fn camel(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}
