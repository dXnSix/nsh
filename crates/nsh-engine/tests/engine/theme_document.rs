//! End to end: a design system's UI set and the app's each register a theme
//! document. Typed keys read their tokens; the app's theme file and
//! `update_tokens` change them; `nsh-base`'s `theme()` draws through the
//! roles the documents map (`popover` is the design system's
//! `surface-overlay`); an embedding app's override by a role's name recolors
//! the token it aliases.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, Color, MountBatch, MountItem, Size,
    element::{AdaptiveColor, Registry, Root, TokenKey, Tokens},
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

const DS: &str = r##"{
    "cssVars": { "theme": { "surface-overlay": "#111111", "action": "#222222" } },
    "tokens": { "surface-overlay": { "kind": "color" }, "action": { "kind": "color" } },
    "roles": { "popover": "surface-overlay", "primary": "action" }
}"##;

const ODDS: &str = r##"{
    "cssVars": { "theme": { "odds-up": "#00aa00" } },
    "tokens": { "odds-up": { "kind": "color" } }
}"##;

const OVERLAY: TokenKey<AdaptiveColor> = TokenKey::new("surface-overlay");
const ACTION: TokenKey<AdaptiveColor> = TokenKey::new("action");
const UP: TokenKey<AdaptiveColor> = TokenKey::new("odds-up");

fn hex(c: AdaptiveColor) -> String {
    format!("{:06x}", Color::from(c.light).0 >> 8)
}

/// Shows each document's tokens by their keys and `nsh-base`'s theme, then
/// changes the design system's `action` with `update_tokens`.
#[component]
fn Shows() -> impl IntoElement {
    set_timeout(Duration::from_millis(100), || {
        update_tokens(Tokens::default().with(ACTION, AdaptiveColor::rgb(0x0000ff, 0x0000ff)))
    });
    text(move || {
        let base = nsh_base::theme();
        format!(
            "overlay {} action {} up {} popover {} primary {}",
            hex(OVERLAY.get().unwrap()),
            hex(ACTION.get().unwrap()),
            hex(UP.get().unwrap()),
            hex(base.popover),
            hex(base.primary)
        )
    })
}

fn start(host: Option<&str>) -> (Engine, Receiver<MountBatch>) {
    let mut ds = Registry::ui_set("ds/native");
    ds.theme_document(DS);
    let mut app = Registry::ui_set("app/native");
    app.theme_document(ODDS);
    let root = Root::new::<Shows>(|| ())
        .theme(r##"{ "cssVars": { "light": { "odds-up": "#00ff00", "surface-overlay": "#333333", "primary": "#444444" } } }"##);
    let (tx, rx) = crossbeam_channel::unbounded();
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
        ui_sets: vec![nsh_base::registry(), ds, app],
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
        login_item: None,
    };
    let host = host.map(str::to_owned);
    let engine = Engine::start_with(
        config,
        || ApproxTextEngine,
        ChannelSink(tx),
        move |control| {
            if let Some(json) = host {
                control.set_theme_overrides(json);
            }
        },
    )
    .unwrap();
    (engine, rx)
}

/// The texts the batches within a moment show, in order.
fn texts(rx: &Receiver<MountBatch>) -> Vec<String> {
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(400)) {
        for item in batch.items {
            if let MountItem::Text { text, .. } = item {
                seen.push(text.text.to_string());
            }
        }
    }
    seen
}

#[test]
fn two_sets_documents_read_by_keys_and_base_draws_through_roles() {
    let (_engine, rx) = start(None);
    let seen = texts(&rx);
    assert_eq!(
        seen,
        [
            // The file over the documents' defaults; `primary` is a role
            // here, so it sets the design system's `action`.
            "overlay 333333 action 444444 up 00ff00 popover 333333 primary 444444",
            // `update_tokens` changes the token the key sets.
            "overlay 333333 action 0000ff up 00ff00 popover 333333 primary 0000ff",
        ]
    );
}

#[test]
fn an_override_by_a_roles_name_recolors_the_token_it_aliases() {
    let (_engine, rx) = start(Some(
        r##"{ "cssVars": { "light": { "popover": "#abcdef", "brand": "#000000" } } }"##,
    ));
    let seen = texts(&rx);
    assert_eq!(
        seen.first().map(String::as_str),
        Some("overlay abcdef action 444444 up 00ff00 popover abcdef primary 444444"),
        "{seen:?}"
    );
}
