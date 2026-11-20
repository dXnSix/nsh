//! The files the UI sets an engine serves ship ([`Registry::assets`]),
//! which the engine puts in its context when it starts ([`ServedAssets`]):
//! a library lists the files another ships (an asset catalog showing a
//! set's icons) by their URIs, `asset:/@<package>/<path>`, without the app
//! naming its package.

use std::sync::Arc;

use super::{AssetFile, Registry, engine_context};

/// The files the UI sets an engine serves ship, each URI once, sorted by
/// URI: the engine's context holds them while it runs
/// ([`served_asset_files`]).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ServedAssets {
    files: Arc<[&'static AssetFile]>,
}

impl ServedAssets {
    /// The files `sets` ship ([`Registry::asset_files`]); a set served
    /// twice lists its files once. Their submodules are sets of their own
    /// here: give [`Registry::sets`] of each.
    pub fn of<'a>(sets: impl IntoIterator<Item = &'a Registry>) -> Self {
        let mut files: Vec<&'static AssetFile> =
            sets.into_iter().flat_map(Registry::asset_files).collect();
        files.sort_by_key(|file| file.uri);
        files.dedup_by_key(|file| file.uri);
        Self {
            files: files.into(),
        }
    }

    /// The files, sorted by URI.
    pub fn files(&self) -> &[&'static AssetFile] {
        &self.files
    }
}

/// The files the UI sets of the engine running this code ship, sorted by
/// URI (`asset:/@<package>/<path>`): what a library lists of another's
/// files, by the URI's start, without the app naming it
/// (`uri.starts_with("asset:/@acme-ui/icons/")`). Empty off an engine's
/// thread, and in an engine whose host gave its context none
/// ([`ServedAssets`]).
pub fn served_asset_files() -> Arc<[&'static AssetFile]> {
    engine_context::<ServedAssets>().map_or_else(|| Arc::from([]), |served| served.files.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{AssetKind, EngineContext};

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

    static FONTS: [AssetFile; 1] = [AssetFile {
        uri: "asset:/@acme-ui/fonts/Inter.ttf",
        source: "/acme-ui/assets/fonts/Inter.ttf",
        kind: AssetKind::Font,
    }];

    #[test]
    fn served_assets_list_every_sets_files_once_by_uri() {
        let mut ui = Registry::ui_set("acme/ui");
        ui.assets(&ICONS);
        let mut fonts = Registry::ui_set("acme/ui/fonts");
        fonts.assets(&FONTS);
        ui.submodule(fonts);
        // Served twice (two crates of the app serve it).
        let sets = [ui.clone(), ui];
        let served = ServedAssets::of(sets.iter().flat_map(Registry::sets));
        let uris: Vec<&str> = served.files().iter().map(|f| f.uri).collect();
        assert_eq!(
            uris,
            [
                "asset:/@acme-ui/fonts/Inter.ttf",
                "asset:/@acme-ui/icons/bell.svg",
                "asset:/@acme-ui/icons/star.svg",
            ]
        );
        // Read from the engine's context, on its thread only.
        assert!(served_asset_files().is_empty());
        let _context = EngineContext::default().with(served).enter();
        let icons: Vec<&str> = served_asset_files()
            .iter()
            .map(|f| f.uri)
            .filter(|uri| uri.starts_with("asset:/@acme-ui/icons/"))
            .collect();
        assert_eq!(
            icons,
            [
                "asset:/@acme-ui/icons/bell.svg",
                "asset:/@acme-ui/icons/star.svg"
            ]
        );
    }
}
