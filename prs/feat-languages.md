# core: Tell apps the user's preferred languages

> **Upstream (checked 2026-11-01):** prepared as [upstream-environment-03-languages](upstream-environment-03-languages.md), not opened: stacks on the time zone PR.

Branch: `feat/languages` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `33456e82`. One commit, `83e9c1af`: `core: Tell
apps the user's preferred languages`. Opened as a pull request into the fork's `main` and merged there with a merge
commit. Not opened upstream: the owner decides and posts there.

## Summary

Apps knew the user's locale (`locale()`, the region format numbers and dates follow) but not the list of languages
the user reads, in their order: the systems keep one (Settings' language list on a phone, a desktop's UI languages,
the browser's), and an app with its own translations picks its text's language from it, falling back along it. The
locale is no stand-in: a user can read English with German dates, and the locale gives no fallbacks.

### Changes

- **`nsh-core`**: `platform::Languages`, the preferred languages as BCP 47 tags, most preferred first, at most 8,
  each once and never empty, held inline so that `Environment` stays `Copy`; `Languages::parse` takes platform
  identifiers (`Locale::parse`'s forms) and leaves out what names no language. `Environment::languages` (default
  `[en-US]`) and the reactive `ui::ambient::languages()` (in the prelude).
- An embedding app that sets its own language (`HostEnvironment::locale`) has it first in the list too.
- **`nsh-engine`**: re-exports `Languages`, copies them into the components' environment, and
  `EnvironmentPins::languages` keeps a test's list whatever the platform reports.
- **`nsh-js`**: `languages` of `nsh/device` (`Accessor<readonly string[]>`), a part of the environment.
- **`nsh-testing`, `nsh-dev`**: `open`'s `languages` option (headless; pinned on real platforms through
  `TestEnvironment::languages`), `[locale]` by default, and `app.setLanguages(tags)` (headless); a list naming no
  language is refused.
- **`nsh-platform-web`**: the browser's `navigator.languages`, the locale's first, followed on `languagechange`.
- **`nsh-platform-ios`**: its environment literal gets `[en-US]`; iOS reports the system's list in a change of its
  own, as do the other native platforms. Until then their apps read `[en-US]`.
- **Docs** (en and zh-CN): `nsh/device`'s `languages`, a "Preferred languages" section of Device and environment
  (and its embedding app's note), `open`'s `languages` and `app.setLanguages` in Testing; `docs/dev/elements.md`,
  `docs/dev/components.md`.
- **Compat**: `mod-device` shows the languages (`en-US` headless); its script covers `[nsh/device] languages`.

## Public API

- Rust: `nsh_core::platform::Languages` (`EN_US`, `CAPACITY`, `one`, `parse`, `preferring`, `first`, `as_slice`,
  `iter`), `Environment::languages`, `ui::Environment::languages`, `ui::ambient::languages()` (prelude),
  `nsh_engine::Languages`, `EnvironmentPins::languages`, `nsh_dev::protocol::TestEnvironment::languages`.
- TSX: `languages` of `nsh/device`; `nsh/test`'s `open({ languages })` and `app.setLanguages(tags)`.

A struct literal that lists every field of `Environment` (as iOS's does) needs the new field.

## Tests

- `platform::tests::languages_parse_platform_identifiers_and_an_embedding_app_goes_first` (`nsh-core`).
- `rust_platform::components_follow_the_preferred_languages` (`nsh-engine`): a component's text picks the first
  language it has, and follows the list.
- `tests/testing/environment` (`nsh-testing`): `open`'s `languages`, the `[locale]` default (`fr_FR` gives
  `fr-FR`), `setLanguages`, and the errors for lists naming no language.
- `NSH_COMPAT_APP=modules cargo xtask compat` against nsh-compat `c83987b`. nsh-compat's newest commit (`f19d590`, #12)
  reads scroll fields this fork's NSH does not have yet (`refresh_indicator`, `pull_value`): the Compat check fails on
  every fork pull request until the fork's `main` takes upstream's again.
