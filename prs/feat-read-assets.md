# core: Let Rust code read the files the app ships

Branch: `feat/read-assets` on `github.com/dXnSix/nsh`, cut from `eb76a17d` (the latest `upstream/main` commit the fork's
`main` holds). One commit, `d626b099`: `core: Let Rust code read the files the app ships`. Not pushed yet; it goes into
the fork's `main` through a pull request with a merge commit. Not opened upstream: the owner decides and posts there
(upstream squash merges).

## Summary

Rust components and modules name a bundled file by its `asset:/` URI (`asset!`, `assets!()`) and hand it to elements
that load URIs, but could not read its bytes. Data files such as translations or settings had to be compiled in with
`include_bytes!`, so every change to them meant a rebuild of the binary. The engine already reads bundled files with
the host's `Assets` (the fonts it registers before the first render); this lets tasks use it too.

### Changes

- **Core**: `ui::read_asset(uri)` (in the prelude) returns an `Answer<Result<Vec<u8>, String>>` to await in a task. It
  is an `Action::ReadAsset` the core keeps by request id, as it keeps fetches: the engine takes the reads with
  `Core::take_asset_reads` and answers them with `Core::answer_asset_read`. Reads nothing waits for any more (the task
  dropped the answer) are dropped before the engine sees them.
- **Engine**: after each turn's commit it reads the files with `EngineConfig::assets` on the blocking pool of the
  components' tasks' runtime (`spawn_blocking`), so a large file or the dev server never holds the engine's thread; the
  read comes back as `Message::AssetRead`. In WebAssembly, which has no threads, it reads in the turn. When the read
  fails and the host serves the file at a URL (`Assets::url`, new, `None` by default), the engine downloads it with its
  network stack (`Requester::Asset`): the body is collected and a status outside 200–299 is an error naming the URL.
  These are the app's own files, so the app's network capabilities do not apply to them.
- **Web**: `PageAssets::url` asks the page where `asset:/…` is served (the new `asset_url` import: `Player.asset`, the
  `assets` map, else `assetBase`), so files the page did not preload are downloaded from there.
- **Docs**: `docs/dev/components.md` (the TS → Rust table and the platform services), and "Reading a file at run time"
  in `website/docs/images-and-assets.md` and its zh-CN page.

Nothing changes for apps that do not call it: no new mount items, no work in a turn without reads.

## Public API

- `nsh_core::ui::read_asset(uri: impl Into<String>) -> Answer<Result<Vec<u8>, String>>` (also
  `nsh_core::ui::prelude::read_asset`): the bytes of a file the app ships, to await in a task.
- `nsh_core::Core::take_asset_reads(&mut self, out: &mut Vec<(u32, String)>)`: the reads tasks asked for, for the
  engine.
- `nsh_core::Core::answer_asset_read(&mut self, id: u32, result: Result<Vec<u8>, String>)`: answers one.
- `nsh_engine::Assets::url(&self, uri: &str) -> Option<String>`, a provided method returning `None`: where a host serves
  the files `Assets::read` does not hold.

## Tests

- `crates/nsh-engine/tests/engine/rust_platform.rs` (`cargo test -p nsh-engine --test engine rust_platform`), new:
  `tasks_read_the_files_the_app_ships` (a file and a missing one, both read on the runtime's `nsh-tasks` threads),
  `files_nothing_waits_for_are_not_read`, `a_host_without_assets_reads_none` and
  `files_the_host_does_not_hold_are_downloaded_from_where_it_serves_them` (a body in two chunks, and a 404).
- `cargo test -p nsh-core -p nsh-engine -p nsh-testing`: passed (core 733, engine 233).
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-core -p nsh-engine -p nsh-testing --all-targets` and
  `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown`: clean. `web/nsh.js` parses
  (`node --input-type=module --check`).
- Not run in a browser: the web download path is covered by the engine test above (a host whose `Assets::url` serves the
  files), not by `nsh test --target web`.
