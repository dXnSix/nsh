//! The names development bundles give modules (their hot keys: source
//! stamps, component instances, kept state, overrides keyed by where an
//! element was written), the same on every machine and system: a module in
//! the app is named by its path there (`src/Card.tsx`); one outside it, a
//! library's, by its package and its path in it (`@acme-ui/src/Card.tsx`),
//! as the files a package ships are (`asset:/@<package>/<path>`), with no
//! version, so an upgrade keeps the names of the files it leaves alone. Its
//! package is the nearest directory above it with a `Cargo.toml` that has a
//! `[package]` (its `name`), else a `package.json` with a `name` (a scoped
//! one keeps its own `@`: `@scope/ui/src/A.tsx`). A module in no package
//! keeps its absolute path.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// What [`ModuleNames::name`] found above a directory: the package it is
/// in (its name, its directory), or none.
type Found = Option<(String, PathBuf)>;

/// Names modules for one build, each directory's package read once.
#[derive(Debug, Default)]
pub(crate) struct ModuleNames {
    packages: Mutex<HashMap<PathBuf, Found>>,
}

impl ModuleNames {
    /// The name of the module at `path` (absolute) in a build whose modules
    /// are named from `root`: its path there, else `@<package>/<path in its
    /// package>`, else `path`; `/`-separated on every system.
    pub(crate) fn name(&self, path: &Path, root: &Path) -> String {
        if let Ok(rel) = path.strip_prefix(root) {
            return slashed(rel);
        }
        let dir = path.parent().filter(|_| path.is_absolute());
        match dir.and_then(|dir| self.package(dir)) {
            Some((package, dir)) => package_name(&package, path.strip_prefix(&dir).unwrap_or(path)),
            None => slashed(path),
        }
    }

    /// The package `dir` is in: its own, else its parent's.
    fn package(&self, dir: &Path) -> Found {
        let mut packages = self.packages.lock().expect("packages lock");
        let mut visited = Vec::new();
        let mut found = None;
        for dir in dir.ancestors() {
            if let Some(known) = packages.get(dir) {
                found = known.clone();
                break;
            }
            visited.push(dir.to_path_buf());
            if let Some(name) = package_in(dir) {
                found = Some((name, dir.to_path_buf()));
                break;
            }
        }
        for dir in visited {
            packages.insert(dir, found.clone());
        }
        found
    }
}

/// `rel` with `/` separators, as module names are written on every system.
fn slashed(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// `@<package>/<rel>`; a scoped npm name (`@scope/ui`) keeps its own `@`.
fn package_name(package: &str, rel: &Path) -> String {
    let at = if package.starts_with('@') { "" } else { "@" };
    format!("{at}{package}/{}", slashed(rel))
}

/// The name of the package whose directory is `dir`: its `Cargo.toml`'s
/// `[package]` name, else its `package.json`'s `name`.
fn package_in(dir: &Path) -> Option<String> {
    if let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml"))
        && let Some(name) = cargo_package(&manifest)
    {
        return Some(name);
    }
    let json = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&json).ok()?;
    json.get("name")?
        .as_str()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

/// The `name` of the `[package]` of the Cargo manifest `text` (a workspace's
/// root manifest without one has none).
fn cargo_package(text: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line.split('#').next().unwrap_or_default().trim() == "[package]";
        } else if in_package
            && let Some(value) = line.strip_prefix("name")
            && let Some(value) = value.trim_start().strip_prefix('=')
        {
            let value = value.trim_start();
            let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            let value = &value[1..];
            return Some(value[..value.find(quote)?].to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nsh-module-names-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dunce::canonicalize(&dir).unwrap()
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn outside_module_names_follow_the_package_of_a_crate() {
        let root = dir("crate");
        let app = root.join("app");
        let lib = root.join("registry/acme-ui-1.2.0");
        write(&app.join("App.tsx"), "");
        write(
            &lib.join("Cargo.toml"),
            "[package]\nname = \"acme-ui\" # the library\nversion = \"1.2.0\"\n\n[dependencies]\nname = \"not this\"\n",
        );
        write(&lib.join("src/deep/Card.preview.tsx"), "");
        let names = ModuleNames::default();
        assert_eq!(
            names.name(&lib.join("src/deep/Card.preview.tsx"), &app),
            "@acme-ui/src/deep/Card.preview.tsx"
        );
        // Inside the app: its path there, as before.
        assert_eq!(names.name(&app.join("src/Card.tsx"), &app), "src/Card.tsx");
        // Read once per directory: a manifest changed during the build
        // leaves the build's names as they were.
        write(&lib.join("Cargo.toml"), "[package]\nname = \"other\"\n");
        assert_eq!(
            names.name(&lib.join("src/deep/Menu.tsx"), &app),
            "@acme-ui/src/deep/Menu.tsx"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn outside_module_names_skip_workspaces_and_unnamed_packages() {
        let root = dir("npm");
        let app = root.join("app");
        // A workspace's manifest and a package.json without a name are no
        // package: the nearest named one above is.
        write(&root.join("repo/Cargo.toml"), "[workspace]\nmembers = []\n");
        write(&root.join("repo/package.json"), "{ \"private\": true }");
        write(
            &root.join("repo/node_modules/@scope/ui/package.json"),
            "{ \"name\": \"@scope/ui\", \"version\": \"1.0.0\" }",
        );
        write(
            &root.join("repo/node_modules/@scope/ui/src/inner/package.json"),
            "{}",
        );
        let names = ModuleNames::default();
        assert_eq!(
            names.name(
                &root.join("repo/node_modules/@scope/ui/src/inner/A.tsx"),
                &app
            ),
            "@scope/ui/src/inner/A.tsx"
        );
        write(
            &root.join("plain/package.json"),
            "{ \"name\": \"plain-ui\" }",
        );
        assert_eq!(
            names.name(&root.join("plain/B.tsx"), &app),
            "@plain-ui/B.tsx"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn outside_module_names_keep_the_path_of_a_module_in_no_package() {
        let root = dir("none");
        let app = root.join("app");
        let stage = root.join("stage/Entry.tsx");
        write(&stage, "");
        // Unless a directory above the temporary one is a package.
        if ModuleNames::default().package(&root).is_none() {
            assert_eq!(
                ModuleNames::default().name(&stage, &app),
                stage.to_string_lossy().replace('\\', "/")
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    /// A development bundle's hot keys (source stamps, component instances)
    /// name a library's module by its package, the app's by their path.
    #[test]
    fn outside_module_names_are_the_hot_keys_of_a_development_bundle() {
        let root = dir("bundle");
        write(
            &root.join("app/App.tsx"),
            "import { mount } from \"nsh\";\nimport { Card } from \"../acme-ui/src/Card\";\nfunction App() { return <Card />; }\nmount(App);\n",
        );
        write(
            &root.join("acme-ui/Cargo.toml"),
            "[package]\nname = \"acme-ui\"\n",
        );
        let card = root.join("acme-ui/src/Card.tsx");
        write(
            &card,
            "import { View } from \"nsh\";\nexport function Card() { return <View />; }\n",
        );
        let (script, _) = crate::bundle_hot(&root.join("app/App.tsx"), &[])
            .unwrap()
            .script();
        assert!(script.contains("\"@acme-ui/src/Card.tsx\""), "{script}");
        assert!(script.contains("\"App.tsx\""), "{script}");
        let absolute = format!("{:?}", slashed(&card));
        assert!(!script.contains(&absolute), "{script}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn outside_module_names_are_slash_separated_on_every_system() {
        assert_eq!(
            package_name("acme-ui", Path::new("src\\deep\\Card.tsx")),
            "@acme-ui/src/deep/Card.tsx"
        );
        assert_eq!(
            package_name("@scope/ui", Path::new("src\\A.tsx")),
            "@scope/ui/src/A.tsx"
        );
        assert_eq!(
            slashed(Path::new("C:\\Users\\me\\A.tsx")),
            "C:/Users/me/A.tsx"
        );
        assert_eq!(
            cargo_package("[package]\nname='single'\n").as_deref(),
            Some("single")
        );
        assert_eq!(cargo_package("[package]\nnamespace = \"x\"\n"), None);
    }
}
