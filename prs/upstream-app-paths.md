# Upstream PR: core: Tell Rust code the app's id and its data and cache directories

Not opened: prepared 2026-11-09 for the owner to post. The body to paste starts below the line.

- Branch: `pr/app-paths/01-core` (local, not pushed), one commit on `upstream/main` `d96fc35c` (#1323): `57d4d988`,
  the fork's `6ffbfa55` (`feat/app-paths`) cherry-picked. One conflict, the prelude's list of functions beside
  upstream's `haptic` (#1328): upstream's list kept with the three names added, then nightly rustfmt over it.
- Size: 13 files, +393 / −45 (code +155 / −35, tests +170 / −5, docs +68 / −5).
- Open it from `dXnSix:pr/app-paths/01-core` into `longbridge/nsh:main`, title (upstream squash-merges):
  `core: Tell Rust code the app's id and its data and cache directories`.
- No compatibility case: it adds no JS export, and `tests/compat` holds what JS apps use (the body's "JS" says why
  there is no JS counterpart).
- Checked locally on `d96fc35c` (Rust 1.99.1, macOS): the new tests and the engine groups below (83 pass),
  `cargo test -p nsh-core --lib engine_context` (3), `cargo test -p nsh-engine --lib cache` (2); clippy `-D warnings`
  of `nsh-core` and `nsh-engine` (all targets), of `nsh-engine` for wasm32 without features and of
  `nsh-platform-web` for wasm32, `cargo xtask clippy-android` (both feature sets); nightly rustfmt `--check` on the
  changed files. On the fork's base (`71aeb06a`) also `cargo check -p nsh-platform-ios --target aarch64-apple-ios`
  and of the macOS crates ([feat-app-paths](feat-app-paths.md)).
- Not run: GPUI's workspace and the Windows and Linux crates (no code of theirs changes; CI builds them).

---

**Depends on:** no other PR; it applies to `main` as it is.

## Description

Rust code cannot find the app's own writable places. A service that imports, moves or deletes the files an app keeps,
or keeps more than the key-value `Storage` holds, has nowhere to put them, and code that a UI set's crate shares
between apps cannot tell which app it runs in. TSX has `process.dataDir`. The engine knows all three (the app's id, its
directory of `EngineConfig::data_dir`, its directory of the cache root) and gives Rust code none of them.

`app_id()`, `data_dir()` and `cache_dir()` (`nsh_core::prelude`) give them:

```rust
use nsh_core::prelude::*;

fn save_note(name: String, text: String) {
    let Some(dir) = data_dir() else { return };
    let saved = blocking(move || -> std::io::Result<()> {
        let notes = dir.join("notes");
        std::fs::create_dir_all(&notes)?;
        std::fs::write(notes.join(name), text)
    });
    spawn(async move {
        if let Ok(Err(error)) = saved.await {
            log::warn!("note not saved: {error}");
        }
    });
}
```

- **The data directory** is the one `process.dataDir` names and `${dataDir}` grants, where the app's `Storage` is
  too: `<EngineConfig::data_dir>/<app id>` (`default` without a valid id). The engine's one helper now builds it for
  JS's `process.dataDir`, the file storage and Rust alike.
- **The cache directory** is the app's in the cache root, `<root>/<app id>`, which the image cache (`images`) is a
  directory of; the system may clear it.
- **The id** is `nsh.json`'s `"id"` as it travels with the app (`app.json`, the dev server's bundle, a package,
  `Root::app_id`).

They live in the engine's context: `EngineContext::with_app_id`, `with_data_dir` and `with_cache_dir` hold them under
types only `engine_context.rs` names, and the engine adds them to `EngineConfig::context` as it enters it, on its
thread and in `LocalEngine::start`, replacing what a host set. So components, module functions in the JS call, ops
and tasks read them alike, as they read the host's context. The environment's signals would not do: the environment is
`Copy` and reactive, these are paths that never change, and the reactive graph is not entered while JS calls a
module function synchronously. Off the engine's thread they are `None`, as `engine_context()` is.

Every host already gives the engine its data directory and cache location, so no platform changes: the desktops and
GPUI (`AppOptions::data_dir`, else `nsh_engine::data_dir()`; `AppOptions::cache`), iOS (`nsh` in Application Support
and in Caches), Android (`nsh` in the files and cache directories), the terminal. In WebAssembly both directories are
`None`: Rust code has no files there, and the web's (`/app-data`, JS's `fs`) are the browser's, through
`EngineConfig::filesystem`.

**JS.** The data directory is `process.dataDir` already. There is no JS app id or cache directory: a TSX app's
storage and files are scoped to it by the runtime (`Storage`, `${dataDir}`), so its code needs no id to find them, and
a cache directory TSX could use needs a `${cacheDir}` root in the capabilities first, a change to the sandbox of its
own. Either can follow when an app needs it.

Docs: "The app's own files" in `website/docs/rust.md`, the `fs/promises` and `process` rows of "From Rust" in
`website/modules/standard-modules.md` (each with its zh-CN page), `docs/dev/components.md` and
`docs/dev/embedding.md`.

## Public API

- `nsh-core` (`nsh_core::element`, also in `nsh_core::prelude`):
  - `app_id() -> Option<String>`: the id of the app the engine runs; `None` for an app without one.
  - `data_dir() -> Option<PathBuf>`: the app's own data directory, the one `process.dataDir` names; `None` where the
    host keeps no files of the app's, and in WebAssembly.
  - `cache_dir() -> Option<PathBuf>`: the app's own cache directory; `None` where the host caches nothing on disk
    (`CacheLocation::Off`), and in WebAssembly.
- `nsh-core` (`nsh_core::element::EngineContext`):
  - `with_app_id(self, id: Option<String>) -> Self`, `with_data_dir(self, dir: Option<PathBuf>) -> Self`,
    `with_cache_dir(self, dir: Option<PathBuf>) -> Self`: what the three read. The engine sets them from its config;
    a test of Rust code that runs without an engine enters a context with them.

`nsh_engine::data_dir()` (the system's directory for apps' data) keeps its name and meaning; code that glob-imports
both `nsh_engine::*` and the prelude and calls `data_dir()` names the one it means.

## How to Test

- `cargo test -p nsh-engine --test engine rust_platform::components_read_the_apps_id_and_its_own_directories`: a
  Rust app shows its id and directories in `Engine` and `LocalEngine` alike (`<data>/com.example.notes`,
  `<caches>/com.example.notes`); an id that names no directory (`a/b`) keeps its id and lives in `default`; no data
  directory and no caches give `None`s.
- `cargo test -p nsh-engine --test engine rust_classes::a_ui_sets_code_reads_the_id_and_the_directories_of_the_app_it_runs_in`:
  a TSX app's UI set reads them in a synchronous call and in a task, and its data directory is the app's
  `process.dataDir`.
- `cargo test -p nsh-core --lib engine_context`: the engine's values replace a host's, and none are read off the
  engine's thread.
- `cargo test -p nsh-engine --lib cache` and the engine's `system`, `storage`, `process`, `fs`, `rust_classes`,
  `rust_platform`, `rust_app`, `images`, `host_modules`, `clock` and `parity` groups, which cover `process.dataDir`,
  the storage and the image cache.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Android and wasm32 runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped: it adds Rust API only, no JS export, so it has no
  case in `tests/compat/`.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no platform code changes; the engine's tests run
  both engines.
