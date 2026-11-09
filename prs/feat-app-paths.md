# core: Tell Rust code the app's id and its data and cache directories

> **Upstream (checked 2026-11-09):** prepared as [upstream-app-paths](upstream-app-paths.md) (`pr/app-paths/01-core`,
> local), not opened: the owner decides.

Branch: `feat/app-paths` on `github.com/dXnSix/nsh`, cut from `71aeb06a` (the newest `upstream/main` commit the fork's
`main` holds). One commit, `6ffbfa55`: `core: Tell Rust code the app's id and its data and cache directories`. Local,
not pushed yet: to open as a pull request into the fork's `main` and merge there with a merge commit. Not opened
upstream: the owner decides and posts there.

## Summary

Rust code on the engine's thread (a component, a UI set's module function, a service either starts) could not find
the app's own writable places: not its data directory, not its cache directory, not even its id. RayX needs them for
its asset service's writable layer (importing, deleting and moving the files an app keeps) and for persistent data
past the key-value `Storage`. JS had `process.dataDir`; the engine knew all three (the app's id, its directory of
`EngineConfig::data_dir`, its directory of the cache root) and gave Rust code none of them.

`app_id()`, `data_dir()` and `cache_dir()` of the prelude give them, as `Option`s that do not change while the app
runs.

### Changes

- **`nsh-core`** (`element/engine_context.rs`): the three accessors, and `EngineContext::with_app_id` /
  `with_data_dir` / `with_cache_dir`, which hold them in the engine's context under types only that module names (so
  no `engine_context::<T>()` reads or spoofs them). The context, not the environment's signals: the environment is
  `Copy` and reactive, these are paths that never change, and the reactive graph is not entered while JS calls a
  module function synchronously (the first version, on `Ambient`, panicked there with "no reactive runtime"), while
  the context is the thread's for the engine's whole run: components, synchronous calls, ops and tasks read it alike.
  Off the engine's thread they are `None`, as `engine_context()` is.
- **`nsh-engine`**: `enter_context` adds them to `EngineConfig::context` as the engine enters it, in `run` (the
  engine's thread) and `LocalEngine::start` (the web, tests), so a host's own values are replaced. The data directory
  is `app_data_dir`, now the one helper behind JS's `process.dataDir` (`SystemConfig::data_dir`), the file storage
  and Rust's `data_dir()`: `<data_dir>/<app_dir_name(app id)>`. The cache directory is `CacheOptions::app_dir`,
  `<root>/<app id>`, which `dir` (the image cache's `<root>/<app id>/images`) now builds on. In WebAssembly both are
  `None`: Rust code has no files there, and the web's `/app-data` is the browser's, which JS's `fs` reaches through
  `EngineConfig::filesystem`.
- **Platforms**: none. Every host already gives the engine its data directory and cache location, and the id travels
  with the app (`app.json`, the dev server's bundle, packages, `Root::app_id`): macOS, Windows, Linux and GPUI
  (`AppOptions::data_dir`, else `nsh_engine::data_dir()`; `AppOptions::cache`, `System` by default), iOS (`nsh` in
  Application Support, `nsh` in Caches), Android (`nsh` in the files directory and in the cache directory), the
  terminal host, the web (the page's id; `None` directories) and `nsh test` headless (`EngineConfig::new`: `None`
  directories). `LocalEngine` goes through `enter_context` as `Engine` does, and the tests run both.
- **JS**: no new export. `process.dataDir` is already the data directory. A TSX app's storage and files are scoped to
  it by the runtime (`Storage`, `${dataDir}`), so its code needs no id to find them, and a cache directory JS could use
  needs a `${cacheDir}` root in the capabilities first, a sandbox design change of its own. Hence no compat case.
- **Docs** (en and zh-CN): "The app's own files" in `website/docs/rust.md` (with a `blocking` example), the `fs/promises`
  and `process` rows of "From Rust" in `website/modules/standard-modules.md`; `docs/dev/components.md` (TS → Rust
  table) and `docs/dev/embedding.md` ("Storage").

## Public API

- Rust: `nsh_core::element::{app_id, data_dir, cache_dir}` (also in `nsh_core::prelude`): `app_id() -> Option<String>`,
  `data_dir() -> Option<PathBuf>`, `cache_dir() -> Option<PathBuf>`.
- Rust: `EngineContext::with_app_id(self, Option<String>) -> Self`, `with_data_dir(self, Option<PathBuf>) -> Self`,
  `with_cache_dir(self, Option<PathBuf>) -> Self`: what the engine sets; a test of Rust code without an engine enters
  a context with them.
- TSX: none.

`nsh_engine::data_dir()` (the system's directory for apps' data) keeps its name: code that glob-imports both
`nsh_engine::*` and the prelude and calls `data_dir()` names the one it means.

## Tests

- `rust_platform::components_read_the_apps_id_and_its_own_directories` (`nsh-engine`): a Rust app in `Engine` and in
  `LocalEngine` shows `<data>/com.example.notes` and `<caches>/com.example.notes`; an id that names no directory
  (`a/b`) keeps its id and lives in `default`; no data directory and no caches give `None`s.
- `rust_classes::a_ui_sets_code_reads_the_id_and_the_directories_of_the_app_it_runs_in` (`nsh-engine`): a TSX app's UI
  set reads them in a synchronous call and in a task; the data directory is the app's `process.dataDir`.
- `element::engine_context::tests::the_apps_id_and_directories_are_its_engines_and_the_last_set_wins` (`nsh-core`).
- Existing: `cargo test -p nsh-engine --lib cache` (2), and the engine groups `system`, `storage`, `process`, `fs`,
  `rust_classes`, `rust_platform`, `rust_app`, `images` (73) and `host_modules`, `external_wake`, `clock`, `parity`
  (14), which cover `process.dataDir`, storage and the context.
- Clippy `-D warnings`: `nsh-core` and `nsh-engine` (all targets); `nsh-engine` for wasm32 without features;
  `nsh-platform-web` for wasm32 with its default features (the engine's `browser`) and with none (a Rust app's
  runtime); `cargo xtask clippy-android` (both feature sets). Type-checked: `nsh-platform-ios` for `aarch64-apple-ios`, `nsh-platform-macos` and `nsh-macos-app`. Nightly
  rustfmt on the changed files.
- Not run: GPUI's workspace and the Windows and Linux crates (their code is unchanged and none of their files
  glob-imports the prelude beside another `app_id` / `data_dir` / `cache_dir`; the fork's CI builds them), devices and
  simulators.
