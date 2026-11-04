# Upstream PR: core: Tell apps the device's time zone

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/environment/02-time-zone` (`61564e75e`), on `pr/web-lifecycle/01-web`: eight commits of its own, the
  core's, one per platform, then the web's. It replaces `pr/time-zone/01-core` to `07-gpui`; every file is the same as in those
  branches. The literals that build an environment outside `nsh-core` (iOS's root view, the engine's `ui_environment`
  and its tests, `nsh-testing`'s) set the new field by assignment, as `pr/environment/00-non-exhaustive` builds them;
  the branch was rebuilt on it 2026-10-30 (the previous heads are `old-pre-nonexh/02-time-zone` locally).
- Size of its own commits: 42 files, +717 / −50 (code +582 / −29, tests +57 / −5, docs +78 / −16).
- Title: `core: Tell apps the device's time zone`. Until the web lifecycle PR is merged, GitHub shows both PRs' commits.

- 2026-11-01: #744 was closed, so this branch is rebuilt on #747's head `fab837934` (rebased on `upstream/main` `e4e464f4` without #744's commit), its seven commits replayed
  (head `796cfc1b5`). Where #744 had environments built by assignment, the new field goes into the struct
  literals upstream keeps (the engine's `ui_environment`, iOS's root view, the test runner's `open`); iOS's
  observers keep their own doc comments, and the Windows Environment row keeps #747's subclass wording. The core commit's test builds its
  environments as literals (clippy's `field_reassign_with_default`).
- 2026-11-02 (after #747 merged as `lifecycle`): rebased onto `upstream/main` `bd30b1b4` through `pr/web-lifecycle/01-web` (its own PR, `upstream-web-lifecycle.md`), `appState` renamed `lifecycle` in what the commits touch, plus an eighth commit, `web: Give browser apps the device's time zone` (the browser runtime's `timeZone`, `environment.test.mjs`). The Linux and GPUI commits' `docs/dev` Environment rows had repeated half a sentence; they now add only the time zone's sentence (GPUI's ends as the fork's `main` has it: read again whenever a window becomes the active one). The zh-CN `modules/nsh.md` line keeps the time zone in the reactive list. Previous heads: `old-pre-lifecycle/origin/*` locally.
- 2026-11-02 (after #822 merged): rebased onto `upstream/main` `828b1f13` without conflicts, patches unchanged (`git range-diff`). Previous heads: `old-pre-822/*` locally.
- 2026-11-04: rebased onto the new `pr/web-lifecycle/01-web` (`upstream/main` `57b1d7ba`): head `f1190540`. Only context moved (upstream's new prelude names and doc rows beside ours); `old-pre-899/*` locally.

---

**Depends on:** #LIFECYCLE_WEB (`web: Give browser apps their lifecycle`), which adds the browser runtime's
`environment.test.mjs` that this PR's web commit extends. Until it is merged this PR shows its commit too; the time
zone's own are the last eight.

## Description

Apps cannot learn the device's time zone. `Date` follows it, but an app that shows times in the user's zone with a
time zone library, names the zone beside a clock or saves a date with its zone needs the zone's name, and needs to
know when it changes.

`timeZone()` of `nsh` and Rust's `time_zone()` (the prelude) give it, reactively, as the system names it: an IANA name
(`Europe/Berlin`), `UTC`, or an offset zone where the system has one (Android's `GMT+01:00`). It is
`Environment::time_zone` (`platform::TimeZone`, inline so that the environment stays `Copy`), which each platform
writes as the system reports it. `Intl`'s default zone and `Date`'s local times stay as they are.

The PR is the core, every platform and the browser runtime, a commit each:

- **Core**: `platform::TimeZone` (`UTC`, `parse`, `as_str`), `Environment::time_zone`, `time_zone()`, `timeZone` in
  `nsh`. The web reports the browser's zone, read again when the page shows or gets the focus. `nsh/test`'s
  `open({ timeZone })` sets it (pinned on real platforms; an invalid name fails `open`) and `app.setTimeZone(name)`
  moves a headless app to another zone; headless apps are in UTC.
- **iOS** and **macOS**: `NSTimeZone.systemTimeZone`'s IANA name; on `NSSystemTimeZoneDidChangeNotification`
  Foundation's cached zone is reset and the environment published again.
- **Android**: `java.util.TimeZone`'s default ID, sent with the environment; `ACTION_TIMEZONE_CHANGED` gives the new
  zone, from the broadcast, which names it before the process's default zone may have caught up.
- **Windows**: the setting's zone (`GetDynamicTimeZoneInformation`'s key name) by its IANA name, from the ICU Windows
  ships (`ucal_getTimeZoneIDForWindowsID`, loaded at run time so that apps start without it, then UTC); read again on
  `WM_TIMECHANGE`.
- **Linux**: the process's own zone (`TZ`, when it names one: its local times follow it), else systemd's timedated
  (`org.freedesktop.timedate1`'s `Timezone`, followed live), else the zone `/etc/localtime` links to, else
  `/etc/timezone`'s.
- **GPUI**: as the native backends name it (Windows' setting through ICU, elsewhere `TZ`, `/etc/localtime`,
  `/etc/timezone`), read again whenever a window becomes the active one, as GPUI reports no change of it.
- **Web** (the browser runtime, where web apps' code runs): `src/browser/engine.rs` sends `timeZone` to the app's
  realm and `browser/environment.js` serves it, UTC until the engine reports.

Docs: `timeZone` in `website/modules/nsh.md`, "Time zone" in `website/docs/device-and-environment.md`, `open`'s
`timeZone` and `setTimeZone` in `website/docs/testing-and-trace.md` (each with its zh-CN page), `docs/dev/elements.md`
and the platforms' pages.

## Public API

- `nsh`: `timeZone: Accessor<string>`, the device's zone as the system names it.
- `nsh/test`: `open`'s `timeZone?: string` (an IANA name; `"UTC"` by default); `App.setTimeZone(name: string):
  Promise<void>` (headless runs).
- `nsh-core`: `platform::TimeZone` (`TimeZone::UTC`, `parse(identifier: &str) -> Option<TimeZone>`,
  `as_str(&self) -> &str`; `Default` is UTC; `Display`); `platform::Environment::time_zone` and
  `element::Environment::time_zone` (new fields with defaults); `element::time_zone() -> TimeZone` (also in the
  prelude), reactive.
- `nsh-engine`: `TimeZone` re-exported; `EnvironmentPins::time_zone: Option<TimeZone>`.
- `nsh-dev`: `TestEnvironment::time_zone: Option<String>` (serialized only when set).

## How to Test

- `cargo test -p nsh-core platform`: `time_zones_parse_platform_identifiers`.
- `cargo test -p nsh-engine --test engine rust_platform`: `components_follow_the_time_zone`.
- `cargo test -p nsh-testing --features bundle --test testing`: "the time zone follows the system's" and "an invalid
  time zone fails open".
- `node --test crates/nsh-platform-web/web/*.test.mjs`.
- Compat: `tests/compat/cases/modules` shows `Time zone: UTC` and claims `covers [nsh] timeZone`.
- On a device: `<Text>{timeZone()}</Text>`, then change the system's time zone while the app runs.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst and Android runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
