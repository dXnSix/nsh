//! An app's crates take entropy on the web through getrandom's custom
//! backend, which the web module defines (`src/random.rs`): a crate that
//! uses getrandom 0.3 and 0.4 links with the module for wasm32, and the
//! linked module takes its bytes from the page (`env.__nsh_random_fill`).
//!
//! It builds `tests/entropy_link/` for `wasm32-unknown-unknown` under
//! `target/entropy-link/` with the flags every web build has. Without the
//! wasm32 target installed it says so and passes.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn a_crate_using_getrandom_links_with_the_web_module() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = web.ancestors().nth(2).expect("the workspace's root");
    if !wasm32_installed() {
        eprintln!("skipped: the wasm32-unknown-unknown target is not installed");
        return;
    }
    // The crate is built inside the workspace's folder, so Cargo reads its
    // `.cargo/config.toml`, and from the workspace's lockfile, so it takes
    // the versions the module is tested with.
    let dir = root.join("target/entropy-link");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let manifest = std::fs::read_to_string(web.join("tests/entropy_link/Cargo.toml.in"))
        .unwrap()
        .replace("@WEB@", web.to_str().unwrap());
    std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
    std::fs::copy(
        web.join("tests/entropy_link/src/lib.rs"),
        dir.join("src/lib.rs"),
    )
    .unwrap();
    std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(&dir)
        .args(["build", "--target", "wasm32-unknown-unknown"])
        .env("CARGO_TARGET_DIR", dir.join("target"))
        // The flags of every web build (`runtime::web_build_flags`), also
        // when the environment sets its own.
        .env_remove("RUSTFLAGS")
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            "-Clink-arg=-zstack-size=8388608\x1f--cfg\x1fgetrandom_backend=\"custom\"",
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the crate does not link with the web module:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let wasm: PathBuf = dir.join("target/wasm32-unknown-unknown/debug/entropy_link.wasm");
    let imports = imports(&std::fs::read(&wasm).unwrap());
    assert!(
        imports.contains(&("env".to_owned(), "__nsh_random_fill".to_owned())),
        "the module asks the page for its bytes: {imports:?}"
    );
    assert!(
        !imports.iter().any(|(_, name)| name.contains("getrandom")),
        "getrandom's backend is defined in the module, not imported: {imports:?}"
    );
}

/// Whether rustup (or the toolchain) has the wasm32 standard library.
fn wasm32_installed() -> bool {
    let Ok(out) = Command::new("rustc")
        .args([
            "--print",
            "target-libdir",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .output()
    else {
        return false;
    };
    let dir = String::from_utf8_lossy(&out.stdout);
    out.status.success()
        && std::fs::read_dir(dir.trim()).is_ok_and(|mut entries| {
            entries
                .any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with("libcore-")))
        })
}

/// The (module, name) pairs of a WebAssembly module's imports.
fn imports(bytes: &[u8]) -> Vec<(String, String)> {
    assert_eq!(&bytes[..8], b"\0asm\x01\0\0\0", "a WebAssembly module");
    let mut at = 8;
    while at < bytes.len() {
        let id = bytes[at];
        at += 1;
        let size = leb(bytes, &mut at) as usize;
        let end = at + size;
        if id == 2 {
            let mut found = Vec::new();
            let mut i = at;
            for _ in 0..leb(bytes, &mut i) {
                let module = name(bytes, &mut i);
                let field = name(bytes, &mut i);
                skip_import(bytes, &mut i);
                found.push((module, field));
            }
            return found;
        }
        at = end;
    }
    Vec::new()
}

fn leb(bytes: &[u8], at: &mut usize) -> u64 {
    let (mut value, mut shift) = (0u64, 0);
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn name(bytes: &[u8], at: &mut usize) -> String {
    let len = leb(bytes, at) as usize;
    let text = String::from_utf8_lossy(&bytes[*at..*at + len]).into_owned();
    *at += len;
    text
}

/// Skips an import's description: a function, table, memory, global or tag.
fn skip_import(bytes: &[u8], at: &mut usize) {
    let kind = bytes[*at];
    *at += 1;
    let limits = |at: &mut usize| {
        let flags = bytes[*at];
        *at += 1;
        leb(bytes, at);
        if flags & 1 != 0 {
            leb(bytes, at);
        }
    };
    match kind {
        0 => {
            leb(bytes, at);
        }
        1 => {
            *at += 1;
            limits(at);
        }
        2 => limits(at),
        3 => *at += 2,
        4 => {
            *at += 1;
            leb(bytes, at);
        }
        other => panic!("an import of kind {other}"),
    }
}
