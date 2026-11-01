# android: Report the user's regional preferences

> **Upstream (checked 2026-11-01):** in the prepared regional PR (not opened), this platform's commit.

Branch: `feat/android-regional` on `github.com/dXnSix/nsh`, stacked on `feat/android-languages` (#65), whose lines it
touches, with `feat/regional` (`core: Tell apps the user's regional preferences`, #71) merged in. One commit,
`837ea05d`: `android: Report the user's regional preferences`. Opened as a pull request into the fork's `main`; once #65 and #71 merge, it is its own commit
only. Not opened upstream: the owner decides and posts there.

## Summary

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` (`Environment::regional`, #71)
read `en-US`'s on Android whatever the user's settings.

### Changes

- **`NshView.java`, `Native.java`, `host.rs`**: `Native.environment` takes the system's format locale's tag
  (`Locale.getDefault(Locale.Category.FORMAT)`, whatever the embedding app's language), whose Unicode extension holds
  the regional preferences Android 14 lets the user set (the first day of the week, the temperature unit:
  `en-US-u-fw-mon-mu-celsius`); `RegionalPreferences::parse` reads it over the region's. And one more value, the
  24-hour time setting (`DateFormat.is24HourFormat`), which sets the hour cycle where the region's disagrees.
- Regional preferences change the configuration, which `NshView` already follows. Switching 24-hour time sends
  `ACTION_TIME_CHANGED`, which the receiver of time zone changes (now `timeChanged`) takes too.

## Public API

None: the accessors (#71) now report Android's settings.

## Tests

- `cargo xtask clippy-android`: clean.
- Android emulator (API 35), `nsh test --target android` with a probe app showing the four, the test host's app
  locale set to `en-US-u-fw-mon-mu-celsius` and the emulator's 24-hour time on: it shows `h23 1 us celsius` (`en-US`'s
  region alone would give `h12 7 us fahrenheit`), and `open`'s `hourCycle: "h12"`, `measurementSystem: "uk"` hold over
  them. The emulator's settings were put back after.
- Not run: a change while the app runs.
