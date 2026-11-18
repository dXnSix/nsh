//! The embedding app's theme overrides as a theme patch's text
//! (`EngineControl::set_theme_overrides`), what the SDKs send: the host
//! layer over the app's document, in the first frame when sent before the
//! start, followed when sent later, and cleared with `set_tokens(None)`.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{ApproxTextEngine, MountBatch, MountItem, Size, prelude::*};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

/// A TSX app whose theme (`nsh.json`) is a document of its own, with
/// `brand`, `primary` and `ring`, showing `brand`, `primary` and `edge`
/// (which it lacks).
fn start(name: &str, overrides: Option<&str>) -> (Engine, Receiver<MountBatch>) {
    let dir =
        crate::temporary_root().join(format!("nsh-theme-overrides-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("nsh.json"),
        r##"{ "theme": { "tokens": {
            "brand": { "kind": "color", "value": "#123456" },
            "primary": { "kind": "color", "value": "#222222" },
            "ring": { "kind": "color", "value": "#333333" }
        } } }"##,
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
        login_item: None,
    };
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        |control| {
            if overrides.is_some() {
                control.set_theme_overrides(overrides);
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

#[test]
fn overrides_sent_before_the_start_are_in_the_first_render() {
    let (_engine, rx) = start("first", Some(r##"{ "values": { "brand": "#abcdef" } }"##));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts(&first), vec!["#abcdef #222222 undefined"]);
}

/// A reference they hold resolves over every layer, theirs included: one
/// naming a token they set reads their value, one naming a token they
/// leave out the app's. A name the app's document lacks (`edge`) is left
/// out. Changed later they are followed, each replacing the last whole, and
/// `set_tokens(None)` gives the app its own back.
#[test]
fn overrides_follow_their_changes_and_resolve_their_references() {
    let (engine, rx) = start("later", None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 undefined"));
    engine.control().set_theme_overrides(Some(
        r##"{ "values": { "brand": "{primary}", "primary": "#00ff00" } }"##,
    ));
    assert_eq!(last_text(&rx).as_deref(), Some("#00ff00 #00ff00 undefined"));
    engine.control().set_theme_overrides(Some(
        r##"{ "values": { "primary": "{ring}", "edge": "#ffffff" } }"##,
    ));
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #333333 undefined"));
    engine.control().set_tokens(None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 undefined"));
}

/// Text that is no JSON changes nothing.
#[test]
fn overrides_that_are_no_json_change_nothing() {
    let (engine, rx) = start("bad", None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 undefined"));
    let said = engine.control().set_theme_overrides(Some("not json"));
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].code, "not-a-document");
    assert_eq!(last_text(&rx), None);
}
