# text: Let apps register fonts while they run

Branch: `feat/register-font` on `github.com/dXnSix/nsh`, stacked on `feat/woff` (`text: Read WOFF and WOFF2 fonts`, #80),
which it uses. One commit, `c26f6133`. Opened as a pull request into the fork's `main`; once the WOFF pull request
merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

Fonts were registered only as the app started: the files a Rust crate's `assets!()` or a TSX import shipped. A font
the app learns of later (one a user picks from a catalog, one a server sends with a document, one the app makes)
had no way in.

### Changes

- **`nsh-core`**:
  - `registerFont(source)` of `nsh` (the runtime module `runtime/fonts.rs`): TrueType, OpenType, WOFF or WOFF2
    data (a `Uint8Array`), or the URL of a file (one of the app's, `asset:/...`, or one on the network, read as
    `fetch` reads it, so the `network` capability applies). Resolves to the font's family name; rejects with an
    `Error` when the URL does not load or the data is no font.
  - Rust's `register_font(data) -> Answer<Result<String, String>>` (the prelude), awaited in a task.
  - Both go through `Action::RegisterFont`: the core reads the family (`nsh_core::font_family`, moved from the
    bundler, which uses it now; a font without one is refused) and registers the font as it does the app's own
    (`Core::register_font`), so text measured before is measured again and text naming the family uses it from then
    on. Platforms are unchanged: they register fonts as before.
- **Docs** (en and zh-CN): "Registering a font while the app runs" in Fonts, `registerFont` on the `nsh` page and in
  the modules index; `docs/dev/components.md`.

## Public API

- Rust: `nsh_core::font_family`, `nsh_core::prelude::register_font`.
- TSX: `registerFont` of `nsh`.

## Tests

- `text::tests::woff_and_woff2_fonts_decompress_to_the_font_they_hold` checks `font_family` too.
- `rust_platform::a_component_registers_a_font_while_the_app_runs` (`nsh-engine`): a component's task registers a
  WOFF2 font and shows its family.
- `tests/testing/fonts` (`nsh-testing`): `registerFont("asset:/Pacifico-Regular.woff2")` resolves to `"Pacifico"`;
  bytes that are no font and a missing file reject with an `Error`.
- `cargo test` of `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-bundler`, `nsh-macros`, `nsh-testing` and GPUI's;
  clippy `-D warnings` for them, the macOS crates, iOS (Mac Catalyst), Android and the web.
