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
