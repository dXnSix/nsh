//! Some of the app's design tokens changed or put back while it runs:
//! `update_tokens` / `reset_tokens` in Rust, `updateTheme` / `resetTheme`
//! in TS, a patch over the active document (`nsh.json`'s, `setTheme`'s).

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_base::theme;
use nsh_core::{
    ApproxTextEngine, Color, Hsla, MountBatch, MountItem, Size,
    element::{Patch, PatchValues, Root},
    prelude::*,
};
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
const steps = [
  () => {
    updateTheme({ values: { brand: "#00ff00" } });
    return "one";
  },
  () => {
    updateTheme({ values: { brand: "#0000ff", accent: "#ff0000" } });
    updateTheme({ values: { brand: "#ffff00" } });
    return "two";
  },
  () => {
    // A value that doesn't fit is said; it still replaces the earlier
    // entry for its name (the patch is kept as given), so `accent` reads
    // the document's. The rest takes.
    const said = updateTheme({ values: { brand: "#00ffff", accent: "#zz" } });
    return `${said.map((d) => `${d.code} ${d.pointer}`).join(", ")}; took`;
  },
  () => {
    updateTheme({ values: { "radius-card": "auto" } });
    return `kept ${token("radius-card")}`;
  },
  () => {
    // A reference resolves after the layers: it follows its target.
    updateTheme({ values: { accent: "{brand}" } });
    return "ref";
  },
  () => {
    resetTheme(["brand"]);
    return "reset";
  },
  () => {
    resetTheme();
    return "all";
  },
  () => {
    setTheme({ tokens: { brand: { kind: "color", value: "#abcdef" } } });
    updateTheme({ values: { brand: "#fedcba" } });
    resetTheme(["brand"]);
    return "set";
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
    <Text>{`${status()} | ${token("brand")} ${token("accent")}`}</Text>
  </View>
));
"##;

/// `updateTheme` changes the tokens it sets and keeps the others; two in a
/// row compose, the later entry winning; an entry that doesn't fit is
/// returned, and replaces the earlier one for its name, while the rest
/// takes; a reference follows its target; `resetTheme` removes the named
/// tokens (or all) from the app's patch, so they read the document again,
/// `nsh.json`'s or the one `setTheme` set. The turn's writes resolve once:
/// `token()` and the classes naming the tokens follow within the frame.
#[test]
fn update_theme_and_reset_theme_change_some_tokens_while_it_runs() {
    let config = r##"{ "theme": { "tokens": {
        "radius-card": { "kind": "number", "value": 14 },
        "brand": { "kind": "color", "value": "#123456" },
        "accent": { "kind": "color", "value": "#222222" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(UPDATE_THEME, Environment::default(), None, Some(config));
    let error = "TypeError: resetTheme: expected token names (string[]) or nothing";
    let batches = crate::components::until(&rx, |b| {
        crate::components::shows(b, &format!("{error} | #abcdef undefined"))
    });
    assert_eq!(
        texts(&batches),
        [
            "start | #123456 #222222",
            "one | #00ff00 #222222",
            "two | #ffff00 #ff0000",
            "bad-value /values/accent; took | #00ffff #222222",
            "kept 14 | #00ffff #222222",
            "ref | #00ffff #00ffff",
            "reset | #123456 #123456",
            "all | #123456 #222222",
            "set | #abcdef undefined",
            &format!("{error} | #abcdef undefined"),
        ]
    );
    assert_eq!(
        backgrounds(&batches),
        [
            Color(0x123456ff),
            Color(0x00ff00ff),
            Color(0xffff00ff),
            Color(0x00ffffff),
            Color(0x123456ff),
            Color(0xabcdefff),
        ]
    );
}

/// A square of the theme's primary color that changes it, then puts it
/// back, while it runs.
#[component]
fn Steps() -> impl IntoElement {
    let primary = |hex: u32| Patch::new().value("primary", Hsla::from(rgb(hex)));
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
    let app_theme = r##"{ "tokens": { "primary": { "kind": "color", "value": "#0000ff" } } }"##;
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(Root::new::<Steps>(|| ()).theme(app_theme)),
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
        support_dir: None,
        data_dir: None,
        cache: nsh_engine::CacheOptions::off(),
        storage: None,
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        audio: None,
        windows: None,
        login_item: None,
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
