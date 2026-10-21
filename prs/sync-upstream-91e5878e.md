# Merge upstream/main 91e5878e

Fork pull request bringing the fork's `main` (`f90a4c89`, `six-gamify-0.1.1-8`) up to `upstream/main` `91e5878e`
(2026-10-21), 53 upstream commits after the last sync (`5f8daece`, fork PR #11). Branch `sync/upstream-91e5878e`, one
merge commit, `f82b9a59`, with parents `origin/main` (`f90a4c89`) and `upstream/main` (`91e5878e`; upstream had two
more commits by the time it was pushed, for the next sync). Fork PR #16, merged with a merge commit as `3c624a0c`
after its checks: every workflow passed (Examples, GPUI on three systems, Linux, Windows, Rust application on three
systems and web, Website, Release `dist` on three systems, CI paths) except `Core / check`, whose one failure is
`compat::nothing_merged_is_removed` reporting `apps/props-0.1.1.2` changed against the fork's `main`: that name was the
fork's own fixture (the offset case) and is now upstream's (the SVG case), the offset case being `props-0.1.1.4`. It
passes again from the next change on, since the fork's `main` now holds upstream's fixture. Not tagged.

## What changed upstream that the fork carried itself

Upstream merged its reviewed versions of four of the fork's own topics. Where they meet the fork's copies, upstream's
version is kept:

- `web: Let an app's Rust crates use wasm-bindgen in the browser` (#228; the fork's `feat/wasm-bindgen`, fork PR #4).
- `core: Load the @1x, @2x or @3x image that fits the screen's density` (#229; `feat/multires-images`, #7).
- `core: Show SVG images on every platform` (#230; `feat/svg-images`, #8).
- `ios: Pass clippy on AppSource as the desktop backends do` (#248; `feat/ios-app-source-lint`, #9 and #10).

The sticky list work (fork PRs #12 to #15) is now upstream as three open pull requests: #280 (web), #281 (core) and
#285 (`stickyHeaderOffset`). The fork takes exactly those three branches, plus the offset's web test and the sentence
in `docs/dev/web.md` that need #280's tracks. When upstream merges them, the next sync resolves to the same content.
The fork's own `props-0.1.1.2` fixture (the offset case) gives way to upstream's `props-0.1.1.2` (the SVG case); the
offset case lives in `props-0.1.1.4`.

## Fork-only topics kept

Upstream does not have these; each is carried on top of upstream's current code:

- `cli: Fail instead of moving to another port with --strict-port` (fork PR #3): `dev.rs`, `main.rs`, `run.rs`,
  `docs/dev/tooling.md`, `website/docs/cli.md` (English and zh-CN).
- `cli: Build an app's development web runtime incrementally` (`web-dev` profile, fork PR #2): `rust_app.rs`,
  `docs/dev/tooling.md`, `docs/dev/web.md`, `website/docs/web.md` (English and zh-CN); upstream's own section on Rust
  apps on the web keeps its place, with the fork's sentence on the incremental development runtime added.
- `macos: Show a border that a state turns visible` (fork PR #5): `decor.rs`, `view.rs`.
- `android: Follow display density changes while the app runs` (fork PR #6): `Core::remount` and the split of
  `ensure_created` into creation and `mount_state`, redone on upstream's `ensure_created` (which now skips overlays),
  `Engine::remount`, the Android host's shared `Density` (beside upstream's test-session setup), `NshView`, `Mounter`,
  `CanvasBox`, `text.rs`.
- The git source of nsh's crates (fork PR #1) needs nothing: upstream's own Git support (#213) and the last sync's
  resolution already cover it.

## Validation

Checks on the merged tree (`f82b9a59`):

- `cargo test --workspace`: 1385 tests in 68 binaries, no failures; `cargo test -p nsh-engine --test engine compat::`:
  21 passed (the fixtures `props-0.1.1.2`, `-0.1.1.3` and `-0.1.1.4` included).
- clippy: the workspace, the macOS crates, the Catalyst line, `nsh-platform-web` for wasm32 and `cargo xtask
  clippy-android` clean, apart from upstream's own warning in `nsh-windows-host/src/lib.rs` (items after a test module).
  Android clippy found `Host::density`, which upstream added for test sessions, reading the fork's shared `Density`
  as an `f32`; it reads `Density::get` now.
- `cargo xtask android --dev`: `libnsh.so` for arm64-v8a and x86_64 and the Java runtime build.
- `node --test crates/nsh-platform-web/web/scroll.test.mjs`: 9 passed.
- `cargo +nightly fmt --all --check`: only upstream's own diffs, in files the fork does not change
  (`nsh-core/src/lib.rs`, `nsh-js/src/modules.rs`, `nsh-testing/src/agent.rs`).

How it was built: upstream's `main`, merged with the three upstream PR branches (`pr/web-sticky`,
`pr/sticky-row-before-pinned`, `pr/list-sticky-offset`), then each file a fork-only topic changes merged three ways
(base `5f8daece`, the fork's `main`, that tree) and its conflicts resolved as above; every other file is upstream's.
The difference from `upstream/main` is 44 files: the sticky work, `--strict-port`, `web-dev`, the macOS focus border
and Android's density.
