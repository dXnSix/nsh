//! Finds an app's entry module: commands take the entry itself or the
//! app's directory (the current one by default), whose `nsh.json` names
//! the entry (`"entry": "app/App.tsx"`, relative to it), else whose
//! `App.tsx` is the entry. That name matches in any case (`app.tsx` too,
//! also on case-sensitive file systems). A directory whose `nsh.json` names
//! a Rust root component (`"root"`) is a Rust app: its entry is that
//! `nsh.json` (`rust_app::is_root`), and its `App.tsx`, if any, is used
//! only when named.
//!
//! The app's directory is the entry's, unless the command names another
//! (`nsh run --app <dir>`, `nsh dev --app <dir>`): then the entry is a
//! module kept outside the app, such as a tool's, and everything else is
//! the app's: its `nsh.json`, crates, assets, `.nsh/` and the files the
//! session watches ([`app_dir`]).

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::bail;
pub(crate) use nsh_bundler::app_dir::ENTRY;

/// The entry module `path` names: the file itself, or the directory's: the
/// one its `nsh.json` names (`"entry"`), else its [`ENTRY`].
pub(crate) fn resolve(path: &Path) -> anyhow::Result<PathBuf> {
    if !path.is_dir() {
        if path.exists() {
            return Ok(path.to_owned());
        }
        bail!("{} does not exist", path.display());
    }
    let config = path.join("nsh.json");
    if nsh_bundler::app_dir::read(&config)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|json| json.get("root").is_some())
    {
        return Ok(config);
    }
    match nsh_bundler::app_dir::config_entry(path) {
        Ok(Some(entry)) if entry.is_file() => return Ok(entry),
        Ok(Some(entry)) => bail!(
            "{}: \"entry\" names {}, which does not exist",
            config.display(),
            entry.display()
        ),
        Ok(None) => {}
        Err(e) => bail!("{}: {e}", config.display()),
    }
    let entry = std::fs::read_dir(path)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(ENTRY))
        });
    match entry {
        Some(entry) => Ok(entry),
        None => bail!(
            "no {ENTRY} in {}: add one, or pass the entry module",
            path.display()
        ),
    }
}

/// The app's directory `--app` named, canonical: one per command.
static APP_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Makes `dir` (`--app`) the directory of the command's app, whatever
/// directory its entry is in.
pub(crate) fn set_app_dir(dir: &Path) -> anyhow::Result<()> {
    let canonical = dunce::canonicalize(dir)
        .ok()
        .filter(|d| d.is_dir())
        .ok_or_else(|| anyhow::anyhow!("--app {}: no such directory", dir.display()))?;
    if APP_DIR.set(canonical).is_err() {
        bail!("--app was given twice");
    }
    Ok(())
}

/// The directory of the app whose entry is `entry`: the one `--app` named
/// ([`set_app_dir`]), else its `nsh.json`'s: the entry's directory (`.` for
/// a bare file name), or the one above it whose `nsh.json` names it.
pub(crate) fn app_dir(entry: &Path) -> PathBuf {
    app_dir_from(entry, APP_DIR.get().map(PathBuf::as_path))
}

/// [`app_dir`] with `given` as `--app`.
fn app_dir_from(entry: &Path, given: Option<&Path>) -> PathBuf {
    if let Some(dir) = given {
        return dir.to_path_buf();
    }
    // The directory of its `nsh.json`: beside it, or the one above that
    // names it (`"entry"`).
    nsh_bundler::app_dir::config_dir_of(entry)
}

/// The app's directory `--app` named, if it did.
pub(crate) fn app_dir_override() -> Option<&'static Path> {
    APP_DIR.get().map(PathBuf::as_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventional_entry() {
        let dir = std::env::temp_dir().join(format!("nsh-entry-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(resolve(&dir).is_err());
        std::fs::write(dir.join("Other.tsx"), "").unwrap();
        assert!(resolve(&dir).is_err());
        std::fs::write(dir.join("app.tsx"), "").unwrap();
        assert_eq!(resolve(&dir).unwrap(), dir.join("app.tsx"));
        // A file is the entry whatever its name.
        assert_eq!(
            resolve(&dir.join("Other.tsx")).unwrap(),
            dir.join("Other.tsx")
        );
        assert!(resolve(&dir.join("missing.tsx")).is_err());
        // A TSX app whose directory is its crate (`nsh init --template
        // mixed`): with no `"root"`, its `App.tsx`.
        std::fs::write(dir.join("nsh.json"), r#"{ "rust": "." }"#).unwrap();
        assert_eq!(resolve(&dir).unwrap(), dir.join("app.tsx"));
        // A Rust app: its `nsh.json`, unless the TSX entry is named.
        std::fs::write(dir.join("nsh.json"), r#"{ "rust": ".", "root": "App" }"#).unwrap();
        assert_eq!(resolve(&dir).unwrap(), dir.join("nsh.json"));
        // A BOM (a Windows editor's UTF-8) does not hide it.
        std::fs::write(
            dir.join("nsh.json"),
            "\u{feff}{ \"rust\": \".\", \"root\": \"App\" }",
        )
        .unwrap();
        assert_eq!(resolve(&dir).unwrap(), dir.join("nsh.json"));
        assert_eq!(resolve(&dir.join("app.tsx")).unwrap(), dir.join("app.tsx"));
        // The entry `nsh.json` names, relative to it.
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::write(dir.join("nsh.json"), r#"{ "entry": "app/Main.tsx" }"#).unwrap();
        assert!(resolve(&dir).is_err());
        std::fs::write(dir.join("app/Main.tsx"), "").unwrap();
        assert_eq!(resolve(&dir).unwrap(), dir.join("app/Main.tsx"));
        std::fs::write(dir.join("nsh.json"), r#"{ "entry": "../Main.tsx" }"#).unwrap();
        assert!(resolve(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_app_directory_is_the_entry_s_unless_named() {
        assert_eq!(
            app_dir_from(Path::new("/a/app/App.tsx"), None),
            Path::new("/a/app")
        );
        assert_eq!(app_dir_from(Path::new("App.tsx"), None), Path::new("."));
        // An entry kept outside its app (`--app`): the named directory.
        assert_eq!(
            app_dir_from(Path::new("/tool/Stage.tsx"), Some(Path::new("/a/app"))),
            Path::new("/a/app")
        );
    }
}
