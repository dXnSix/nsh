//! `theme_keys!("design/theme.json")`: a typed key (`TokenKey<V>`) for each
//! token a theme document (`docs/dev/theme-format.md`) declares, in modules
//! by the token's path (`colors.backgrounds.primary` is
//! `colors::backgrounds::PRIMARY`) or at the top by its name when it has
//! none, its doc the token's, and an enum of each modifier's contexts in
//! `modifiers` (`modifiers::Brand::Forest`).
//!
//! The document is read with `nsh_theme::read`, as the runtime reads it. Its
//! errors stop generation, each a `compile_error!` naming its pointer; its
//! warnings (an unknown member) do not. Names that would make no Rust item,
//! or two items of one name, are refused: a keyword that can't be a raw
//! identifier (`self`, `Self`, `super`, `crate`), a top-level group named
//! `modifiers`, and two groups, constants, modifiers or contexts that differ
//! only in case or separators.
//!
//! Read at compile time: a changed document builds the crate again.

use std::{collections::BTreeMap, path::PathBuf};

use nsh_theme::{Modifier, ThemeDocument, ValueType};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::LitStr;

/// The prefix of a path read under the crate's `OUT_DIR` (a document its
/// build script writes).
const OUT_DIR: &str = "$OUT_DIR/";

/// The keys of the document at `lit` (relative to the crate's manifest, or
/// under `OUT_DIR` after `$OUT_DIR/`).
pub(crate) fn expand(lit: LitStr) -> syn::Result<TokenStream2> {
    let span = lit.span();
    let path = document_path(&lit)?;
    let shown = path.display();
    let text = std::fs::read_to_string(&path).map_err(|e| {
        syn::Error::new(span, format!("cannot read the theme document {shown}: {e}"))
    })?;
    let read = nsh_theme::read(&text);
    let mut errors: Option<syn::Error> = None;
    for diagnostic in read.diagnostics.iter().filter(|d| d.is_error()) {
        let at = if diagnostic.pointer.is_empty() {
            String::new()
        } else {
            format!(" at `{}`", diagnostic.pointer)
        };
        let error = syn::Error::new(
            span,
            format!(
                "{shown}: error {}{at}: {}",
                diagnostic.code, diagnostic.message
            ),
        );
        match &mut errors {
            Some(errors) => errors.combine(error),
            None => errors = Some(error),
        }
    }
    if let Some(errors) = errors {
        return Err(errors);
    }
    let document = read.document;
    let nsh_core = crate::root::nsh_core();
    let root = tree(&document, span)?;
    let items = root.items(&nsh_core, span)?;
    let modifiers = modifier_enums(&document, &nsh_core, span)?;
    let path = path.to_string_lossy().into_owned();
    Ok(quote! {
        // The document is read here, so a change of it builds the crate
        // again.
        const _: &str = ::core::include_str!(#path);
        #items
        #modifiers
    })
}

/// The document's tokens as a tree of groups, each token a key placed by
/// its path, or at the top by its name.
fn tree(document: &ThemeDocument, span: Span) -> syn::Result<Group> {
    let mut root = Group::default();
    for (name, token) in document.tokens.iter() {
        let (groups, leaf): (Vec<&str>, &str) = match &token.path {
            Some(key_path) => {
                let segments: Vec<&str> = key_path.split('.').collect();
                if segments.iter().any(|s| s.is_empty()) {
                    return Err(syn::Error::new(
                        span,
                        format!("`tokens.{name}`: the path `{key_path}` has an empty segment"),
                    ));
                }
                let (leaf, groups) = segments.split_last().expect("split gives one segment");
                (groups.to_vec(), leaf)
            }
            None => (Vec::new(), name),
        };
        let mut group = &mut root;
        for (depth, segment) in groups.iter().enumerate() {
            let module = module_name(segment);
            refuse_keyword(
                &module,
                || format!("the group `{segment}` of `tokens.{name}`"),
                span,
            )?;
            if depth == 0 && module == "modifiers" {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`tokens.{name}`: the group `{segment}` would be the module `modifiers`, \
                         which holds the modifiers' enums; rename the group"
                    ),
                ));
            }
            let entry = group
                .groups
                .entry(module.clone())
                .or_insert_with(|| Subgroup {
                    segment: (*segment).to_owned(),
                    token: name.to_owned(),
                    group: Group::default(),
                });
            if entry.segment != *segment {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "the groups `{}` (of `tokens.{}`) and `{segment}` (of `tokens.{name}`) \
                         differ only in case or separators: both would be the module `{module}`",
                        entry.segment, entry.token
                    ),
                ));
            }
            group = &mut entry.group;
        }
        let constant = constant_name(leaf);
        if let Some(other) = group.keys.get(&constant) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`tokens.{}` and `tokens.{name}` differ only in case or separators: both \
                     would be the key `{constant}` of their group",
                    other.name
                ),
            ));
        }
        let ty = rust_type(token.value_type).ok_or_else(|| {
            syn::Error::new(
                span,
                format!(
                    "`tokens.{name}`: the value type `{}` has no Rust type",
                    token.value_type.name()
                ),
            )
        })?;
        group.keys.insert(
            constant,
            Key {
                name: name.to_owned(),
                kind: token.value_type.name(),
                doc: token.doc.clone().unwrap_or_default(),
                ty,
            },
        );
    }
    Ok(root)
}

/// A group of the document's tree: its subgroups (modules) and keys, each
/// by its Rust name.
#[derive(Default)]
struct Group {
    groups: BTreeMap<String, Subgroup>,
    keys: BTreeMap<String, Key>,
}

/// A subgroup: the segment as written, the first token placed in it, and
/// what it holds.
struct Subgroup {
    segment: String,
    token: String,
    group: Group,
}

/// A token's key.
struct Key {
    name: String,
    kind: &'static str,
    doc: String,
    ty: fn(&TokenStream2) -> TokenStream2,
}

impl Group {
    fn items(&self, nsh_core: &TokenStream2, span: Span) -> syn::Result<TokenStream2> {
        let mut out = TokenStream2::new();
        for (name, key) in &self.keys {
            let ident = format_ident!("{name}", span = span);
            let ty = (key.ty)(nsh_core);
            let token = &key.name;
            let doc = (!key.doc.is_empty()).then(|| {
                let doc = format!(" {}", key.doc);
                quote!(#[doc = #doc] #[doc = ""])
            });
            let named = format!(" The {} token `{token}`.", key.kind);
            out.extend(quote! {
                #doc
                #[doc = #named]
                pub const #ident: #nsh_core::element::TokenKey<#ty> =
                    #nsh_core::element::TokenKey::new(#token);
            });
        }
        for (name, subgroup) in &self.groups {
            let ident = if is_keyword(name) {
                syn::Ident::new_raw(name, span)
            } else {
                format_ident!("{name}", span = span)
            };
            let items = subgroup.group.items(nsh_core, span)?;
            let doc = format!(" The tokens of the group `{}`.", subgroup.segment);
            out.extend(quote! {
                #[doc = #doc]
                #[allow(non_snake_case)]
                pub mod #ident {
                    #items
                }
            });
        }
        Ok(out)
    }
}

/// `pub mod modifiers` with an enum of each modifier's contexts, in the
/// document's order (`brand`: `Brand { Ocean, Forest }`), or nothing when
/// the document has none.
fn modifier_enums(
    document: &ThemeDocument,
    nsh_core: &TokenStream2,
    span: Span,
) -> syn::Result<TokenStream2> {
    if document.modifiers.is_empty() {
        return Ok(TokenStream2::new());
    }
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    let mut out = TokenStream2::new();
    for (name, modifier) in document.modifiers.iter() {
        let ty = type_name(name);
        refuse_keyword(&ty, || format!("the modifier `{name}`"), span)?;
        if let Some(other) = seen.insert(ty.clone(), name) {
            return Err(syn::Error::new(
                span,
                format!(
                    "the modifiers `{other}` and `{name}` differ only in case or separators: \
                     both would be the type `{ty}`"
                ),
            ));
        }
        out.extend(modifier_enum(name, &ty, modifier, nsh_core, span)?);
    }
    Ok(quote! {
        /// The theme's modifiers, each an enum of its contexts.
        pub mod modifiers {
            #out
        }
    })
}

/// The enum of `modifier`'s contexts, named `ty`.
fn modifier_enum(
    name: &str,
    ty: &str,
    modifier: &Modifier,
    nsh_core: &TokenStream2,
    span: Span,
) -> syn::Result<TokenStream2> {
    let mut variants: Vec<(String, &str)> = Vec::new();
    for context in modifier.contexts.keys() {
        let variant = type_name(context);
        refuse_keyword(
            &variant,
            || format!("the context `{context}` of the modifier `{name}`"),
            span,
        )?;
        if let Some((_, other)) = variants.iter().find(|(v, _)| *v == variant) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`modifiers.{name}`: the contexts `{other}` and `{context}` differ only in \
                     case or separators: both would be the variant `{variant}`"
                ),
            ));
        }
        variants.push((variant, context));
    }
    let ident = format_ident!("{ty}", span = span);
    let idents: Vec<_> = variants
        .iter()
        .map(|(v, _)| format_ident!("{v}", span = span))
        .collect();
    let names: Vec<&str> = variants.iter().map(|(_, c)| *c).collect();
    let default = format_ident!("{}", type_name(&modifier.default), span = span);
    let doc = format!(" The contexts of the theme modifier `{name}`.");
    let docs: Vec<String> = names
        .iter()
        .map(|c| format!(" The context `{c}`."))
        .collect();
    Ok(quote! {
        #[doc = #doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #ident {
            #( #[doc = #docs] #idents, )*
        }

        impl #ident {
            /// The modifier's name.
            pub const NAME: &'static str = #name;
            /// Its contexts, in the document's order.
            pub const ALL: &'static [Self] = &[#(Self::#idents),*];
            /// The context it has when nothing else is picked.
            pub const DEFAULT: Self = Self::#default;

            /// The context's name.
            pub const fn name(self) -> &'static str {
                match self {
                    #( Self::#idents => #names, )*
                }
            }

            /// Gives the modifier this context
            /// (`nsh_core::element::set_modifier`).
            pub fn set(self) {
                #nsh_core::element::set_modifier(Self::NAME, self.name())
            }

            /// The context the modifier has now where it is read,
            /// reactive (`nsh_core::element::modifier`).
            pub fn current() -> ::core::option::Option<Self> {
                let context = #nsh_core::element::modifier(Self::NAME)?;
                Self::ALL.iter().copied().find(|c| c.name() == &*context)
            }
        }

        /// A context as a `ThemeScope` picks it
        /// (`.modifiers(Some(Brand::Forest.into()))`).
        impl #nsh_core::element::ModifierContext for #ident {
            const MODIFIER: &'static str = Self::NAME;
            fn context(self) -> &'static str {
                self.name()
            }
        }
    })
}

/// Refuses `ident`, the Rust name of `what`, when it is a keyword that
/// can't be a raw identifier.
fn refuse_keyword(ident: &str, what: impl FnOnce() -> String, span: Span) -> syn::Result<()> {
    if matches!(ident, "self" | "Self" | "super" | "crate") {
        return Err(syn::Error::new(
            span,
            format!(
                "{} would be named `{ident}`, a keyword that can't be a raw identifier; rename it",
                what()
            ),
        ));
    }
    Ok(())
}

/// A name as a type's or a variant's (`brand` `Brand`, `high-contrast`
/// `HighContrast`), `_` in front of a digit.
fn type_name(name: &str) -> String {
    let name: String = words(name)
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    first.to_ascii_uppercase().to_string() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

/// The Rust type of a value type's values (`element/theme_values.rs`'
/// table), `None` for one this crate doesn't know yet.
fn rust_type(value_type: ValueType) -> Option<fn(&TokenStream2) -> TokenStream2> {
    Some(match value_type {
        ValueType::Color => |c| quote!(#c::color::Hsla),
        ValueType::Number => |_| quote!(::core::primitive::f32),
        ValueType::Font => |_| quote!(::std::sync::Arc<::core::primitive::str>),
        ValueType::String => |_| quote!(::std::string::String),
        ValueType::Boolean => |_| quote!(::core::primitive::bool),
        ValueType::Duration => |_| quote!(::core::time::Duration),
        ValueType::Easing => |c| quote!(#c::element::CubicBezier),
        ValueType::Typography => |c| quote!(#c::element::Typography),
        ValueType::Shadow => |c| quote!(::std::sync::Arc<[#c::BoxShadow]>),
        ValueType::Gradient => |c| quote!(#c::Gradient),
        ValueType::Blur => |c| quote!(#c::element::Blur),
        ValueType::Border => |c| quote!(#c::element::Border),
        ValueType::Transition => |c| quote!(#c::element::TransitionToken),
        _ => return None,
    })
}

/// A path segment as a module's name: snake case (`backgrounds`,
/// `type_scale`), `_` in front of a digit.
fn module_name(segment: &str) -> String {
    let name = words(segment).join("_").to_lowercase();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

/// A path's last segment, or a name, as a constant's name: upper snake
/// case (`PRIMARY`, `TEXT_XL`), `_` in front of a digit.
fn constant_name(segment: &str) -> String {
    let name = words(segment).join("_").to_uppercase();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

/// The words of a segment: split at anything but a letter or a digit, and
/// where a lower-case letter meets an upper-case one (`fontSize`).
fn words(segment: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut lower = false;
    for c in segment.chars() {
        if !c.is_ascii_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            lower = false;
            continue;
        }
        if c.is_ascii_uppercase() && lower && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    if words.is_empty() {
        words.push("unnamed".into());
    }
    words
}

/// Whether `name` is a Rust keyword, which a module takes raw (`r#type`).
/// `self`, `super` and `crate` can't be raw: [`refuse_keyword`] refuses
/// them first.
fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "static"
            | "struct"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "gen"
    )
}

/// The document's path: after `$OUT_DIR/`, under the crate's `OUT_DIR` (a
/// document its build script writes); otherwise `lit` relative to the
/// calling crate's manifest directory (or the directory a crate `nsh` built
/// from its own manifest names, as `asset!` reads it).
fn document_path(lit: &LitStr) -> syn::Result<PathBuf> {
    let value = lit.value();
    if let Some(rest) = value.strip_prefix(OUT_DIR) {
        let dir = std::env::var_os("OUT_DIR").ok_or_else(|| {
            syn::Error::new(
                lit.span(),
                "`$OUT_DIR/`: the crate has no `OUT_DIR`, which Cargo sets for a crate with a \
                 build script",
            )
        })?;
        return Ok(PathBuf::from(dir).join(rest));
    }
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        syn::Error::new(
            lit.span(),
            "theme documents are read in crates built by Cargo",
        )
    })?;
    let dir = PathBuf::from(dir);
    let dir = std::fs::read_to_string(dir.join("nsh-source-dir"))
        .map(PathBuf::from)
        .unwrap_or(dir);
    Ok(dir.join(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_become_rust_names() {
        assert_eq!(module_name("backgrounds"), "backgrounds");
        assert_eq!(module_name("type-scale"), "type_scale");
        assert_eq!(module_name("fontSize"), "font_size");
        assert_eq!(module_name("2xl"), "_2xl");
        assert_eq!(constant_name("primary"), "PRIMARY");
        assert_eq!(constant_name("text-xl"), "TEXT_XL");
        assert_eq!(constant_name("500"), "_500");
        assert_eq!(constant_name("lineHeight"), "LINE_HEIGHT");
        assert_eq!(constant_name("ink.brand.forest"), "INK_BRAND_FOREST");
        assert_eq!(type_name("brand"), "Brand");
        assert_eq!(type_name("high-contrast"), "HighContrast");
        assert_eq!(type_name("colorMode"), "ColorMode");
        assert_eq!(type_name("2x"), "_2x");
        assert_eq!(type_name("self"), "Self");
    }
}
