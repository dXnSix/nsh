//! The active theme document, and how the runtime reports a theme's
//! diagnostics.
//!
//! A theme document (`docs/dev/theme-format.md`, read by `nsh-theme`)
//! declares tokens with a value type and a value, modifiers whose contexts
//! change token values, and roles. One document is active at a time:
//!
//! 1. the app's theme: [`super::Root::theme`] or `nsh.json`'s `"theme"`,
//!    replaced at run time by [`super::set_theme`] (TS: `setTheme`);
//! 2. else the document of the first UI set, in the host's set order, that
//!    registers one ([`super::Registry::theme_document`]); each other set's
//!    gives the warning `document-unused`;
//! 3. else the document of [`super::Registry::fallback_theme_document`].
//!
//! Documents are never merged. A text that is no document (`not-a-document`)
//! or is in the old `cssVars` format (`legacy-format`) is never active.
//!
//! No theme input panics: each diagnostic is logged once, with
//! `log::warn!` and the target `nsh::theme`, as `{code} {pointer}:
//! {message}`, and a document with errors loads with its good entries.

use nsh_theme::{Diagnostic, ThemeDocument, code};

/// The log target of a theme's diagnostics.
pub const LOG_TARGET: &str = "nsh::theme";

/// Logs each of `diagnostics` (`{code} {pointer}: {message}`).
pub(crate) fn report(diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        log::warn!(target: LOG_TARGET, "{} {}: {}", d.code, d.pointer, d.message);
    }
}

/// Whether `diagnostics` refuse the text whole: it is no document, or one
/// of the old format. Such a text is never active.
pub(crate) fn refused(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code == code::NOT_A_DOCUMENT || d.code == code::LEGACY_FORMAT)
}

/// Reads theme document `text` and logs its diagnostics: the document
/// (`None` when it is refused whole, [`refused`]) and the diagnostics.
pub(crate) fn read_document(text: &str) -> (Option<ThemeDocument>, Vec<Diagnostic>) {
    let read = nsh_theme::read(text);
    report(&read.diagnostics);
    let document = (!refused(&read.diagnostics)).then_some(read.document);
    (document, read.diagnostics)
}

/// Reads patch `text` (the app's or the host's overrides) and logs its
/// diagnostics. Text that is no patch gives an empty one.
pub(crate) fn read_patch(text: &str) -> (nsh_theme::Patch, Vec<Diagnostic>) {
    let (patch, diagnostics) = nsh_theme::read_patch(text);
    report(&diagnostics);
    (patch, diagnostics)
}

/// The warning a pick of `context` for modifier `name` gives in `document`
/// (`unknown-modifier`, `unknown-context`), `None` when both exist.
pub(crate) fn unknown_pick(
    document: &ThemeDocument,
    name: &str,
    context: &str,
) -> Option<Diagnostic> {
    let Some(modifier) = document.modifiers.get(name) else {
        return Some(Diagnostic::warning(
            code::UNKNOWN_MODIFIER,
            pointer(["modifiers", name]),
            format!("The theme has no modifier `{name}`; the pick is ignored."),
        ));
    };
    (!modifier.contexts.contains_key(context)).then(|| {
        Diagnostic::warning(
            code::UNKNOWN_CONTEXT,
            pointer(["modifiers", name, "contexts", context]),
            format!("The modifier `{name}` has no context `{context}`; the pick is ignored."),
        )
    })
}

/// A JSON pointer of `parts`, each escaped (`~0`, `~1`).
fn pointer<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for part in parts {
        out.push('/');
        out.push_str(&part.replace('~', "~0").replace('/', "~1"));
    }
    out
}
