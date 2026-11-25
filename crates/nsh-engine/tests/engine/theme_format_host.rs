//! The embedding app's theme on a native host, in the theme format
//! (`docs/dev/theme-format.md`, sections 8 and 9): the host layer, one
//! patch above the app's own overrides, which
//! `EngineControl::set_theme_overrides` (a patch's text) and
//! `EngineControl::set_tokens` (typed values) each replace whole and either
//! clears with `None`; and the host's document, which
//! `EngineControl::set_theme` activates as the app's.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{ApproxTextEngine, Color, Hsla, MountBatch, MountItem, Size, prelude::*};
use nsh_engine::{App, Engine, EngineConfig, EngineControl, Environment, Patch};

use crate::ChannelSink;

/// The app's theme (`nsh.json`): a document of its own, with a dark
/// context, and overrides of its own (`"themeOverrides"`).
const CONFIG: &str = r##"{
    "theme": {
        "tokens": {
            "brand": { "kind": "color", "value": "#123456" },
            "ink": { "kind": "color", "value": "#222222" },
            "edge": { "kind": "color", "value": "#333333" }
        },
        "modifiers": { "colorMode": { "default": "light", "contexts": {
            "light": {}, "dark": { "brand": "#654321", "ink": "#dddddd" } } } },
        "resolutionOrder": ["colorMode"]
    },
    "themeOverrides": { "values": { "edge": "#444444" } }
}"##;

/// Shows the three tokens, `brand ink edge`.
const SHOW: &str = "import { mount, Text, token } from \"nsh\";\n\
    mount(() => <Text>{`${token(\"brand\")} ${token(\"ink\")} ${token(\"edge\")}`}</Text>);\n";

/// Starts TSX app `source` under `nsh.json` [`CONFIG`], `setup` sending the
/// embedding app's input before the start.
fn start(
    name: &str,
    source: &str,
    setup: impl FnOnce(&EngineControl),
) -> (Engine, Receiver<MountBatch>) {
    let dir = crate::temporary_root().join(format!(
        "nsh-theme-format-host-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("nsh.json"), CONFIG).unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(&entry, source).unwrap();
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
        audio: None,
        cache: Default::default(),
        windows: None,
        login_item: None,
    };
    let engine = Engine::start_with(config, || ApproxTextEngine, ChannelSink(tx), setup).unwrap();
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

/// The codes of `diagnostics`.
fn codes(diagnostics: &[nsh_engine::Diagnostic]) -> Vec<&'static str> {
    diagnostics.iter().map(|d| d.code).collect()
}

/// A patch of `brand`'s color, typed.
fn brand(rgb: u32) -> Patch {
    Patch::new().value("brand", Hsla::from(Color((rgb << 8) | 0xff)))
}

/// Sent before the start, the host's patch is in the first render, over
/// the app's document and over its own overrides (`edge`), with no
/// diagnostics; what it leaves out stays the app's.
#[test]
fn set_theme_overrides_puts_its_patch_above_the_apps_overrides_from_the_first_render() {
    let (_engine, rx) = start("first", SHOW, |control| {
        let said = control.set_theme_overrides(Some(
            r##"{ "values": { "brand": "#abcdef", "edge": "#555555" } }"##,
        ));
        assert!(said.is_empty(), "{said:?}");
    });
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts(&first), vec!["#abcdef #222222 #555555".to_owned()]);
}

/// Sent while the app runs, the patch is followed, its references resolved
/// over the layers below it; each call replaces the whole layer; `None`
/// gives the app its own theme back, its overrides included.
#[test]
fn set_theme_overrides_replaces_the_layer_and_none_clears_it() {
    let (engine, rx) = start("later", SHOW, |_| {});
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #444444"));
    let control = engine.control();
    control.set_theme_overrides(Some(
        r##"{ "values": { "brand": "{ink}", "ink": "#00ff00" } }"##,
    ));
    assert_eq!(last_text(&rx).as_deref(), Some("#00ff00 #00ff00 #444444"));
    // A second patch replaces the first whole: `brand` and `ink` go back.
    control.set_theme_overrides(Some(r##"{ "values": { "edge": "#ff0000" } }"##));
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #ff0000"));
    control.set_theme_overrides(None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #444444"));
}

/// `set_theme_overrides` returns what reading the patch said: text that is
/// no patch is refused whole and changes nothing; a patch with a bad entry
/// is taken without it. A value for a name the app's document does not
/// declare is left out (`undeclared-token`, logged by the core).
#[test]
fn set_theme_overrides_returns_its_diagnostics_and_text_that_is_no_patch_changes_nothing() {
    let (engine, rx) = start("diagnostics", SHOW, |_| {});
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #444444"));
    let control = engine.control();
    control.set_theme_overrides(Some(r##"{ "values": { "brand": "#abcdef" } }"##));
    assert_eq!(last_text(&rx).as_deref(), Some("#abcdef #222222 #444444"));
    for text in ["not json", "[]", r##""#ff0000""##] {
        assert_eq!(
            codes(&control.set_theme_overrides(Some(text))),
            ["not-a-document"]
        );
    }
    assert_eq!(last_text(&rx), None);
    let said = control.set_theme_overrides(Some(
        r##"{ "values": { "ink": "#00ff00", "nothing": "#ffffff" }, "roles": { "primary": 5 }, "extra": 1 }"##,
    ));
    assert_eq!(codes(&said), ["bad-value", "unknown-member"]);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #00ff00 #444444"));
}

/// `set_tokens` puts typed values in the same layer: `set_theme_overrides`
/// replaces what it set and the other way round, in either order, and
/// `None` from either clears the layer, whichever set it.
#[test]
fn set_tokens_and_set_theme_overrides_share_one_layer_in_either_order() {
    let (engine, rx) = start("one-layer", SHOW, |_| {});
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #444444"));
    let control = engine.control();
    control.set_tokens(Some(brand(0xabcdef)));
    assert_eq!(last_text(&rx).as_deref(), Some("#abcdef #222222 #444444"));
    control.set_theme_overrides(Some(r##"{ "values": { "ink": "#00ff00" } }"##));
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #00ff00 #444444"));
    control.set_tokens(None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #444444"));
    control.set_theme_overrides(Some(r##"{ "values": { "ink": "#00ff00" } }"##));
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #00ff00 #444444"));
    control.set_tokens(Some(brand(0xabcdef)));
    assert_eq!(last_text(&rx).as_deref(), Some("#abcdef #222222 #444444"));
    control.set_theme_overrides(None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #222222 #444444"));
}

/// Steps 100 ms apart, each shown with the tokens in the frame after it:
/// the app picks its dark context, then updates its own overrides.
const APP_WRITES: &str = r##"import { mount, setModifier, signal, Text, token, updateTheme } from "nsh";

const status = signal("start");
const steps = [
  () => {
    setModifier("colorMode", "dark");
    return "dark";
  },
  () => {
    updateTheme({ values: { brand: "#0000ff", ink: "#0000ff" } });
    return "updated";
  },
];
steps.forEach((step, i) => setTimeout(() => status.set(step()), 100 * (i + 1)));
mount(() => <Text>{`${status()} ${token("brand")} ${token("ink")}`}</Text>);
"##;

/// The host's override holds over the app's writes: a context the app
/// picks (`setModifier`) and its own updates (`updateTheme`) change what
/// the host leaves out, never what it sets.
#[test]
fn a_host_override_survives_the_apps_set_modifier_and_update_theme() {
    let (_engine, rx) = start("app-writes", APP_WRITES, |control| {
        control.set_theme_overrides(Some(r##"{ "values": { "brand": "#abcdef" } }"##));
    });
    let batches =
        crate::components::until(&rx, |b| texts(b).iter().any(|t| t.starts_with("updated")));
    let shown: Vec<String> = batches.iter().flat_map(texts).collect();
    assert_eq!(
        shown,
        [
            "start #abcdef #222222",
            "dark #abcdef #dddddd",
            "updated #abcdef #0000ff",
        ]
    );
}

/// `set_theme` activates the host's document as the app's, over its
/// built-in one (`nsh.json`'s), with the host layer still above it;
/// `None` gives the built-in one back. A text refused whole (an old theme,
/// no document) returns why and changes nothing.
#[test]
fn set_theme_activates_the_hosts_document_and_none_gives_the_apps_back() {
    let host = r##"{ "tokens": {
        "brand": { "kind": "color", "value": "#0a0a0a" },
        "ink": { "kind": "color", "value": "#0b0b0b" }
    } }"##;
    let (engine, rx) = start("document", SHOW, |control| {
        assert!(control.set_theme(Some(host)).is_empty());
        control.set_theme_overrides(Some(r##"{ "values": { "ink": "#00ff00" } }"##));
    });
    // The host's document has no `edge`: the app's override of it is left
    // out.
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(texts(&first), vec!["#0a0a0a #00ff00 undefined".to_owned()]);
    let control = engine.control();
    let legacy = r##"{ "cssVars": { "light": {} } }"##; // the old format, for nsh theme convert
    for (text, code) in [("{", "not-a-document"), (legacy, "legacy-format")] {
        assert_eq!(codes(&control.set_theme(Some(text))), [code]);
    }
    assert_eq!(last_text(&rx), None);
    control.set_theme(None);
    assert_eq!(last_text(&rx).as_deref(), Some("#123456 #00ff00 #444444"));
}
