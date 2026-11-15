//! `theme_keys!("design/theme.json")`: a typed key (`TokenKey<V>`) for each
//! token a theme document's `"tokens"` section declares, in modules by the
//! token's path (`colors.backgrounds.primary` is
//! `colors::backgrounds::PRIMARY`), its doc the token's. Read at compile
//! time: a changed document builds the crate again.

use std::{collections::BTreeMap, path::PathBuf};

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::LitStr;

/// The keys of the document at `lit` (relative to the crate's manifest).
pub(crate) fn expand(lit: LitStr) -> syn::Result<TokenStream2> {
    let span = lit.span();
    let path = document_path(&lit)?;
    let text = std::fs::read_to_string(&path).map_err(|e| {
        syn::Error::new(
            span,
            format!("cannot read the theme document {}: {e}", path.display()),
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| syn::Error::new(span, format!("{}: not JSON: {e}", path.display())))?;
    let Some(tokens) = value.get("tokens").and_then(serde_json::Value::as_object) else {
        return Err(syn::Error::new(
            span,
            format!(
                "{}: no `\"tokens\"` section declares its tokens",
                path.display()
            ),
        ));
    };
    let mut root = Group::default();
    for (name, entry) in tokens {
        let kind = entry
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| syn::Error::new(span, format!("`tokens.{name}`: no `kind`")))?;
        let ty = value_type(kind)
            .ok_or_else(|| syn::Error::new(span, format!("`tokens.{name}`: no kind `{kind}`")))?;
        let doc = entry
            .get("doc")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let key_path = entry
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(name);
        let segments: Vec<&str> = key_path.split('.').collect();
        if segments.iter().any(|s| s.is_empty()) {
            return Err(syn::Error::new(
                span,
                format!("`tokens.{name}`: the path `{key_path}` has an empty segment"),
            ));
        }
        let (leaf, groups) = segments.split_last().expect("split gives one segment");
        let mut group = &mut root;
        for segment in groups {
            group = group.groups.entry(module_name(segment)).or_default();
        }
        let constant = constant_name(leaf);
        if let Some(other) = group.keys.get(&constant) {
            return Err(syn::Error::new(
                span,
                format!(
                    "`tokens.{name}` and `tokens.{}` are both the key `{constant}` of their group",
                    other.name
                ),
            ));
        }
        group.keys.insert(
            constant,
            Key {
                name: name.clone(),
                kind: kind.to_owned(),
                doc: doc.to_owned(),
                ty,
            },
        );
    }
    let nsh_core = crate::root::nsh_core();
    let items = root.items(&nsh_core, span)?;
    let path = path.to_string_lossy().into_owned();
    Ok(quote! {
        // The document is read here, so a change of it builds the crate
        // again.
        const _: &str = ::core::include_str!(#path);
        #items
    })
}

/// A group of the document's tree: its subgroups (modules) and keys.
#[derive(Default)]
struct Group {
    groups: BTreeMap<String, Group>,
    keys: BTreeMap<String, Key>,
}

/// A token's key.
struct Key {
    name: String,
    kind: String,
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
        for (name, group) in &self.groups {
            let ident = if matches!(name.as_str(), "self" | "super" | "crate") {
                format_ident!("{name}_", span = span)
            } else if is_keyword(name) {
                syn::Ident::new_raw(name, span)
            } else {
                format_ident!("{name}", span = span)
            };
            let items = group.items(nsh_core, span)?;
            let doc = format!(" The tokens of the group `{name}`.");
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

/// The Rust type of a kind's value.
fn value_type(kind: &str) -> Option<fn(&TokenStream2) -> TokenStream2> {
    Some(match kind {
        "color" => |c| quote!(#c::element::AdaptiveColor),
        "number" | "weight" | "leading" => |_| quote!(::core::primitive::f32),
        "radius" | "spacing" | "text" | "tracking" | "blur" => |c| quote!(#c::geometry::Points),
        "font" => |c| quote!(#c::element::SharedString),
        "shadow" => |c| quote!(#c::element::AdaptiveShadow),
        "gradient" => |c| quote!(#c::element::AdaptiveGradient),
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

/// A path's last segment as a constant's name: upper snake case
/// (`PRIMARY`, `TEXT_XL`), `_` in front of a digit.
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

/// The document's path: `lit` relative to the calling crate's manifest
/// directory (or the directory a crate `nsh` built from its own manifest
/// names, as `asset!` reads it).
fn document_path(lit: &LitStr) -> syn::Result<PathBuf> {
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
    Ok(dir.join(lit.value()))
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
    }
}
