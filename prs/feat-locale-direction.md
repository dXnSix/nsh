# i18n: Make the layout direction follow the language setLocale sets

> **Upstream (checked 2026-11-09):** prepared as [upstream-locale-direction](upstream-locale-direction.md)
> (`pr/locale-direction/01-core`, local), not opened: the owner decides.

Branch: `feat/locale-direction` on `github.com/dXnSix/nsh`, cut from `71aeb06a` (the newest `upstream/main` commit the
fork's `main` holds). Two commits: `da9573cf` `i18n: Make the layout direction follow the language setLocale sets`,
`d54f10ce` `ios: Keep UIKit's views left to right in an app localized right to left` (head). Local, not pushed yet: to
open as a pull request into the fork's `main` and merge there with a merge commit. Not opened upstream: the owner
decides and posts there.

Merges cleanly with `feat/app-paths` and `feat/will-terminate`. Conflicts with the fork's `main` in
`crates/nsh-engine/tests/engine/main.rs` only (`mod location;` from the location feature beside the new
`mod locale_direction;`): keep both, `locale_direction` first.

## Summary

Upstream's `setLocale` (#1234; Rust `set_locale`) changes `locale()`, the preferred languages, `Intl` and `t`, but not
the layout direction: an app that switches into Arabic or Hebrew keeps laying out left to right, and one that switches
into English on an Arabic device keeps laying out right to left. RayX hands the language an app picks at run time
(`I18nService::set_language`) to `set_locale` (`rayx_app`'s `i18n_locale.rs`), so its apps need the direction to
follow.

Now it does: right to left for a locale written so, left to right for the others, the platform's direction again with
`setLocale(null)` / `set_locale(None)`. A view's own `direction` style still sets its subtree's.

### Design

- **Where.** The engine's `apply_environment` already writes the app's locale over the platform's environment (as
  `HostEnvironment` does for an embedding app's). It now writes the locale's direction there too: one line,
  `env.rtl = locale.rtl()`. Everything that follows the environment's `rtl` follows: the root's `LayoutDirection` that
  JS sends (nsh-js `dom.set_direction`; the browser runtime's `environment.js`), the root the core sets for a Rust app
  (`surface_rtl`), `layout_rtl()` and `nsh-base`'s controls that read it, `rtl:` classes, directed alignment, and the
  platforms' mirroring through `MountItem::LayoutDirection`. The direction arrives in the same batch as the texts the
  language changed (the engine's `relocale` round). `set_locale(None)` applies the environment as the platform
  reported it, with its own `rtl`.
- **The rule** (`Locale::rtl`, doc-hidden). NSH derives a direction from a locale in two platform places, neither
  reachable from the core: the web page's `rightToLeft` (`nsh.js`: the browser's `Intl.Locale` text info, else its
  likely script among seven, else a regex of twelve languages) and GPUI's `rtl_language` (twelve language codes; it
  ignores the script, so `az-Arab` is left to right and `sd-Deva` right to left, and the region, so `pa-PK` is left to
  right). The core takes the web's rule with CLDR's data: the script the tag names decides (CLDR's 37 right-to-left
  scripts); without one, the script its language is most likely written in, in its region: 20 languages CLDR has
  locale data for (`ar ars bal ckb dv fa he ks lrc mzn nqo ps sd sdh syr ug ur yi`, and `iw`, `ji`, the old codes Java
  reports), with the 19 language-region pairs whose likely script runs the other way (`pa-PK`, `uz-AF`, `az-IR`,
  `kk-CN`, ... right to left; `sd-IN`, `ug-KZ`, `ug-MN` left to right).
- **Checked against ICU.** Node's ICU 78 (CLDR 48, the version of the core's regional tables): every canonical
  language ICU has locale data for (276) plus `iw` and `ji`, alone and with each of the 311 regions, and every
  four-letter script with three languages: 1,457,664 tags, `Intl.Locale(tag).getTextInfo().direction` equals the rule
  for all. Outside the rule: tags ICU canonicalizes first (`ara`, `heb`, `arb`; `Locale::parse` keeps them as written,
  and platforms report canonical tags) and languages without CLDR locale data (`pnb`, `rhg`, ...).
- **An explicit direction.** A view's `direction` style keeps its subtree's whatever the locale (layout, `rtl:`
  classes, alignment): the engine test pins a row `ltr` and it stays. The root's own direction is the surface's (the
  core ignores a `direction` style on the root; JS and the core set it from the environment), and `layout_rtl()` reads
  that, as it did with the system's language.
- **Embedding apps.** `AppView::set_locale` / `NshView.setLocale` / `nsh_app_set_locale` (`HostEnvironment::apply`)
  still change the locale without the direction. Making them follow too is one line in `HostEnvironment::apply`, but a
  behavior change of the embedding API with docs on every platform's page: left for the owner, not in this branch.

### Changes

- **`nsh-core`**: `platform::Locale::rtl` (doc-hidden) with `RTL_SCRIPTS` and `RTL_LANGUAGES`; module docs of
  `core/services.rs`, `layout_rtl`, `i18n::set_locale`.
- **`nsh-engine`**: `apply_environment` writes `locale.rtl()` with the app's locale.
- **`nsh-js`**: `setLocale`'s declaration names the layout direction.
- **iOS** (second commit): `mount::mirror` left a left-to-right view's `semanticContentAttribute` unspecified, which
  UIKit resolves to its own direction, the app's localization's. In a host app localized in Arabic or Hebrew an NSH app
  switched into English would get right-to-left text fields, navigation bars and back gestures in a left-to-right
  layout. It now forces left to right where UIKit's own direction is right to left
  (`UIView.userInterfaceLayoutDirectionForSemanticContentAttribute(.unspecified)`, read once), on the views the
  surface has when the direction changes and on the ones it makes after. NSH's own packaged iOS apps have only `en`
  (`CFBundleDevelopmentRegion`), so nothing changes for them. No other platform needs a change: Android sets
  `View.setLayoutDirection` LTR or RTL explicitly, and the desktops, GPUI and the web follow the mount item.
- **Compat**: `tests/compat/cases/modules/mod-i18n-direction.tsx` (`covers [nsh/i18n] setLocale`): a row's frames from
  the left, from the right after `setLocale("ar")`, from the left after `"en"` and after `null`; a row with
  `direction: "ltr"` stays.
- **Docs** (en and zh-CN): `setLocale` in `website/modules/nsh-i18n.md` and `website/docs/i18n.md`, "Right to left" in
  `website/docs/device-and-environment.md`; `docs/dev/elements.md` ("Right to left", and the iOS mirroring),
  `docs/dev/js-modules.md` (the `nsh/i18n` row).

## Public API

No new API. Behavior change: `setLocale(locale)` (`nsh/i18n`) and `set_locale(Some(locale))` (`nsh_core::i18n`, the
prelude) set the layout direction to the locale's; `null` / `None` return to the platform's.
`nsh_core::platform::Locale::rtl(&self) -> bool` is new and `#[doc(hidden)]`, the rule the engine applies.

## Tests

- New: `cargo test -p nsh-engine --test engine locale_direction` (3): `a_tsx_apps_layout_follows_the_language_it_sets`
  (Arabic turns the root and the row, in the batch that shows `ar-EG`; English and `null` turn them back; the pinned
  row stays), `an_app_on_a_right_to_left_platform_turns_and_comes_back` (English on an Arabic platform, then `null`),
  `a_rust_apps_layout_follows_the_language_it_sets` (`layout_rtl()` and the root after `set_locale(he)` and `None`).
  They fail without the engine's line. `cargo test -p nsh-core --lib platform::tests` (10) with
  `a_locale_is_right_to_left_by_its_script_or_its_likely_one`.
- Compat (on `pr/locale-direction/01-core`, [upstream-locale-direction](upstream-locale-direction.md)):
  `NSH_COMPAT_APP=modules cargo xtask compat` passes with `mod-i18n-direction`'s steps, and fails without the
  engine's line.
- Existing: the engine's `rust_platform` group (14, its direction test included) and
  `locale_and_font_scale_follow_the_environment`; `cargo test -p nsh-js declarations` (18); `cargo test -p nsh-core
  --lib i18n` (3).
- Clippy `-D warnings`: `nsh-core`, `nsh-engine`, `nsh-js` (all targets); `nsh-platform-ios` for `aarch64-apple-ios`
  and `aarch64-apple-ios-macabi` (`embed`, `dev`); `nsh-engine` for wasm32 without features, `nsh-platform-web` for
  wasm32 with its default features and with none. Nightly rustfmt `--check` on the changed files.
- Not run: any device, simulator or desktop window (the iOS change is type-checked only: no host localized right to
  left was run), the browser runtime end to end (it takes the same environment), `cargo xtask clippy-android` and the
  Windows, Linux and GPUI crates (no code of theirs changes), the website's tests (no `node_modules` here).
