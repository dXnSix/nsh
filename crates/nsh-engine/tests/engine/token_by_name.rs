//! `token(name)` follows its token: a binding that read a token runs again
//! when that token's value changes (or the appearance does), and a change
//! of another token runs nothing of it. A binding that read two tokens
//! runs once when both change. A change of most of the tokens read (a new
//! theme) runs every reader once.

use nsh_core::{MountBatch, MountItem};
use nsh_engine::Environment;

/// A TSX app with a text per token read, each counting its runs, changing
/// its theme a step every 100 ms.
const READERS: &str = r##"import { mount, setColorScheme, signal, Text, token, updateTheme, View } from "nsh";

const runs = { brand: 0, accent: 0, both: 0, extra: 0, others: 0 };
const status = signal("start");
const steps = [
  () => {
    updateTheme({ cssVars: { light: { brand: "#00ff00" } } });
    return "step 1";
  },
  () => {
    updateTheme({ cssVars: { light: { accent: "#0000ff" } } });
    return "step 2";
  },
  () => {
    updateTheme({ cssVars: { light: { brand: "#111111", accent: "#333333" } } });
    return "step 3";
  },
  () => {
    // A token the theme did not have: what read its name gets it.
    updateTheme({ cssVars: { light: { extra: "#ff0000" } } });
    return "step 4";
  },
  () => {
    // A token nothing here read.
    updateTheme({ cssVars: { theme: { radius: "4px" } } });
    return "step 5";
  },
  () => {
    // Most of the six names read here: every reader runs, once.
    updateTheme({ cssVars: { light: { brand: "#aaaaaa", accent: "#bbbbbb", one: "#010101", two: "#020202" } } });
    return "step 6";
  },
  () => {
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
    let config = r##"{ "theme": { "cssVars": { "light": { "brand": "#123456", "accent": "#222222", "one": "#111111", "two": "#222222", "three": "#333333" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(READERS, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    let brand = texts(&batches, "brand ");
    let accent = texts(&batches, "accent ");
    let both = texts(&batches, "both ");
    let extra = texts(&batches, "extra ");
    let others = texts(&batches, "others ");
    // Its own token, both tokens, most tokens: one run each. Not for
    // `accent`, `extra` or `radius`.
    assert_eq!(
        brand[..4],
        [
            "brand #123456ff run 1",
            "brand #00ff00ff run 2",
            "brand #111111ff run 3",
            "brand #aaaaaaff run 4"
        ]
    );
    assert_eq!(
        accent[..4],
        [
            "accent #222222ff run 1",
            "accent #0000ffff run 2",
            "accent #333333ff run 3",
            "accent #bbbbbbff run 4"
        ]
    );
    // Either token; once when both change at once.
    assert_eq!(
        both[..5],
        [
            "both #123456ff #222222ff run 1",
            "both #00ff00ff #222222ff run 2",
            "both #00ff00ff #0000ffff run 3",
            "both #111111ff #333333ff run 4",
            "both #aaaaaaff #bbbbbbff run 5"
        ]
    );
    // A name the tokens lacked is followed too; a change of most tokens
    // runs it with the rest, its own unchanged.
    assert_eq!(
        extra[..3],
        [
            "extra undefined run 1",
            "extra #ff0000ff run 2",
            "extra #ff0000ff run 3"
        ]
    );
    // Untouched until most tokens change.
    assert_eq!(
        others[..2],
        [
            "others #111111ff #222222ff #333333ff run 1",
            "others #010101ff #020202ff #333333ff run 2"
        ]
    );
    // The appearance runs every one of them once more, and that is all:
    // `radius` ran none.
    for (texts, runs) in [
        (&brand, 5),
        (&accent, 5),
        (&both, 6),
        (&extra, 4),
        (&others, 3),
    ] {
        assert_eq!(texts.len(), runs, "{texts:?}");
        let last = texts.last().unwrap();
        assert!(last.ends_with(&format!("run {runs}")), "{last}");
    }
}

/// More names than get a signal of their own: every one still follows its
/// token (through the signal of all of them).
const MANY_NAMES: &str = r##"import { mount, signal, Text, token, updateTheme, View } from "nsh";

for (let i = 0; i < 1100; i++) token(`made-up-${i}`);
const status = signal("start");
setTimeout(() => {
  updateTheme({ cssVars: { light: { late: "#00ff00" } } });
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
    let config = r##"{ "theme": { "cssVars": { "light": { "late": "#123456" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(MANY_NAMES, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "done"));
    assert_eq!(
        texts(&batches, "late "),
        ["late #123456ff", "late #00ff00ff"]
    );
}

/// 1 200 readers of a theme's 1 200 colors, and 1 100 readers of names no
/// theme has, each counting its runs.
const MANY: &str = r##"import { mount, signal, Text, token, updateTheme, View } from "nsh";

let runs = 0;
let made_up = 0;
const status = signal("start");
setTimeout(() => {
  updateTheme({ cssVars: { light: { "c-0": "#abcdef" } } });
  status.set("step 1");
}, 100);
setTimeout(() => {
  // A name made up past the signals' limit, which the theme gains.
  updateTheme({ cssVars: { light: { "made-1099": "#123123" } } });
  status.set("step 2");
}, 200);
setTimeout(() => status.set("done"), 300);

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
"##;

/// A theme of more than 1 024 names gives each name read a signal of its
/// own: a change of one token runs its reader alone. Names no theme has get
/// one up to the limit; past it they follow every change, and one the
/// theme gains reaches its reader.
#[test]
fn a_large_themes_names_each_keep_a_signal() {
    let mut light = serde_json::Map::new();
    for i in 0..1200 {
        light.insert(format!("c-{i}"), format!("#{:06x}", 0x100000 + i).into());
    }
    let config = serde_json::json!({ "theme": { "cssVars": { "light": light } } }).to_string();
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
    assert!(one.iter().any(|t| t == "c-0 #abcdefff 1201"), "{one:?}");
    assert_eq!(
        one.iter().filter(|t| t.starts_with("c-")).count(),
        1,
        "{one:?}"
    );
    assert!(
        one.iter().any(|t| t.starts_with("runs step 1 1201 ")),
        "{one:?}"
    );
    // The made-up name the theme gained reaches its reader.
    let gained = step("step 2");
    assert!(
        gained.iter().any(|t| t.starts_with("made-1099 #123123ff ")),
        "{gained:?}"
    );
    assert_eq!(
        gained.iter().filter(|t| t.starts_with("c-")).count(),
        0,
        "{gained:?}"
    );
}
