# js: Write Intl's dates in the environment's zone, with a time zone database

> **Upstream (checked 2026-11-01):** not sent (the owner's decision, 2026-10-30).

Branch: `feat/intl-time-zone` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `a30e8fd1`. One commit, `246b6284`: `js:
Write Intl's dates in the environment's zone, with a time zone database`. Opened as a pull request into the fork's
`main` and merged there with a merge commit. Not opened upstream: the owner decides and posts there.

## Summary

`nsh-js`'s `Intl` had no time zone database: it wrote dates in the process's local zone (QuickJS's `Date` offsets),
UTC or a fixed offset, named the local zone from `TZ` and `/etc/localtime`, and refused every other zone. So:

- on Android and Windows, `resolvedOptions().timeZone` was an offset (`+01:00`), and
  `new Intl.DateTimeFormat("en", { timeZone: timeZone() })` threw a `RangeError` for the device's own zone;
- `{ timeZone: "Asia/Tokyo" }` threw everywhere;
- headless tests wrote the machine's local times whatever `open`'s `timeZone` said.

### Changes

- **`nsh-js`** depends on `jiff` with its bundled IANA database (`tzdb-bundle-always`): the same rules on every
  platform, whatever the system's.
- `Intl.DateTimeFormat` (and `Date`'s `toLocale*String`s): `timeZone` takes any zone of the database, in any case
  (`asia/tokyo` resolves to `Asia/Tokyo`), as well as `UTC` and offsets; its times have the zone's offset at that
  instant, daylight saving time included. Each zone is parsed once per thread.
- Without `timeZone`, dates are written in the environment's zone (`timeZone()`, #50): a zone of the database, `UTC`,
  an offset zone as Android names one (`GMT+01:00`), or, for a name the database does not know, QuickJS's local
  offsets as before. `resolvedOptions().timeZone` names it.
- `timeZoneName` keeps its `GMT+1` style.
- **Docs** (en and zh-CN): the globals page's `Intl` dates, `nsh/device`'s `timeZone`, Device and environment's time
  zone section, Testing's `timeZone` option; `nsh/device`'s typings.

Not changed: `Date`'s own local times (`getHours`, `toString`, `getTimezoneOffset`) stay QuickJS's: the process's zone,
the browser's on the web (`nsh-platform-web` wraps QuickJS's `localtime_r`). Making them follow the environment's zone
needs QuickJS (`longbridge/rquickjs`).

## Behavior change

A date formatted without `timeZone` is written in the environment's zone instead of the process's. On a device the
two are the same zone (the platforms report the system's); in headless tests the environment's is `UTC` unless
`open` sets one, so tests' `toLocaleString()`s no longer depend on the machine's zone. On the web the environment's
zone is the browser's, as the local zone was.

## Size

The bundled database costs about 2% (measured with this change against the fork's `main`):

| Build | Before | After | Added |
|---|---|---|---|
| Web runtime (`--profile web --features bytecode-only`), raw | 13.11 MB | 13.40 MB | +290 KB |
| ... gzip / brotli | 4.70 / 3.61 MB | 4.79 / 3.67 MB | +92 / +56 KB |
| macOS release host (unstripped) | 26.30 MB | 26.85 MB | +557 KB |

The zone data is 207 KB of it. A system's own database (macOS, Linux, Android) would save that natively, at the cost
of rules that differ by platform and OS version; the owner chose the same data everywhere.

## Public API

None added: `Intl` takes the zones the standard allows.

## Tests

- Headless Chrome, `nsh test --target web` with a probe app (this Mac in `Europe/Zagreb`): a fixed instant without
  `timeZone` shows `17:04:05` (CEST) and `resolvedOptions().timeZone` is `Europe/Zagreb`, as on `main`, and
  `timeZone: "Asia/Tokyo"` shows `00:04:05`, where `main` throws a `RangeError`.
- `cargo test -p nsh-js -p nsh-core -p nsh-engine` and `cargo test -p nsh-testing --features bundle` (1313 passed),
  clippy `-D warnings` for those crates and for `nsh-platform-web` (wasm32), `cargo xtask compat` (every app,
  every recorded release).
- `host::tests::intl_formats_dates_in_any_zone_and_the_environments` (`nsh-js`): Tokyo, Berlin in summer and winter,
  Buenos Aires, a name in another case, `timeZoneName`, an unknown zone, and the default following the environment
  (`UTC`, `America/New_York`, `GMT+01:00`).
