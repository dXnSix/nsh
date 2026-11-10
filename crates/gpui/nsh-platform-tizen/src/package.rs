//! What a TPK holds of the app, in its read-only resources (`res/`) under
//! `nsh/`, as Linux's packages hold it next to their runner: the bundle
//! (`bundle.nshc`, `capabilities.json` and `bundle.js.map` beside it), the
//! assets (`assets/`), and the app's name, id and appearance
//! (`app.json`). `nsh package --target tizen` writes it.

use std::path::{Path, PathBuf};

use nsh_platform_gpui::Appearance;

/// The directory under the package's resources.
#[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
pub(crate) const DIR: &str = "nsh";
/// The bundle in it.
#[cfg_attr(any(not(target_vendor = "tizen"), feature = "dev"), allow(dead_code))]
pub(crate) const BUNDLE: &str = "bundle.nshc";

/// The app as its package describes it.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Package {
    /// The package's `nsh` directory.
    pub(crate) dir: PathBuf,
    /// `nsh.json` `"id"`.
    pub(crate) id: Option<String>,
    /// `nsh.json` `"name"`: what the exit question names.
    pub(crate) name: Option<String>,
    /// `nsh.json` `"appearance"`.
    pub(crate) appearance: Appearance,
}

impl Package {
    /// The package in `dir`, with `app_json` its `app.json` (`None`: it has
    /// none, or it could not be read; a field that is no string is left
    /// out).
    pub(crate) fn of(dir: &Path, app_json: Option<&str>) -> Package {
        let info = app_json.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
        let field = |key: &str| {
            info.as_ref()
                .and_then(|info| info.get(key)?.as_str().map(str::to_owned))
        };
        Package {
            dir: dir.to_owned(),
            id: field("id"),
            name: field("name"),
            appearance: match field("appearance").as_deref() {
                Some("light") => Appearance::Light,
                Some("dark") => Appearance::Dark,
                _ => Appearance::System,
            },
        }
    }

    /// Reads the package in `dir`.
    #[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
    pub(crate) fn read(dir: &Path) -> Package {
        let json = std::fs::read_to_string(dir.join("app.json")).ok();
        Package::of(dir, json.as_deref())
    }

    /// The bundle's file.
    #[cfg_attr(any(not(target_vendor = "tizen"), feature = "dev"), allow(dead_code))]
    pub(crate) fn bundle(&self) -> PathBuf {
        self.dir.join(BUNDLE)
    }

    /// The assets' directory (`asset:/` URIs).
    #[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
    pub(crate) fn assets(&self) -> PathBuf {
        self.dir.join("assets")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apps_name_id_and_appearance() {
        let dir = Path::new("/opt/usr/apps/AbCdEf1234/res/nsh");
        let package = Package::of(
            dir,
            Some(r#"{ "name": "Gallery", "id": "com.example.gallery", "appearance": "dark" }"#),
        );
        assert_eq!(package.name.as_deref(), Some("Gallery"));
        assert_eq!(package.id.as_deref(), Some("com.example.gallery"));
        assert_eq!(package.appearance, Appearance::Dark);
        assert_eq!(package.bundle(), dir.join("bundle.nshc"));
        assert_eq!(package.assets(), dir.join("assets"));
        // No app.json, or a broken one: the defaults.
        for json in [
            None,
            Some("{"),
            Some(r#"{ "name": 3, "appearance": "sepia" }"#),
        ] {
            let package = Package::of(dir, json);
            assert_eq!(package.name, None, "{json:?}");
            assert_eq!(package.id, None);
            assert_eq!(package.appearance, Appearance::System);
        }
    }
}
