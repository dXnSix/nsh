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
