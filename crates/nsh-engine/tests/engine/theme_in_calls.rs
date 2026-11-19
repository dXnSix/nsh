//! Rust's theme writes inside a synchronous call from JS, through an
//! engine: a class extension's method (`builder.addTheme(..)`) or a module
//! function that calls `set_modifier`, `set_color_scheme`, `update_tokens`
//! or `reset_all_tokens` before `mount` sends the write with the entry's
//! turn, as JS's `setModifier` does, so the first frame shows it.

use std::time::Duration;

use nsh_core::{
    Color, MountBatch, MountItem,
    element::{Patch, PatchValues, Registry, set_modifier},
    platform::Appearance,
    prelude::*,
};
use nsh_engine::Environment;

use crate::components::start_with_sets;

/// An app builder, as a framework's TS class is.
#[class]
pub(crate) struct Builder;

#[class]
impl Builder {
    /// A builder.
    pub(crate) fn new() -> Self {
        Builder
    }
}

/// The methods a theming crate adds to the builder.
pub(crate) struct Theming;

#[class_extension(Builder)]
impl Theming {
    /// Picks the theme's `colorMode` context `mode`, and with `dark` the
    /// surface's appearance too.
    pub(crate) fn add_theme(
        this: Instance<Builder>,
        mode: String,
        dark: bool,
    ) -> Instance<Builder> {
        set_modifier("colorMode", &mode);
        if dark {
            set_color_scheme(Appearance::Dark);
        }
        this
    }
}

/// Overrides from module functions.
#[module]
pub(crate) struct Paint;

#[module]
impl Paint {
    /// Overrides token `name` with the color `hex` (`0xrrggbb`).
    pub(crate) fn color(name: String, hex: u32) {
        update_tokens(Patch::new().value(name.as_str(), Hsla::from(rgb(hex))));
    }

    /// Empties the overrides.
    pub(crate) fn clear() {
        reset_all_tokens();
    }
}

fn sets() -> Vec<Registry> {
    let mut builder = Registry::ui_set("test/builder");
    builder.class::<Builder>();
    let mut theming = Registry::ui_set("test/theming");
    theming.class_extension::<Theming>().module::<Paint>();
    vec![builder, theming]
}

/// The app's theme: a surface its `colorMode` modifier darkens.
const CONFIG: &str = r##"{ "theme": {
    "tokens": {
        "gray-1": { "kind": "color", "value": "#fcfcfc" },
        "surface": { "kind": "color", "value": "{gray-1}" }
    },
    "modifiers": {
        "colorMode": { "default": "light", "contexts": { "light": {}, "dark": { "gray-1": "#111111" } } }
    },
    "resolutionOrder": ["colorMode"]
} }"##;

/// The app: `setup` (the entry's top level, before `mount`), then a
/// square of the surface and a text of the modifier's context.
fn app(setup: &str) -> String {
    format!(
        r#"import {{ modifier, mount, setColorScheme, setModifier, Text, View }} from "nsh";
import {{ Builder }} from "test/builder";
import {{ Paint }} from "test/theming";

void setColorScheme;
void setModifier;
void Paint;
{setup}
mount(() => (
  <View>
    <View class="h-4 bg-surface" />
    <Text>{{`mode ${{modifier("colorMode")}}`}}</Text>
  </View>
));
"#
    )
}

/// The first frame of the app `setup` starts.
fn first_frame(setup: &str) -> MountBatch {
    let (_engine, rx) = start_with_sets(
        &app(setup),
        Environment::default(),
        None,
        Some(CONFIG),
        sets(),
    );
    rx.recv_timeout(Duration::from_secs(10)).unwrap()
}

/// The backgrounds `batch` draws.
fn backgrounds(batch: &MountBatch) -> Vec<Color> {
    batch
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff != 0 => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect()
}

/// The texts `batch` shows.
fn texts(batch: &MountBatch) -> Vec<String> {
    batch
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(text.text.to_string()),
            _ => None,
        })
        .collect()
}

/// The appearance `batch` sets, if it sets one.
fn appearance(batch: &MountBatch) -> Option<Appearance> {
    batch.items.iter().find_map(|item| match item {
        MountItem::Appearance { appearance } => Some(*appearance),
        _ => None,
    })
}

#[test]
fn a_modifier_a_class_extension_picks_before_mount_is_in_the_first_frame() {
    let first = first_frame(r#"new Builder().addTheme("dark", false);"#);
    assert_eq!(backgrounds(&first), [Color(0x111111ff)], "{first:?}");
    assert_eq!(texts(&first), ["mode dark"]);
}

#[test]
fn the_first_frame_is_the_one_js_s_set_modifier_gives() {
    let rust = first_frame(r#"new Builder().addTheme("dark", false);"#);
    let js = first_frame(r#"setModifier("colorMode", "dark");"#);
    assert_eq!(backgrounds(&rust), backgrounds(&js));
    assert_eq!(texts(&rust), texts(&js));
}

#[test]
fn without_the_pick_the_first_frame_shows_the_default_context() {
    let first = first_frame("new Builder();");
    assert_eq!(backgrounds(&first), [Color(0xfcfcfcff)]);
    assert_eq!(texts(&first), ["mode light"]);
}

#[test]
fn an_appearance_a_class_extension_sets_before_mount_is_in_the_first_frame() {
    let rust = first_frame(r#"new Builder().addTheme("dark", true);"#);
    assert_eq!(appearance(&rust), Some(Appearance::Dark), "{rust:?}");
    let js = first_frame(r#"setModifier("colorMode", "dark"); setColorScheme("dark");"#);
    assert_eq!(appearance(&rust), appearance(&js));
}

#[test]
fn overrides_module_functions_write_before_mount_are_in_the_first_frame() {
    let painted = first_frame("Paint.color(\"gray-1\", 0x00ff00);");
    assert_eq!(backgrounds(&painted), [Color(0x00ff00ff)]);
    let cleared = first_frame("Paint.color(\"gray-1\", 0x00ff00); Paint.clear();");
    assert_eq!(backgrounds(&cleared), [Color(0xfcfcfcff)]);
}
