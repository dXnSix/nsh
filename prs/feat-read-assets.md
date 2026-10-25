# engine: Let fetch read the files the app ships

Branch: `feat/read-assets` on `github.com/dXnSix/nsh`, cut from `eb76a17d` (the latest `upstream/main` commit the fork's
`main` holds). One commit, `c4aa5e54`: `engine: Let fetch read the files the app ships`. Opened as a pull request into
the fork's `main` and merged there with a merge commit. Not opened upstream: the owner decides and posts there (upstream
squash merges).

## Summary

TSX and Rust components name a bundled file by its `asset:/` URL (an imported image's `url`, `asset!`'s
`FileAsset::uri`, `assets!()`) and hand it to elements that load URLs, but could not read its bytes: `fetch` refused the
scheme ("unsupported URL scheme `asset`"), so data files such as translations or settings had to be compiled into the
bundle or the binary, and every change to them meant a rebuild. The engine already reads bundled files with the host's
`Assets` (the fonts it registers before the first render); `fetch` now uses it too.

### Changes

- **JS `fetch`** (`nsh-js` `web/fetch.rs`) hands `asset:` URLs to the engine as it hands `http(s)`.
- **Engine** (`Net`): a request of an `asset:/` URL, from JS or from a Rust task (`ui::fetch`), is answered from
  `EngineConfig::assets`, with no capability check: these are the app's own files. The read runs on the blocking pool of
  the components' tasks' runtime (`spawn_blocking`), so a large file or the dev server never holds the engine's thread,
  and comes back as `Message::AssetRead`; in WebAssembly, which has no threads, it runs in the turn. The response is
  `200 OK` with the bytes and `Content-Length` (no body for `HEAD`; other methods fail). A cancelled request drops the
  read's result.
- **Downloads**: when the read fails and the host serves the file at a URL (`Assets::url`, new, `None` by default), the
  engine downloads it with its network stack. A status other than 2xx fails it as a missing file does, so a missing file
  is a network error naming it on every platform.
- **Web**: `PageAssets::url` asks the page where `asset:/…` is served (the new `asset_url` import: `Player.asset`, the
  `assets` map, else `assetBase`), so files the page did not preload are downloaded from there.
- **Docs**: `docs/dev/components.md` (Rust's `fetch`), "Reading a file at run time" in `website/docs/images-and-assets.md`,
  the `asset:/` bullet in `website/docs/networking.md`, `fetch`'s declaration in `website/modules/globals.md`, and their
  zh-CN pages.

Nothing changes for requests of other schemes, and no work is done in a turn without them.

## Public API

- JS `fetch(input, init)`: also accepts `asset:/` URLs (before, it rejected them with a `TypeError`): a widening.
- Rust `nsh_core::ui::fetch(request)`: the same for `asset:/` URLs.
- `nsh_engine::Assets::url(&self, uri: &str) -> Option<String>`, a provided method returning `None`: where a host serves
  the files `Assets::read` does not hold.

## Tests

- New group `assets` in `crates/nsh-engine/tests/engine/` (`cargo test -p nsh-engine --test engine assets::`):
  - `tasks_read_the_files_the_app_ships`: a file and a missing one, read on the runtime's `nsh-tasks` threads with no
    network stack;
  - `files_nothing_waits_for_are_not_read`;
  - `a_host_without_assets_reads_none`;
  - `files_the_host_does_not_hold_are_downloaded_from_where_it_serves_them`: a body in two chunks, and a 404 failing as
    a missing file;
  - `js_reads_the_files_the_app_ships_with_fetch`: status, `Content-Length` and body from TSX, and the `TypeError` of a
    missing file.
- Compat: `fetch.asset` and `fetch.assetMissing` in `tests/compat/cases/web/web-fetch.tsx` (an imported PNG read back),
  with their steps in `script.txt`. `NSH_COMPAT_APP=web cargo test checkout_apps_run` in `nsh-compat` runs the web app
  through; the run then stops at the `events` app's `covers [props] pasteBehavior` claim, which `nsh-compat`'s latest
  commit (#13) expects of a newer NSH than this base.
- `cargo test -p nsh-core -p nsh-engine -p nsh-testing -p nsh-js`: passed (engine 234).
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-core -p nsh-engine -p nsh-testing -p nsh-js --all-targets` and
  `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown`: clean; `web/nsh.js` parses
  (`node --input-type=module --check`).
- Not run in a browser: the web download path is covered by the engine test above (a host whose `Assets::url` serves the
  files), not by `nsh test --target web`.
