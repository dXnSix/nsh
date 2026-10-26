# text: Name the families of fontFamily lists

Branch: `feat/font-fallbacks` on `github.com/dXnSix/nsh`, stacked on `feat/register-font` (#81), whose Fonts docs it
extends. One commit, `1ca80297`. Opened as a pull request into the fork's `main`; once #78, #80 and #81 merge, it is its
own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`fontFamily` took a list only where a platform parsed one itself: Pango on Linux, the browser, GPUI's and Windows'
first installed family. CoreText and Android read the whole value as one family name, so a list always gave the
system font, and no platform fell back from one family of the list to the next for the characters the first lacks
(Linux and the web excepted).

### Changes

- **`nsh-core`**: `nsh_core::font_families(value)`, a `fontFamily` value's families as CSS lists them, most preferred
  first: names trimmed and unquoted, the generic families as `FamilyName::System`, `Serif`, `Monospace` and `Rounded`.
  The `fontFamily` prop's TS doc says it takes a list.
- **Docs** (en and zh-CN): "Fallback lists" in Fonts (text set in the first family the platform has, a character it
  lacks from the next that has it, then the system's fallback; what each platform does), `fontFamily` in the Style
  reference.
- The platforms follow in pull requests of their own (macOS, iOS, Windows, GPUI, Android); Linux and the web already
  fall back along lists.

## Public API

Rust: `nsh_core::font_families`, `nsh_core::FamilyName`.

## Tests

- `style::tests::font_family_lists_name_their_families_in_order` (`nsh-core`).
- `cargo clippy -p nsh-core --all-targets -- -D warnings`: clean.
