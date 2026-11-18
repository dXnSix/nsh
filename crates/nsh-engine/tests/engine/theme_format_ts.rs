//! TypeScript reads and writes themes in the theme format
//! (`docs/dev/theme-format.md`) on a native host (nsh-js).
//!
//! Reads (`token()`, `role()`, `tokens.*`) give resolved values in the
//! format's canonical forms, and track by name. Writes (`setTheme`,
//! `updateTheme`, `resetTheme`, `setModifier`) are ops the core applies
//! with the turn's others, resolving once; `token()` reads the result from
//! the next turn on, within the same frame. The first two return their
//! diagnostics at once. A `ThemeScope` resolves its patch and picks
//! with the same stack.

use std::time::Duration;

use nsh_core::{Color, MountBatch, MountItem};
use nsh_engine::Environment;

use crate::components::{start_with, until};

/// The app's theme: primitives, semantic tokens referring to them, one
/// token of each value type, two modifiers and two role mappings.
const CONFIG: &str = r##"{ "theme": {
    "name": "Test",
    "tokens": {
        "gray-1": { "kind": "color", "value": "#fcfcfc" },
        "gray-12": { "kind": "color", "value": "#202020" },
        "surface": { "kind": "color", "value": "{gray-1}", "path": "surface.app", "doc": "The app's background." },
        "text": { "kind": "color", "value": "{gray-12}" },
        "veil": { "kind": "color", "value": "#00000080" },
        "space": { "kind": "number", "value": 12 },
        "ratio": { "kind": "number", "value": 0.5 },
        "font-ui": { "kind": "font", "value": "Inter, system-ui, sans-serif" },
        "label": { "kind": "string", "value": "OK" },
        "beta": { "kind": "boolean", "value": true },
        "fast": { "kind": "duration", "value": "0.15s" },
        "ease": { "kind": "easing", "value": "ease-in" },
        "body": { "kind": "typography", "value": { "fontFamily": "{font-ui}", "fontSize": 14, "fontWeight": 400, "lineHeight": "20px" } },
        "lift": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 3 }] },
        "hero": { "kind": "gradient", "value": { "type": "linear", "angle": 135, "stops": [{ "color": "#0090ff", "position": 0 }, { "color": "{gray-12}", "position": 1 }] } },
        "glass": { "kind": "blur", "value": { "type": "background", "radius": 16 } },
        "edge": { "kind": "border", "value": { "color": "{text}", "width": 1, "style": "dashed" } },
        "press": { "kind": "transition", "value": { "duration": "100ms", "easing": [0.2, 0, 0, 1] } },
        "settle": { "kind": "transition", "value": { "duration": "200ms", "easing": "ease-out" } }
    },
    "modifiers": {
        "colorMode": { "default": "light", "contexts": { "light": {}, "dark": { "gray-1": "#111111", "gray-12": "#eeeeee" } } },
        "density": { "default": "regular", "contexts": { "regular": {}, "compact": { "space": 8 } } }
    },
    "resolutionOrder": ["density", "colorMode"],
    "roles": { "background": "surface", "foreground": "text" }
} }"##;

/// [`CONFIG`] with the app's overrides (`nsh.json`'s `"themeOverrides"`).
fn with_overrides() -> String {
    let mut config: serde_json::Value = serde_json::from_str(CONFIG).unwrap();
    config["themeOverrides"] = serde_json::json!({ "values": { "label": "Yes" } });
    config.to_string()
}

/// The texts of `batch`.
fn texts_of(batch: &MountBatch) -> Vec<String> {
    batch
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

/// The batches `app` under `config` shows until a text starting with
/// `done` does.
fn batches_with(app: &str, config: &str) -> Vec<MountBatch> {
    let (_engine, rx) = start_with(app, Environment::default(), None, Some(config));
    until(&rx, |b| texts_of(b).iter().any(|t| t.starts_with("done")))
}

/// The texts `app` under `config` shows until `done`, in order.
fn run_with(app: &str, config: &str) -> Vec<String> {
    let texts: Vec<String> = batches_with(app, config)
        .iter()
        .flat_map(texts_of)
        .collect();
    assert!(texts.iter().any(|t| t.starts_with("done")), "{texts:?}");
    texts
}

/// [`run_with`] under [`CONFIG`].
fn run(app: &str) -> Vec<String> {
    run_with(app, CONFIG)
}

/// The last of `texts` starting with `label`.
fn last<'a>(texts: &'a [String], label: &str) -> &'a str {
    texts
        .iter()
        .rev()
        .find(|t| t.starts_with(label))
        .unwrap_or_else(|| panic!("no `{label}` in {texts:?}"))
}

const READS: &str = r##"import { mount, Text, token } from "nsh";

const shown = () => {
  const body = token("body") ?? {};
  const lift = token("lift") ?? [{}];
  const hero = token("hero") ?? { stops: [] };
  const glass = token("glass") ?? {};
  const edge = token("edge") ?? {};
  const press = token("press") ?? { easing: [] };
  const settle = token("settle") ?? {};
  return [
    `surface ${token("surface")}`,
    `veil ${token("veil")}`,
    `space ${token("space")} ${typeof token("space")}`,
    `ratio ${token("ratio")}`,
    `font ${token("font-ui")}`,
    `label ${token("label")}`,
    `beta ${token("beta")} ${typeof token("beta")}`,
    `fast ${token("fast")}`,
    `ease ${JSON.stringify(token("ease"))}`,
    `body ${body.fontFamily}|${body.fontSize}|${body.fontWeight}|${body.lineHeight}|${body.letterSpacing}`,
    `lift ${lift.length} ${lift[0].color} ${lift[0].x} ${lift[0].y} ${lift[0].blur}`,
    `hero ${hero.type} ${hero.angle} ${hero.stops.map((s) => `${s.color}@${s.position}`).join(",")}`,
    `glass ${glass.type} ${glass.radius}`,
    `edge ${edge.color} ${edge.width} ${edge.style}`,
    `press ${press.duration} ${press.delay} ${press.easing.join(",")}`,
    `settle ${settle.duration} ${JSON.stringify(settle.easing)}`,
    `none ${token("nothing")}`,
  ].join("; ");
};

mount(() => <Text>{`done ${shown()}`}</Text>);
"##;

/// `token()` gives each value type in its canonical JSON form, references
/// resolved (a compound's members too), the members at their default left
/// out, an easing as it was read (a keyword stays one, in a transition
/// too), and `undefined` for a name the document lacks.
#[test]
fn token_gives_each_value_type_in_its_canonical_form() {
    let texts = run(READS);
    assert_eq!(
        last(&texts, "done"),
        "done surface #fcfcfc; veil #00000080; space 12 number; ratio 0.5; \
         font Inter, system-ui, sans-serif; label OK; beta true boolean; fast 150ms; \
         ease \"ease-in\"; body Inter, system-ui, sans-serif|14|400|20px|undefined; \
         lift 1 #0000001a undefined 1 3; hero linear 135 #0090ff@0,#202020@1; \
         glass background 16; edge #202020 1 dashed; press 100ms undefined 0.2,0,0,1; \
         settle 200ms \"ease-out\"; none undefined"
    );
}

const ROLES: &str = r##"import { mount, role, Text } from "nsh";

mount(() => (
  <Text>{`done background ${role("background")}; foreground ${role("foreground")}; primary ${role("primary")}`}</Text>
));
"##;

/// `role()` gives the token the document maps the role onto, as
/// `token()` gives it; a role nothing maps reads the token of its name,
/// here none.
#[test]
fn role_gives_the_token_the_document_maps_it_onto() {
    let texts = run(ROLES);
    assert_eq!(
        last(&texts, "done"),
        "done background #fcfcfc; foreground #202020; primary undefined"
    );
}

const DICTIONARY: &str = r##"import { mount, Text, tokens } from "nsh";

const shown = () => {
  const surface = tokens.get("surface") ?? {};
  const body = tokens.get("body") ?? { value: {} };
  const entry = tokens.entries().find((e) => e.name === "surface") ?? {};
  return [
    `get ${surface.kind} ${surface.value}`,
    `body ${body.kind} ${body.value.fontSize}`,
    `missing ${tokens.get("nothing")}`,
    `kind ${tokens.kind("hero")} ${tokens.kind("nothing")}`,
    `grays ${tokens.names({ kind: "color", prefix: "gray" }).join(",")}`,
    `entry ${entry.kind} ${entry.path} ${entry.doc}`,
    `count ${tokens.names().length} ${tokens.entries().length}`,
  ].join("; ");
};

mount(() => <Text>{`done ${shown()}`}</Text>);
"##;

/// `tokens.get` gives `{ kind, value }`, the value as `token()` gives it;
/// `kind`, `names` and `entries` list what the active document declares.
#[test]
fn the_token_dictionary_gives_kinds_values_names_and_entries() {
    let texts = run(DICTIONARY);
    assert_eq!(
        last(&texts, "done"),
        "done get color #fcfcfc; body typography 14; missing undefined; \
         kind gradient undefined; grays gray-1,gray-12; \
         entry color surface.app The app's background.; count 19 19"
    );
}

const TRACKING: &str = r##"import { effect, mount, signal, Text, token, updateTheme } from "nsh";

const runs = { surface: 0, space: 0 };
const status = signal("start");
let first = "";

function App() {
  effect(() => {
    token("surface");
    runs.surface += 1;
  });
  effect(() => {
    token("space");
    runs.space += 1;
  });
  return <Text>{status()}</Text>;
}

// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  // What the first frame's readers ran: once, with the app's document.
  () => (first = `${runs.surface} ${runs.space}`),
  () => updateTheme({ values: { space: 10 } }),
  // A primitive `surface` refers to: what read `surface` follows.
  () => updateTheme({ values: { "gray-1": "#ffffff" } }),
  // A token nothing here read.
  () => updateTheme({ values: { label: "Yes" } }),
  () => status.set(`done ${first} ${runs.surface} ${runs.space} ${token("surface")} ${token("space")}`),
]);

mount(() => <App />);
"##;

/// A reader runs again only when a token it read changes, a change
/// through a reference included; a change of a token nothing read runs
/// nothing. In the first frame each ran once, with the app's document: the
/// entry reads it from its setup on, and the core's install of it changes
/// nothing.
#[test]
fn a_reader_runs_again_only_for_the_names_it_read() {
    let texts = run(TRACKING);
    assert_eq!(last(&texts, "done"), "done 1 1 2 2 #ffffff 10");
}

const UPDATE: &str = r##"import { mount, signal, Text, token, updateTheme } from "nsh";

const status = signal("start");
let listed = "";
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    const said = updateTheme({ values: { veil: "#ff0000", nothing: "#ffffff", space: "wide" } });
    listed = said.map((d) => `${d.severity} ${d.code} ${d.pointer}`).join(", ");
  },
  () => status.set(`done ${listed}; veil ${token("veil")} space ${token("space")}`),
]);

mount(() => <Text>{status()}</Text>);
"##;

/// `updateTheme` merges what it can into the app's patch and returns what
/// the active document refuses: a name it lacks, a value that doesn't fit.
#[test]
fn update_theme_returns_what_it_leaves_out() {
    let texts = run(UPDATE);
    assert_eq!(
        last(&texts, "done"),
        "done error undeclared-token /values/nothing, error bad-value /values/space; \
         veil #ff0000 space 12"
    );
}

const SET_THEME: &str = r##"import { mount, setTheme, signal, Text, token, tokens } from "nsh";

const status = signal("start");
let errors = [];
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    const said = setTheme({
      tokens: {
        surface: { kind: "color", value: "#123456" },
        bad: { kind: "dimension", value: 1 },
        wrong: { kind: "color", value: "white" },
      },
    });
    // The token each error names (`/tokens/<name>…`).
    errors = said
      .filter((d) => d.severity === "error")
      .map((d) => `${d.code} ${d.pointer.split("/")[2]}`);
  },
  () =>
    status.set(
      `done ${errors.join(", ")}; surface ${token("surface")} space ${token("space")} kind ${tokens.kind("bad")}`,
    ),
]);

mount(() => <Text>{status()}</Text>);
"##;

/// `setTheme` activates a document whole, in place of the app's: its bad
/// entries are left out and returned, and a token only the old one had is
/// gone.
#[test]
fn set_theme_activates_a_document_and_returns_its_diagnostics() {
    let texts = run(SET_THEME);
    assert_eq!(
        last(&texts, "done"),
        "done bad-kind bad, bad-value wrong; surface #123456 space undefined kind undefined"
    );
}

const SET_NULL: &str = r##"import { mount, setTheme, signal, Text, token } from "nsh";

const status = signal("start");
let set = "";
let said = [];
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => setTheme({ tokens: { surface: { kind: "color", value: "#123456" } } }),
  () => {
    set = `${token("surface")} ${token("space")}`;
    said = setTheme(null);
  },
  () => status.set(`done ${set} ${said.length} ${token("surface")} ${token("space")}`),
]);

mount(() => <Text>{status()}</Text>);
"##;

/// `setTheme(null)` gives the app's built-in document back.
#[test]
fn set_theme_null_gives_the_built_in_document_back() {
    let texts = run(SET_NULL);
    assert_eq!(last(&texts, "done"), "done #123456 undefined 0 #fcfcfc 12");
}

const LEGACY: &str = r##"import { mount, setTheme, signal, Text, token, View } from "nsh";

const status = signal("start");
let listed = "";
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    const said = setTheme({ cssVars: { light: { surface: "#ff0000" } } });
    listed = said
      .map((d) => `${d.severity} ${d.code} "${d.pointer}" ${d.message.includes("nsh theme convert")}`)
      .join(", ");
  },
  () => status.set(`done ${listed}; surface ${token("surface")}`),
]);

mount(() => (
  <View>
    <View class="h-4 bg-surface" />
    <Text>{status()}</Text>
  </View>
));
"##;

/// An old `cssVars` theme is never activated: `setTheme` returns its
/// `legacy-format` error, naming the converter, and the active document
/// stays, in TS and in the classes the core draws.
#[test]
fn set_theme_of_an_old_cssvars_theme_keeps_the_active_document() {
    let batches = batches_with(LEGACY, CONFIG);
    let texts: Vec<String> = batches.iter().flat_map(texts_of).collect();
    assert_eq!(
        last(&texts, "done"),
        "done error legacy-format \"\" true; surface #fcfcfc"
    );
    let mut drawn: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff != 0 => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect();
    drawn.dedup();
    assert_eq!(drawn, [Color(0xfcfcfcff)]);
}

const TYPE_ERRORS: &str = r##"import { mount, setTheme, signal, Text, token, updateTheme } from "nsh";

const status = signal("start");
setTimeout(() => {
  const tries = [
    () => setTheme("{}"),
    () => setTheme(12),
    () => setTheme(true),
    () => updateTheme(null),
    () => updateTheme("{}"),
    () => updateTheme(undefined),
  ];
  const out = tries.map((f) => {
    try {
      f();
      return "taken";
    } catch (e) {
      return e instanceof TypeError ? e.message : `${e}`;
    }
  });
  status.set(`done ${out.join(" | ")}; surface ${token("surface")}`);
}, 30);

mount(() => <Text>{status()}</Text>);
"##;

/// `setTheme` takes an object or `null`, `updateTheme` an object: anything
/// else throws a `TypeError` and changes nothing.
#[test]
fn set_theme_and_update_theme_throw_a_type_error_on_anything_but_an_object() {
    let texts = run(TYPE_ERRORS);
    let set = "setTheme: expected a theme document (an object) or null";
    let update = "updateTheme: expected a theme patch (an object)";
    assert_eq!(
        last(&texts, "done"),
        format!("done {set} | {set} | {set} | {update} | {update} | {update}; surface #fcfcfc")
    );
}

const RESET: &str = r##"import { mount, resetTheme, signal, Text, token, updateTheme } from "nsh";

const status = signal("start");
const out = [];
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    out.push(`start ${token("label")}`);
    updateTheme({ values: { space: 10, veil: "#ff0000" } });
  },
  () => {
    out.push(`updated ${token("space")} ${token("veil")}`);
    resetTheme(["space"]);
  },
  () => {
    out.push(`one ${token("space")} ${token("veil")} ${token("label")}`);
    resetTheme();
  },
  () => {
    out.push(`all ${token("space")} ${token("veil")} ${token("label")}`);
    try {
      resetTheme("space");
      out.push("taken");
    } catch (e) {
      out.push(`${e.name}: ${e.message}`);
    }
    status.set(`done ${out.join("; ")}`);
  },
]);

mount(() => <Text>{status()}</Text>);
"##;

/// `resetTheme(names)` removes names from the app's one patch, and
/// `resetTheme()` empties it, `themeOverrides`' entries included: a reset
/// never copies values.
#[test]
fn reset_theme_removes_names_from_the_apps_patch() {
    let texts = run_with(RESET, &with_overrides());
    assert_eq!(
        last(&texts, "done"),
        "done start Yes; updated 10 #ff0000; one 12 #ff0000 Yes; all 12 #00000080 OK; \
         TypeError: resetTheme: expected token names (string[]) or nothing"
    );
}

const PICKS: &str = r##"import { modifier, modifiers, mount, setModifier, signal, Text, token, View } from "nsh";

const status = signal("start");
const axes = () =>
  modifiers()
    .map((m) => `${m.name}:${m.contexts.join("|")}:${m.default}:${m.selected}`)
    .join(" ");
const out = [];
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    out.push(`before ${axes()} ${modifier("colorMode")} ${token("surface")}`);
    setModifier("colorMode", "dark");
  },
  () => {
    out.push(`after ${axes()} ${modifier("colorMode")} ${token("surface")} ${token("text")}`);
    status.set(`done ${out.join("; ")}`);
  },
]);

mount(() => (
  <View>
    <Text>{`shown ${modifier("colorMode")} ${token("surface")}`}</Text>
    <Text>{status()}</Text>
  </View>
));
"##;

/// `setModifier` picks a context, `token()` reading its values once the
/// core applied it;
/// `modifier()` gives the context shown and `modifiers()` lists the
/// modifiers in `resolutionOrder` with their contexts, default and pick.
#[test]
fn set_modifier_picks_a_context_and_modifiers_list_them_in_resolution_order() {
    let texts = run(PICKS);
    assert_eq!(
        last(&texts, "done"),
        "done before density:regular|compact:regular:regular colorMode:light|dark:light:light light #fcfcfc; \
         after density:regular|compact:regular:regular colorMode:light|dark:light:dark dark #111111 #eeeeee"
    );
    assert_eq!(last(&texts, "shown"), "shown dark #111111");
}

const REFUSED: &str = r##"import { modifier, mount, setModifier, signal, Text, token } from "nsh";

const status = signal("start");
const pick = (name, context) => {
  try {
    setModifier(name, context);
    return "taken";
  } catch (e) {
    return `${e.name}: ${e.message}`;
  }
};
setTimeout(() => {
  const out = [pick("brand", "forest"), pick("colorMode", "sepia")];
  out.push(`${modifier("colorMode")} ${token("surface")}`);
  status.set(`done ${out.join("; ")}`);
}, 30);

mount(() => <Text>{status()}</Text>);
"##;

/// `setModifier` of a modifier the active document lacks, or of a context
/// its modifier lacks, throws a `TypeError` naming it and changes nothing.
#[test]
fn set_modifier_of_what_the_document_lacks_throws_and_changes_nothing() {
    let texts = run(REFUSED);
    assert_eq!(
        last(&texts, "done"),
        "done TypeError: setModifier: the theme has no modifier `brand`; \
         TypeError: setModifier: the modifier `colorMode` has no context `sepia`; light #fcfcfc"
    );
}

const SAME_TURN: &str = r##"import { modifier, mount, setModifier, setTheme, signal, Text, token } from "nsh";

const status = signal("start");
const pick = (name, context) => {
  try {
    setModifier(name, context);
    return "taken";
  } catch (e) {
    return `${e.name}: ${e.message}`;
  }
};
const out = [];
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    setTheme({
      tokens: { accent: { kind: "color", value: "#111111" } },
      modifiers: {
        brand: { default: "ocean", contexts: { ocean: { accent: "#0000ff" }, forest: { accent: "#00ff00" } } },
      },
      resolutionOrder: ["brand"],
    });
    out.push(pick("brand", "forest"));
    out.push(pick("colorMode", "dark"));
  },
  () => status.set(`done ${out.join("; ")}; later ${token("accent")} ${modifier("brand")}`),
]);

mount(() => <Text>{status()}</Text>);
"##;

/// `setModifier` checks the document active in this turn: one `setTheme`
/// activated just before it has the modifier, and the old one's is gone.
#[test]
fn set_modifier_checks_the_document_set_in_the_same_turn() {
    let texts = run(SAME_TURN);
    assert_eq!(
        last(&texts, "done"),
        "done taken; TypeError: setModifier: the theme has no modifier `colorMode`; \
         later #00ff00 forest"
    );
}

const KEPT_PICKS: &str = r##"import { modifier, mount, setModifier, setTheme, signal, Text, token } from "nsh";

const status = signal("start");
const moded = {
  tokens: { surface: { kind: "color", value: "#fafafa" } },
  modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { surface: "#0a0a0a" } } } },
  resolutionOrder: ["colorMode"],
};
const out = [];
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    setModifier("colorMode", "dark");
    setTheme(moded);
  },
  () => {
    out.push(`kept ${modifier("colorMode")} ${token("surface")}`);
    setTheme({ tokens: { surface: { kind: "color", value: "#fafafa" } } });
  },
  () => {
    out.push(`gone ${modifier("colorMode")} ${token("surface")}`);
    setTheme(moded);
  },
  () => {
    out.push(`back ${modifier("colorMode")} ${token("surface")}`);
    status.set(`done ${out.join("; ")}`);
  },
]);

mount(() => <Text>{status()}</Text>);
"##;

/// Activating another document keeps the picks it has; a document without
/// the modifier drops the pick, so it shows its default after.
#[test]
fn set_theme_keeps_the_picks_the_new_document_has() {
    let texts = run(KEPT_PICKS);
    assert_eq!(
        last(&texts, "done"),
        "done kept dark #0a0a0a; gone undefined #fafafa; back light #fafafa"
    );
}

const OVER_CONTEXTS: &str = r##"import { mount, setModifier, signal, Text, token, updateTheme } from "nsh";

const status = signal("start");
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    updateTheme({ values: { "gray-1": "#abcdef" } });
    setModifier("colorMode", "dark");
  },
  () => status.set(`done ${token("surface")} ${token("text")}`),
]);

mount(() => <Text>{status()}</Text>);
"##;

/// The app's overrides win over a picked context; what they leave alone
/// takes the context's values.
#[test]
fn overrides_win_over_a_picked_context() {
    let texts = run(OVER_CONTEXTS);
    assert_eq!(last(&texts, "done"), "done #abcdef #eeeeee");
}

const ROLE_PATCH: &str = r##"import { effect, mount, role, signal, Text, token, updateTheme, View } from "nsh";

const status = signal("start");
let runs = 0;
let before = 0;
let said = [];

function App() {
  effect(() => {
    role("background");
    runs += 1;
  });
  return (
    <View>
      <Text>{`shown ${role("background")}`}</Text>
      <Text>{status()}</Text>
    </View>
  );
}

// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    before = runs;
    said = updateTheme({ roles: { background: "text" } });
  },
  () => status.set(`done ${said.length} ${role("background")} ${token("surface")} ${runs - before}`),
]);

mount(() => <App />);
"##;

/// A patch that only re-maps a role reaches what read the role, once, and
/// changes no token.
#[test]
fn a_role_only_patch_reaches_role_readers() {
    let texts = run(ROLE_PATCH);
    assert_eq!(last(&texts, "done"), "done 0 #202020 #fcfcfc 1");
    assert_eq!(last(&texts, "shown"), "shown #202020");
}

const WRONG_TYPE: &str = r##"import { mount, role, signal, Text, updateTheme } from "nsh";

const status = signal("start");
let listed = "";
// Each step runs in a turn of its own, after the core applied the one
// before it: a write's result is read in the next turn.
const run = (steps, i = 0) => {
  if (i < steps.length) setTimeout(() => (steps[i](), run(steps, i + 1)), 40);
};
run([
  () => {
    const said = updateTheme({ roles: { background: "space", popover: "space" } });
    listed = said.map((d) => `${d.severity} ${d.code} ${d.pointer}`).join(", ");
  },
  () => status.set(`done ${listed}; ${role("background")} ${role("popover")}`),
]);

mount(() => <Text>{status()}</Text>);
"##;

/// The app maps color roles onto a number token: each mapping is returned
/// (`role-type`) and left out, so `background` reads the document's
/// mapping and `popover`, which the document doesn't map, the token of its
/// own name, here none.
#[test]
fn a_role_mapped_onto_a_token_of_the_wrong_type_is_returned_and_left_out() {
    let texts = run(WRONG_TYPE);
    assert_eq!(
        last(&texts, "done"),
        "done error role-type /roles/background, error role-type /roles/popover; #fcfcfc undefined"
    );
}

const FIRST_RENDER: &str = r##"import { mount, Text, token } from "nsh";

mount(() => <Text>{`done ${token("surface")} ${token("label")}`}</Text>);
"##;

/// The app's document and overrides (`nsh.json`'s `"theme"` and
/// `"themeOverrides"`) are in the first frame: the core resolves them with
/// the entry's first turn, and what read a token in it reads them before
/// the frame is sent.
#[test]
fn the_first_render_reads_the_apps_document_and_overrides() {
    let (_engine, rx) = start_with(
        FIRST_RENDER,
        Environment::default(),
        None,
        Some(&with_overrides()),
    );
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts_of(&first), ["done #fcfcfc Yes"]);
}

const TOP_LEVEL: &str = r##"import { mount, role, Text, token, tokens } from "nsh";

// Read as the module runs, before anything renders or the core resolved
// the app's theme.
const top = [
  token("surface"),
  token("label"),
  role("background"),
  tokens.get("space")?.value,
  token("ease"),
  tokens.names().length,
].join(" ");

mount(() => <Text>{`done ${top}`}</Text>);
"##;

/// A read at the module's top level gives the app's document and
/// overrides: the runtime reads them from the bundle's setup on, which
/// runs before any module of the app.
#[test]
fn a_top_level_read_gives_the_apps_document() {
    let (_engine, rx) = start_with(
        TOP_LEVEL,
        Environment::default(),
        None,
        Some(&with_overrides()),
    );
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts_of(&first), ["done #fcfcfc Yes #fcfcfc 12 ease-in 19"]);
}

const SCOPES: &str = r##"import { mount, role, Text, ThemeScope, token, View } from "nsh";

mount(() => (
  <View>
    <Text>{`out ${token("surface")} ${token("text")} ${role("background")}`}</Text>
    <ThemeScope theme={{ values: { surface: "#ff0000" } }} modifiers={{ colorMode: "dark" }}>
      <Text>{`in ${token("surface")} ${token("text")} ${role("background")}`}</Text>
    </ThemeScope>
    <ThemeScope theme={{ roles: { background: "text" } }}>
      <Text>{`mapped ${role("background")}`}</Text>
    </ThemeScope>
    <Text>done</Text>
  </View>
));
"##;

/// A JS `ThemeScope` resolves its subtree with the same stack: its picks
/// over the app's, its patch after the app's (winning over its picks), a
/// patch that only re-maps a role reaching `role()`.
#[test]
fn a_theme_scope_reads_its_patch_and_its_picks() {
    let texts = run(SCOPES);
    assert_eq!(last(&texts, "out"), "out #fcfcfc #202020 #fcfcfc");
    assert_eq!(last(&texts, "in"), "in #ff0000 #eeeeee #ff0000");
    assert_eq!(last(&texts, "mapped"), "mapped #202020");
}

const CLASSES: &str = r##"import { mount, resetTheme, setModifier, signal, Text, updateTheme, View } from "nsh";

const status = signal("start");
setTimeout(() => updateTheme({ values: { "gray-1": "#00ff00" } }), 30);
setTimeout(() => resetTheme(), 60);
setTimeout(() => setModifier("colorMode", "dark"), 90);
setTimeout(() => status.set("done"), 150);

mount(() => (
  <View>
    <View class="h-4 bg-surface" />
    <Text>{status()}</Text>
  </View>
));
"##;

/// The classes naming a token follow each write: an override of a
/// primitive the token refers to, its reset and a picked context.
#[test]
fn classes_follow_the_writes() {
    let batches = batches_with(CLASSES, CONFIG);
    let mut drawn: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff != 0 => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect();
    drawn.dedup();
    assert_eq!(
        drawn,
        [
            Color(0xfcfcfcff),
            Color(0x00ff00ff),
            Color(0xfcfcfcff),
            Color(0x111111ff)
        ]
    );
}
