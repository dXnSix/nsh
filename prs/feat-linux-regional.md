# linux: Report the user's regional preferences

Branch: `feat/linux-regional` on `github.com/dXnSix/nsh`, stacked on `feat/linux-languages` (#68), whose lines it
touches, with `feat/regional` (`core: Tell apps the user's regional preferences`, #71) merged in. One commit,
`1b5856da`: `linux: Report the user's regional preferences`. Opened as a pull request into the fork's `main`; once #68 and #71 merge, it is its own commit
only. Not opened upstream: the owner decides and posts there.

## Summary

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` (`Environment::regional`, #71)
read `en-US`'s on Linux whatever the user's settings.

### Changes

- **`system.rs`**: the environment carries the regions' of the POSIX locales (`RegionalPreferences::parse`):
  `LC_TIME`'s times and weeks and `LC_MEASUREMENT`'s units, each the first set of `LC_ALL`, its own and `LANG`, the
  locale's where none is; read once, as the locale is. GNOME's time format (`org.gnome.desktop.interface`'s
  `clock-format`, `24h` or `12h`) sets the hour cycle where the region's disagrees, live (`changed::clock-format`),
  where the schema is installed (looked up first: `gio::Settings` aborts on a schema it does not find).
- **Docs**: the Environment row of `docs/dev/linux.md` and the module doc of `system.rs`.

## Public API

None: the accessors (#71) now report Linux's settings.

## Tests

- `system::tests::regional_preferences_follow_the_posix_locales`: `LANG` alone, `LC_TIME` and `LC_MEASUREMENT` apart,
  `LC_ALL` over both, nothing set (run on macOS in a scratch crate with this branch's `nsh-core`).
- Type-checked on macOS: the new functions against `gio` 0.22.10 (pkg-config bypassed), `cargo clippy -- -D
  warnings`: clean.
- Not run on Linux: this pull request's CI builds and tests it.
