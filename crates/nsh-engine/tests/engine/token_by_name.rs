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
