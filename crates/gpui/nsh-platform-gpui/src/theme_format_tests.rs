//! The embedding app's theme on GPUI, in the theme format
//! (`docs/dev/theme-format.md`): `AppView::set_theme_json` activates the
//! host's document through the engine (`EngineControl::set_theme_document`),
//! `AppView::set_theme_overrides` sets the engine's host layer
//! (`EngineControl::set_tokens`), both kept for the engines the surface
//! starts ([`HostTheme`]); and the window background follows the resolved
//! `background` role ([`Surface::tokens_changed`]). The engine runs
//! headless here, on its own thread, as the surface starts it.

use std::{sync::Arc, time::Duration};

use crossbeam_channel::{Receiver, Sender};
use nsh_core::{
    ApproxTextEngine, Color, Command, MountBatch, MountItem, Size,
    element::{Root, Tokens},
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig, Environment, MountSink};

use crate::surface::{HostTheme, Surface, theme_background};

/// Hands the engine's batches to the test.
struct ChannelSink(Sender<MountBatch>);

impl MountSink for ChannelSink {
    fn submit(&self, batch: MountBatch) {
        let _ = self.0.send(batch);
    }
}

#[component]
fn Blank() -> impl IntoElement {
    view()
}

/// The app's own document: a light `background` and a `brand`.
const APP_THEME: &str = r##"{ "tokens": {
    "background": { "kind": "color", "value": "#fafafa" },
    "brand": { "kind": "color", "value": "#123456" }
} }"##;

/// The host's document: its `background` maps onto a surface token, dark
/// by default.
const HOST_THEME: &str = r##"{
    "tokens": { "surface": { "kind": "color", "value": "#eeeeee" } },
    "modifiers": { "colorMode": { "default": "dark", "contexts": {
        "light": {}, "dark": { "surface": "#102030" } } } },
    "resolutionOrder": ["colorMode"],
    "roles": { "background": "surface" }
}"##;

/// Starts a Rust app drawn with [`APP_THEME`], `host` given to the engine
/// before its first frame as the surface gives it.
fn start(host: &HostTheme) -> (Engine, Receiver<MountBatch>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(Root::new::<Blank>(|| ()).theme(APP_THEME)),
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
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        |control| host.send(control),
    )
    .unwrap();
    (engine, rx)
}

/// The tokens of the last `Command::Tokens` the next batches carry, until
/// a moment passes without one: what the surface's window background
/// follows.
fn last_tokens(rx: &Receiver<MountBatch>) -> Option<Arc<Tokens>> {
    let mut last = None;
    let mut wait = Duration::from_secs(5);
    while let Ok(batch) = rx.recv_timeout(wait) {
        wait = Duration::from_millis(500);
        for item in batch.items {
            if let MountItem::Command {
                command: Command::Tokens(tokens),
                ..
            } = item
            {
                last = Some(tokens);
            }
        }
    }
    last
}

/// `tokens`' `background` and `brand` as RGBA (0: none).
fn colors(tokens: &Tokens) -> (u32, u32) {
    let rgba = |name: &str| tokens.color(name).map_or(0, |c| Color::from(c).0);
    (theme_background(tokens).map_or(0, |c| c.0), rgba("brand"))
}

/// The host's document is active as the app's from the engine's first
/// frame, its roles resolved in its default context; `None` gives the
/// app's own document back. A text refused whole is an error and keeps
/// the document.
#[test]
fn set_theme_json_activates_the_hosts_document_through_the_engine() {
    let mut host = HostTheme::default();
    let document = host.set_document(Some(HOST_THEME)).unwrap();
    assert!(document.is_some());
    let (engine, rx) = start(&host);
    let tokens = last_tokens(&rx).expect("the engine's tokens");
    assert_eq!(colors(&tokens), (0x102030ff, 0));
    let legacy = r##"{ "cssVars": { "light": {} } }"##; // the old format, for nsh theme convert
    for refused in ["{", "[]", legacy] {
        assert!(host.set_document(Some(refused)).is_err(), "{refused}");
    }
    assert!(host.document.is_some());
    engine
        .control()
        .set_theme_document(host.set_document(None).unwrap());
    let tokens = last_tokens(&rx).expect("the engine's tokens");
    assert_eq!(colors(&tokens), (0xfafafaff, 0x123456ff));
}

/// The host's patch is the engine's host layer over the app's document,
/// from the first frame; replaced, then cleared with `None`, the app's own
/// values come back. Text that is no patch is an error and keeps it.
#[test]
fn set_theme_overrides_sets_the_host_patch_through_the_engine() {
    let mut host = HostTheme::default();
    host.set_patch(Some(
        r##"{ "values": { "background": "#abcdef", "brand": "#00ff00" } }"##,
    ))
    .unwrap();
    let (engine, rx) = start(&host);
    let tokens = last_tokens(&rx).expect("the engine's tokens");
    assert_eq!(colors(&tokens), (0xabcdefff, 0x00ff00ff));
    for refused in ["not json", "[]", "1"] {
        assert!(host.set_patch(Some(refused)).is_err(), "{refused}");
    }
    assert!(host.patch.is_some());
    let control = engine.control();
    control.set_tokens(
        host.set_patch(Some(r##"{ "values": { "brand": "#ff0000" } }"##))
            .unwrap(),
    );
    let tokens = last_tokens(&rx).expect("the engine's tokens");
    assert_eq!(colors(&tokens), (0xfafafaff, 0xff0000ff));
    control.set_tokens(host.set_patch(None).unwrap());
    let tokens = last_tokens(&rx).expect("the engine's tokens");
    assert_eq!(colors(&tokens), (0xfafafaff, 0x123456ff));
}

/// The window background is the resolved `background` role of an app with
/// a theme of its own, in the context picked; the default theme's light
/// one when the role resolves to nothing; the system's without a theme.
/// A change redraws the surface.
#[test]
fn the_window_background_follows_the_resolved_role() {
    let surface = Surface::new(Environment::default());
    let tokens = |json: &str, themed: bool| {
        let document = nsh_core::element::theme::read_document(json).0.unwrap();
        let (mut tokens, _) = Tokens::resolve(Arc::new(document), Default::default(), Vec::new());
        tokens.set_themed(themed);
        tokens
    };
    let system = surface.background();
    surface.drawing();
    surface.tokens_changed(&tokens(HOST_THEME, true));
    assert_eq!(surface.background(), Color(0x102030ff));
    assert!(surface.invalidated());
    surface.drawing();
    let unresolved = r##"{ "tokens": { "brand": { "kind": "color", "value": "#123456" } } }"##;
    surface.tokens_changed(&tokens(unresolved, true));
    assert_eq!(surface.background(), Color(0xffffffff));
    surface.drawing();
    surface.tokens_changed(&tokens(unresolved, true));
    assert!(
        !surface.invalidated(),
        "the same background redraws nothing"
    );
    surface.tokens_changed(&tokens(HOST_THEME, false));
    assert_eq!(surface.background(), system);
}
