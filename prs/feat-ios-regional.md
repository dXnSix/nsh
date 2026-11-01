# ios: Report the user's regional preferences

> **Upstream (checked 2026-11-01):** in the prepared regional PR (not opened), this platform's commit.

Branch: `feat/ios-regional` on `github.com/dXnSix/nsh`, stacked on `feat/ios-languages` (#64), whose lines it
touches, with `feat/regional` (`core: Tell apps the user's regional preferences`, #71) merged in. One commit,
`929168a1`: `ios: Report the user's regional preferences`. Opened as a pull request into the fork's `main`; once #64 and #71 merge, it is its own commit
only. Not opened upstream: the owner decides and posts there.

## Summary

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` (`Environment::regional`, #71)
read `en-US`'s on iOS whatever the user's settings.

### Changes

- **`surface.rs`**: the environment carries Settings' own (General > Language & Region), over the region's
  (`RegionalPreferences::parse` of `NSLocale.currentLocale`'s identifier): the hour letter of `NSDateFormatter`'s
  format for the `j` template (24-hour time), `NSCalendar.currentCalendar.firstWeekday`, `NSLocale`'s measurement
  system and the `AppleTemperatureUnit` default, as on macOS.
- iOS changes these, and the region format, while apps run (it restarts them only for the language list), and posts
  `NSCurrentLocaleDidChangeNotification`: the view now publishes the environment again on it, so `locale()` follows
  the region format live too.
- **`Cargo.toml`**: `objc2-foundation`'s `NSCalendar`, `NSDateFormatter` and `NSFormatter` (the macOS pull request
  adds the same lines).

## Public API

None: the accessors (#71) now report iOS's settings.

## Tests

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features
  nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`: clean.
- iPhone 17 Pro Max simulator (iOS 26), its region Croatia (`h23 1 metric celsius`), `nsh test --target ios` with a
  probe app showing the four: as it is, the app shows `h23 1 metric celsius`; with the simulator's
  `AppleICUForce12HourTime`, `AppleFirstWeekday` (Sunday), `AppleMeasurementUnits` / `AppleMetricUnits` (inches) and
  `AppleTemperatureUnit` (Fahrenheit) set, it shows `h12 7 us fahrenheit`, and `open`'s `hourCycle: "h23"`,
  `firstDayOfWeek: 1` hold over them. The simulator's settings were put back after.
- Not run: a change while the app runs.
