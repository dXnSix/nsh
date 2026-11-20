//! The files the UI sets an engine serves ship, which a library lists by
//! URI while the engine runs it (`nsh_core::element::served_asset_files`),
//! without the app naming the package that ships them.

use nsh_core::{
    ApproxTextEngine, Size,
    element::{AssetFile, AssetKind, Registry, Root, served_asset_files},
    prelude::*,
};
use nsh_engine::{App, Engine, EngineConfig};

use crate::{ChannelSink, rust_classes::shown};

static ICONS: [AssetFile; 2] = [
    AssetFile {
        uri: "asset:/@acme-ui/icons/star.svg",
        source: "/acme-ui/assets/icons/star.svg",
        kind: AssetKind::Image,
    },
    AssetFile {
        uri: "asset:/@acme-ui/icons/bell.svg",
        source: "/acme-ui/assets/icons/bell.svg",
        kind: AssetKind::Image,
    },
];

static CLIPS: [AssetFile; 1] = [AssetFile {
    uri: "asset:/@acme-ui/media/intro.mp4",
    source: "/acme-ui/assets/media/intro.mp4",
    kind: AssetKind::File,
}];

/// A catalog of the icons `@acme-ui` ships, which the app does not name,
/// of all the files the sets ship.
#[component]
fn Catalog() -> impl IntoElement {
    let files = served_asset_files();
    let icons: Vec<&str> = files
        .iter()
        .map(|file| file.uri)
        .filter(|uri| uri.starts_with("asset:/@acme-ui/icons/"))
        .collect();
    let label = format!("icons {} of {}", icons.join(" "), files.len());
    view().child(text(move || label.clone()))
}

#[test]
fn served_assets_are_listed_while_the_engine_runs() {
    let mut ui = Registry::ui_set("acme/ui");
    ui.assets(&ICONS);
    let mut media = Registry::ui_set("acme/ui/media");
    media.assets(&CLIPS);
    ui.submodule(media);
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut config = EngineConfig::new(App::Rust(Root::of::<Catalog>()));
    config.viewport = Size::new(px(390.0), px(844.0));
    config.ui_sets = vec![ui];
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    assert_eq!(
        shown(&rx, "icons"),
        "icons asset:/@acme-ui/icons/bell.svg asset:/@acme-ui/icons/star.svg of 3"
    );
    drop(engine);
}
