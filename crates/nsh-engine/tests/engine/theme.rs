//! The embedding app's theme: sent before the start it is in the first
//! frame, sent later it updates what read the theme, and it wins over the
//! app's own theme until cleared. The app's own theme set while it runs
//! (`setTheme`).

use std::{collections::BTreeMap, time::Duration};

use crossbeam_channel::Receiver;
use nsh_base::{Theme, theme};
use nsh_core::{
    ApproxTextEngine, Color, MountBatch, MountItem, Size,
    element::{Patch, Root},
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

/// A square of the theme's primary color.
#[component]
fn Swatch() -> impl IntoElement {
    view().size_10().bg(move || theme().primary)
}

/// The primary color of a theme that sets none: `nsh-base`'s default.
fn default_primary() -> Color {
    Color::from(Theme::default().primary)
}

/// A theme patch of `values` (`{ "primary": "#00ff00" }`).
fn patch(values: &str) -> Patch {
    nsh_theme::read_patch(&format!(r#"{{ "values": {values} }}"#)).0
}

fn themed(hex: &str) -> Patch {
    patch(&format!(r#"{{ "primary": "{hex}" }}"#))
}

/// The Rust app's own document: a `primary` its overrides and the host's
/// patch can set.
const SWATCH_THEME: &str =
    r##"{ "tokens": { "primary": { "kind": "color", "value": "#888888" } } }"##;

/// [`Swatch`] under [`SWATCH_THEME`] with the app's overrides `app_theme`,
/// and the host's patch `host_theme` sent before the start.
fn start(app_theme: Option<Patch>, host_theme: Option<Patch>) -> (Engine, Receiver<MountBatch>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut root = Root::new::<Swatch>(|| ()).theme(SWATCH_THEME);
    if let Some(theme) = app_theme {
        root = root.theme_overrides(nsh_theme::write_patch(&theme));
    }
    let config = EngineConfig {
        app: App::Rust(root),
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
        login_item: None,
        data_dir: None,
    };
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        |control| {
            if let Some(theme) = host_theme {
                control.set_tokens(Some(theme));
            }
        },
    )
    .unwrap();
    (engine, rx)
}

/// The backgrounds of the batches within a moment.
fn backgrounds(rx: &Receiver<MountBatch>) -> Vec<Color> {
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(300)) {
        for item in &batch.items {
            if let MountItem::ViewStyle { style, .. } = item
                && style.background_color.0 & 0xff != 0
            {
                seen.push(style.background_color);
            }
        }
    }
    seen
}

#[test]
fn a_host_theme_sent_before_the_start_is_in_the_first_frame() {
    let (_engine, rx) = start(Some(themed("#0000ff")), Some(themed("#00ff00")));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let colors: Vec<Color> = first
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::ViewStyle { style, .. } => Some(style.background_color),
            _ => None,
        })
        .collect();
    assert_eq!(colors, vec![Color(0x00ff00ff)]);
}

#[test]
fn the_host_theme_updates_the_views_and_gives_way_to_the_apps_when_cleared() {
    let (engine, rx) = start(Some(themed("#0000ff")), None);
    assert_eq!(backgrounds(&rx), vec![Color(0x0000ffff)]);
    engine.control().set_tokens(Some(themed("#00ff00")));
    assert_eq!(backgrounds(&rx), vec![Color(0x00ff00ff)]);
    engine.control().set_tokens(None);
    assert_eq!(backgrounds(&rx), vec![Color(0x0000ffff)]);
}

#[test]
fn a_tsx_apps_token_classes_follow_the_host_theme() {
    let dir = std::env::temp_dir().join(format!("nsh-theme-tokens-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(
        &entry,
        "import { mount, View } from \"nsh\";\n\
         mount(() => <View class=\"h-10 w-10 bg-primary\" />);\n",
    )
    .unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
        ..Default::default()
    })
    .unwrap();
    let bytecode = nsh_js::compile_bytecode(&bundle.code, false).unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        // SAFETY: compiled just now by this runtime.
        app: App::Js(unsafe { nsh_engine::Bundle::bytecode(bytecode) }),
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
        ui_sets: vec![nsh_base::registry()],
        runtime: None,
        support_dir: None,
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
        login_item: None,
        data_dir: None,
    };
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    assert_eq!(backgrounds(&rx), vec![default_primary()]);
    engine.control().set_tokens(Some(themed("#00ff00")));
    assert_eq!(backgrounds(&rx), vec![Color(0x00ff00ff)]);
    engine.control().set_tokens(None);
    assert_eq!(backgrounds(&rx), vec![default_primary()]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The backgrounds the views draw within a moment, by node id.
fn backgrounds_by_id(rx: &Receiver<MountBatch>) -> BTreeMap<u32, Color> {
    let mut seen = BTreeMap::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(300)) {
        for item in &batch.items {
            if let MountItem::ViewStyle { id, style } = item
                && style.background_color.0 & 0xff != 0
            {
                seen.insert(*id, style.background_color);
            }
        }
    }
    seen
}

/// The SDKs' theme overrides, a patch: what the host sets wins over the
/// app's own theme (`nsh.json`), and what it leaves out stays the app's.
#[test]
fn a_hosts_theme_overrides_win_over_the_apps_tokens_they_set_and_keep_the_rest() {
    let dir = std::env::temp_dir().join(format!("nsh-theme-overrides-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("nsh.json"),
        r##"{ "theme": { "tokens": {
            "primary": { "kind": "color", "value": "#0000ff" },
            "background": { "kind": "color", "value": "#ffff00" }
        } } }"##,
    )
    .unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(
        &entry,
        "import { mount, View } from \"nsh\";\n\
         mount(() => (\n\
           <View>\n\
             <View class=\"h-10 w-10 bg-primary\" />\n\
             <View class=\"h-10 w-10 bg-background\" />\n\
           </View>\n\
         ));\n",
    )
    .unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
        ..Default::default()
    })
    .unwrap();
    let bytecode = nsh_js::compile_bytecode(&bundle.code, false).unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        // SAFETY: compiled just now by this runtime.
        app: App::Js(unsafe { nsh_engine::Bundle::bytecode(bytecode) }),
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
        ui_sets: vec![nsh_base::registry()],
        runtime: None,
        support_dir: None,
        storage: None,
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        speech: None,
        audio: None,
        cache: Default::default(),
        windows: None,
        login_item: None,
        data_dir: None,
    };
    // A brand's primary, set before the app runs.
    let overrides = patch(r##"{ "primary": "#e11d48" }"##);
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        |control| control.set_tokens(Some(overrides)),
    )
    .unwrap();
    let colors = |seen: BTreeMap<u32, Color>| seen.into_values().collect::<Vec<_>>();
    // The host's primary; the app's own background stays.
    assert_eq!(
        colors(backgrounds_by_id(&rx)),
        vec![Color(0xe11d48ff), Color(0xffff00ff)]
    );
    // Changed live, then cleared: the app's own again.
    engine
        .control()
        .set_tokens(Some(patch(r##"{ "primary": "#00ff00" }"##)));
    assert_eq!(colors(backgrounds_by_id(&rx)), vec![Color(0x00ff00ff)]);
    engine.control().set_tokens(None);
    assert!(colors(backgrounds_by_id(&rx)).contains(&Color(0x0000ffff)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The texts of a batch.
fn texts(batch: &MountBatch) -> Vec<String> {
    batch
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

/// The last text the batches within a moment show.
fn last_text(rx: &Receiver<MountBatch>) -> Option<String> {
    let mut last = None;
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(300)) {
        if let Some(text) = texts(&batch).pop() {
            last = Some(text);
        }
    }
    last
}

/// A TSX app whose theme (`nsh.json`) adds a token, `brand`, which it reads
/// once as its entry evaluates and again where it renders, started with
/// the embedding app's tokens `host` sent before the start.
fn start_branded(name: &str, host: Option<Patch>) -> (Engine, Receiver<MountBatch>) {
    let dir = crate::temporary_root().join(format!("nsh-theme-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("nsh.json"),
        r##"{ "theme": { "tokens": { "brand": { "kind": "color", "value": "#123456" } } } }"##,
    )
    .unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(
        &entry,
        "import { mount, Text, token } from \"nsh\";\n\
         const atStart = token(\"brand\");\n\
         mount(() => <Text>{`${atStart} ${token(\"brand\")}`}</Text>);\n",
    )
    .unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
        ..Default::default()
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let bytecode = nsh_js::compile_bytecode(&bundle.code, false).unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        // SAFETY: compiled just now by this runtime.
        app: App::Js(unsafe { nsh_engine::Bundle::bytecode(bytecode) }),
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
        ui_sets: vec![nsh_base::registry()],
        runtime: None,
        support_dir: None,
        storage: None,
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        speech: None,
        audio: None,
        cache: Default::default(),
        windows: None,
        login_item: None,
        data_dir: None,
    };
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        |control| {
            if host.is_some() {
                control.set_tokens(host);
            }
        },
    )
    .unwrap();
    (engine, rx)
}

/// The app's theme reaches the core with the entry's first turn, which
/// resolves it once: what the render reads follows before the first frame
/// is sent, so the frame shows it. What the entry read once, as it started,
/// is the app's too: the runtime reads it from the bundle's setup on.
#[test]
fn the_apps_theme_tokens_are_read_in_the_first_render() {
    let (_engine, rx) = start_branded("first-render", None);
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts(&first), vec!["#123456 #123456".to_owned()]);
}

/// The embedding app's tokens sent before the start win in the first
/// render too; changed later, what read them reactively follows.
#[test]
fn the_host_tokens_win_in_the_first_render_and_reads_follow_their_changes() {
    let overrides = |hex: &str| patch(&format!(r#"{{ "brand": "{hex}" }}"#));
    let (engine, rx) = start_branded("host-first-render", Some(overrides("#abcdef")));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts(&first), vec!["#abcdef #abcdef".to_owned()]);
    engine.control().set_tokens(Some(overrides("#00ff00")));
    assert_eq!(last_text(&rx).as_deref(), Some("#abcdef #00ff00"));
    engine.control().set_tokens(None);
    assert_eq!(last_text(&rx).as_deref(), Some("#abcdef #123456"));
}

/// A TSX app that sets its theme while it runs: a step every 100 ms, each
/// shown with what `token("brand")` reads in the frame after it, and a
/// view drawn with `bg-brand`.
const SET_THEME: &str = r##"import { effect, mount, setModifier, setTheme, signal, Text, token, View } from "nsh";

const status = signal("start");
let runs = 0;
const steps = [
  () => {
    setTheme({
      tokens: { brand: { kind: "color", value: "#00ff00" } },
      modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { brand: "#003300" } } } },
      resolutionOrder: ["colorMode"],
    });
    return "set";
  },
  () => {
    setModifier("colorMode", "dark");
    return "dark";
  },
  () => {
    setModifier("colorMode", "light");
    const said = setTheme({ cssVars: { light: { brand: "#0000ff" } } }); // refused: nsh theme convert converts it
    return `${said.map((d) => d.code).join(",")}; kept`;
  },
  () => {
    setTheme(null);
    return "back";
  },
  () => `runs ${runs}`,
];
steps.forEach((step, i) => setTimeout(() => status.set(step()), 100 * (i + 1)));

function App() {
  effect(() => {
    token("brand");
    runs += 1;
  });
  return (
    <View>
      <View class="h-4 bg-brand" />
      <Text>{`${status()} ${token("brand")}`}</Text>
    </View>
  );
}

mount(() => <App />);
"##;

/// `setTheme` replaces the app's document while it runs: `token()` and the
/// classes naming its tokens follow within the frame, under each context
/// picked; an old theme (`nsh theme convert`'s input) is refused whole and the document stays;
/// `null` gives back `nsh.json`'s. Each change runs what read a token once,
/// and the first frame once, with the app's document.
#[test]
fn set_theme_replaces_the_apps_theme_while_it_runs() {
    let config =
        r##"{ "theme": { "tokens": { "brand": { "kind": "color", "value": "#123456" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(SET_THEME, Environment::default(), None, Some(config));
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, "runs 5 #123456"));
    let texts: Vec<&str> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(&*text.text),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        [
            "start #123456",
            "set #00ff00",
            "dark #003300",
            "legacy-format; kept #00ff00",
            "back #123456",
            "runs 5 #123456",
        ]
    );
    let mut backgrounds: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::ViewStyle { style, .. } if style.background_color.0 & 0xff != 0 => {
                Some(style.background_color)
            }
            _ => None,
        })
        .collect();
    backgrounds.dedup();
    assert_eq!(
        backgrounds,
        [
            Color(0x123456ff),
            Color(0x00ff00ff),
            Color(0x003300ff),
            Color(0x00ff00ff),
            Color(0x123456ff),
        ]
    );
}

/// `setTheme` refuses whole only what is no document or one of the old
/// format (`nsh theme convert`'s input). A document with a bad entry is activated with the
/// entry left out. Each returns its diagnostics, and the core draws the
/// last document of the turn: the turn's writes resolve once.
#[test]
fn set_theme_refuses_what_is_no_document_and_leaves_out_bad_entries() {
    let source = r##"import { mount, setTheme, signal, Text, token, View } from "nsh";
const status = signal("start");
const green = { tokens: { brand: { kind: "color", value: "#00ff00" } } };
setTimeout(() => {
  const invalid = [
    { cssVars: null }, // the old format, which nsh theme convert converts
    { cssVars: { light: { brand: "rgb(255 0 0" } } }, // the same, nsh theme convert's input
    [],
    [{ tokens: {} }],
    { tokens: { brand: { kind: "color", value: "#zz" } } },
    { tokens: { brand: { kind: "color", value: "rgb(255 0 0)" } } },
    { tokens: { brand: { kind: "color", value: "{nothing}" } } },
  ];
  const results = invalid.map((theme) => {
    setTheme(green);
    return setTheme(theme).map((d) => d.code).join(",");
  });
  setTheme(green);
  status.set(results.join("; "));
}, 100);
setTimeout(() => status.set(`${status()}; now ${token("brand")}`), 200);
mount(() => <View><View class="h-4 bg-brand" /><Text>{status()}</Text></View>);
"##;
    let config =
        r##"{ "theme": { "tokens": { "brand": { "kind": "color", "value": "#123456" } } } }"##;
    let (_engine, rx) =
        crate::components::start_with(source, Environment::default(), None, Some(config));
    let expected = [
        "legacy-format",
        "legacy-format",
        "not-a-document",
        "not-a-document",
        "bad-value",
        "bad-value",
        "reference-missing",
        "now #00ff00",
    ]
    .join("; ");
    let batches = crate::components::until(&rx, |b| crate::components::shows(b, &expected));
    let texts: Vec<&str> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(&*text.text),
            _ => None,
        })
        .collect();
    assert_eq!(texts.last().copied(), Some(expected.as_str()));
    let themed = batches
        .iter()
        .flat_map(|b| &b.items)
        .find_map(|item| match item {
            MountItem::ViewStyle { id, style } if style.background_color == Color(0x123456ff) => {
                Some(*id)
            }
            _ => None,
        })
        .expect("the built theme paints the themed view before the timer runs");
    let mut backgrounds: Vec<Color> = batches
        .iter()
        .flat_map(|b| &b.items)
        .filter_map(|item| match item {
            // Keep transparent changes too: dropping the token must not
            // leave the preceding opaque color as the last value seen.
            MountItem::ViewStyle { id, style } if *id == themed => Some(style.background_color),
            _ => None,
        })
        .collect();
    backgrounds.dedup();
    assert_eq!(backgrounds, [Color(0x123456ff), Color(0x00ff00ff)]);
}
