# macos: Fall back along fontFamily lists

> **Upstream (checked 2026-11-01):** not opened, with [feat-font-fallbacks](feat-font-fallbacks.md).

Branch: `feat/macos-font-fallbacks` on `github.com/dXnSix/nsh`, stacked on `feat/font-fallbacks` (`text: Name the families
of fontFamily lists`, #82). One commit, `42b85414`. Opened as a pull request into the fork's `main`; once the stack
below it merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

CoreText read a `fontFamily` list as one family name, which no font has: text was set in the system font.

### Changes

- **`text.rs`**: `make_font` resolves the list (`nsh_core::font_families`): the first family CoreText has is the font, and the others it has are its cascade list (`kCTFontCascadeListAttribute`), which CoreText takes a character the first lacks from before the system's fallback fonts. A list none of whose families exists warns as a single missing family did.
- **Docs**: the Text row of `docs/dev/macos.md`.

## Public API

None: `fontFamily` lists (#82) work on macOS.

## Tests

- `text::tests::a_family_list_sets_text_in_its_first_installed_family` (`nsh-platform-macos`): `"No Such Family, Menlo, 'Courier', sans-serif"` is Menlo with a cascade list of two; a list of missing families is not one of them; a single family has no cascade list.
- `cargo clippy --all-targets -p nsh-platform-macos --features nsh-platform-macos/dev -- -D warnings`: clean.
