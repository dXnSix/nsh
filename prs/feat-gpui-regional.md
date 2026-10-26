# gpui: Report the user's regional preferences

Branch: `feat/gpui-regional` on `github.com/dXnSix/nsh`, stacked on `feat/gpui-languages` (#69), whose lines it
touches, with `feat/regional` (`core: Tell apps the user's regional preferences`, #71) merged in. One commit,
`a906f1e9`: `gpui: Report the user's regional preferences`. Opened as a pull request into the fork's `main`; once #69 and #71 merge, it is its own commit
only. Not opened upstream: the owner decides and posts there.

## Summary

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` (`Environment::regional`, #71)
read `en-US`'s on GPUI whatever the user's settings.

### Changes

- **`system.rs`**: each OS backend reads them as the native backends do, over the region's
  (`RegionalPreferences::parse`):
  - macOS: the hour letter of the time format for the `j` template (`CFDateFormatterCreateDateFormatFromTemplate`),
    `CFCalendar`'s first weekday, the locale's `kCFLocaleMeasurementSystem` and the `AppleTemperatureUnit`
    preference;
  - Windows: the regional format's `LOCALE_STIMEFORMAT`, `LOCALE_IFIRSTDAYOFWEEK` and `LOCALE_IMEASURE`
    (`GetLocaleInfoEx`, with the user's changes);
  - Linux: the regions of `LC_TIME` (times, weeks) and `LC_MEASUREMENT` (units), with GNOME's `clock-format`, which
    the settings portal reports (read at start and followed with the other `org.gnome.desktop.interface` keys), over
    the hour cycle.
- **Docs**: the Environment row of `docs/dev/gpui.md`.

## Public API

None: the accessors (#71) now report each OS's settings.

## Tests

- `cargo test -p nsh-platform-gpui --features shot` on macOS: all pass, with
  `system::tests::regional_preferences_follow_the_posix_locales`; clippy `-D warnings` clean.
- macOS 26: a test printing what the backend reads (not committed) gave this Mac's `H23, Monday, Metric, Celsius`.
- Type-checked on macOS: the Windows functions for `x86_64-pc-windows-msvc`, the portal's `clock-format` against
  `zbus` 5.19.0, in scratch crates: clean.
- Not run on Windows or Linux: this pull request's CI covers them.
