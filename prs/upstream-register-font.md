# Upstream PR: text: Let apps register fonts while they run

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/register-font/01-core`, on `upstream/main` `201d780a`: copies of the WOFF and the asset-reading commits,
  then its own.
- Size of its own commit: 27 files, +322 / −25 (code +135 / −21, tests +109, docs +77 / −4), one test font
  (`Pacifico-Regular.woff2`, 121 KB).
- Its compatibility case (`tests/compat/cases/layout/core-register-font.tsx`) was added 2026-10-30, as a new export
  lands with one; the fork's own commit has none.
- Title: `text: Let apps register fonts while they run`.


- 2026-10-31: restacked locally on #746's review fix (`366d830f8`: WOFF, asset reading, its fix `eb04bbb04`, then its
  own); one conflict, its `registerFont` row beside the extended `fetch` row in `docs/dev/components.md` (both kept).
  `cargo test -p nsh-engine --test engine assets` (6) and `font` (3) pass. Not pushed yet: the fork's branch is still
  `9fabfce60` (a force-push, the owner's OK first).

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
