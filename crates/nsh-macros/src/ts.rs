//! The TypeScript text the macros take: `#[ts("<T>(token: ServiceKey<T>): T")]`
//! signatures and `#[class(generic = "T")]` type parameters. They are read
//! only as far as checking them against the Rust side needs: the brackets
//! balance, the parameters are named and typed, and they are the Rust
//! function's. The types themselves are TypeScript's to check.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, LitStr};

/// A signature: `<T>(token: ServiceKey<T>): T`.
#[derive(Debug, PartialEq)]
pub(crate) struct Signature {
    /// Its type parameters as written, brackets included (`<T>`), or empty.
    pub(crate) generics: String,
    /// Its parameters.
    pub(crate) params: Vec<SigParam>,
    /// Its result's type, if it gives one (`void` is none).
    pub(crate) returns: Option<String>,
}

/// A parameter of a [`Signature`]: `token: ServiceKey<T>`.
#[derive(Debug, PartialEq)]
pub(crate) struct SigParam {
    pub(crate) name: String,
    /// `name?: T`.
    pub(crate) optional: bool,
    pub(crate) ts: String,
}

/// What a signature is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SigKind {
    /// A method, static or module function: type parameters allowed.
    Function,
    Constructor,
    /// A class's getter or a module's: `(): T`.
    Getter,
    Setter,
}

/// The Rust side of a signature: what it is, the parameters JS passes
/// (their names in TS, and whether each is optional: a trailing `Option`)
/// and whether it has a result (not `()`).
pub(crate) struct Shape<'a> {
    pub(crate) kind: SigKind,
    pub(crate) params: &'a [(String, bool)],
    pub(crate) value: bool,
}

/// A character of the text outside strings and comments, and the depth of
/// brackets it is at (an opening bracket's is outside it, a closing one's
/// inside).
struct Token {
    at: usize,
    c: char,
    depth: usize,
}

/// The characters of `text` that count: outside string literals, template
/// literals and comments, each with its depth. The brackets `()`, `[]`,
/// `{}` and `<>` must balance (`>` of `=>` closes nothing).
fn scan(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut open: Vec<char> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        let next = chars.get(i + 1).map(|&(_, c)| c);
        match c {
            '"' | '\'' | '`' => {
                // To the closing quote; a template's `${..}` holds no quote
                // that would end it early in a type.
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return Err(format!("the string at {at} is never closed")),
                        Some(&(_, '\\')) => i += 2,
                        Some(&(_, q)) if q == c => break,
                        Some(_) => i += 1,
                    }
                }
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i].1 != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if next == Some('*') => {
                let rest = &text[at + 2..];
                let Some(end) = rest.find("*/") else {
                    return Err("a comment is never closed".to_owned());
                };
                let end = at + 2 + end + 2;
                while i < chars.len() && chars[i].0 < end {
                    i += 1;
                }
                continue;
            }
            '(' | '[' | '{' | '<' => {
                out.push(Token {
                    at,
                    c,
                    depth: open.len(),
                });
                open.push(c);
            }
            // `=>`: an arrow, not a bracket (and nothing the reading needs).
            '>' if i > 0 && chars[i - 1].1 == '=' => {}
            ')' | ']' | '}' | '>' => {
                let opener = match c {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '<',
                };
                match open.pop() {
                    Some(o) if o == opener => {}
                    Some(o) => return Err(format!("`{c}` closes `{o}`")),
                    None => return Err(format!("`{c}` closes nothing")),
                }
                out.push(Token {
                    at,
                    c,
                    depth: open.len() + 1,
                });
            }
            _ => out.push(Token {
                at,
                c,
                depth: open.len(),
            }),
        }
        i += 1;
    }
    match open.last() {
        Some(o) => Err(format!("`{o}` is never closed")),
        None => Ok(out),
    }
}

/// The pieces of `text` between its commas outside brackets, trimmed; a
/// trailing comma ends no piece.
fn split_commas(text: &str) -> Result<Vec<&str>, String> {
    let mut out = Vec::new();
    let mut start = 0;
    for t in scan(text)? {
        if t.c == ',' && t.depth == 0 {
            out.push(text[start..t.at].trim());
            start = t.at + 1;
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() || !out.is_empty() {
        out.push(last);
    }
    if out.last() == Some(&"") {
        out.pop();
    }
    if out.contains(&"") {
        return Err("two commas with nothing between them".to_owned());
    }
    Ok(out)
}

/// The identifier `text` starts with (`T`, `token`), and the rest.
fn identifier(text: &str) -> Option<(&str, &str)> {
    let end = text
        .char_indices()
        .find(|&(i, c)| {
            !(c == '_' || c == '$' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        })
        .map_or(text.len(), |(i, _)| i);
    (end > 0).then(|| (&text[..end], &text[end..]))
}

/// A type parameter: `in out V = never`.
#[derive(Debug, PartialEq)]
pub(crate) struct TypeParam {
    pub(crate) name: String,
    /// It has a variance (`in` or `out` before its name).
    pub(crate) variance: bool,
    /// Its whole declaration.
    pub(crate) text: String,
}

/// Type parameters (`T`, `T extends object = object, U`, without the
/// brackets).
pub(crate) fn type_params(text: &str) -> Result<Vec<TypeParam>, String> {
    let pieces = split_commas(text)?;
    if pieces.is_empty() {
        return Err("no type parameters: name at least one (`T`)".to_owned());
    }
    let mut out: Vec<TypeParam> = Vec::new();
    for piece in pieces {
        // `const T`, `in T`, `out T`, `in out T`.
        let mut rest = piece;
        let mut variance = false;
        loop {
            match identifier(rest) {
                Some((m, after))
                    if matches!(m, "const" | "in" | "out")
                        && after.starts_with(char::is_whitespace) =>
                {
                    variance |= m != "const";
                    rest = after.trim_start();
                }
                _ => break,
            }
        }
        let Some((name, after)) = identifier(rest) else {
            return Err(format!("`{piece}` names no type parameter"));
        };
        let after = after.trim_start();
        let constrained = after
            .strip_prefix("extends")
            .is_some_and(|a| a.starts_with(char::is_whitespace) && !a.trim().is_empty());
        let defaulted = after
            .strip_prefix('=')
            .is_some_and(|a| !a.starts_with('>') && !a.trim().is_empty());
        if !after.is_empty() && !constrained && !defaulted {
            return Err(format!(
                "`{piece}`: a type parameter is a name, then `extends T` or `= T`"
            ));
        }
        if out.iter().any(|p| p.name == name) {
            return Err(format!("type parameter `{name}` is named twice"));
        }
        out.push(TypeParam {
            name: name.to_owned(),
            variance,
            text: piece.to_owned(),
        });
    }
    Ok(out)
}

/// Reads signature `text`: `<T>(token: ServiceKey<T>): T`.
pub(crate) fn signature(text: &str) -> Result<Signature, String> {
    let text = text.trim();
    let tokens = scan(text)?;
    let shape = "a signature is `(name: Type): Result`, with `<T>` before it when it is generic";
    // The bracket closing the one opened at token `i`.
    let close_of = |i: usize| {
        let depth = tokens[i].depth + 1;
        tokens[i + 1..]
            .iter()
            .position(|t| matches!(t.c, ')' | ']' | '}' | '>') && t.depth == depth)
            .map(|j| i + 1 + j)
            .expect("scanned brackets balance")
    };
    let mut i = 0;
    let mut generics = String::new();
    if tokens.first().is_some_and(|t| t.c == '<') {
        let close = close_of(0);
        type_params(&text[1..tokens[close].at])?;
        generics = text[..=tokens[close].at].to_owned();
        i = close + 1;
    }
    // Whitespace between them counts for nothing.
    while tokens.get(i).is_some_and(|t| t.c.is_whitespace()) {
        i += 1;
    }
    if tokens.get(i).is_none_or(|t| t.c != '(') {
        return Err(shape.to_owned());
    }
    let close = close_of(i);
    let inner = &text[tokens[i].at + 1..tokens[close].at];
    let mut params = Vec::new();
    for piece in split_commas(inner)? {
        if piece.starts_with("...") {
            return Err(format!(
                "`{piece}`: a rest parameter, which a Rust function does not take"
            ));
        }
        let Some((name, after)) = identifier(piece) else {
            return Err(format!(
                "`{piece}`: each parameter is named (`name: Type`), as the Rust function's"
            ));
        };
        if name == "this" {
            return Err(format!(
                "`{piece}`: a `this` parameter is not one JS passes; leave it out"
            ));
        }
        let after = after.trim_start();
        let (optional, after) = match after.strip_prefix('?') {
            Some(a) => (true, a.trim_start()),
            None => (false, after),
        };
        let ts = after.strip_prefix(':').map(str::trim).unwrap_or_default();
        if ts.is_empty() {
            return Err(format!(
                "`{piece}`: give the parameter its type (`{name}: Type`)"
            ));
        }
        params.push(SigParam {
            name: name.to_owned(),
            optional,
            ts: ts.to_owned(),
        });
    }
    let rest = text[tokens[close].at + 1..].trim();
    let returns = if rest.is_empty() {
        None
    } else {
        match rest.strip_prefix(':').map(str::trim) {
            Some("") | None => {
                return Err(format!(
                    "`{rest}`: after the parameters comes the result (`: Type`), or nothing"
                ));
            }
            Some("void") => None,
            Some(ts) => Some(ts.to_owned()),
        }
    };
    Ok(Signature {
        generics,
        params,
        returns,
    })
}

/// Checks `sig` against the Rust function it declares: its parameters are
/// the ones JS passes, by name and in order (trailing optional ones may be
/// left out; `?` only on an optional one), and it gives a result when, and
/// only when, the function returns a value.
pub(crate) fn check(sig: &Signature, shape: &Shape<'_>) -> Result<(), String> {
    let what = match shape.kind {
        SigKind::Function => "the function",
        SigKind::Constructor => "the constructor",
        SigKind::Getter => "the getter",
        SigKind::Setter => "the setter",
    };
    if !sig.generics.is_empty() && shape.kind != SigKind::Function {
        return Err(format!(
            "{what} has no type parameters in TypeScript (only methods, statics and functions do)"
        ));
    }
    let rust: Vec<&str> = shape.params.iter().map(|(n, _)| n.as_str()).collect();
    if sig.params.len() > shape.params.len() {
        let n = sig.params.len();
        return Err(format!(
            "{n} {}, but JS passes {what} {} ({})",
            if n == 1 { "parameter" } else { "parameters" },
            shape.params.len(),
            list(&rust)
        ));
    }
    for (p, (name, optional)) in sig.params.iter().zip(shape.params) {
        if &p.name != name {
            return Err(format!(
                "`{}` is `{name}` in Rust: the signature's parameters are the Rust function's, \
                 in its order ({})",
                p.name,
                list(&rust)
            ));
        }
        if p.optional && !optional {
            return Err(format!(
                "`{name}?`: `{name}` is required in Rust (a trailing `Option` is optional)"
            ));
        }
    }
    if let Some((name, _)) = shape.params[sig.params.len()..]
        .iter()
        .find(|(_, optional)| !optional)
    {
        return Err(format!(
            "`{name}` is left out, but it is required in Rust ({})",
            list(&rust)
        ));
    }
    match (shape.kind, &sig.returns) {
        (SigKind::Constructor | SigKind::Setter, Some(_)) => Err(format!(
            "{what} gives no result in TypeScript: leave `: Type` out"
        )),
        (SigKind::Constructor | SigKind::Setter, None) => Ok(()),
        (_, None) if shape.value => Err(format!(
            "{what} returns a value: give its type after the parameters (`: Type`)"
        )),
        (_, Some(ts)) if !shape.value => Err(format!(
            "{what} returns nothing, so its result is `void`, not `{ts}`: leave it out"
        )),
        _ => Ok(()),
    }
}

/// `a, b` for the messages; `none` when empty.
fn list(names: &[&str]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The `Overload` of `#[ts("…")]` attribute `attr` on a function of
/// `shape`: the signature it reads, checked against the function.
pub(crate) fn overload(attr: &Attribute, shape: &Shape<'_>) -> syn::Result<TokenStream2> {
    let nsh_core = crate::root::nsh_core();
    let ui = quote!(#nsh_core::element);
    let lit: LitStr = attr.parse_args().map_err(|e| {
        syn::Error::new(
            e.span(),
            "`#[ts(\"…\")]` takes the TypeScript signature as a string: \
             `#[ts(\"<T>(token: Key<T>): T\")]`",
        )
    })?;
    let sig = signature(&lit.value())
        .and_then(|sig| check(&sig, shape).map(|()| sig))
        .map_err(|e| syn::Error::new(lit.span(), e))?;
    let generics = &sig.generics;
    let params = sig.params.iter().map(|p| {
        let (name, ts, optional) = (&p.name, &p.ts, p.optional);
        quote! {
            (
                #name,
                |_: &mut #ui::TsTypes| ::std::string::String::from(#nsh_core::decl!(#ts)),
                #optional,
            )
        }
    });
    let returns = match &sig.returns {
        Some(ts) => quote! {
            ::core::option::Option::Some(
                |_: &mut #ui::TsTypes| ::std::string::String::from(#nsh_core::decl!(#ts))
            )
        },
        None => quote!(::core::option::Option::None),
    };
    Ok(quote! {
        #ui::Overload {
            generics: #nsh_core::decl!(#generics),
            params: &[#(#params),*],
            returns: #returns,
        }
    })
}

#[cfg(test)]
mod ts_tests {
    use super::*;

    fn param(name: &str, optional: bool, ts: &str) -> SigParam {
        SigParam {
            name: name.to_owned(),
            optional,
            ts: ts.to_owned(),
        }
    }

    #[test]
    fn signatures_read_type_parameters_parameters_and_the_result() {
        assert_eq!(
            signature("<T>(token: { readonly prototype: T } | ServiceToken<T>): T").unwrap(),
            Signature {
                generics: "<T>".to_owned(),
                params: vec![param(
                    "token",
                    false,
                    "{ readonly prototype: T } | ServiceToken<T>"
                )],
                returns: Some("T".to_owned()),
            }
        );
        // Arrows, nested generics, strings with brackets and commas.
        assert_eq!(
            signature(
                " <T, U extends Map<string, T> = Map<string, T>>(key: ServiceKey<T>, \
                 factory: (provider: ServiceProvider, at: [number, string]) => T, \
                 label?: \"a, (b\" | null): this "
            )
            .unwrap(),
            Signature {
                generics: "<T, U extends Map<string, T> = Map<string, T>>".to_owned(),
                params: vec![
                    param("key", false, "ServiceKey<T>"),
                    param(
                        "factory",
                        false,
                        "(provider: ServiceProvider, at: [number, string]) => T"
                    ),
                    param("label", true, "\"a, (b\" | null"),
                ],
                returns: Some("this".to_owned()),
            }
        );
        // Nothing, `void` and a trailing comma.
        assert_eq!(
            signature("(x: number,)").unwrap(),
            Signature {
                generics: String::new(),
                params: vec![param("x", false, "number")],
                returns: None,
            }
        );
        assert_eq!(signature("(): void").unwrap().returns, None);
        assert_eq!(
            signature("(): { value: T }").unwrap().returns.as_deref(),
            Some("{ value: T }")
        );
        assert_eq!(
            signature("<T>(f: () => T /* (not a bracket */): T")
                .unwrap()
                .params[0]
                .ts,
            "() => T /* (not a bracket */"
        );
    }

    #[test]
    fn signatures_refuse_what_is_no_signature() {
        for (text, message) in [
            ("T", "a signature is `(name: Type): Result`"),
            ("(x: number", "`(` is never closed"),
            ("(x: Map<string, number>)>", "`>` closes nothing"),
            ("(x: Array<number)>", "`)` closes `<`"),
            ("<>(x: number)", "no type parameters"),
            ("<T, T>(x: T)", "type parameter `T` is named twice"),
            (
                "<T U>(x: T)",
                "a type parameter is a name, then `extends T` or `= T`",
            ),
            ("(...rest: number[])", "a rest parameter"),
            ("({ a }: Options)", "each parameter is named"),
            ("(this: Foo, x: number)", "a `this` parameter"),
            ("(x)", "give the parameter its type"),
            ("(x: number,, y: number)", "two commas"),
            (
                "(x: number) => number",
                "after the parameters comes the result",
            ),
            ("(x: \"open)", "never closed"),
        ] {
            let error = signature(text).unwrap_err();
            assert!(error.contains(message), "{text}: {error}");
        }
    }

    #[test]
    fn type_parameters_read_names_modifiers_constraints_and_defaults() {
        let param = |name: &str, variance, text: &str| TypeParam {
            name: name.to_owned(),
            variance,
            text: text.to_owned(),
        };
        assert_eq!(
            type_params("T, const U extends readonly unknown[], in out V = never").unwrap(),
            vec![
                param("T", false, "T"),
                param("U", false, "const U extends readonly unknown[]"),
                param("V", true, "in out V = never"),
            ]
        );
        assert!(type_params("out T").unwrap()[0].variance);
        assert!(type_params("").unwrap_err().contains("name at least one"));
        assert!(
            type_params("T extends")
                .unwrap_err()
                .contains("a name, then")
        );
        assert!(
            type_params("1T")
                .unwrap_err()
                .contains("names no type parameter")
        );
    }

    fn params(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter().map(|&(n, o)| (n.to_owned(), o)).collect()
    }

    fn checked(
        text: &str,
        kind: SigKind,
        rust: &[(&str, bool)],
        value: bool,
    ) -> Result<(), String> {
        let rust = params(rust);
        check(
            &signature(text).unwrap(),
            &Shape {
                kind,
                params: &rust,
                value,
            },
        )
    }

    #[test]
    fn signatures_are_checked_against_the_rust_function() {
        use SigKind::*;
        let token = [("token", false), ("name", true)];
        assert_eq!(
            checked("<T>(token: Key<T>): T", Function, &token, true),
            Ok(())
        );
        assert_eq!(
            checked(
                "<T>(token: Key<T>, name?: string): T",
                Function,
                &token,
                true
            ),
            Ok(())
        );
        // An optional one may be declared required.
        assert_eq!(
            checked("(token: Key, name: string): T", Function, &token, true),
            Ok(())
        );
        for (text, kind, value, message) in [
            (
                "(key: Key<T>): T",
                Function,
                true,
                "`key` is `token` in Rust",
            ),
            (
                "(token: Key, name: string, more: number): T",
                Function,
                true,
                "3 parameters, but JS passes the function 2 (`token`, `name`)",
            ),
            (
                "(): T",
                Function,
                true,
                "`token` is left out, but it is required",
            ),
            (
                "(token?: Key): T",
                Function,
                true,
                "`token?`: `token` is required in Rust",
            ),
            (
                "(token: Key)",
                Function,
                true,
                "returns a value: give its type",
            ),
            (
                "(token: Key): T",
                Function,
                false,
                "returns nothing, so its result is `void`, not `T`",
            ),
            (
                "<T>(token: Key<T>)",
                Constructor,
                false,
                "the constructor has no type parameters",
            ),
            (
                "(token: Key): T",
                Constructor,
                false,
                "the constructor gives no result",
            ),
        ] {
            let error = checked(text, kind, &token, value).unwrap_err();
            assert!(error.contains(message), "{text}: {error}");
        }
        assert_eq!(checked("(): T", Getter, &[], true), Ok(()));
        assert!(
            checked("<T>(): T", Getter, &[], true)
                .unwrap_err()
                .contains("the getter has no type parameters")
        );
        assert_eq!(
            checked("(value: T)", Setter, &[("value", false)], false),
            Ok(())
        );
        assert!(
            checked("(): T", Function, &[], false)
                .unwrap_err()
                .contains("returns nothing")
        );
        assert!(
            checked("(x: T)", Function, &[], false)
                .unwrap_err()
                .contains("JS passes the function 0 (none)")
        );
    }
}
