//! nsh-theme's tests, one module per group: `document` (reading, values,
//! writing, editing), `resolve` (modifiers, references, layers, patches,
//! roles) and `spec` (the examples of `docs/dev/theme-format.md`).

mod document;
mod resolve;
mod spec;

use nsh_theme::{Diagnostic, Read, Severity};

/// The codes of `diagnostics`, in order.
fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
    diagnostics.iter().map(|d| d.code).collect()
}

/// The (code, pointer) pairs of `diagnostics`, sorted.
fn sorted(diagnostics: &[Diagnostic]) -> Vec<(&'static str, String)> {
    let mut pairs: Vec<_> = diagnostics
        .iter()
        .map(|d| (d.code, d.pointer.clone()))
        .collect();
    pairs.sort();
    pairs
}

/// Reads `text` and asserts it gives no diagnostic.
fn clean(text: &str) -> Read {
    let read = nsh_theme::read(text);
    assert!(
        read.diagnostics.is_empty(),
        "unexpected diagnostics: {:#?}",
        read.diagnostics
    );
    read
}

/// Reads `text` and asserts it gives exactly one diagnostic, of `code`, at
/// `pointer`, of the code's severity.
fn one(text: &str, code: &str, pointer: &str) -> Read {
    let read = nsh_theme::read(text);
    assert_eq!(
        read.diagnostics.len(),
        1,
        "expected one {code}: {:#?}",
        read.diagnostics
    );
    let diagnostic = &read.diagnostics[0];
    assert_eq!(diagnostic.code, code, "{diagnostic:#?}");
    assert_eq!(diagnostic.pointer, pointer, "{diagnostic:#?}");
    let severity = nsh_theme::code::ALL
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, s)| *s)
        .unwrap_or(Severity::Error);
    assert_eq!(diagnostic.severity, severity);
    assert!(!diagnostic.message.is_empty());
    read
}

/// A document of one token of `kind` with `value` (JSON text).
fn token_doc(kind: &str, value: &str) -> String {
    format!(r#"{{ "tokens": {{ "t": {{ "kind": "{kind}", "value": {value} }} }} }}"#)
}
