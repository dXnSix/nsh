//! Theme modifiers from TSX: `nsh.json`'s theme document with a `brand`
//! modifier, its default context at first, `setModifier` picking another
//! (`token()`, `modifier()` and `modifiers()` following within the frame), and a context the document lacks refused with a `TypeError`.

use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const APP: &str = r##"import { modifier, modifiers, mount, setModifier, signal, Text, token, View } from "nsh";

const step = signal(0);
const said = signal("none");
setTimeout(() => {
  setModifier("brand" as never, "forest" as never);
  said.set("picked");
  step.set(1);
}, 30);
setTimeout(() => {
  try {
    setModifier("brand" as never, "jungle" as never);
    said.set("taken");
  } catch (e) {
    said.set(`refused ${(e as Error).name}: ${(e as Error).message}`);
  }
  step.set(2);
}, 60);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`accent ${token("accent" as never)}`}</Text>
    <Text>{`brand ${modifier("brand" as never)}`}</Text>
    <Text>{`axes ${modifiers().map(m => `${m.name}:${m.contexts.join("|")}:${m.default}`).join(" ")}`}</Text>
    <Text>{`said ${said()}`}</Text>
  </View>
));
"##;

const CONFIG: &str = r##"{ "theme": {
    "tokens": { "accent": { "kind": "color", "value": "#111111" } },
    "modifiers": { "brand": { "default": "ocean", "contexts": {
        "ocean": { "accent": "#0000ff" },
        "forest": { "accent": "#00ff00" } } } },
    "resolutionOrder": ["brand"] } }"##;

#[test]
fn set_modifier_picks_a_context_and_what_reads_its_tokens_follows() {
    let (_engine, rx) = start_with(APP, Environment::default(), None, Some(CONFIG));
    let batches = until(&rx, |b| shows(b, "step 0"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    assert!(said("accent #0000ff"), "{batches:?}");
    assert!(said("brand ocean"), "{batches:?}");
    assert!(said("axes brand:ocean|forest:ocean"), "{batches:?}");
    let batches = until(&rx, |b| shows(b, "step 1"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    // What read the tokens follows within the frame.
    assert!(said("said picked"), "{batches:?}");
    assert!(said("accent #00ff00"), "{batches:?}");
    assert!(said("brand forest"), "{batches:?}");
    let batches = until(&rx, |b| shows(b, "step 2"));
    assert!(
        batches.iter().any(|b| shows(
            b,
            "said refused TypeError: setModifier: the modifier `brand` has no context `jungle`"
        )),
        "{batches:?}"
    );
}

const SCOPES: &str = r##"import { mount, setModifier, signal, Text, ThemeScope, token, View } from "nsh";

const step = signal(0);
const brand = signal<string | undefined>("forest");
setTimeout(() => { setModifier("brand" as never, "forest" as never); brand.set("ocean"); step.set(1); }, 30);
setTimeout(() => { brand.set(undefined); step.set(2); }, 60);

function Reader(props: { name: string }) {
  return <Text>{`${props.name} ${token("accent" as never)}`}</Text>;
}

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Reader name="out" />
    <ThemeScope modifiers={(brand() ? { brand: brand() } : undefined) as never}>
      <Reader name="in" />
    </ThemeScope>
  </View>
));
"##;

#[test]
fn a_theme_scope_picks_its_own_contexts() {
    let (_engine, rx) = start_with(SCOPES, Environment::default(), None, Some(CONFIG));
    let seen = |step: &str| {
        let batches = until(&rx, |b| shows(b, step));
        assert!(batches.iter().any(|b| shows(b, step)), "{batches:?}");
        batches
    };
    let said =
        |batches: &[nsh_core::MountBatch], text: &str| batches.iter().any(|b| shows(b, text));
    // The scope's brand in it, the app's outside.
    let batches = seen("step 0");
    assert!(said(&batches, "out #0000ff"), "{batches:?}");
    assert!(said(&batches, "in #00ff00"), "{batches:?}");
    // The app picks forest, the scope ocean: each keeps its own.
    let batches = seen("step 1");
    assert!(said(&batches, "out #00ff00"), "{batches:?}");
    assert!(said(&batches, "in #0000ff"), "{batches:?}");
    // Without modifiers of its own the scope is the app's.
    let batches = seen("step 2");
    assert!(said(&batches, "in #00ff00"), "{batches:?}");
}

const LIVE: &str = r##"import { modifier, mount, resetTheme, setModifier, setTheme, signal, Text, token, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  setModifier("brand", "forest");
  updateTheme({ values: { accent: "#123456" } });
  step.set(1);
}, 30);
setTimeout(() => {
  resetTheme();
  setTheme({
    tokens: { accent: { kind: "color", value: "#111111" } },
    modifiers: { brand: { default: "ocean", contexts: { ocean: { accent: "#0000ff" }, forest: { accent: "#00ff00" } } } },
    resolutionOrder: ["brand"],
  });
  step.set(2);
}, 60);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`accent ${token("accent")} ${modifier("brand")}`}</Text>
  </View>
));
"##;

/// `updateTheme` wins over the picked context, and `setTheme` keeps the
/// context the app picked.
#[test]
fn update_theme_wins_over_contexts_and_set_theme_keeps_the_picks() {
    let (_engine, rx) = start_with(LIVE, Environment::default(), None, Some(CONFIG));
    let batches = until(&rx, |b| shows(b, "step 1"));
    let said =
        |batches: &[nsh_core::MountBatch], text: &str| batches.iter().any(|b| shows(b, text));
    assert!(said(&batches, "accent #123456 forest"), "{batches:?}");
    let batches = until(&rx, |b| shows(b, "step 2"));
    assert!(said(&batches, "accent #00ff00 forest"), "{batches:?}");
}
