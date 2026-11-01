# Upstream PR: engine: Let fetch read the files the app ships

Opened 2026-10-31 as [longbridge/nsh#746](https://github.com/longbridge/nsh/pull/746), at the owner's
decision; the body below the line is the one posted.

- Branch: `pr/read-assets/01-core`, on `upstream/main` `201d780a`: the feature's commit, then the two review fixes
  (head `a13982b40`).
- Size: 18 files, +713 / −15 with the review fixes; the feature's commit alone was +562 / −15 (code +204 / −10, tests
  +297 / −2, docs +61 / −3), with one test image (`flag.png`, 73 bytes).
- Title: `engine: Let fetch read the files the app ships`.


## Review (2026-10-31)

huacnlee requested changes (static analysis, "Reviewed by ChatGPT on behalf of @huacnlee"), one inline comment:

1. [P1] Keep asset fetches inside the configured asset root (`r4429677887`, `crates/nsh-engine/src/lib.rs`): on iOS
   with `AssetBase::Directory`, `asset:////tmp/x` reaches `AssetReader::read`, whose `dir.join(path)` takes the
   absolute path, and `fetch` returned the bytes. It holds, and not only on iOS: every host follows `..` out of its
   directory (the core's `resolve` trims leading slashes only).
   Fixed in `66a31164d` (`engine: Keep fetches of bundled files inside the app's files`): the engine refuses, before
   any host reads it, a path with an empty, `.` or `..` segment (percent-encoded too) or a backslash, NUL or colon,
   as the network error a missing file gives; `files_outside_the_assets_are_refused` reads through a reader that joins
   as iOS's does, beside a file outside its directory, and reads that file twice without the check. Docs: the user
   page (en, zh-CN) and the `fetch` row of `docs/dev/components.md`. Checked: `cargo test -p nsh-engine --test engine
   assets` (6), `--lib` (35), clippy `-D warnings` (all targets), nightly rustfmt.
   Pushed (fast-forward) and replied (`r4429733277`); resolving the thread through the API was refused here, so it
   is open for the reviewer.

## Review (2026-11-01)

huacnlee's second pass, a reply in the same thread (`r4433885726`, static inspection):

2. [P2] The refusal still entered `asset_read`, whose error branch resolves `Assets::url` and downloads it, so on a
   host that serves its files (the web) a refused path reached the network. It held, and a method other than `GET` /
   `HEAD` took the same path: a `POST` of a bundled file read it through its served URL.
   Fixed in `a13982b40` (`engine: Fail refused fetches of bundled files before any host serves them`): `Net::asset`
   fails both at once, as a terminal network error, before any host reads the file or is asked for its URL.
   `ServedAssets` records every URL it is asked for, and `refused_files_are_neither_looked_up_nor_downloaded` fetches
   `asset:/../outside.txt`, a `POST` and a `GET` of a bundled file: only the `GET` reaches the resolver and the
   download (without the fix the refused path is downloaded and the `POST` reads the file). Docs: the `fetch` row of
   `docs/dev/components.md`, the user page (en, zh-CN). Checked: nightly rustfmt, clippy `-D warnings` of `nsh-engine`
   (all targets) and of `nsh-platform-web` (wasm32, `compiler`), `cargo test -p nsh-engine` (305 + 35 + 1).
   Fork first: dXnSix/nsh#99 (`fix/asset-refusals`, the same commit on the fork's `main`) passed every check on its
   first run: Fork CI's iOS, Android, macOS and web jobs (run 41258620144), Core, Linux, Windows, Rust application,
   GPUI and Examples.
   Pushed (fast-forward, `66a31164d..a13982b40`) and replied (`r4435859082`); the thread is open for the reviewer.

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
