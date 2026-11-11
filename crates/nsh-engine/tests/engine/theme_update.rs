//! Some of the app's design tokens changed or put back while it runs:
//! `update_tokens` / `reset_tokens` in Rust, `updateTheme` / `resetTheme`
//! in TS, over the theme the app has as a whole (`nsh.json`'s, `setTheme`'s).

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_base::{Theme, theme};
use nsh_core::{ApproxTextEngine, Color, MountBatch, MountItem, Size, element::Root, prelude::*};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

/// The backgrounds the batches draw, in order, without repeats.
fn backgrounds(batches: &[MountBatch]) -> Vec<Color> {
    let mut seen: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff != 0 => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect();
    seen.dedup();
    seen
}

/// The texts the batches show, in order.
fn texts(batches: &[MountBatch]) -> Vec<&str> {
    batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(&*text.text),
            _ => None,
        })
        .collect()
}

/// A TSX app changing some of its theme's tokens while it runs: a step
/// every 100 ms, each showing what `token()` reads right after it, and a
/// view drawn with `bg-brand`.
const UPDATE_THEME: &str = r##"import { mount, resetTheme, setTheme, signal, Text, token, updateTheme, View } from "nsh";

const status = signal("start");
const both = () => `${token("brand")} ${token("accent")}`;
const steps = [
  () => {
    updateTheme({ cssVars: { light: { brand: "#00ff00" } } });
    return `one ${both()}`;
  },
  () => {
    updateTheme({ cssVars: { light: { brand: "#0000ff", accent: "#ff0000" } } });
    updateTheme({ cssVars: { light: { brand: "#ffff00" } } });
    return `two ${both()}`;
  },
  () => {
    try {
      updateTheme({ cssVars: { light: { brand: "#00ffff", accent: "#zz" } } });
      return "taken";
    } catch (e) {
      return `${e.name}: ${e.message}; kept ${both()}`;
    }
  },
  () => {
    // A value of no kind a token has is left out: the token keeps its value.
    updateTheme({ cssVars: { theme: { "radius-card": "auto" } } });
    return `kept ${token("radius-card")}`;
  },
  () => {
    // A `var()` it does not set names the token shown now.
    updateTheme({ cssVars: { light: { accent: "var(--brand)" } } });
    return `var ${both()}`;
  },
  () => {
    resetTheme(["brand"]);
    return `reset ${both()}`;
  },
  () => {
    resetTheme();
    return `all ${both()}`;
  },
  () => {
    setTheme({ cssVars: { light: { brand: "#abcdef" } } });
    updateTheme({ cssVars: { light: { brand: "#fedcba" } } });
    resetTheme(["brand"]);
    return `set ${token("brand")}`;
  },
  () => {
    try {
      resetTheme("brand");
      return "taken";
    } catch (e) {
      return `${e.name}: ${e.message}`;
    }
  },
];
steps.forEach((step, i) => setTimeout(() => status.set(step()), 100 * (i + 1)));

mount(() => (
  <View>
    <View class="h-4 bg-brand" />
    <Text>{status()}</Text>
  </View>
));
"##;

/// `updateTheme` changes the tokens it sets and keeps the others; two in a
/// row compose; one with a value NSH cannot read throws a `TypeError` and
/// changes nothing, and one it leaves out keeps its value; a `var()`
/// names the token shown now; `resetTheme` puts the named tokens (or all) back to the
/// theme's, `nsh.json`'s or the one `setTheme` set. `token()` reads each
/// change at once, and the classes naming the tokens follow.
#[test]
fn update_theme_and_reset_theme_change_some_tokens_while_it_runs() {
    let config = r##"{ "theme": { "cssVars": { "theme": { "radius-card": "14px" }, "light": { "brand": "#123456", "accent": "#222222" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(UPDATE_THEME, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| {
        crate::components::shows(
            b,
            "TypeError: resetTheme: expected token names (string[]) or nothing",
        )
    });
    assert_eq!(
        texts(&batches),
        [
            "start",
            "one #00ff00ff #222222ff",
            "two #ffff00ff #ff0000ff",
            "TypeError: updateTheme: `accent`: #zz is no color; kept #ffff00ff #ff0000ff",
            "kept 14",
            "var #ffff00ff #ffff00ff",
            "reset #123456ff #ffff00ff",
            "all #123456ff #222222ff",
            "set #abcdefff",
            "TypeError: resetTheme: expected token names (string[]) or nothing",
        ]
    );
    assert_eq!(
        backgrounds(&batches),
        [
            Color(0x123456ff),
            Color(0x00ff00ff),
            Color(0xffff00ff),
            Color(0x123456ff),
            Color(0xabcdefff),
        ]
    );
}

/// A square of the theme's primary color that changes it, then puts it
/// back, while it runs.
#[component]
fn Steps() -> impl IntoElement {
    let primary = |hex: u32| {
        let mut tokens = nsh_core::element::Tokens::default();
        tokens.set_color("primary", rgb(hex).into());
        tokens
    };
    set_timeout(Duration::from_millis(100), move || {
        update_tokens(primary(0x00ff00));
        update_tokens(primary(0xff0000));
    });
    set_timeout(Duration::from_millis(200), || reset_tokens(["primary"]));
    view().size_10().bg(move || theme().primary)
}

/// A Rust app's `update_tokens` and `reset_tokens`: two updates in one turn
/// compose, and a reset goes back to the app's theme (its root's), not the
/// UI set's default.
#[test]
fn a_rust_app_updates_and_resets_its_tokens() {
    let app_theme = Theme::from_json(r##"{ "cssVars": { "light": { "primary": "#0000ff" } } }"##)
        .unwrap()
        .into_tokens();
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(Root::new::<Steps>(|| ()).tokens(app_theme)),
        viewport: Size::new(px(390.0), px(844.0)),
        trace: false,
        content_culling: false,
        network: None,
        websocket: None,
        refuse_connections: None,
        filesystem: None,
        assets: None,
        environment: Environment::default(),
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: Vec::new(),
        runtime: None,
        data_dir: None,
        cache: nsh_engine::CacheOptions::off(),
        storage: None,
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        speech: None,
        audio: None,
        windows: None,
    };
    let _engine: Engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    let seen = collect(&rx);
    assert_eq!(
        backgrounds(&seen),
        [Color(0x0000ffff), Color(0xff0000ff), Color(0x0000ffff)]
    );
}

/// The batches within a moment.
fn collect(rx: &Receiver<MountBatch>) -> Vec<MountBatch> {
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(500)) {
        seen.push(batch);
    }
    seen
}
