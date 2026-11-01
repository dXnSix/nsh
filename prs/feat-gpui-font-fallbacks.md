# gpui: Fall back along fontFamily lists

> **Upstream (checked 2026-11-01):** not opened, with [feat-font-fallbacks](feat-font-fallbacks.md).

Branch: `feat/gpui-font-fallbacks` on `github.com/dXnSix/nsh`, stacked on `feat/font-fallbacks` (`text: Name the families
of fontFamily lists`, #82). One commit, `388f7c13`. Opened as a pull request into the fork's `main`; once the stack
below it merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

GPUI set text in the first installed family of a `fontFamily` list, and a character it lacked came from the platform's fallbacks only.

### Changes

- **`text.rs`**: the list's other installed families are the font's `FontFallbacks`, in order, before the platform's (`TEXT_FALLBACKS`).
- **Docs**: the Text row of `docs/dev/gpui.md`.

## Public API

None: `fontFamily` lists (#82) work on GPUI.

## Tests

- `text::tests::a_family_list_sets_text_in_its_first_family_and_falls_back_to_the_others` (`nsh-platform-gpui`; GPUI's text tests run on Linux and Windows, not on macOS).
- `cargo test -p nsh-platform-gpui --features shot` and clippy on macOS: clean.
