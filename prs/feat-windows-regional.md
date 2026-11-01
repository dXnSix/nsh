# windows: Report the user's regional preferences

> **Upstream (checked 2026-11-01):** in the prepared regional PR (not opened), this platform's commit.

Branch: `feat/windows-regional` on `github.com/dXnSix/nsh`, stacked on `feat/windows-languages` (#67), whose lines it
touches, with `feat/regional` (`core: Tell apps the user's regional preferences`, #71) merged in. One commit,
`3e0673fe`: `windows: Report the user's regional preferences`. Opened as a pull request into the fork's `main`; once #67 and #71 merge, it is its own commit
only. Not opened upstream: the owner decides and posts there.

## Summary

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` (`Environment::regional`, #71)
read `en-US`'s on Windows whatever the user's settings.

### Changes

- **`system.rs`**: the environment carries the regional format's own, with the user's changes to it
  (`GetLocaleInfoEx` of the user's default locale), over the region's (`RegionalPreferences::parse` of the locale
  name): the hour letter of its time format (`LOCALE_STIMEFORMAT`, `HourCycle::of_pattern`), its first day of the
  week (`LOCALE_IFIRSTDAYOFWEEK`) and its measurement system (`LOCALE_IMEASURE`: metric or US; metric keeps the
  UK's units, which Windows does not tell apart). Windows has no temperature unit setting: it is the region's.
  They are re-read on every `WM_SETTINGCHANGE`, as the other settings are.
- **Docs**: the Environment row of `docs/dev/windows.md` and the module doc of `system.rs`.

## Public API

None: the accessors (#71) now report Windows' settings.

## Tests

- Type-checked on macOS: the new functions in a scratch crate with `windows` 0.62 and this branch's `nsh-core`,
  `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings`: clean.
- Not run on Windows: this pull request's CI builds and tests it.
