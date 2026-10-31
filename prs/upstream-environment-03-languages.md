# Upstream PR: core: Tell apps the user's preferred languages

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/environment/03-languages` (`e8053e214`), on `pr/environment/02-time-zone`: seven commits of its own, the
  core's, then one per platform. It replaces `pr/languages/01-core` to `07-gpui`; every file is the same as in those
  branches, except `crates/gpui/Cargo.lock`: `pr/languages/07-gpui` added `jiff`'s entries there, which nothing in it
  uses (they belong to `pr/intl-time-zone/01-js`, which adds them itself), and cargo drops them on any build; this
  branch leaves them out. The literals that build an environment outside `nsh-core` (iOS's root view, the engine's
  `ui_environment` and its tests, `nsh-testing`'s) set the new field by assignment, as
  `pr/environment/00-non-exhaustive` builds them; the branch was rebuilt on it 2026-10-30 (the previous heads are
  `old-pre-nonexh/03-languages` locally).
- Size of its own commits: 40 files, +650 / −52 (code +496 / −31, tests +58 / −5, docs +96 / −16).
- Title: `core: Tell apps the user's preferred languages`.

---

**Depends on:** #TIME_ZONE (`core: Tell apps the device's time zone`), which depends on #747 and #744:
each adds to the environment's fields, the test runner's `open` options and the docs the one before it adds. Until those
are merged this PR shows their commits too; the languages' own are the last seven.

## Description

Apps know the user's locale, the region format numbers and dates follow, but not the languages the user reads, in
their order: the systems keep a list, and an app with its own translations picks its text's language from it, falling
back along it.

`languages()` of `nsh` and Rust's `languages()` (the prelude) give it, reactively, as BCP 47 tags, most preferred
first: `Environment::languages` (`platform::Languages`, at most 8, never empty, inline so that the environment stays
`Copy`), which each platform writes as the system reports it. An embedding app's own language comes first.

The PR is the core and every platform, a commit each:

- **Core**: `platform::Languages`, `Environment::languages`, `languages()`, `languages` in `nsh`. The web reports
  `navigator.languages`. `nsh/test`'s `open({ languages })` sets them (`[locale]` otherwise; pinned on real platforms)
  and `app.setLanguages(tags)` changes them in headless runs.
- **iOS**: `NSLocale.preferredLanguages` (Settings' list, the app's own language first when the user chose one for
  it), read as the environment is published: iOS restarts apps when the user changes the list.
- **Android**: the configuration's `LocaleList` (Android 13's per-app language first), after the embedding app's own
  language when it sets one; a configuration change sends it again.
- **macOS**: `NSLocale.preferredLanguages`, read again with the other settings, as on
  `NSCurrentLocaleDidChangeNotification`.
- **Windows**: the user's display languages (`GetUserPreferredUILanguages`), read again on `WM_SETTINGCHANGE`.
- **Linux**: GLib's language names: `LANGUAGE`'s list, else `LC_ALL`, `LC_MESSAGES` and `LANG`, each with its
  fallbacks.
- **GPUI**: as the native backends read it (`GetUserPreferredUILanguages` on Windows,
  `CFLocaleCopyPreferredLanguages` on macOS, gettext's `LANGUAGE` then the messages' locale on Linux).

Docs: `languages` in `website/modules/nsh.md`, "Preferred languages" in `website/docs/device-and-environment.md`,
`open`'s `languages` and `setLanguages` in `website/docs/testing-and-trace.md` (each with its zh-CN page),
`docs/dev/elements.md` and the platforms' pages.

## Public API

- `nsh`: `languages: Accessor<readonly string[]>`, the preferred languages, most preferred first.
- `nsh/test`: `open`'s `languages?: string[]` (`[locale]` by default); `App.setLanguages(tags: string[]):
  Promise<void>` (headless runs).
- `nsh-core`: `platform::Languages` (`CAPACITY`, `EN_US`, `one(locale)`, `parse(identifiers) -> Option<Languages>`,
  `preferring(&self, locale) -> Languages`, `first`, `as_slice`, `iter`; `Default` is `[en-US]`);
  `platform::Environment::languages` and `element::Environment::languages` (new fields with defaults);
  `element::languages() -> Languages` (also in the prelude), reactive.
- `nsh-engine`: `Languages` re-exported; `EnvironmentPins::languages: Option<Languages>`.
- `nsh-dev`: `TestEnvironment::languages: Option<Vec<String>>` (serialized only when set).

## How to Test

- `cargo test -p nsh-core platform`: `languages_parse_platform_identifiers_and_an_embedding_app_goes_first`.
- `cargo test -p nsh-engine --test engine rust_platform`: `components_follow_the_preferred_languages`.
- `cargo test -p nsh-testing --features bundle --test testing`: "the languages are the locale's unless open lists
  others" and "the languages follow the system's".
- `(cd crates/gpui && cargo test)`: `posix_languages_list_languages_then_messages_with_their_fallbacks`.
- Compat: `tests/compat/cases/modules` shows `Languages: en-US` and claims `covers [nsh] languages`.
- On a device: `<Text>{languages().join(", ")}</Text>`, then change the system's (or the app's) language list.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst and Android runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
