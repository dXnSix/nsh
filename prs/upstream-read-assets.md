# Upstream PR: engine: Let fetch read the files the app ships

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/read-assets/01-core`, one commit on `upstream/main` `201d780a`.
- Size: 18 files, +562 / −15 (code +204 / −10, tests +297 / −2, docs +61 / −3), one test image (`flag.png`, 73 bytes).
- Title: `engine: Let fetch read the files the app ships`.

---

**Depends on:** no other PR; it applies to `main` as it is. The PR that lets apps register fonts while they run
(`text: Let apps register fonts while they run`) builds on it.

## Description

TSX and Rust components name a bundled file by its `asset:/` URL (an imported image's `url`, `asset!`'s
`FileAsset::uri`, `assets!()`) and hand it to elements that load URLs, but cannot read its bytes: `fetch` refuses the
scheme, so data files such as translations or settings have to be compiled into the bundle or the binary, and every
change to them means a rebuild.

`fetch` (JS's, and Rust's) now answers `asset:/` URLs with the app's own files:

- The engine reads them with the host's `Assets` on the runtime's blocking pool, so a large file or the dev server
  never holds the engine's thread, and answers 200 with the bytes and their `Content-Length`.
- A host that serves files it does not hold says where with the new `Assets::url` (the web page, which holds only what
  it preloaded), and the engine downloads them with its network stack.
- They are the app's own files, so the app's network capabilities do not apply to them. A file the host does not
  have, or does not serve, rejects as a network error that names it, on every platform alike.

Docs: "Reading a file at run time" in `website/docs/images-and-assets.md`, `website/docs/networking.md` and `fetch` in
`website/modules/globals.md` (each with its zh-CN page), `docs/dev/components.md`.

## Public API

- `fetch` (JS and Rust): `asset:/` URLs, answered with the app's files.
- `nsh-engine`: `Assets::url(&self, uri: &str) -> Option<String>`, where the host serves `uri` when `read` does not
  have it; a provided method returning `None`, so existing hosts keep building.

## How to Test

- `cargo test -p nsh-engine --test engine assets`: `tasks_read_the_files_the_app_ships`,
  `files_nothing_waits_for_are_not_read`, `a_host_without_assets_reads_none`,
  `files_the_host_does_not_hold_are_downloaded_from_where_it_serves_them` and
  `js_reads_the_files_the_app_ships_with_fetch`.
- Compat: `tests/compat/cases/web` fetches its `images/flag.png` (`200`, 73 bytes, the PNG signature) and a missing
  file (a `TypeError`).
- In an app: `await (await fetch(image.url)).bytes()` of an imported file, in development and in a package.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, wasm32 included (Windows, Linux
  and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commit, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
