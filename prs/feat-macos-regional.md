# macos: Report the user's regional preferences

Branch: `feat/macos-regional` on `github.com/dXnSix/nsh`, stacked on `feat/macos-languages` (#66), whose lines it
touches, with `feat/regional` (`core: Tell apps the user's regional preferences`, #71) merged in. One commit,
`1e87c964`: `macos: Report the user's regional preferences`. Opened as a pull request into the fork's `main`; once #66 and #71 merge, it is its own commit
only. Not opened upstream: the owner decides and posts there.

## Summary

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` (`Environment::regional`, #71)
read `en-US`'s on macOS whatever the user's settings.

### Changes

- **`system.rs`**: the environment carries System Settings' own, over the region's
  (`RegionalPreferences::parse` of `NSLocale.currentLocale`'s identifier):
  - the hour cycle: the hour letter of `NSDateFormatter`'s format for the `j` template (`HourCycle::of_pattern`),
    which 24-hour time changes;
  - the first day of the week: `NSCalendar.currentCalendar.firstWeekday`;
  - the measurement system: `NSLocale`'s `NSLocaleMeasurementSystem` (`Metric`, `U.S.`, `U.K.`);
  - the temperature unit: the `AppleTemperatureUnit` default (`Celsius`, `Fahrenheit`), set once the user picks one.

  They are read again with the system's other settings, on the notifications the module already follows
  (`NSCurrentLocaleDidChangeNotification` among them).
- **`Cargo.toml`**: `objc2-foundation`'s `NSCalendar`, `NSDateFormatter` and `NSFormatter` (the iOS pull request
  adds the same lines).
- **Docs**: the Environment row of `docs/dev/macos.md` and the module doc of `system.rs`.

## Public API

None: the accessors (#71) now report macOS's settings.

## Tests

- `cargo clippy --all-targets -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app --features
  nsh-platform-macos/dev -- -D warnings`: clean.
- macOS 26, `nsh test --target macos` with a probe app showing the four: it shows this Mac's
  `h23 1 metric celsius` (its region is Croatia; `en-US`'s would be `h12 7 us fahrenheit`), and `open`'s
  `hourCycle: "h12"`, `firstDayOfWeek: 7`, `temperatureUnit: "fahrenheit"` hold over them.
- Not run: a change while the app runs (it would change the user's own settings).
