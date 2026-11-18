//! `token(name)` follows its token: a binding that read a token runs again
//! when that token's value changes, and a change of another token, or of
//! the appearance (which no token follows), runs nothing of it. A binding
//! that read two tokens runs once when both change. A change of most of
//! the tokens read runs every reader once.

use nsh_core::{MountBatch, MountItem};
use nsh_engine::Environment;

/// The app's document: five colors and a radius.
const DOCUMENT: &str = r##"{ "tokens": {
    "brand": { "kind": "color", "value": "#123456" },
    "accent": { "kind": "color", "value": "#222222" },
    "one": { "kind": "color", "value": "#111111" },
    "two": { "kind": "color", "value": "#222222" },
    "three": { "kind": "color", "value": "#333333" },
    "radius": { "kind": "number", "value": 8 } } }"##;

/// A TSX app with a text per token read, each counting its runs, changing
/// its theme a step every 100 ms.
const READERS: &str = r##"import { mount, setColorScheme, setTheme, signal, Text, token, updateTheme, View } from "nsh";

const runs = { brand: 0, accent: 0, both: 0, extra: 0, others: 0 };
const status = signal("start");
const steps = [
  () => {
    updateTheme({ values: { brand: "#00ff00" } });
    return "step 1";
  },
  () => {
    updateTheme({ values: { accent: "#0000ff" } });
    return "step 2";
  },
  () => {
    updateTheme({ values: { brand: "#111111", accent: "#333333" } });
    return "step 3";
  },
  () => {
    // A document with a token the first lacked: what read its name gets
    // it; the overrides still apply to the others.
    setTheme({ tokens: { ...DOCUMENT.tokens, extra: { kind: "color", value: "#ff0000" } } });
    return "step 4";
  },
  () => {
    // A token nothing here read.
    updateTheme({ values: { radius: 4 } });
    return "step 5";
  },
  () => {
    // Most of the six names read here: every reader runs, once.
    updateTheme({ values: { brand: "#aaaaaa", accent: "#bbbbbb", one: "#010101", two: "#020202" } });
    return "step 6";
  },
  () => {
    // The appearance: no token follows it.
    setColorScheme("dark");
    return "step 7";
  },
  () => "done",
];
steps.forEach((step, i) => setTimeout(() => status.set(step()), 100 * (i + 1)));

mount(() => (
  <View>
    <Text>{`brand ${token("brand")} run ${++runs.brand}`}</Text>
    <Text>{`accent ${token("accent")} run ${++runs.accent}`}</Text>
    <Text>{`both ${token("brand")} ${token("accent")} run ${++runs.both}`}</Text>
    <Text>{`extra ${token("extra")} run ${++runs.extra}`}</Text>
    <Text>{`others ${token("one")} ${token("two")} ${token("three")} run ${++runs.others}`}</Text>
    <Text>{status()}</Text>
  </View>
));
"##;

/// The texts the batches show that start with `prefix`, in order.
fn texts<'a>(batches: &'a [MountBatch], prefix: &str) -> Vec<&'a str> {
    batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::Text { text, .. } if text.text.starts_with(prefix) => Some(&*text.text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_binding_runs_again_only_for_the_tokens_it_read() {
    let config = format!(r#"{{ "theme": {DOCUMENT} }}"#);
    let app = READERS.replace(
        "const runs",
        &format!("const DOCUMENT = {DOCUMENT};\nconst runs"),
    );
    let (_engine, rx) =
        crate::components::start_with(&app, Environment::default(), None, Some(&config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    // The first frame ran each once, with the app's document (the entry
    // reads it from its setup on). From then on: its own token,
    // both tokens, most tokens, one run each. Not for `accent`, `extra`,
    // `radius` or the appearance.
    assert_eq!(
        texts(&batches, "brand "),
        [
            "brand #123456 run 1",
            "brand #00ff00 run 2",
            "brand #111111 run 3",
            "brand #aaaaaa run 4"
        ]
    );
    assert_eq!(
        texts(&batches, "accent "),
        [
            "accent #222222 run 1",
            "accent #0000ff run 2",
            "accent #333333 run 3",
            "accent #bbbbbb run 4"
        ]
    );
    // Either token; once when both change at once.
    assert_eq!(
        texts(&batches, "both "),
        [
            "both #123456 #222222 run 1",
            "both #00ff00 #222222 run 2",
            "both #00ff00 #0000ff run 3",
            "both #111111 #333333 run 4",
            "both #aaaaaa #bbbbbb run 5"
        ]
    );
    // A name the document lacked is followed too; a change of most tokens
    // runs it with the rest, its own unchanged.
    assert_eq!(
        texts(&batches, "extra "),
        [
            "extra undefined run 1",
            "extra #ff0000 run 2",
            "extra #ff0000 run 3"
        ]
    );
    // Untouched until most tokens change.
    assert_eq!(
        texts(&batches, "others "),
        [
            "others #111111 #222222 #333333 run 1",
            "others #010101 #020202 #333333 run 2"
        ]
    );
}

/// More names than get a signal of their own: every one still follows its
/// token (through the signal of all of them).
const MANY_NAMES: &str = r##"import { mount, signal, Text, token, updateTheme, View } from "nsh";

for (let i = 0; i < 1100; i++) token(`made-up-${i}`);
const status = signal("start");
setTimeout(() => {
  updateTheme({ values: { late: "#00ff00" } });
  status.set("done");
}, 100);

mount(() => (
  <View>
    <Text>{`late ${token("late")}`}</Text>
    <Text>{status()}</Text>
  </View>
));
"##;

#[test]
fn a_name_past_the_bound_still_follows_its_token() {
    let config =
        r##"{ "theme": { "tokens": { "late": { "kind": "color", "value": "#123456" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(MANY_NAMES, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    assert_eq!(texts(&batches, "late "), ["late #123456", "late #00ff00"]);
}

/// 1 200 readers of a theme's 1 200 colors, and 1 100 readers of names no
/// theme has, each counting its runs.
const MANY: &str = r##"import { mount, setTheme, signal, Text, token, updateTheme, View } from "nsh";

let runs = 0;
let made_up = 0;
const status = signal("start");

const theme = Array.from({ length: 1200 }, (_, i) => i);
const madeUp = Array.from({ length: 1100 }, (_, i) => i);

mount(() => (
  <View>
    {theme.map((i) => <Text>{`c-${i} ${token(`c-${i}` as never)} ${++runs}`}</Text>)}
    {madeUp.map((i) => <Text>{`made-${i} ${token(`made-${i}` as never)} ${++made_up}`}</Text>)}
    <Text>{`runs ${status()} ${runs} ${made_up}`}</Text>
    <Text>{status()}</Text>
  </View>
));

// Each step starts 100 ms after the one before it ran, counted from the
// mount: on a slow machine a mount this large outlasts fixed delays, which
// then come due together and leave a step no batch of its own.
const color = (i: number) => "#" + (0x100000 + i).toString(16).padStart(6, "0");
const doc = {
  tokens: Object.fromEntries(theme.map((i) => [`c-${i}`, { kind: "color", value: color(i) }])),
};
const steps = [
  () => {
    updateTheme({ values: { "c-0": "#abcdef" } });
    status.set("step 1");
  },
  () => {
    // A name made up past the signals' limit, which another document has.
    setTheme({ tokens: { ...doc.tokens, "made-1099": { kind: "color", value: "#123123" } } });
    status.set("step 2");
  },
  () => status.set("done"),
];
const next = (i: number) => {
  if (i < steps.length) setTimeout(() => (steps[i](), next(i + 1)), 100);
};
next(0);
"##;

/// A theme of more than 1 024 names gives each name read a signal of its
/// own: a change of one token runs its reader alone. Names no theme has get
/// one up to the limit; past it they follow every change, and one another
/// document has reaches its reader.
#[test]
fn a_large_themes_names_each_keep_a_signal() {
    let mut tokens = serde_json::Map::new();
    for i in 0..1200 {
        let value = format!("#{:06x}", 0x100000 + i);
        tokens.insert(
            format!("c-{i}"),
            serde_json::json!({ "kind": "color", "value": value }),
        );
    }
    let config = serde_json::json!({ "theme": { "tokens": tokens } }).to_string();
    let (_engine, rx) =
        crate::components::start_with(MANY, Environment::default(), None, Some(&config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    let step = |s: &str| {
        let at = batches
            .iter()
            .position(|b| crate::components::shows(b, s))
            .unwrap();
        batches[at]
            .items
            .iter()
            .filter_map(|i| match i {
                MountItem::Text { text, .. } => Some(text.text.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // One token changed: its reader ran, no other theme reader did; the
    // made-up names past the limit ran, as they follow every change.
    let one = step("step 1");
    // The first frame ran each reader once, with the app's document:
    // 1 200 runs before this one.
    assert!(one.iter().any(|t| t == "c-0 #abcdef 1201"), "{one:?}");
    assert_eq!(
        one.iter().filter(|t| t.starts_with("c-")).count(),
        1,
        "{one:?}"
    );
    assert!(
        // The status text renders in the write's turn, before the core
        // applied it: its count is the one before `c-0`'s reader ran.
        one.iter().any(|t| t.starts_with("runs step 1 1200 ")),
        "{one:?}"
    );
    // The made-up name the theme gained reaches its reader.
    let gained = step("step 2");
    assert!(
        gained.iter().any(|t| t.starts_with("made-1099 #123123 ")),
        "{gained:?}"
    );
    assert_eq!(
        gained.iter().filter(|t| t.starts_with("c-")).count(),
        0,
        "{gained:?}"
    );
}
