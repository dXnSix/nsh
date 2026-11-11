//! `ThemeScope` from TSX: a view whose content is shown in an appearance
//! of its own. `dark:` classes and theme colors take the scope's side in
//! the core; `colorScheme()` and `token()` read the scope around what
//! calls them, and a Rust component in a scope reads it too.

use nsh_core::{Color, MountBatch, MountItem};
use nsh_engine::Environment;

use crate::components::{shows, start_with, until};

const THEME: &str = r##"{ "theme": { "cssVars": {
    "light": { "up": "#00ff00" }, "dark": { "up": "#004400" } } } }"##;

const SCOPES: &str = r#"import { colorScheme, mount, setColorScheme, signal, Text, ThemeScope, token, View } from "nsh";

const scheme = signal<"light" | "dark" | undefined>("dark");
const step = signal(0);
setTimeout(() => { scheme.set("light"); step.set(1); }, 30);
setTimeout(() => { setColorScheme("dark"); step.set(2); }, 60);
setTimeout(() => { scheme.set(undefined); step.set(3); }, 90);

function Reader(props: { name: string }) {
  return (
    <View class="bg-white dark:bg-black">
      <Text>{`${props.name} ${colorScheme()} ${token("up")}`}</Text>
    </View>
  );
}

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Reader name="out" />
    <ThemeScope appearance={scheme()} class="p-2">
      <Text>{`inline ${colorScheme()}`}</Text>
      <Reader name="in" />
      <ThemeScope>
        <Reader name="nested" />
      </ThemeScope>
    </ThemeScope>
  </View>
));
"#;

/// The texts shown once `step` is, by all the batches up to it, sorted.
fn texts_at(rx: &crossbeam_channel::Receiver<MountBatch>, step: &str) -> Vec<String> {
    let batches = until(rx, |b| shows(b, step));
    assert!(batches.iter().any(|b| shows(b, step)), "{batches:?}");
    let mut texts: Vec<String> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| match i {
            MountItem::Text { text, .. } if !text.text.starts_with("step") => {
                Some(text.text.clone())
            }
            _ => None,
        })
        .collect();
    // Which reader runs first is no contract.
    texts.sort();
    texts
}

#[test]
fn what_a_scope_renders_reads_its_appearance() {
    let (_engine, rx) = start_with(SCOPES, Environment::default(), None, Some(THEME));
    // The scope is dark in a light app, and so is the scope inside it.
    assert_eq!(
        texts_at(&rx, "step 0"),
        [
            "in dark #004400ff",
            "inline dark",
            "nested dark #004400ff",
            "out light #00ff00ff"
        ]
    );
    // The scope turns light: only what it holds reads again.
    assert_eq!(
        texts_at(&rx, "step 1"),
        [
            "in light #00ff00ff",
            "inline light",
            "nested light #00ff00ff"
        ]
    );
    // The app turns dark: the scope stays as it says.
    assert_eq!(texts_at(&rx, "step 2"), ["out dark #004400ff"]);
    // Without an appearance of its own it is shown as the app.
    assert_eq!(
        texts_at(&rx, "step 3"),
        ["in dark #004400ff", "inline dark", "nested dark #004400ff"]
    );
}

const CLASSES: &str = r#"import { mount, Text, ThemeScope, View } from "nsh";

mount(() => (
  <View class="bg-white dark:bg-black">
    <ThemeScope appearance="dark" class="bg-white dark:bg-black">
      <View class="bg-white dark:bg-black" />
    </ThemeScope>
    <Text>shown</Text>
  </View>
));
"#;

#[test]
fn classes_in_a_scope_take_its_appearance() {
    let (_engine, rx) = start_with(CLASSES, Environment::default(), None, None);
    let batches = until(&rx, |b| shows(b, "shown"));
    let backgrounds: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| match i {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff == 0xff => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect();
    // The view around the scope, the scope, the view in it.
    assert_eq!(
        backgrounds,
        [Color(0xffff_ffff), Color(0x0000_00ff), Color(0x0000_00ff)]
    );
    // The platform shows its own controls in the scope's view dark.
    assert!(batches.iter().flat_map(|b| &b.items).any(|i| matches!(
        i,
        MountItem::ViewProps { props, .. }
            if props.appearance == nsh_core::platform::Appearance::Dark
    )));
}

const RUST_INSIDE: &str = r#"import { mount, Text, ThemeScope, View } from "nsh";
import { Swatch } from "test/ui";

mount(() => (
  <View>
    <Swatch />
    <ThemeScope appearance="dark">
      <Swatch />
    </ThemeScope>
    <Text>shown</Text>
  </View>
));
"#;

#[test]
fn a_rust_component_in_a_scope_reads_its_appearance() {
    let (_engine, rx) = start_with(RUST_INSIDE, Environment::default(), None, None);
    let batches = until(&rx, |b| shows(b, "shown"));
    let said: Vec<&str> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| match i {
            MountItem::Text { text, .. } if text.text.starts_with("swatch") => {
                Some(text.text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(said, ["swatch light", "swatch dark"]);
}

const TOKENS: &str = r##"import { mount, signal, Text, ThemeScope, type Theme, token, updateTheme, View } from "nsh";
import { TokenSwatch } from "test/ui";

const theme = signal<Theme | undefined>({ cssVars: { light: { up: "#7c3aed" }, dark: { up: "#a78bfa" } } });
const step = signal(0);
setTimeout(() => { theme.set({ cssVars: { light: { up: "#db2777" } } }); step.set(1); }, 30);
setTimeout(() => { updateTheme({ cssVars: { light: { down: "#111111" }, dark: { down: "#111111" } } }); step.set(2); }, 60);
setTimeout(() => { theme.set(undefined); step.set(3); }, 90);

function Reader(props: { name: string }) {
  return (
    <View class="bg-up">
      <Text>{`${props.name} ${token("up")} ${token("down")}`}</Text>
    </View>
  );
}

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Reader name="out" />
    <ThemeScope theme={theme()}>
      <Reader name="in" />
      <TokenSwatch />
      <ThemeScope appearance="dark">
        <Reader name="dark" />
      </ThemeScope>
    </ThemeScope>
  </View>
));
"##;

const TOKENS_THEME: &str = r##"{ "theme": { "cssVars": {
    "light": { "up": "#00ff00", "down": "#ff0000" },
    "dark": { "up": "#004400", "down": "#440000" } } } }"##;

/// The backgrounds the batches up to `step` gave views, sorted.
fn at(rx: &crossbeam_channel::Receiver<MountBatch>, step: &str) -> (Vec<String>, Vec<Color>) {
    let batches = until(rx, |b| shows(b, step));
    assert!(batches.iter().any(|b| shows(b, step)), "{batches:?}");
    let mut texts: Vec<String> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| match i {
            MountItem::Text { text, .. } if !text.text.starts_with("step") => {
                Some(text.text.clone())
            }
            _ => None,
        })
        .collect();
    texts.sort();
    let mut colors: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|i| match i {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff == 0xff => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect();
    colors.sort_by_key(|c| c.0);
    (texts, colors)
}

#[test]
fn what_a_scope_renders_reads_and_draws_its_tokens() {
    let (_engine, rx) = start_with(TOKENS, Environment::default(), None, Some(TOKENS_THEME));
    // The scope sets `up` and leaves `down`: in it `token()`, the class
    // and a Rust component read its `up`, in each appearance.
    let (texts, colors) = at(&rx, "step 0");
    assert_eq!(
        texts,
        [
            "dark #a78bfaff #440000ff",
            "in #7c3aedff #ff0000ff",
            "out #00ff00ff #ff0000ff",
            "swatch 7c3aed"
        ]
    );
    assert_eq!(
        colors,
        [Color(0x00ff_00ff), Color(0x7c3a_edff), Color(0xa78b_faff)]
    );
    // The scope's theme changes: only what it holds follows (its dark
    // value is the light one now).
    let (texts, colors) = at(&rx, "step 1");
    assert_eq!(
        texts,
        [
            "dark #db2777ff #440000ff",
            "in #db2777ff #ff0000ff",
            "swatch db2777"
        ]
    );
    assert_eq!(colors, [Color(0xdb27_77ff), Color(0xdb27_77ff)]);
    // The app changes a token the scope leaves alone: it reaches into it.
    let (texts, _) = at(&rx, "step 2");
    assert_eq!(
        texts,
        [
            "dark #db2777ff #111111ff",
            "in #db2777ff #111111ff",
            "out #00ff00ff #111111ff"
        ]
    );
    // Without a theme it is drawn with the app's.
    let (texts, colors) = at(&rx, "step 3");
    assert_eq!(
        texts,
        [
            "dark #004400ff #111111ff",
            "in #00ff00ff #111111ff",
            "swatch 00ff00"
        ]
    );
    assert_eq!(colors, [Color(0x0044_00ff), Color(0x00ff_00ff)]);
}
