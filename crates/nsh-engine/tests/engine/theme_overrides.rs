//! The embedding app's theme overrides as a theme file
//! (`EngineControl::set_theme_overrides`), read by the core with the app's
//! UI set against the app's defaults: the same tokens the SDKs made with
//! `nsh_base::theme_overrides` before, in the first frame when sent before
//! the start, followed when sent later, and cleared with `set_tokens(None)`.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{ApproxTextEngine, MountBatch, MountItem, Size, prelude::*};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

/// A TSX app whose theme (`nsh.json`) adds `brand`, showing it.
fn start(name: &str, overrides: Option<&str>) -> (Engine, Receiver<MountBatch>) {
    let dir =
        crate::temporary_root().join(format!("nsh-theme-overrides-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("nsh.json"),
        r##"{ "theme": { "cssVars": { "light": { "brand": "#123456" } } } }"##,
    )
    .unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(
        &entry,
        "import { mount, Text, token } from \"nsh\";\n\
         mount(() => <Text>{`${token(\"brand\")} ${token(\"primary\")} ${token(\"edge\")}`}</Text>);\n",
    )
    .unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
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
        data_dir: None,
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
    };
    let overrides = overrides.map(str::to_owned);
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        |control| {
            if let Some(json) = overrides {
                control.set_theme_overrides(json);
            }
        },
    )
    .unwrap();
    (engine, rx)
}

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

fn hex(c: nsh_core::element::AdaptiveColor) -> String {
    format!("#{:08x}", nsh_core::Color::from(c.light).0)
}

/// The default theme's primary, as `token()` shows it.
fn primary() -> String {
    hex(nsh_base::Theme::default().primary)
}

#[test]
fn overrides_sent_before_the_start_are_in_the_first_render() {
    let (_engine, rx) = start(
        "first",
        Some(r##"{ "cssVars": { "light": { "brand": "#abcdef" } } }"##),
    );
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        texts(&first),
        vec![format!("#abcdefff {} undefined", primary())]
    );
}

/// A `var()` they name and do not set is the default theme's token, not
/// the app's (`nsh.json`'s `brand`): one naming a token only the app has is
/// left out. Changed later they are followed, and `set_tokens(None)` gives
/// the app's own back.
#[test]
fn overrides_follow_their_changes_and_name_the_default_theme() {
    let (engine, rx) = start("later", None);
    assert_eq!(
        last_text(&rx),
        Some(format!("#123456ff {} undefined", primary()))
    );
    engine.control().set_theme_overrides(
        r##"{ "cssVars": { "light": { "brand": "var(--primary)", "primary": "#00ff00" } } }"##,
    );
    // The file's own `primary` is the one `var(--primary)` names.
    assert_eq!(
        last_text(&rx).as_deref(),
        Some("#00ff00ff #00ff00ff undefined")
    );
    engine.control().set_theme_overrides(
        r##"{ "cssVars": { "light": { "primary": "var(--ring)", "edge": "var(--brand)" } } }"##,
    );
    // `ring`, which it does not set, is the default theme's; `brand`, only
    // the app's, is none of the default theme's, so `edge` is left out.
    let ring = hex(nsh_base::Theme::default().ring);
    assert_eq!(last_text(&rx), Some(format!("#123456ff {ring} undefined")));
    engine.control().set_tokens(None);
    assert_eq!(
        last_text(&rx),
        Some(format!("#123456ff {} undefined", primary()))
    );
}

/// Text that is no JSON changes nothing.
#[test]
fn overrides_that_are_no_json_change_nothing() {
    let (engine, rx) = start("bad", None);
    assert_eq!(
        last_text(&rx),
        Some(format!("#123456ff {} undefined", primary()))
    );
    engine.control().set_theme_overrides("not json");
    assert_eq!(last_text(&rx), None);
}
