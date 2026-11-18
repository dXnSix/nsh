//! End to end: a design system's UI set and another set each register a
//! theme document. Documents are never merged: the first set's is active,
//! the other's unused, and the app's own document replaces both. Typed keys
//! read the active document's tokens and `update_tokens` changes them;
//! `nsh-base`'s `theme()` draws through the roles the document maps
//! (`popover` is the design system's `surface-overlay`); an embedding app's
//! overrides are a patch over the active document.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, Color, MountBatch, MountItem, Size,
    element::{AdaptiveColor, Patch, PatchValues, Registry, Root, TokenKey},
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig, Environment};

use crate::ChannelSink;

const DS: &str = r##"{
    "tokens": {
        "surface-overlay": { "kind": "color", "value": "#111111" },
        "action": { "kind": "color", "value": "#222222" } },
    "roles": { "popover": "surface-overlay", "primary": "action" }
}"##;

const ODDS: &str = r##"{
    "tokens": { "odds-up": { "kind": "color", "value": "#00aa00" } }
}"##;

/// The app's document: both sets' tokens, its own values, the design
/// system's roles.
const APP: &str = r##"{
    "tokens": {
        "surface-overlay": { "kind": "color", "value": "#333333" },
        "action": { "kind": "color", "value": "#444444" },
        "odds-up": { "kind": "color", "value": "#00ff00" } },
    "roles": { "popover": "surface-overlay", "primary": "action" }
}"##;

const OVERLAY: TokenKey<nsh_core::Hsla> = TokenKey::new("surface-overlay");
const ACTION: TokenKey<nsh_core::Hsla> = TokenKey::new("action");
const UP: TokenKey<nsh_core::Hsla> = TokenKey::new("odds-up");

fn hex(c: Option<impl Into<AdaptiveColor>>) -> String {
    c.map_or("none".to_owned(), |c| {
        format!("{:06x}", Color::from(c.into().light).0 >> 8)
    })
}

/// Shows each document's tokens by their keys and `nsh-base`'s theme, then
/// changes the design system's `action` with `update_tokens`.
#[component]
fn Shows() -> impl IntoElement {
    set_timeout(Duration::from_millis(100), || {
        update_tokens(Patch::new().key(ACTION, AdaptiveColor::rgb(0x0000ff, 0x0000ff).light))
    });
    text(move || {
        let base = nsh_base::theme();
        format!(
            "overlay {} action {} up {} popover {} primary {}",
            hex(OVERLAY.get()),
            hex(ACTION.get()),
            hex(UP.get()),
            hex(Some(base.popover)),
            hex(Some(base.primary))
        )
    })
}

fn start(app_theme: bool, host: Option<&str>) -> (Engine, Receiver<MountBatch>) {
    let mut ds = Registry::ui_set("ds/native");
    ds.theme_document(DS);
    let mut app = Registry::ui_set("app/native");
    app.theme_document(ODDS);
    let mut root = Root::new::<Shows>(|| ());
    if app_theme {
        root = root.theme(APP);
    }
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
                control.set_theme_overrides(Some(&json));
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

/// Without a document of the app's, the first set's is active and the
/// other's unused: its token reads nothing.
#[test]
fn the_first_sets_document_is_active_and_the_others_unused() {
    let (_engine, rx) = start(false, None);
    let seen = texts(&rx);
    assert_eq!(
        seen.first().map(String::as_str),
        Some("overlay 111111 action 222222 up none popover 111111 primary 222222"),
        "{seen:?}"
    );
}

#[test]
fn the_apps_document_replaces_the_sets_and_base_draws_through_its_roles() {
    let (_engine, rx) = start(true, None);
    let seen = texts(&rx);
    assert_eq!(
        seen,
        [
            // The app's document; `primary` is a role it maps onto
            // `action`.
            "overlay 333333 action 444444 up 00ff00 popover 333333 primary 444444",
            // `update_tokens` changes the token the key sets.
            "overlay 333333 action 0000ff up 00ff00 popover 333333 primary 0000ff",
        ]
    );
}

/// The embedding app's overrides are a patch over the active document: a
/// value recolors its token and the role mapped onto it; a name the
/// document lacks is left out.
#[test]
fn the_hosts_overrides_recolor_a_token_and_the_role_mapped_onto_it() {
    let (_engine, rx) = start(
        true,
        Some(r##"{ "values": { "surface-overlay": "#abcdef", "brand": "#000000" } }"##),
    );
    let seen = texts(&rx);
    assert_eq!(
        seen.first().map(String::as_str),
        Some("overlay abcdef action 444444 up 00ff00 popover abcdef primary 444444"),
        "{seen:?}"
    );
}
