//! Theme modifiers from TSX: `nsh.json`'s theme with a `brand` modifier,
//! its default context at first, `setModifier` picking another (`token()`
//! reading it at once, `modifier()` and `modifiers()` following), and a
//! context the theme has not refused with a `TypeError`.

use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const APP: &str = r##"import { modifier, modifiers, mount, setModifier, signal, Text, token, View } from "nsh";

const step = signal(0);
const said = signal("none");
setTimeout(() => {
  setModifier("brand" as never, "forest" as never);
  said.set(`${token("accent" as never)}`);
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
    "cssVars": { "light": { "accent": "#111111" } },
    "modifiers": { "brand": { "default": "ocean", "contexts": {
        "ocean": { "light": { "accent": "#0000ff" } },
        "forest": { "light": { "accent": "#00ff00" } } } } } } }"##;

#[test]
fn set_modifier_picks_a_context_and_what_reads_its_tokens_follows() {
    let (_engine, rx) = start_with(APP, Environment::default(), None, Some(CONFIG));
    let batches = until(&rx, |b| shows(b, "step 0"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    assert!(said("accent #0000ffff"), "{batches:?}");
    assert!(said("brand ocean"), "{batches:?}");
    assert!(said("axes brand:ocean|forest:ocean"), "{batches:?}");
    let batches = until(&rx, |b| shows(b, "step 1"));
    let said = |text: &str| batches.iter().any(|b| shows(b, text));
    // `token()` reads the context at once, in the same turn.
    assert!(said("said #00ff00ff"), "{batches:?}");
    assert!(said("accent #00ff00ff"), "{batches:?}");
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
    assert!(said(&batches, "out #0000ffff"), "{batches:?}");
    assert!(said(&batches, "in #00ff00ff"), "{batches:?}");
    // The app picks forest, the scope ocean: each keeps its own.
    let batches = seen("step 1");
    assert!(said(&batches, "out #00ff00ff"), "{batches:?}");
    assert!(said(&batches, "in #0000ffff"), "{batches:?}");
    // Without modifiers of its own the scope is the app's.
    let batches = seen("step 2");
    assert!(said(&batches, "in #00ff00ff"), "{batches:?}");
}
