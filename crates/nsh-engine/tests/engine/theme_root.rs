//! A Rust app's theme document on its root (`Root::theme`): the engine
//! activates it before the first frame, in place of the UI set's; the
//! root's overrides (`Root::theme_overrides`) are a patch over it, and
//! `reset_tokens` goes back to the document.

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

/// A square of the theme's primary color.
#[component]
fn Swatch() -> impl IntoElement {
    view().size_10().bg(move || theme().primary)
}

/// A square of the theme's primary color that changes it, then puts it
/// back.
#[component]
fn Resets() -> impl IntoElement {
    set_timeout(Duration::from_millis(100), || {
        update_tokens(Patch::new().value("primary", Hsla::from(rgb(0xff0000))));
    });
    set_timeout(Duration::from_millis(200), || reset_tokens(["primary"]));
    view().size_10().bg(move || theme().primary)
}

fn start(root: Root) -> (Engine, Receiver<MountBatch>) {
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
        ui_sets: vec![nsh_base::registry()],
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
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    (engine, rx)
}

/// The views' backgrounds in a batch.
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

const BLUE: &str = r##"{ "tokens": { "primary": { "kind": "color", "value": "#0000ff" } } }"##;

#[test]
fn a_rust_apps_theme_file_is_in_the_first_frame() {
    let (_engine, rx) = start(Root::new::<Swatch>(|| ()).theme(BLUE));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), vec![Color(0x0000ffff)]);
}

/// A square of the role `card`.
#[component]
fn Card() -> impl IntoElement {
    view().size_10().bg(move || theme().card)
}

/// The root's overrides are a patch over its document: they win for the
/// tokens it declares (`primary`), and are left out for the ones it lacks
/// (`card`, which `nsh-base`'s fallback document has, but the app's
/// document replaces it). Without a document of the app's they are over
/// the UI set's.
#[test]
fn the_roots_overrides_are_a_patch_over_its_document() {
    let tokens = Patch::new()
        .value("primary", Hsla::from(rgb(0x00ff00)))
        .value("card", Hsla::from(rgb(0xff00ff)));
    let tokens = nsh_theme::write_patch(&tokens);
    let (_engine, rx) = start(
        Root::new::<Swatch>(|| ())
            .theme_overrides(tokens.clone())
            .theme(BLUE),
    );
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), vec![Color(0x00ff00ff)]);
    let (_engine, rx) = start(
        Root::new::<Card>(|| ())
            .theme_overrides(tokens.clone())
            .theme(BLUE),
    );
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), Vec::<Color>::new());
    let (_engine, rx) = start(Root::new::<Card>(|| ()).theme_overrides(tokens));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), vec![Color(0xff00ffff)]);
}

/// A reset goes back to the document's value.
#[test]
fn a_reset_goes_back_to_the_theme_file() {
    let (_engine, rx) = start(Root::new::<Resets>(|| ()).theme(BLUE));
    let mut seen = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(500)) {
        seen.extend(backgrounds(&batch));
    }
    seen.dedup();
    assert_eq!(
        seen,
        [Color(0x0000ffff), Color(0xff0000ff), Color(0x0000ffff)]
    );
}
