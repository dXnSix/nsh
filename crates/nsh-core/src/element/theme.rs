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
//! or is in the old format (`legacy-format`) is never active.
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

/// Whether `diagnostics` (of [`read_document`] or [`read_patch`]) refuse
/// the text whole: it is no document or patch (`not-a-document`), or a
/// document of the old format (`legacy-format`). Such a text is never
/// active, and a host that is given it changes nothing.
pub fn refused(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code == code::NOT_A_DOCUMENT || d.code == code::LEGACY_FORMAT)
}

/// Reads theme document `text` and logs its diagnostics (`nsh::theme`):
/// the document (`None` when it is refused whole: no document,
/// `not-a-document`, or one of the old format, `legacy-format`)
/// and the diagnostics. The script runtimes read TS's `setTheme` with it.
pub fn read_document(text: &str) -> (Option<ThemeDocument>, Vec<Diagnostic>) {
    let read = nsh_theme::read(text);
    report(&read.diagnostics);
    let document = (!refused(&read.diagnostics)).then_some(read.document);
    (document, read.diagnostics)
}

/// Reads patch `text` (the app's or the host's overrides, TS's
/// `updateTheme`) and logs its diagnostics (`nsh::theme`). Text that is no
/// patch gives an empty one.
pub fn read_patch(text: &str) -> (nsh_theme::Patch, Vec<Diagnostic>) {
    let (patch, diagnostics) = nsh_theme::read_patch(text);
    report(&diagnostics);
    (patch, diagnostics)
}

/// The warning a pick of `context` for modifier `name` gives in `document`
/// (`unknown-modifier`, `unknown-context`), `None` when both exist: what
/// the core's `set_modifier` logs, and TS's `setModifier` throws as a
/// `TypeError`.
pub fn unknown_pick(document: &ThemeDocument, name: &str, context: &str) -> Option<Diagnostic> {
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

/// Reads the theme a bundle's setup ships (`setup.ui`: `{ "document": …,
/// "overrides": … }`, each `nsh.json`'s `"theme"` and `"themeOverrides"` as
/// written, or `null`) and logs its diagnostics: the app's built-in document
/// (`None`: none, or one refused whole, as [`read_document`] refuses it),
/// its overrides (`None`: none) and the diagnostics. Text that is no such
/// object gives one `not-a-document` error and neither.
pub fn read_app_theme(
    text: &str,
) -> (
    Option<ThemeDocument>,
    Option<nsh_theme::Patch>,
    Vec<Diagnostic>,
) {
    // Taken apart by nsh-theme, which keeps the members' order (a
    // `serde_json::Value` sorts them without serde_json's `preserve_order`,
    // and a document's order is its own: its modifiers' contexts).
    let (setup, mut said) = nsh_theme::object_members(text);
    let Some(setup) = setup else {
        let said = vec![Diagnostic::error(
            code::NOT_A_DOCUMENT,
            "",
            "The app's theme setup is no `{ document, overrides }` object; the app has no theme \
             of its own.",
        )];
        report(&said);
        return (None, None, said);
    };
    report(&said);
    let part = |key: &str| {
        setup
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, text)| text.clone())
            .filter(|text| text.trim() != "null")
    };
    let document = part("document").and_then(|text| {
        let (document, diagnostics) = read_document(&text);
        said.extend(diagnostics);
        document
    });
    let overrides = part("overrides").map(|text| {
        let (patch, diagnostics) = read_patch(&text);
        said.extend(diagnostics);
        patch
    });
    (document, overrides, said)
}

/// Keeps the picks of `selection` whose modifier and context `document`
/// has: what activating `document` keeps of the app's picks (the others
/// show their defaults again).
pub(crate) fn keep_picks(selection: &mut nsh_theme::Selection, document: &ThemeDocument) {
    selection.picks.retain(|name, context| {
        document
            .modifiers
            .get(name)
            .is_some_and(|m| m.contexts.contains_key(context))
    });
}

/// `diagnostics` as TS's `setTheme` and `updateTheme` return them: an
/// array of `{ severity, code, pointer, message }` (`ThemeDiagnostic`), the
/// same on every script runtime.
pub fn diagnostics_json(diagnostics: &[Diagnostic]) -> crate::JsonValue {
    diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "severity": d.severity.name(),
                "code": d.code,
                "pointer": d.pointer,
                "message": d.message,
            })
        })
        .collect()
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
