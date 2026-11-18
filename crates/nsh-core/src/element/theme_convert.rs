//! Conversion only: an old `cssVars` theme (shadcn's `{ "cssVars": {
//! "theme": { … }, "light": { … }, "dark": { … } } }`, with the old
//! `tokens`, `modifiers` and `roles` sections) written as a theme document
//! in the theme format (`docs/dev/theme-format.md`, section 12).
//!
//! [`convert`] reads the old file with the old reader, kept here and nowhere
//! else (its `cssVars` reading, kind guessing, `var()` copying and
//! typography parts, the old per-appearance values and the old modifiers'
//! application, and how the old runtime layered an app's theme over a UI
//! set's default), over the old base it extended when one is given, and
//! writes what it read as one document with `nsh_theme::write`. The runtime
//! never calls it and never reads `cssVars`: only `nsh theme convert` and
//! tests do. Nothing writes `cssVars`.
//!
//! ```
//! use nsh_core::element::theme_convert::convert;
//!
//! let (text, diagnostics) = convert(
//!     r##"{ "cssVars": { "light": { "surface": "#ffffff" }, "dark": { "surface": "#111111" } } }"##,
//!     None,
//! );
//! assert!(diagnostics.is_empty());
//! let read = nsh_theme::read(&text);
//! assert!(read.diagnostics.is_empty());
//! assert!(read.document.modifiers.contains_key("colorMode"));
//! ```

mod modifiers;
mod order;
mod reader;
mod store;
#[cfg(test)]
mod tests;
mod writer;

use nsh_theme::Diagnostic;

use crate::JsonValue;

/// The codes of the diagnostics [`convert`] reports beside nsh-theme's own
/// (`nsh_theme::code::NOT_A_DOCUMENT` for a text that is no JSON object).
pub mod code {
    /// The old theme, or the base it is converted over, already has a
    /// modifier named `colorMode`, the name the converter gives the
    /// appearance: it is not converted (error).
    pub const COLOR_MODE_TAKEN: &str = "color-mode-taken";
    /// The old reader left this out of the old theme: a value of no kind
    /// a token has, a name nothing declares, a modifier it could not read
    /// (warning).
    pub const LEFT_OUT: &str = "left-out";
    /// The theme format can't express this, and it is left out: a
    /// shadow of no layer, several gradients in one value, a gradient
    /// toward a corner, a role onto no token (warning).
    pub const INEXPRESSIBLE: &str = "inexpressible";
}

/// nsh-base's default theme as it was in the old format (shadcn's
/// `neutral`): the library default that old app themes extended, to pass
/// as `over` when converting one of them ([`convert`]'s rule 7).
pub const OLD_DEFAULT: &str = include_str!("theme_convert/fixtures/default-theme.json");

/// Converts the old `cssVars` theme `old` to a theme document in the theme
/// format, returned as its text, with what it reported.
///
/// With `over`, `old` is an app's theme that extended `over`, a library's
/// default theme in the old format, and the document is complete, as the
/// old runtime layered them (rule 7).
///
/// The rules (`docs/dev/theme-format.md`, section 12):
/// 1. every name of `cssVars`' sections and every declared token becomes a
///    token: number, radius, spacing, text, weight, tracking, leading and
///    blur become `number`, a typography a `typography` built from its size
///    and parts, a name with no declaration the kind the old reader guessed;
///    paths and docs are copied;
/// 2. a token's value is the `theme` value, else the `light` one, in the
///    format's forms (`#rrggbb[aa]`, points, structured shadows and
///    gradients);
/// 3. when any value differs between `light` and `dark`, the modifier
///    `colorMode` (default `light`, `light` empty, `dark` with the dark
///    values) comes first in `resolutionOrder`;
/// 4. the old modifiers keep their names, contexts and defaults, after it;
///    a context's value that differs by appearance refers to a helper token
///    `<token>.<modifier>.<context>` holding the light value, which
///    `colorMode`'s `dark` sets to the dark one;
/// 5. a whole `var(--x)`, of any type, becomes `{x}` (`x`'s token when `x`
///    is a role), unless a context of an old modifier sets `x` or a token
///    `x` refers to, `x` has another type, or the old reader read a value
///    that `x`'s differs from (a string's own text `var(--x)`): then the old
///    reader's value stays, so the reference resolves to what the old
///    reader read in every combination. A whole `var(--x)` the old reader
///    read no value of (a font's, a boolean's) becomes `{x}` on the same
///    conditions, and its left-out is not reported. Any other `var()` is
///    copied;
/// 6. roles are copied;
/// 7. `over` makes one complete document, as the old runtime read it: a
///    base that declares `tokens` is a registered theme document, and `old`
///    is read by its declarations; an `old` that declares `tokens` over a
///    base that declares none replaces it, and only its own names are kept;
/// 8. what the format can't express is reported ([`code::INEXPRESSIBLE`])
///    and left out.
///
/// Nothing is converted, and the text is empty, when `old` or `over` is no
/// JSON object (`not-a-document`) or has a modifier named `colorMode`
/// ([`code::COLOR_MODE_TAKEN`]). Never panics.
pub fn convert(old: &str, over: Option<&str>) -> (String, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    for (text, base) in [(Some(old), false), (over, true)] {
        let Some(text) = text else {
            continue;
        };
        let which = if base { "the base theme" } else { "the theme" };
        match serde_json::from_str::<JsonValue>(text) {
            Ok(value) if value.is_object() => {
                if value
                    .get("modifiers")
                    .and_then(|m| m.get(writer::COLOR_MODE))
                    .is_some()
                {
                    diagnostics.push(Diagnostic::error(
                        code::COLOR_MODE_TAKEN,
                        reader::pointer(&["modifiers", writer::COLOR_MODE]),
                        format!(
                            "{which} has a modifier named `colorMode`, the name the conversion \
                             gives the appearance; it is not converted"
                        ),
                    ));
                }
            }
            Ok(_) => diagnostics.push(Diagnostic::error(
                nsh_theme::code::NOT_A_DOCUMENT,
                "",
                format!("{which} is not a JSON object; it is not converted"),
            )),
            Err(e) => diagnostics.push(Diagnostic::error(
                nsh_theme::code::NOT_A_DOCUMENT,
                "",
                format!("{which} is not JSON ({e}); it is not converted"),
            )),
        }
    }
    if !diagnostics.is_empty() {
        return (String::new(), diagnostics);
    }
    let old = match reader::read_old(old, over) {
        Ok(old) => old,
        Err((base, e)) => {
            let which = if base { "the base theme" } else { "the theme" };
            diagnostics.push(Diagnostic::error(
                nsh_theme::code::NOT_A_DOCUMENT,
                "",
                format!("{which}: {e}; it is not converted"),
            ));
            return (String::new(), diagnostics);
        }
    };
    let mut written = Vec::new();
    let (document, recovered) = writer::document(&old, &mut written);
    for (base, left_out) in &old.left_out {
        // A whole `var(--x)` the old reader read no value of is a reference
        // now (rule 5).
        let sentence = left_out.to_string();
        if !base
            && recovered
                .iter()
                .any(|name| sentence.starts_with(&format!("`{name}`")))
        {
            continue;
        }
        let at = match left_out.token() {
            Some(token) if !base => old.written.get(token).cloned().unwrap_or_default(),
            _ => String::new(),
        };
        let which = if *base { "the base theme: " } else { "" };
        diagnostics.push(Diagnostic::warning(
            code::LEFT_OUT,
            at,
            format!("{which}{left_out}; the old reader left it out"),
        ));
    }
    diagnostics.extend(written);
    (nsh_theme::write(&document), diagnostics)
}
