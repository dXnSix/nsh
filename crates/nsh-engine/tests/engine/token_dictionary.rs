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
