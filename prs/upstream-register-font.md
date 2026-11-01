# Upstream PR: text: Let apps register fonts while they run

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/register-font/01-core`, on `upstream/main` `201d780a`: copies of the WOFF and the asset-reading commits
  (#746 with both review fixes), then its own (head `992932b1e`).
- Size of its own commit: 27 files, +322 / −25 (code +135 / −21, tests +109, docs +77 / −4), one test font
  (`Pacifico-Regular.woff2`, 121 KB).
- Its compatibility case (`tests/compat/cases/layout/core-register-font.tsx`) was added 2026-10-30, as a new export
  lands with one; the fork's own commit has none.
- Title: `text: Let apps register fonts while they run`.


- 2026-10-31: restacked locally on #746's review fix (`366d830f8`: WOFF, asset reading, its fix `eb04bbb04`, then its
  own); one conflict, its `registerFont` row beside the extended `fetch` row in `docs/dev/components.md` (both kept).
  `cargo test -p nsh-engine --test engine assets` (6) and `font` (3) pass. Force-pushed to the fork (the owner's OK),
  over `9fabfce60`.
- 2026-11-01: restacked on #746's second review fix (`992932b1e`: WOFF, asset reading, its fixes `eb04bbb04` and
  `9d9f7805c`, the latter #746's `a13982b40`, then its own); the same `components.md` conflict, replayed by rerere and
  checked. Its own commit is unchanged; `registerFont` reads a URL with `fetch`'s `GET`, which the new refusal lets
  through. Checked: nightly rustfmt on the changed files, clippy `-D warnings` (`nsh-core`, `nsh-engine`,
  `nsh-bundler`, `nsh-testing` with `bundle`, all targets), `cargo test -p nsh-engine --test engine` (306),
  `-p nsh-testing --features bundle --test testing fonts`, `-p nsh-core --lib` (925), `-p nsh-bundler --features
  bundle` (66). Force-pushed to the fork (the owner's OK) over `366d830f8`, then the fork's Core, Rust application,
  Examples, Windows, Linux and GPUI workflows run on it (41313375092, 41313379536, 41313381664, 41313383726,
  41313385644, 41313387572; CI paths 41313373173 passed). Compat's job is disabled upstream (`if: false`), so it ran
  here: `NSH_COMPAT_APP=layout cargo xtask compat` passes (`checkout_apps_run`), and so do the `core-register-font`
  case's steps headless (`Registered: Pacifico, true`, `No font: refused, true`).

---

**Depends on:** #745 (`text: Read WOFF and WOFF2 fonts`: it registers WOFF and WOFF2 data) and #746
(`engine: Let fetch read the files the app ships`: it reads `asset:/` URLs as `fetch` does). This branch carries both
commits; once they are merged it shows only its own, the last.

## Description

Fonts are registered only as the app starts: the files a Rust crate's `assets!()` or a TSX import ships. A font the
app learns of later (one a user picks from a catalog, one a server sends with a document, one the app makes) has no way
in.

`registerFont(source)` of `nsh` takes TrueType, OpenType, WOFF or WOFF2 data, or the URL of a file (one of the app's,
`asset:/`, or one on the network, read as `fetch` reads it: the `network` capability applies), and resolves to the
font's family name; Rust components await `register_font(data)` (the prelude). Both go through the core's
`Action::RegisterFont`: the core reads the family (`nsh_core::font_family`, moved from the bundler; a font without one
is refused) and registers the font as it does the app's own, so text measured before is measured again and text naming
the family uses it from then on.

Docs: "Registering a font while the app runs" in `website/docs/fonts.md`, `registerFont` in `website/modules/nsh.md`
and `website/modules/index.md` (each with its zh-CN page), `docs/dev/components.md`.

## Public API

- `nsh`: `registerFont(source: string | Uint8Array): Promise<string>`, registers a font and resolves to its family
  name; rejects with an `Error` when the URL does not load or the data is no font.
- `nsh-core`: `register_font(data: impl Into<Vec<u8>>) -> Answer<Result<String, String>>` (also in the prelude), to
  await in a task; `font_family(data: &[u8]) -> Result<String, String>`, the family a font registers under (its
  typographic family, name 16, else its family, name 1; moved from `nsh-bundler`, which now calls it).

## How to Test

- `cargo test -p nsh-engine --test engine rust_platform`: `a_component_registers_a_font_while_the_app_runs`.
- `cargo test -p nsh-testing --features bundle --test testing fonts`: "a font registered while the app runs names its
  family" (a WOFF2 file by its `asset:/` URL, then text in it), "data that is no font is refused", "a font that does
  not load is refused".
- Compat: `tests/compat/cases/layout`'s `core-register-font` registers the app's Pacifico again by its `asset:/` URL
  (`Registered: Pacifico, true`: the family its import gives) and data that is no font (`No font: refused, true`), and
  claims `covers [nsh] registerFont` (`NSH_COMPAT_APP=layout cargo xtask compat`).
- In an app: `fontFamily: await registerFont(url)` for a font downloaded at run time.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commit, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
