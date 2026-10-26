# windows: Fall back along fontFamily lists

Branch: `feat/windows-font-fallbacks` on `github.com/dXnSix/nsh`, stacked on `feat/font-fallbacks` (`text: Name the families
of fontFamily lists`, #82). One commit, `3e2a1dd4`. Opened as a pull request into the fork's `main`; once the stack
below it merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

DirectWrite set text in the first family of a `fontFamily` list Windows has, and a character it lacked came from the system's fallback, never from the list's other families.

### Changes

- **`text.rs`**: the list's other installed and registered families make an `IDWriteFontFallback` (each family over all of Unicode, in the list's order, then the system's fallback: `IDWriteFactory2::CreateFontFallbackBuilder`), kept on the resolved face and set on the paragraph's `IDWriteTextFormat1`. DirectWrite takes one fallback per layout, so a paragraph follows its first style's list.
- **Docs**: the Text row of `docs/dev/windows.md`.

## Public API

None: `fontFamily` lists (#82) work on Windows.

## Tests

- `text::tests::a_family_list_falls_back_to_its_other_families` (`nsh-platform-windows`, Windows CI): `"No Such Family, Pacifico, Consolas"` is the registered Pacifico with a fallback; `"Consolas"` has none; a paragraph in Latin and Chinese lays out.
- Type-checked on macOS: the fallback functions in a scratch crate with `windows` 0.62, `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings`: clean.
