# core: Tell apps the device's time zone

> **Upstream (checked 2026-11-01):** prepared as [upstream-environment-02-time-zone](upstream-environment-02-time-zone.md), not opened: stacks on #747.

Branch: `feat/time-zone` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `4faf2cfd`. One commit, `9a325b21`: `core: Tell
apps the device's time zone`. Opened as a pull request into the fork's `main` and merged there with a merge commit.
Not opened upstream: the owner decides and posts there.

## Summary

Apps could not learn the device's time zone. `Date` follows it, but an app that shows times in the user's zone with a
time zone library, names the zone beside a clock, or saves a date with the zone it was made in needs the zone's name,
and needs to know when it changes (the user travels, or changes the setting). `Intl`'s
`resolvedOptions().timeZone` guesses it from `TZ` and `/etc/localtime`, which Android and Windows do not have, and
says nothing when it changes.

### Changes

- **`nsh-core`**: `platform::TimeZone`, a zone's name as the system gives it (an IANA name, `Europe/Berlin`, or an
  offset zone where the system has one, Android's `GMT+01:00`), held inline so that `Environment` stays `Copy`;
  `TimeZone::parse` takes platform identifiers (POSIX `TZ`'s `:Europe/Berlin`, paths into a zone database) and
  refuses what names no zone. `Environment::time_zone` (default `UTC`) and the reactive `ui::ambient::time_zone()`
  (in the prelude).
- **`nsh-engine`**: re-exports `TimeZone`, copies the zone into the components' environment, and
  `EnvironmentPins::time_zone` keeps a test's zone whatever the platform reports.
- **`nsh-js`**: `timeZone` of `nsh/device` (`Accessor<string>`), a part of the environment like `locale`.
- **`nsh-testing`, `nsh-dev`**: `open`'s `timeZone` option (headless, and pinned on real platforms through
  `TestEnvironment::time_zone`), and `app.setTimeZone(name)` (headless), which reject what is not a zone.
- **`nsh-platform-web`**: the browser's zone (`Intl.DateTimeFormat().resolvedOptions().timeZone`), read again when
  the page shows or gets the focus: browsers tell no change of the system's zone.
- **`nsh-platform-ios`**: its environment literal gets `UTC`; iOS reports the system's zone in a change of its own,
  as do the other native platforms. Until then their apps read `UTC`.
- **Docs** (en and zh-CN): `nsh/device`'s `timeZone`, a "Time zone" section of Device and environment, `open`'s
  `timeZone` and `app.setTimeZone` in Testing; `docs/dev/elements.md`, `docs/dev/components.md`.
- **Compat**: `mod-device` shows the time zone (`UTC` headless); its script covers `[nsh/device] timeZone`.

Not changed: `Intl`'s default zone and `Date`'s local times stay what they were (the process's zone, QuickJS's).

## Public API

- Rust: `nsh_core::platform::TimeZone` (`UTC`, `parse`, `as_str`, `Display`), `Environment::time_zone`,
  `ui::Environment::time_zone`, `ui::ambient::time_zone()` (prelude), `nsh_engine::TimeZone`,
  `EnvironmentPins::time_zone`, `nsh_dev::protocol::TestEnvironment::time_zone`.
- TSX: `timeZone` of `nsh/device`; `nsh/test`'s `open({ timeZone })` and `app.setTimeZone(name)`.

A struct literal that lists every field of `Environment` (as iOS's did) needs the new field.

## Tests

- `platform::tests::time_zones_parse_platform_identifiers` (`nsh-core`).
- `rust_platform::components_follow_the_time_zone` (`nsh-engine`): a component's text and effect follow the zone.
- `tests/testing/environment` (`nsh-testing`): `open`'s `timeZone`, the `UTC` default, `setTimeZone`, and the errors
  for names that are no zone.
- `NSH_COMPAT_APP=modules cargo xtask compat`.
