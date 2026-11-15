//! The token dictionary from TSX (`tokens` of `nsh`): a token with its kind
//! and both appearances' values, the kind alone, the names (of a kind, with
//! a prefix) and the entries with what declares them, reactive to a token
//! added while the app runs.

use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const APP: &str = r##"import { mount, signal, Text, tokens, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ cssVars: { theme: { "radius-chip": "4px" } }, tokens: { "radius-chip": { kind: "radius" } } });
  step.set(1);
}, 30);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`get ${JSON.stringify(tokens.get("page"))}`}</Text>
    <Text>{`kind ${tokens.kind("radius-card")} ${tokens.kind("nothing")}`}</Text>
    <Text>{`radii ${tokens.names({ kind: "radius" }).join(",")}`}</Text>
    <Text>{`entry ${JSON.stringify(tokens.entries().find(e => e.name === "page"))}`}</Text>
  </View>
));
"##;

const CONFIG: &str = r##"{ "theme": {
    "cssVars": { "light": { "page": "#ffffff" }, "dark": { "page": "#000000" }, "theme": { "radius-card": "12px" } },
    "tokens": {
        "page": { "kind": "color", "path": "colors.page", "doc": "The page." },
        "radius-card": { "kind": "radius" } } } }"##;

#[test]
fn tokens_reads_the_dictionary_and_follows_a_name_added() {
    let (_engine, rx) = start_with(APP, Environment::default(), None, Some(CONFIG));
    let batches = until(&rx, |b| shows(b, "step 0"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    assert!(
        said(r##"get {"kind":"color","light":"#ffffffff","dark":"#000000ff"}"##),
        "{batches:?}"
    );
    assert!(said("kind radius undefined"), "{batches:?}");
    assert!(said("radii radius-card"), "{batches:?}");
    assert!(
        said(
            r#"entry {"name":"page","kind":"color","path":"colors.page","doc":"The page.","owner":"the app's theme"}"#
        ),
        "{batches:?}"
    );
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches
            .iter()
            .any(|b| shows(b, "radii radius-card,radius-chip")),
        "{batches:?}"
    );
}

const KINDS_APP: &str = r##"import { mount, Text, token, tokens, View } from "nsh";

mount(() => (
  <View>
    <Text>{`string ${JSON.stringify(token("brand-name" as never))}`}</Text>
    <Text>{`boolean ${JSON.stringify(token("feature-beta" as never))}`}</Text>
    <Text>{`duration ${JSON.stringify(token("duration-fast" as never))}`}</Text>
    <Text>{`easing ${JSON.stringify(token("ease-snappy" as never))}`}</Text>
    <Text>{`typography ${JSON.stringify(tokens.get("text-title"))}`}</Text>
    <Text class="transition-colors duration-fast ease-snappy text-title">styled</Text>
  </View>
));
"##;

const KINDS_CONFIG: &str = r##"{ "theme": {
    "cssVars": { "theme": {
        "brand-name": "Acme", "feature-beta": true, "duration-fast": "120ms",
        "ease-snappy": "cubic-bezier(0.2, 0, 0, 1)",
        "text-title": "24px", "text-title--font-weight": "650" } },
    "tokens": {
        "brand-name": { "kind": "string" }, "feature-beta": { "kind": "boolean" },
        "duration-fast": { "kind": "duration" }, "ease-snappy": { "kind": "easing" },
        "text-title": { "kind": "typography" } } } }"##;

/// String, boolean, duration, easing and typography tokens from TSX:
/// `token()` gives each as its kind, `tokens.get` a text style with its
/// parts, and their classes apply.
#[test]
fn token_reads_strings_booleans_durations_easings_and_text_styles() {
    let (_engine, rx) = start_with(KINDS_APP, Environment::default(), None, Some(KINDS_CONFIG));
    let batches = until(&rx, |b| shows(b, "styled"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    for text in [
        "string \"Acme\"",
        "boolean true",
        "duration 120",
        "easing \"cubic-bezier(0.2, 0, 0, 1)\"",
        "typography {\"kind\":\"typography\",\"light\":{\"size\":24,\"fontWeight\":650},\"dark\":{\"size\":24,\"fontWeight\":650}}",
    ] {
        assert!(said(text), "{text}: {batches:?}");
    }
    let styled = batches.iter().flat_map(|b| &b.items).any(|i| matches!(
        i,
        nsh_core::MountItem::ViewStyle { style, .. }
            if style.transition.is_some_and(|t| t.duration == 120.0 && t.easing == [0.2, 0.0, 0.0, 1.0])
    ));
    assert!(styled, "{batches:?}");
}
