//! nsh-core tells the crates that depend on it whether the dev tools' hooks
//! are compiled in (`build.rs`, `links = "nsh-core"`): a dependent's build
//! script sees `DEP_NSH_CORE_INSPECT=1` with `inspect` and nothing without.
//!
//! It checks `tests/inspect_links/` under `target/inspect-links/` twice, with
//! and without the fixture's feature that turns `nsh-core/inspect` on, and
//! reads what the fixture's build script printed.

use std::{path::Path, process::Command};

#[test]
fn a_dependent_build_script_sees_whether_inspect_is_on() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = core.ancestors().nth(2).expect("the workspace's root");
    // Built inside the workspace's folder, so Cargo reads its
    // `.cargo/config.toml`, and from the workspace's lockfile.
    let dir = root.join("target/inspect-links");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let manifest = std::fs::read_to_string(core.join("tests/inspect_links/Cargo.toml.in"))
        .unwrap()
        .replace("@CORE@", core.to_str().unwrap());
    std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
    for file in ["build.rs", "src/lib.rs"] {
        std::fs::copy(core.join("tests/inspect_links").join(file), dir.join(file)).unwrap();
    }
    std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();

    assert_eq!(seen(&dir, &["--features", "inspect"]), "1");
    assert_eq!(seen(&dir, &[]), "");
}

/// What the fixture's build script saw with `args`.
fn seen(dir: &Path, args: &[&str]) -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(dir)
        .arg("check")
        .args(args)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the fixture does not build:\n{stderr}"
    );
    let line = stderr
        .lines()
        .find_map(|line| line.split_once("DEP_NSH_CORE_INSPECT=["))
        .unwrap_or_else(|| panic!("the fixture's build script printed nothing:\n{stderr}"))
        .1;
    line.trim_end_matches(']').to_owned()
}
