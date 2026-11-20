//! Imports of the files the app's UI sets ship (`Registry::assets`: a
//! crate's `assets/` directory, published at `asset:/@<package>/<path>`):
//! `import logo from "asset:/@acme-ui/images/logo.png"` is checked against
//! the files the sets the hosts serve ship (their descriptions' `assets`,
//! [`crate::BundleOptions::ui_sets`]) and gives what an app's own import of
//! that kind of file gives: an image its `ImageAsset` (`{ url, size,
//! scales }`, read from the set's file and its density variants among the
//! set's files), a font its family name (the engine registers the fonts the
//! sets ship before the first render), any other file its URL; with `?url`,
//! its URL whatever it is. Nothing is shipped with the app's own assets:
//! the set ships its files.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use nsh_core::element::description::SetDescription;
use rolldown::{
    ModuleType,
    plugin::{HookLoadOutput, HookResolveIdOutput},
};

use super::{
    FONT_EXTENSIONS, IMAGE_EXTENSIONS, NshPlugin, URL_SUFFIX, density, font_family, has_density,
    has_extension, image_module, svg_asset,
};

/// What a set's file is imported by: its URL's start.
pub(crate) const PREFIX: &str = "asset:/@";

/// The id an import of a set's file resolves to: this, then its path among
/// the app's assets (`@acme-ui/images/logo.png`), then `?url` for a `?url`
/// import.
const ID: &str = "\0nsh-set-asset:";

/// The files the UI sets ship, by their path among the app's assets
/// (`@acme-ui/images/logo.png`): where each was built.
#[derive(Debug, Default)]
pub(crate) struct SetAssets {
    files: BTreeMap<String, PathBuf>,
}

impl SetAssets {
    /// The files `ui_sets` ship.
    pub(crate) fn new(ui_sets: &[SetDescription]) -> Self {
        Self {
            files: ui_sets
                .iter()
                .flat_map(|set| &set.assets)
                .map(|file| (file.path.clone(), PathBuf::from(&file.source)))
                .collect(),
        }
    }

    /// Why no set ships `path`: the closest files they do ship (by edit
    /// distance, a typo of the package's name too), or that they ship none.
    fn missing(&self, path: &str) -> String {
        let mut near: Vec<(usize, &str)> = self
            .files
            .keys()
            .map(|file| (crate::compile::edit_distance(path, file), file.as_str()))
            .collect();
        if near.is_empty() {
            return "the app's UI sets ship no files".to_owned();
        }
        near.sort();
        let near: Vec<String> = near
            .iter()
            .take(3)
            .map(|(_, file)| format!("`asset:/{file}`"))
            .collect();
        format!(
            "no UI set the app serves ships this file; the closest: {}",
            near.join(", ")
        )
    }
}

/// `path`'s variant at `scale` among a set's files (`@acme-ui/logo.png`
/// at 2: `@acme-ui/logo@2x.png`).
fn variant(path: &str, scale: f32) -> Option<String> {
    let (dir, file) = path.rsplit_once('/')?;
    let (stem, ext) = file.rsplit_once('.')?;
    Some(format!("{dir}/{stem}@{scale}x.{ext}"))
}

fn js(code: String) -> HookLoadOutput {
    HookLoadOutput {
        code: code.into(),
        module_type: Some(ModuleType::Js),
        ..Default::default()
    }
}

impl NshPlugin {
    /// The id of the import `spec` (`asset:/@<package>/<path>`, `?url` or
    /// not) of a file a UI set ships; `Err` names the closest ones when no
    /// set ships it (`from`: the importer, as messages name it).
    pub(super) fn resolve_set_asset(
        &self,
        spec: &str,
        from: &str,
    ) -> anyhow::Result<HookResolveIdOutput> {
        let (url, suffix) = match spec.strip_suffix(URL_SUFFIX) {
            Some(url) => (url, URL_SUFFIX),
            None => (spec, ""),
        };
        let path = &url["asset:/".len()..];
        if !self.set_assets.files.contains_key(path) {
            let why = self.set_assets.missing(path);
            return Err(self.refuse(format!("{from}`{spec}`: {why}")));
        }
        Ok(HookResolveIdOutput::from_id(format!("{ID}{path}{suffix}")))
    }

    /// The module of `id` when it is the import of a set's file
    /// ([`NshPlugin::resolve_set_asset`]); `None` when it is not one.
    pub(super) fn load_set_asset(&self, id: &str) -> Option<anyhow::Result<HookLoadOutput>> {
        let path = id.strip_prefix(ID)?;
        let (path, url_only) = match path.strip_suffix(URL_SUFFIX) {
            Some(path) => (path, true),
            None => (path, false),
        };
        let source = self.set_assets.files.get(path)?;
        Some(self.set_asset_module(path, source, url_only))
    }

    /// The module of the import of the set's file `path` (built at
    /// `source`): its URL when `url_only` (`?url`), else what the app's own
    /// import of that kind of file gives.
    fn set_asset_module(
        &self,
        path: &str,
        source: &Path,
        url_only: bool,
    ) -> anyhow::Result<HookLoadOutput> {
        let url = format!("asset:/{path}");
        let fail = |detail: String| self.refuse(format!("{url}\n{detail}"));
        let named = Path::new(path);
        if url_only || !has_extension(named, IMAGE_EXTENSIONS) {
            if !url_only && has_extension(named, FONT_EXTENSIONS) {
                let data = std::fs::read(source).map_err(|e| fail(e.to_string()))?;
                let family =
                    font_family(&data).map_err(|e| fail(format!("cannot read font: {e}")))?;
                return Ok(js(format!("export default {family:?};")));
            }
            return Ok(js(format!("export default {url:?};")));
        }
        if has_extension(named, &["svg"]) {
            let data = std::fs::read(source).map_err(|e| fail(e.to_string()))?;
            let (size, _) = svg_asset(path, &data).map_err(|e| fail(e.to_string()))?;
            let size = (size.width.0.to_string(), size.height.0.to_string());
            return Ok(js(image_module(&url, size, &[])));
        }
        let pixels =
            imagesize::size(source).map_err(|e| fail(format!("cannot read image: {e}")))?;
        let scale = density(named);
        let size = (pixels.width as f64 / scale, pixels.height as f64 / scale);
        // Its variants are the set's files beside it, as `assets!()` shipped
        // them.
        let scales: Vec<f32> = if has_density(named) {
            Vec::new()
        } else {
            nsh_core::IMAGE_SCALES
                .into_iter()
                .filter(|s| *s != 1.0)
                .filter(|s| {
                    variant(path, *s).is_some_and(|v| self.set_assets.files.contains_key(&v))
                })
                .collect()
        };
        Ok(js(image_module(&url, size, &scales)))
    }
}

#[cfg(test)]
mod tests {
    use nsh_core::element::description::AssetDescription;

    use super::*;
    use crate::{BundleOptions, bundle, bundle_hot};

    /// The header of a `w`×`h` PNG.
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        out.extend(w.to_be_bytes());
        out.extend(h.to_be_bytes());
        out.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        out
    }

    /// A set `acme/ui` of the package `acme-ui` shipping `files` (path in
    /// its `assets/`, contents), written to `dir`.
    fn set(dir: &Path, files: &[(&str, Vec<u8>)]) -> SetDescription {
        let mut set: SetDescription = (&nsh_core::element::Registry::ui_set("acme/ui")).into();
        for (path, data) in files {
            let source = dir.join("assets").join(path);
            std::fs::create_dir_all(source.parent().unwrap()).unwrap();
            std::fs::write(&source, data).unwrap();
            set.assets.push(AssetDescription {
                path: format!("@acme-ui/{path}"),
                source: source.to_string_lossy().into_owned(),
            });
        }
        set
    }

    /// An app importing `imports` (default import name, specifier) and
    /// showing them as text, beside the set's crate in `dir`.
    fn app(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nsh-set-assets-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app")).unwrap();
        dunce::canonicalize(&dir).unwrap()
    }

    fn write_app(dir: &Path, imports: &[(&str, &str)]) {
        let mut code = String::from("import { mount, Text } from \"nsh\";\n");
        for (name, spec) in imports {
            code.push_str(&format!("import {name} from {spec:?};\n"));
        }
        let names: Vec<&str> = imports.iter().map(|(n, _)| *n).collect();
        code.push_str(&format!(
            "mount(() => <Text>{{JSON.stringify([{}])}}</Text>);\n",
            names.join(", ")
        ));
        std::fs::write(dir.join("app/App.tsx"), code).unwrap();
    }

    fn try_bundle(
        dir: &Path,
        sets: Vec<SetDescription>,
    ) -> Result<crate::Bundle, crate::BundleError> {
        bundle(&BundleOptions {
            entry: dir.join("app/App.tsx"),
            ui_sets: sets,
            ..Default::default()
        })
    }

    #[test]
    fn set_asset_imports_give_what_the_apps_own_imports_give() {
        let dir = app("kinds");
        let font =
            include_bytes!("../../../../tests/compat/cases/layout/fonts/Pacifico-Regular.ttf");
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8"/>"#;
        let acme = set(
            &dir.join("acme-ui"),
            &[
                ("images/logo.png", png(40, 30)),
                ("images/logo@2x.png", png(80, 60)),
                ("images/logo@3x.png", png(120, 90)),
                ("images/badge@2x.png", png(20, 10)),
                ("icons/mark.svg", svg.to_vec()),
                ("fonts/Pacifico.ttf", font.to_vec()),
                ("data/cities.json", b"{\"secret\":\"in-the-file\"}".to_vec()),
            ],
        );
        write_app(
            &dir,
            &[
                ("logo", "asset:/@acme-ui/images/logo.png"),
                ("badge", "asset:/@acme-ui/images/badge@2x.png"),
                ("mark", "asset:/@acme-ui/icons/mark.svg"),
                ("pacifico", "asset:/@acme-ui/fonts/Pacifico.ttf"),
                ("cities", "asset:/@acme-ui/data/cities.json"),
                ("logoUrl", "asset:/@acme-ui/images/logo.png?url"),
            ],
        );
        let built = try_bundle(&dir, vec![acme.clone()]).unwrap();
        // As written, whatever the printer's spacing.
        let bare = |code: &str| code.split_whitespace().collect::<String>();
        let code = bare(&built.code);
        for module in [
            "url: \"asset:/@acme-ui/images/logo.png\", size: { width: 40, height: 30 }, scales: [1, 2, 3]",
            "url: \"asset:/@acme-ui/images/badge@2x.png\", size: { width: 10, height: 5 } }",
            "url: \"asset:/@acme-ui/icons/mark.svg\", size: { width: 12, height: 8 } }",
            "\"Pacifico\"",
            "\"asset:/@acme-ui/data/cities.json\"",
        ] {
            assert!(code.contains(&bare(module)), "{module}: {code}");
        }
        // A URL, not the data; and the set ships its files, not the app.
        assert!(!code.contains("in-the-file"), "{code}");
        assert!(built.assets.is_empty(), "{:?}", built.assets);
        // A hot bundle gives the same.
        let hot = bundle_hot(&dir.join("app/App.tsx"), &[acme]).unwrap();
        let (script, _) = hot.script();
        assert!(bare(&script).contains("scales:[1,2,3]"), "{script}");
        assert!(hot.assets.is_empty(), "{:?}", hot.assets);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn set_asset_imports_of_files_no_set_ships_name_the_closest() {
        let dir = app("missing");
        let acme = set(
            &dir.join("acme-ui"),
            &[
                ("images/logo.png", png(4, 4)),
                ("images/logo@2x.png", png(8, 8)),
                ("images/other.png", png(4, 4)),
                ("fonts/Inter.ttf", b"not read".to_vec()),
            ],
        );
        for (spec, closest) in [
            (
                "asset:/@acme-ui/images/logo.pgn",
                "`asset:/@acme-ui/images/logo.png`",
            ),
            (
                "asset:/@acme-uii/images/other.png",
                "`asset:/@acme-ui/images/other.png`",
            ),
        ] {
            write_app(&dir, &[("a", spec)]);
            let error = try_bundle(&dir, vec![acme.clone()]).unwrap_err().plain;
            assert!(
                error.contains(&format!(
                    "App.tsx: `{spec}`: no UI set the app serves ships this file; the closest: {closest}"
                )),
                "{error}"
            );
        }
        write_app(&dir, &[("a", "asset:/@acme-ui/images/logo.png")]);
        let error = try_bundle(&dir, Vec::new()).unwrap_err().plain;
        assert!(error.contains("the app's UI sets ship no files"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn set_asset_variants_are_the_sets_files_beside_it() {
        assert_eq!(
            variant("@acme-ui/images/logo.png", 1.5).as_deref(),
            Some("@acme-ui/images/logo@1.5x.png")
        );
        assert_eq!(
            variant("@acme-ui/logo.png", 2.0).as_deref(),
            Some("@acme-ui/logo@2x.png")
        );
    }
}
