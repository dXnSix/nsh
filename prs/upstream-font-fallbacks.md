# Upstream PR: text: Fall back along fontFamily lists

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/font-fallbacks/01-core` (`ddc03e232`), six commits on `upstream/main` `4f2502cd`: the core's, then one per
  platform (iOS, Android, macOS, Windows, GPUI). Linux (Pango) and the web (the browser) already fall back along lists.
- Source: the fork's `1ca80297f` (core), `45e666ef8` (iOS), `61d9331aa` (Android), `42b85414c` (macOS), `3e2a1dd42` with
  its test fix `e7a812cfc` (Windows) and `388f7c135` with its test fix `03acc6610` (GPUI), the fixes folded into their
  platform's commit. On the fork the core commit was stacked on register-font, whose Fonts docs it extended; here it
  stands on `main`: the Fonts page gains its "Fallback lists" section alone (en, zh-CN), and the macOS and GPUI Text rows
  keep upstream's newer wording with the one clause each platform adds.
- Size: 14 files, +558 / −146.
- No new JS surface: `fontFamily` stays a string, now read as a CSS list; the Rust items are new (`font_families`,
  `FamilyName`).

---

**Depends on:** no other PR; it applies to `main` as it is.

## Description

`fontFamily` took a list only where a platform parsed one itself: Pango on Linux and the browser read CSS lists, GPUI
and Windows set text in the list's first installed family, and CoreText and Android read the whole value as one family
name, so a list gave the system font. No platform but Linux and the web fell back from one family of a list to the next
for the characters the first lacks.

`fontFamily` now takes a list, as CSS does (`"Brand, 'Noto Sans SC', sans-serif"`): text is set in the first family the
platform has, a character that family lacks comes from the next family in the list that has it, then from the system's
fallback fonts. A brand font that covers Latin only can show Chinese in a matching font, or a font the app downloads can
name a system font to show until it arrives.

The PR is the core and every platform, a commit each:

- **Core**: `nsh_core::font_families(value)`, a `fontFamily` value's families as CSS lists them, most preferred first
  (names trimmed and unquoted, generic families as `FamilyName::System`, `Serif`, `Monospace`, `Rounded`); the
  `fontFamily` prop's TS doc says it takes a list.
- **iOS** and **macOS**: the first family CoreText has is the font, the others it has its cascade list
  (`kCTFontCascadeListAttribute`), with UIKit's or AppKit's designs for the generic families.
- **Android**: the first family Android has (registered, generic or installed) is the typeface; when that one is a font
  the app registers, the registered fonts after it in the list are its fallbacks (Android 10 and later,
  `Typeface.CustomFallbackBuilder`).
- **Windows**: the list's other installed and registered families make an `IDWriteFontFallback`, in the list's order,
  then the system's fallback (one per paragraph: its first style's list).
- **GPUI**: the list's other installed families are the font's `FontFallbacks`, in order, before `TEXT_FALLBACKS`.

Docs: "Fallback lists" in `website/docs/fonts.md` and `fontFamily` in `website/modules/style-reference.md` (each with its
zh-CN page); the Text rows of `docs/dev/macos.md`, `docs/dev/windows.md` and `docs/dev/gpui.md`.

## Public API

- `nsh-core`: `font_families(value: &str) -> impl Iterator<Item = FamilyName<'_>>` (also `nsh_core::FamilyName`), a
  `fontFamily` value's families, most preferred first.
- `fontFamily` (TS and Rust's `.font_family(..)`): a CSS list of families, read in order; one family reads as before.

## How to Test

- `cargo test -p nsh-core style::tests::font_family_lists_name_their_families_in_order`.
- `cargo test -p nsh-platform-macos text::tests::a_family_list_sets_text_in_its_first_installed_family` (macOS):
  `"No Such Family, Menlo, 'Courier', sans-serif"` is Menlo with a cascade list of two.
- Windows (CI): `text::tests::a_family_list_falls_back_to_its_other_families` and the list case of
  `registered_fonts_lay_out` (registering is process-wide, so a registered family in a list is checked there).
- GPUI (CI, Linux and Windows): the list case of `registered_fonts_lay_out`.
- iOS shares macOS's code; Android: on a device, a `Text` with `"No Such Family, sans-serif"` sets text in the default
  sans-serif typeface instead of failing over to the default font for the whole value.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped: one family reads as before; a list is new.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
