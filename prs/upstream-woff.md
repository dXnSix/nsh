# Upstream PR: text: Read WOFF and WOFF2 fonts

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/woff/01-core`, one commit on `upstream/main` `201d780a`.
- Size: 21 files, +175 / −27 (code +101 / −10, docs +9 / −9, lock files +65 / −8: `wuff` and what it uses), two test
  fonts (`Pacifico-Regular.woff` 144 KB and `.woff2` 121 KB, the repository's Pacifico, OFL).
- Title: `text: Read WOFF and WOFF2 fonts`.

---

**Depends on:** no other PR; it applies to `main` as it is. The PR that lets apps register fonts while they run
(`text: Let apps register fonts while they run`) builds on it.

## Description

Apps ship fonts as TrueType or OpenType files only: an imported `.woff` or `.woff2` is a plain file, and a Rust
crate's `assets!()` ships it as one. Web fonts are mostly WOFF2, often half the size of their TrueType.

`nsh_core::font_data` reads a font file's TrueType or OpenType data, decompressing WOFF and WOFF2 (`wuff`, pure Rust;
WOFF2's transformed glyph tables rebuilt), and the core registers fonts through it, so every platform's text engine
gets the data it already takes:

- **Bundler**: imports `.woff` and `.woff2` files as fonts: their family name, their type declarations in
  `.nsh/types`.
- **Rust**: `asset!()` and `assets!()` ship them as `FontAsset`s, their family read from the font they hold.
- **Web**: the page loads them before the app starts, as it does TrueType fonts (`nsh package --target web`).

Docs: `website/docs/fonts.md`, `images-and-assets.md` and `project-structure.md` (each with its zh-CN page), and
`docs/dev/components.md`.

## Public API

- `nsh-core`: `font_data(data: &[u8]) -> Result<Cow<'_, [u8]>, String>`, a font file's TrueType or OpenType data
  (`data` itself, or what a WOFF or WOFF2 file holds).
- TSX: `import font from "./fonts/Name.woff2"` (and `.woff`) is a font import, as `.ttf` and `.otf` are.
- `nsh-macros`: `asset!()` / `assets!()` make `FontAsset`s of `.woff` and `.woff2` files.

## How to Test

- `cargo test -p nsh-core woff_and_woff2_fonts_decompress_to_the_font_they_hold`.
- `cargo test -p nsh-bundler --features bundle a_font_import_names_its_family_from_truetype_woff_and_woff2`.
- `cargo test -p nsh-macros a_woff_or_woff2_font_names_the_family_it_holds`.
- In an app: import a `.woff2` font and name its family in `fontFamily`.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commit, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
