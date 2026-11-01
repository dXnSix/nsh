# ios: Fall back along fontFamily lists

> **Upstream (checked 2026-11-01):** not opened, with [feat-font-fallbacks](feat-font-fallbacks.md).

Branch: `feat/ios-font-fallbacks` on `github.com/dXnSix/nsh`, stacked on `feat/font-fallbacks` (`text: Name the families
of fontFamily lists`, #82). One commit, `45e666ef`. Opened as a pull request into the fork's `main`; once the stack
below it merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

CoreText read a `fontFamily` list as one family name, which no font has: text was set in the system font.

### Changes

- **`text.rs`**: `make_font` resolves the list as macOS's engine does (#83): the first family it has is the font, the others it has its cascade list (`kCTFontCascadeListAttribute`), with UIKit's system designs for the generic families.

## Public API

None: `fontFamily` lists (#82) work on iOS.

## Tests

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`: clean.
- The same code as macOS's, whose test covers it on this Mac.
