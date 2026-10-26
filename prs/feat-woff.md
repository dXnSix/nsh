# text: Read WOFF and WOFF2 fonts

Branch: `feat/woff` on `github.com/dXnSix/nsh`, cut from the upstream sync (`sync/upstream-8c65c8d0`, #78). One
commit, `5f3150b0`. Opened as a pull request into the fork's `main`; once #78 merges, it is its own commit only. Not
opened upstream: the owner decides and posts there.

## Summary

Apps shipped fonts as TrueType or OpenType files only: an imported `.woff` or `.woff2` was a plain file, and a Rust
crate's `assets!()` shipped it as one. Web fonts are mostly WOFF2, often half the size of their TrueType.

### Changes

- **`nsh-core`**: `nsh_core::font_data(data)`, a font file's TrueType or OpenType data: the data itself, or what a
  WOFF or WOFF2 file holds, decompressed ([`wuff`](https://crates.io/crates/wuff) 0.2.9, MIT, pure Rust; WOFF2's
  transformed glyph tables rebuilt). `Core::register_font` goes through it, so every platform's text engine gets the
  data it already takes: no platform changes.
- **`nsh-bundler`**: `.woff` and `.woff2` imports are fonts (their family name, read from the font they hold;
  their `declare module` types).
- **`nsh-macros`**: `assets!()` ships them as `FontAsset`s, `asset!` reads their family (decompressing with
  `wuff` too: the macro crate cannot use `nsh-core`).
- **`nsh-cli`**: the web page loads them before the app starts, as it does TrueType fonts (the dev server already
  served their MIME types).
- **Docs** (en and zh-CN): Fonts, Images and assets, Project structure; `docs/dev/components.md`.

## Size

`wuff` brings `brotli-decompressor` and `flate2` into the web runtime (the native hosts have `flate2` already):
`nsh_platform_web.wasm` (`--profile web --features bytecode-only`) grows by 216 KB raw (+1.4%, 16.37 MB to
16.59 MB) and 24 KB brotli-compressed (+0.6%, 4.21 MB to 4.23 MB).

## Public API

- Rust: `nsh_core::font_data`.
- TSX: `.woff` and `.woff2` imports, as `.ttf`.

## Tests

- `text::tests::woff_and_woff2_fonts_decompress_to_the_font_they_hold` (`nsh-core`): Pacifico as WOFF and WOFF2
  (`crates/nsh-core/tests/fonts/`, made from the TrueType the repository has) decompresses to a font with its
  family, glyph count and glyphs; TrueType comes back as it is; a broken WOFF2 is refused.
- `plugin::tests::a_font_import_names_its_family_from_truetype_woff_and_woff2` (`nsh-bundler`), and
  `assets::tests::a_woff_or_woff2_font_names_the_family_it_holds` (`nsh-macros`).
- With the run-time registration (#81) on top: `cargo test` of `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-bundler`,
  `nsh-macros`, `nsh-testing` and GPUI's; clippy `-D warnings` for them, the macOS crates, iOS (Mac Catalyst),
  Android and the web.
