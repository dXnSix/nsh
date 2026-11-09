# Upstream PR: i18n: Make the layout direction follow the language setLocale sets

Not opened: prepared 2026-11-09 for the owner to post. The body to paste starts below the line.

- Branch: `pr/locale-direction/01-core` (local, not pushed, tracking nothing), two commits on `upstream/main`
  `87c7e8da` (#1352), the fork's `feat/locale-direction` cherry-picked without conflicts: `d1c4efb3` i18n, `17939c72`
  ios (head).
- One pull request with the iOS fix as a commit of its own: without the core's change no NSH app reaches the iOS case
  (a left-to-right layout in a host app localized right to left), and the fix means nothing without it.
- Size: 20 files, +377 / −28 (code +84 / −16, tests +275, docs +18 / −12).
- Open it from `dXnSix:pr/locale-direction/01-core` into `longbridge/nsh:main`, title (upstream squash-merges):
  `i18n: Make the layout direction follow the language setLocale sets`.
- Compatibility case: `tests/compat/cases/modules/mod-i18n-direction.tsx` (`covers [nsh/i18n] setLocale`): a row's
  boxes' frames from the left, from the right after `setLocale("ar")`, from the left after `"en"` and after `null`; a
  row with `direction: "ltr"` stays.
- Compat: `NSH_COMPAT_APP=modules cargo xtask compat` passes (`checkout_apps_run`, 32 tests), and fails as it should
  with the engine's `env.rtl = locale.rtl()` left out (`step 71 frame id:first 290 0 100 20: laid out at 0 0 100 20`;
  not committed).
- Checked locally on `87c7e8da` (Rust 1.99.1, macOS): the new engine group and unit test, the engine's `rust_platform`
  group and `locale_and_font_scale_follow_the_environment` (18 pass), `cargo test -p nsh-core --lib platform::tests`
  (10), `cargo test -p nsh-js declarations` (18); clippy `-D warnings` of `nsh-core`, `nsh-engine` and `nsh-js` (all
  targets) and of `nsh-platform-ios` for `aarch64-apple-ios` (`embed`, `dev`); nightly rustfmt `--check` on the
  changed files. On the fork's base (`71aeb06a`) also `nsh-platform-ios` for `aarch64-apple-ios-macabi`, `nsh-engine`
  and `nsh-platform-web` for wasm32 ([feat-locale-direction](feat-locale-direction.md)).
- Not run: devices, simulators and desktop windows (the iOS change is type-checked only), the browser runtime end to
  end, `cargo xtask clippy-android`, the Windows, Linux and GPUI crates (no code of theirs changes), the website's tests
  (the zh-CN pages link `#从右到左`, which #1353 rewrites to `#right-to-left`).
- Left out, for the owner: an embedding app's own locale (`AppView::set_locale`, `NshView.setLocale`,
  `nsh_app_set_locale`, through `HostEnvironment::apply`) still keeps the system's direction; following it too is one
  line there, but a change of the embedding API documented on every platform's page.

---

**Depends on:** no other PR; it applies to `main` as it is.

## Description

`setLocale` (#1234; Rust `set_locale`) shows the app in a language the user picks inside it: `locale()`, the
preferred languages, `Intl` and `t` follow it. The layout direction does not. An app that switches into Arabic or
Hebrew keeps laying out left to right, and one that switches into English on an Arabic device keeps laying out right
to left, with its rows, logical properties, `rtl:` classes and native widgets the wrong way round.

Now the direction follows the locale `setLocale` sets: right to left for a locale written so, left to right for the
others, the platform's direction again with `setLocale(null)`. A view's own `direction` style still sets its subtree's.

```tsx
import { setLocale } from "nsh/i18n";

setLocale("ar"); // Arabic, laid out right to left
setLocale("en-GB"); // left to right, on an Arabic device too
setLocale(null); // the device's language and direction
```

**Where.** The engine already writes the app's locale over the platform's environment (`apply_environment`, as an
embedding app's `HostEnvironment` is). It now writes the locale's direction there too, so everything that follows the
environment's `rtl` follows: the root's `LayoutDirection` that JS sends (QuickJS and the browser runtime), the root the
core sets for a Rust app, `layout_rtl()` and the controls that read it, `rtl:` classes, directed alignment and the
platforms' mirroring. The direction arrives in the same batch as the texts the new language changes.

**The rule** (`Locale::rtl`, doc-hidden). NSH's two existing derivations are platform code the core cannot reach: the
web page's `rightToLeft` in `nsh.js` (the browser's `Intl.Locale` text info, else its likely script, else a language
list) and GPUI's `rtl_language`, a list of language codes that ignores the script (`az-Arab`, `sd-Deva`) and the
region (`pa-PK`). The core takes the web page's rule with CLDR's data:

- a script the tag names decides: CLDR's 37 right-to-left scripts (`Arab`, `Hebr`, `Thaa`, `Syrc`, `Nkoo`, `Adlm`,
  `Rohg`, ... and the historic ones);
- without one, the script its language is most likely written in, in its region, as CLDR's likely subtags give it:
  the 20 languages CLDR has locale data for whose likely script is right to left (`ar ars bal ckb dv fa he ks lrc mzn
  nqo ps sd sdh syr ug ur yi`, with `iw` and `ji`, the old codes Java reports), and the 19 language-region pairs whose
  likely script runs the other way (`pa-PK`, `uz-AF`, `az-IR`, `kk-CN`, ... are right to left; `sd-IN`, `ug-KZ` and
  `ug-MN` left to right).

It matches ICU 78 (CLDR 48, the version of the core's regional tables) on every tag we could build: each of the 276
canonical languages ICU has locale data for, and `iw` and `ji`, alone and with each of the 311 regions, and every
four-letter script with three languages, 1,457,664 tags compared with `Intl.Locale(tag).getTextInfo().direction` in
Node. Not covered: aliases ICU canonicalizes first (`ara`, `heb`; platforms report canonical tags) and languages CLDR
has no locale data for.

**iOS** (a commit of its own). A view whose `semanticContentAttribute` is unspecified takes UIKit's own direction, the
app's localization's, and NSH left a left-to-right view unspecified. In a host app localized in Arabic or Hebrew, an
NSH app switched into English would get right-to-left text fields, navigation bars and back gestures inside its
left-to-right layout. `mirror` now forces left to right where UIKit's own direction is right to left
(`userInterfaceLayoutDirectionForSemanticContentAttribute(.unspecified)`, read once), on the surface's views when the
direction changes and on the ones it makes after. Elsewhere nothing changes; NSH's packaged iOS apps are localized in
English only. The other platforms follow `MountItem::LayoutDirection` already (Android sets LTR or RTL explicitly).

Docs: `setLocale` in `website/modules/nsh-i18n.md` and `website/docs/i18n.md`, "Right to left" in
`website/docs/device-and-environment.md` (each with its zh-CN page); `docs/dev/elements.md` and `docs/dev/js-modules.md`.

## Public API

No new API. Behavior change:

- `nsh/i18n`'s `setLocale(locale: string | null): void` and `nsh_core::i18n::set_locale(Option<Locale>)` (also in
  `prelude`) now set the layout direction to the locale's: right to left for one written so, left to right otherwise;
  `null` / `None` return to the platform's.
- `nsh_core::platform::Locale::rtl(&self) -> bool` is new and `#[doc(hidden)]`: the rule the engine applies, for NSH's
  crates.

## How to Test

- `cargo test -p nsh-engine --test engine locale_direction`: a TSX app's root and a row turn right to left with
  `setLocale("ar-EG")`, in the batch that shows the new locale, and back with `"en-GB"` and `null`, while a row with
  `direction: "ltr"` stays; on a right-to-left platform English turns it left to right and `null` back; a Rust app's
  `layout_rtl()` and root follow `set_locale(he)` and `None`. The three fail without the engine's change.
- `cargo test -p nsh-core --lib platform::tests`: `a_locale_is_right_to_left_by_its_script_or_its_likely_one`.
- Compat: `tests/compat/cases/modules`' `mod-i18n-direction` claims `covers [nsh/i18n] setLocale`
  (`NSH_COMPAT_APP=modules cargo xtask compat`).
- The engine's `rust_platform` group; `cargo test -p nsh-js declarations`.
- On a device or desktop: an app with a language picker calling `setLocale("ar")` mirrors at once and `setLocale(null)`
  restores it; on iOS, inside a host app localized in Arabic, `setLocale("en")` lays the navigation bar and text fields
  out left to right.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the iOS and wasm32 runs included
  (Android, Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no demo changes; the engine's tests run TSX and Rust
  apps, and no device run was made.
