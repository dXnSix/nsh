//! A Rust app's theme file on its root (`Root::theme`): the engine applies
//! it before the first frame, through the UI set that reads themes, over
//! the tokens `Root::tokens` gives; it is the app's theme, which
//! `reset_tokens` goes back to.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_base::theme;
use nsh_core::{ApproxTextEngine, Color, MountBatch, MountItem, Size, element::Root, prelude::*};
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
        let mut tokens = nsh_core::element::Tokens::default();
        tokens.set_color("primary", rgb(0xff0000).into());
        update_tokens(tokens);
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

const BLUE: &str = r##"{ "cssVars": { "light": { "primary": "#0000ff" } } }"##;

#[test]
fn a_rust_apps_theme_file_is_in_the_first_frame() {
    let (_engine, rx) = start(Root::new::<Swatch>(|| ()).theme(BLUE));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), vec![Color(0x0000ffff)]);
}

/// A square of the token `card`, which only the root's tokens change.
#[component]
fn Card() -> impl IntoElement {
    view().size_10().bg(move || theme().card)
}

/// The theme file replaces the root's tokens: it is read over the UI set's
/// defaults, not over them.
#[test]
fn the_theme_file_replaces_the_roots_tokens() {
    let mut tokens = nsh_base::Theme::default().into_tokens();
    tokens.set_color("primary", rgb(0x00ff00).into());
    tokens.set_color("card", rgb(0xff00ff).into());
    let (_engine, rx) = start(
        Root::new::<Swatch>(|| ())
            .tokens(tokens.clone())
            .theme(BLUE),
    );
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), vec![Color(0x0000ffff)]);
    let (_engine, rx) = start(Root::new::<Card>(|| ()).tokens(tokens.clone()).theme(BLUE));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        backgrounds(&first),
        vec![Color::from(nsh_base::Theme::default().card.light)]
    );
    // `Root::tokens` alone keeps working.
    let (_engine, rx) = start(Root::new::<Swatch>(|| ()).tokens(tokens));
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(backgrounds(&first), vec![Color(0x00ff00ff)]);
}

/// A reset goes back to the theme file's value.
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
