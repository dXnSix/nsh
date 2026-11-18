//! The `nsh` CLI in the theme format (TOOL-BUNDLE, TOOL-TYPES): the Rust
//! app runner passes `nsh.json`'s `"theme"` to `Root::theme` and its
//! `"themeOverrides"` to `Root::theme_overrides` (a file, or an inline
//! member written to the workspace), passes no old `cssVars` theme, and
//! warns of what the members say; `nsh init` prints the theme's diagnostics
//! and types the active document (B10).

use std::path::{Path, PathBuf};

use nsh_core::element::{Registry, description::SetDescription};

use crate::rust_app::{root_config, root_fn, theme_warnings};

const DOCUMENT: &str = r##"{
    "tokens": {
        "brand": { "kind": "color", "value": "#123456", "doc": "The brand's color." },
        "corner": { "kind": "number", "value": 6 }
    }
}"##;

const OVERRIDES: &str = r##"{ "values": { "corner": 10 } }"##;

/// An app directory with `files` (`nsh.json` among them), and its
/// workspace (`.nsh/rust`), each its own.
fn app(files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "nsh-cli-theme-format-{}-{call}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let workspace = dir.join(".nsh/rust");
    std::fs::create_dir_all(&workspace).unwrap();
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    (dunce::canonicalize(&dir).unwrap(), workspace)
}

fn config(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("nsh.json")).unwrap()).unwrap()
}

/// `app_root()` of the app in `dir`, as the runners generate it.
fn runner(dir: &Path, workspace: &Path) -> String {
    let root = root_config(dir, &config(dir), workspace).unwrap();
    root_fn("", "app_native", &root)
}

/// The `include_str!` of `path`, as the runner writes it.
fn included(path: &Path) -> String {
    format!("include_str!({:?})", path.to_string_lossy())
}

#[test]
fn the_runner_passes_the_theme_and_the_overrides_files() {
    let (dir, workspace) = app(&[
        (
            "nsh.json",
            r#"{ "root": "App", "theme": "theme.json", "themeOverrides": "overrides.json" }"#,
        ),
        ("theme.json", DOCUMENT),
        ("overrides.json", OVERRIDES),
    ]);
    let code = runner(&dir, &workspace);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        code.contains(&format!(".theme({})", included(&dir.join("theme.json")))),
        "{code}"
    );
    assert!(
        code.contains(&format!(
            ".theme_overrides({})",
            included(&dir.join("overrides.json"))
        )),
        "{code}"
    );
}

#[test]
fn inline_members_are_written_to_the_workspace() {
    let config =
        format!(r#"{{ "root": "App", "theme": {DOCUMENT}, "themeOverrides": {OVERRIDES} }}"#);
    let (dir, workspace) = app(&[("nsh.json", &config)]);
    let code = runner(&dir, &workspace);
    let read = |file: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(workspace.join(file)).unwrap()).unwrap()
    };
    let (theme, overrides) = (read("theme.json"), read("theme-overrides.json"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        code.contains(&format!(
            ".theme({})",
            included(&workspace.join("theme.json"))
        )),
        "{code}"
    );
    assert!(
        code.contains(&format!(
            ".theme_overrides({})",
            included(&workspace.join("theme-overrides.json"))
        )),
        "{code}"
    );
    assert_eq!(
        theme,
        serde_json::from_str::<serde_json::Value>(DOCUMENT).unwrap()
    );
    assert_eq!(
        overrides,
        serde_json::from_str::<serde_json::Value>(OVERRIDES).unwrap()
    );
}

#[test]
fn an_old_cssvars_theme_is_not_passed_and_is_warned_of() {
    let (dir, workspace) = app(&[
        (
            "nsh.json",
            r#"{ "root": "App", "theme": "theme.json", "themeOverrides": "overrides.json" }"#,
        ),
        (
            "theme.json",
            r##"{ "cssVars": { "light": { "primary": "#e11d48" } } }"##,
        ),
        ("overrides.json", r#"{ "values": { "radius": 10 } }"#),
    ]);
    let root = root_config(&dir, &config(&dir), &workspace).unwrap();
    let code = root_fn("", "app_native", &root);
    let base: SetDescription = nsh_base::registry().into();
    let warned = theme_warnings(&root, &[base]).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    // Nothing converts it: the root has no document of the app's, and the
    // overrides layer over `nsh/base`'s fallback.
    assert!(!code.contains(".theme("), "{code}");
    assert!(code.contains(".theme_overrides("), "{code}");
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(
        warned[0].starts_with("theme.json: legacy-format: ")
            && warned[0].contains("nsh theme convert"),
        "{warned:?}"
    );
}

#[test]
fn the_runners_warnings_name_the_file_and_the_pointer() {
    let (dir, workspace) = app(&[
        (
            "nsh.json",
            r#"{ "root": "App", "theme": "theme.json", "themeOverrides": { "values": { "nope": 1 } } }"#,
        ),
        (
            "theme.json",
            r##"{ "tokens": { "brand": { "kind": "color", "value": "blue" } }, "roles": { "banner": "brand" } }"##,
        ),
    ]);
    let root = root_config(&dir, &config(&dir), &workspace).unwrap();
    let mut kit = Registry::ui_set("kit/native");
    kit.roles([nsh_core::element::Role::<nsh_core::Hsla>::new("banner").declaration("", true)]);
    let warned = theme_warnings(&root, &[SetDescription::from(&kit)]).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    for prefix in [
        "theme.json#/tokens/brand: bad-value: ",
        "nsh.json#/themeOverrides/values/nope: undeclared-token: ",
        "theme.json#/roles/banner: role-unmapped: ",
    ] {
        assert!(
            warned.iter().any(|w| w.starts_with(prefix)),
            "{prefix}\n{warned:?}"
        );
    }
}

#[test]
fn nsh_init_prints_the_themes_diagnostics() {
    let (dir, _) = app(&[
        ("nsh.json", r#"{ "theme": "theme.json" }"#),
        (
            "theme.json",
            r##"{ "tokens": {
                "brand": { "kind": "color", "value": "#123456" },
                "corner": { "kind": "number", "value": "wide" }
            } }"##,
        ),
    ]);
    let (files, warnings) = crate::init::theme_types(&dir, &[]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("theme.json#/tokens/corner: bad-value: "),
        "{warnings:?}"
    );
    // The types are the app's document's, its bad entry left out: no
    // silent fallback to another theme's names.
    let tokens = &files
        .iter()
        .find(|(path, _)| path == "nsh/tokens.d.ts")
        .unwrap()
        .1;
    assert!(tokens.contains("\"brand\": string;"), "{tokens}");
    assert!(!tokens.contains("\"corner\""), "{tokens}");
}

#[test]
fn nsh_init_prints_a_theme_it_cannot_read() {
    let (dir, _) = app(&[("nsh.json", r#"{ "theme": "missing.json" }"#)]);
    let base: SetDescription = nsh_base::registry().into();
    let (files, warnings) = crate::init::theme_types(&dir, &[base]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("nsh.json: theme: missing.json: "),
        "{warnings:?}"
    );
    assert!(
        warnings[0].ends_with(".nsh/types names the UI sets' theme meanwhile"),
        "{warnings:?}"
    );
    let names: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(
        names,
        ["nsh/tokens.d.ts", "nsh/modifiers.d.ts", "nsh/roles.d.ts"]
    );
    // The sets' document meanwhile: `nsh/base`'s fallback.
    assert!(
        files[0].1.contains("\"primary\": string;"),
        "{}",
        files[0].1
    );
    assert!(
        files[1].1.contains("\"colorMode\": \"light\" | \"dark\";"),
        "{}",
        files[1].1
    );
}
