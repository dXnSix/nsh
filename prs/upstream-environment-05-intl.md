# Upstream PR: js: Write Intl's dates in the user's time zone and hour cycle

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/environment/05-intl` (`0d34b5133`), on `pr/environment/04-regional`: two commits of its own, `js: Write
  Intl's dates in the environment's zone, with a time zone database`, then `js: Write Intl's times in the user's hour
  cycle by default`. It replaces `pr/intl-time-zone/01-js` and `pr/intl-hour-cycle/01-js` (the same commits, as the
  latter carried both).
- Size of its own commits: 16 files, +304 / −38 (code +200 / −22, two tests among it; docs +20 / −16; lock files +84:
  `jiff` and `jiff-tzdb` in `Cargo.lock` and in GPUI's).
- Title: `js: Write Intl's dates in the user's time zone and hour cycle`.
- Rebased 2026-10-30 for upstream #603 (web apps run on the browser's engine): the time zone commit no longer counts
  the database in the web runtime, and the docs say a browser uses its own database.

---

**Depends on:** #REGIONAL (`core: Tell apps the user's regional preferences`), which depends on #LANGUAGES,
#TIME_ZONE and #APP_STATE: `Intl` writes dates in `timeZone()` and times in `hourCycle()`, which those PRs add. Until
they are merged this PR shows their commits too; its own are the last two.

## Description

`nsh-js`'s `Intl` writes dates without the user's settings:

- **No time zone database.** It writes dates in the process's local zone, UTC or a fixed offset, and refuses every
  other zone. On Android and Windows, which name no local zone in `TZ` or `/etc/localtime`,
  `resolvedOptions().timeZone` is an offset and `{ timeZone: timeZone() }` throws; `{ timeZone: "Asia/Tokyo" }` throws
  on every native platform; headless tests write the machine's local times whatever `open`'s `timeZone` says.
- **The locale's hour cycle.** It writes times in the locale's own hour cycle, so an app shows a 12-hour clock to a
  user who switched to 24-hour time unless it passes `hourCycle()` itself.

Two commits, one per setting:

- **Time zone.** `nsh-js` carries the IANA database (`jiff`'s, bundled: the same rules on every native platform).
  `timeZone` takes any of its zones, in any case, with their offsets at each instant, daylight saving time included.
  Without `timeZone`, dates are in the environment's zone (`timeZone()`), which `resolvedOptions()` names; a name the
  database does not know keeps QuickJS's local offsets. `Date`'s own local times (`getHours()`, `toString()`) stay as
  they are. The bundled data is about 200 KB of each native host.
- **Hour cycle.** The device's locale's times follow the user's hour cycle (`Environment::regional`):
  `toLocaleTimeString()` and `Intl.DateTimeFormat()` without a locale, or with the device's, follow `hourCycle()`, and
  `resolvedOptions()` names it. Another locale's times keep that locale's cycle, and an `hourCycle` or `hour12` option
  wins.

Web apps run on the browser's own `Intl`: its database, and its hour cycle for its language, which is what
`hourCycle()` reports there.

Docs: `Intl`'s dates in `website/modules/globals.md`, the time zone and regional preferences sections of
`website/docs/device-and-environment.md` (the hour cycle example now leaves the option out), `timeZone` and
`hourCycle` in `website/modules/nsh.md`, and `open`'s `timeZone` in `website/docs/testing-and-trace.md` (each with its
zh-CN page).

## Public API

No new items: `Intl.DateTimeFormat`'s `timeZone` option takes the IANA zones the standard allows, and its defaults
for the zone and the hour cycle are the user's. `nsh-js` depends on `jiff` (0.2, `std` and `tzdb-bundle-always`, no
default features).

## Breaking Changes

Nothing apps use is removed; two defaults change, toward the user's settings:

- Dates that `Intl` writes without `timeZone` are in `timeZone()`'s zone: on the native platforms that is the
  system's zone, as before, and in headless runs the test's (`UTC` unless `open` sets `timeZone`), where it was the
  machine's local zone.
- Times that `Intl` writes for the device's locale without `hourCycle` or `hour12` follow `hourCycle()` where they
  followed the locale's region; they differ only where the user chose another cycle than the region's. Headless runs
  keep the locale's region's unless `open` sets `hourCycle`.

A headless test on a machine in Berlin, at 15:04 UTC in summer (CEST):

```diff
  new Date(at).toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" })
- "5:04 PM"   // the machine's zone
+ "3:04 PM"   // the test's zone, UTC unless open({ timeZone }) sets another
```

An en-US device whose user switched to 24-hour time:

```diff
  new Date(at).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
- "3:04 PM"   // the region's 12-hour cycle
+ "15:04"     // the user's (hourCycle() is "h23")
```

Passing `timeZone` or `hourCycle` keeps a fixed output, as before.

## How to Test

- `cargo test -p nsh-js intl_formats_dates_in_any_zone_and_the_environments`: Tokyo, Kolkata (named in lower case),
  Berlin, Buenos Aires, `timeZoneName`, an unknown zone (a `RangeError`), and the default following the environment
  (`UTC`, `America/New_York`, `GMT+01:00`).
- `cargo test -p nsh-js intl_writes_times_in_the_users_hour_cycle`: en-US times in `h12`, then in `h23` once the user
  switches to 24-hour time (`resolvedOptions().hourCycle` with them), `ja-JP` times in Japan's own cycle, and an
  explicit `hourCycle: "h12"` winning.
- `cargo test -p nsh-testing --features bundle --test testing`: the environment app under `open({ timeZone })`.
- In an app on Android or Windows: `new Date().toLocaleTimeString()`, `{ timeZone: "Asia/Tokyo" }`, and the time
  again after switching 24-hour time in the system's settings.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst run included
  (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped (see Breaking Changes for the two defaults).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
