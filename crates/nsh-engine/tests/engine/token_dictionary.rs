//! The token dictionary from TSX (`tokens` of `nsh`): a token as
//! `{ kind, value }`, the kind alone, the names (of a kind, with a prefix)
//! and the entries with what the document declares of them, reactive to a
//! name another document adds while the app runs.

use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const APP: &str = r##"import { mount, setTheme, signal, Text, tokens, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  setTheme({
    tokens: {
      page: { kind: "color", value: "#ffffff" },
      "radius-card": { kind: "number", value: 12 },
      "radius-chip": { kind: "number", value: 4 },
    },
  });
  step.set(1);
}, 30);

const entry = () => tokens.entries().find((e) => e.name === "page") ?? {};

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`get ${tokens.get("page")?.kind} ${tokens.get("page")?.value}`}</Text>
    <Text>{`kind ${tokens.kind("radius-card")} ${tokens.kind("nothing")}`}</Text>
    <Text>{`radii ${tokens.names({ kind: "number", prefix: "radius" }).join(",")}`}</Text>
    <Text>{`entry ${entry().kind} ${entry().path} ${entry().doc}`}</Text>
  </View>
));
"##;

const CONFIG: &str = r##"{ "theme": {
    "tokens": {
        "page": { "kind": "color", "value": "#ffffff", "path": "colors.page", "doc": "The page." },
        "radius-card": { "kind": "number", "value": 12 } } } }"##;

#[test]
fn tokens_reads_the_dictionary_and_follows_a_name_added() {
    let (_engine, rx) = start_with(APP, Environment::default(), None, Some(CONFIG));
    let batches = until(&rx, |b| shows(b, "step 0"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    assert!(said("get color #ffffff"), "{batches:?}");
    assert!(said("kind number undefined"), "{batches:?}");
    assert!(said("radii radius-card"), "{batches:?}");
    assert!(said("entry color colors.page The page."), "{batches:?}");
    // Another document: what lists the names follows.
    let batches = until(&rx, |b| shows(b, "step 1"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    assert!(said("radii radius-card,radius-chip"), "{batches:?}");
    assert!(said("entry color undefined undefined"), "{batches:?}");
}

const KINDS_APP: &str = r##"import { mount, Text, token, tokens, View } from "nsh";

const title = () => tokens.get("text-title") ?? { value: {} };

mount(() => (
  <View>
    <Text>{`string ${JSON.stringify(token("brand-name"))}`}</Text>
    <Text>{`boolean ${JSON.stringify(token("feature-beta"))}`}</Text>
    <Text>{`duration ${JSON.stringify(token("duration-fast"))}`}</Text>
    <Text>{`easing ${JSON.stringify(token("ease-snappy"))}`}</Text>
    <Text>{`typography ${title().kind} ${title().value.fontSize} ${title().value.fontWeight} ${title().value.lineHeight}`}</Text>
    <Text class="transition-colors duration-fast ease-snappy text-title">styled</Text>
  </View>
));
"##;

const KINDS_CONFIG: &str = r##"{ "theme": {
    "tokens": {
        "brand-name": { "kind": "string", "value": "Acme" },
        "feature-beta": { "kind": "boolean", "value": true },
        "duration-fast": { "kind": "duration", "value": "120ms" },
        "ease-snappy": { "kind": "easing", "value": [0.2, 0, 0, 1] },
        "text-title": { "kind": "typography", "value": { "fontSize": 24, "fontWeight": 650 } } } } }"##;

/// String, boolean, duration, easing and typography tokens from TSX:
/// `token()` gives each in its canonical form, `tokens.get` a text style
/// as an object of its members, and their classes apply.
#[test]
fn token_reads_strings_booleans_durations_easings_and_text_styles() {
    let (_engine, rx) = start_with(KINDS_APP, Environment::default(), None, Some(KINDS_CONFIG));
    let batches = until(&rx, |b| shows(b, "styled"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    for text in [
        "string \"Acme\"",
        "boolean true",
        "duration \"120ms\"",
        "easing [0.2,0,0,1]",
        "typography typography 24 650 undefined",
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
