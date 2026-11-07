# Upstream PR: text: Fall back along fontFamily lists

Opened 2026-11-01 as [longbridge/nsh#773](https://github.com/longbridge/nsh/pull/773), at the owner's decision; the body
below the line is the one posted. Fork-tested first: Core, Rust application, Examples, Windows, Linux and GPUI all
green on `ddc03e232`.

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

## Review (2026-11-01)

huacnlee requested changes (one inline comment, `r4437223029`, ChatGPT on his behalf): [P2] `split(',')` broke the valid
CSS list `'ACME, Inc', serif` into two families, in the core and in Android's own parser. Fixed in `b3434ed2f`
(`text: Keep commas inside quoted fontFamily names`): both read a quoted name to its closing quote, commas and all, and a
quoted name is always a family's name, as in CSS. `quoted_family_names_keep_their_commas` fails on the old parser; the
Fonts page says a quoted name keeps its commas (en, zh-CN). Fork first: `tmp/font-fallbacks-quotes` (Core, Rust
application, Examples, Windows, Linux, GPUI) and dXnSix/nsh#105 (Fork CI, two iOS flaky retries in unrelated tests), all
green. Pushed (fast-forward `ddc03e232..b3434ed2f`) and replied (`r4437471313`); the thread is open for the reviewer.

## Review 2 (2026-11-02)

huacnlee requested changes again (Kai, an AI agent for him): (1) Windows `family_names`, Linux `resolve_family` and GPUI
`family_candidates` still `split(',')`, so quoted names with commas broke there; Android's parser still read a quoted
`'serif'` as the generic family; (2) `FamilyName` public without `#[non_exhaustive]`; (3) the GPUI list assertion in
`registered_fonts_lay_out` passed on main; nit: Android `chained()` dropped a trailing generic family with no other
registered font.

Fixed in four fast-forward commits on `b3434ed2f`: `032fae8ca` (`FamilyName` `#[non_exhaustive]`, plus `Cursive` /
`Fantasy`, which Windows and GPUI already mapped; iOS / macOS pass them over as before), `6a17d662d` (the three platforms
read lists with `font_families`; Linux leaves a name with a comma out, since Pango splits its family string at commas;
Android `Family(name, generic)` marks only unquoted generics), `c670f69d0` (GPUI: the fallbacks start with the list's
monospace font), `d8adae82e` (Android: chain whenever the first generic family after the font is not sans-serif). Docs:
Fonts page (en, zh-CN: a cursive / fantasy row, quoted generic names, Linux's names without commas, Android's generic
fallback), `docs/dev/{gpui,windows,linux}.md`.

Fork first: `tmp/font-fallbacks-review` (Core, Linux, Windows, GPUI, Rust application, Examples, CI paths; Compat is
`if: false` upstream) and dXnSix/nsh#116 (the same commits on the fork's `main`: Fork CI's macOS, iOS, Android, web).
Locally: nightly fmt, `nsh-core` list tests, `nsh-platform-macos text::` (7), clippy core + macOS; Catalyst, Android and
GPUI clippy left to CI (disk at 43 GB free).

**Superseded upstream (2026-11-02 16:12 UTC):** huacnlee merged main into the PR branch (`c05a77ab6`), squash-merged #773
as `bd30b1b4f` at 16:08 UTC, and opened longbridge/nsh#820 (`agent/font-fallbacks-followup`) with the same five fixes. So
the commits above were never pushed to the PR branch. What #820 lacks compared with them: docs (the Fonts page still
promises that a quoted name keeps its commas, which Linux now skips; nothing on quoted generic names or Android's generic
fallback), and `cursive` / `fantasy` stay `FamilyName::Named`, so a quoted `'cursive'` still maps to Comic Sans MS on
Windows and GPUI. The user chose to stand down quietly (no comment on #820, no follow-up PR): dXnSix/nsh#116 closed
unmerged and its runs cancelled; the commits stay on `fix/font-family-review` (fork main) and `tmp/font-fallbacks-review`
(old PR base) if the docs or `Cursive` / `Fantasy` are ever wanted after #820 lands.

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
