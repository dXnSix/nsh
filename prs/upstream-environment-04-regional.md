# Upstream PR: core: Tell apps the user's regional preferences

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/environment/04-regional`, on `pr/environment/03-languages`: eight commits of its own, GPUI's boxed dev
  events (which clippy asks for once the test environment grows), the core's, then one per platform. It replaces
  `pr/regional/01-core` to `07-gpui`; every file is the same as in those branches.
- Size of its own commits: 42 files, +1703 / −85 (code +1416 / −64, tests +101 / −5, docs +186 / −16). The largest
  files: `nsh-core`'s `platform.rs` +478 (the four types, their parsing and docs, CLDR 48's region lists, a test),
  GPUI's `system.rs` +259, Linux's +113.
- Title: `core: Tell apps the user's regional preferences`.

---

**Depends on:** #LANGUAGES (`core: Tell apps the user's preferred languages`), which depends on #TIME_ZONE and
#APP_STATE: each adds to the environment's fields, the test runner's `open` options and the docs the one before it
adds. Until those are merged this PR shows their commits too; the regional preferences' own are the last eight.

## Description

Apps know the user's locale but not what the systems keep beside it of how the user reads times, weeks and
measurements: 12- or 24-hour time, the day weeks start on, the measurement system and the temperature unit (Language &
Region on Apple platforms, Android's regional preferences, Windows' region settings). A clock, a calendar or a
weather app shows them as the user set them, not as the locale's region would.

`hourCycle()`, `firstDayOfWeek()`, `measurementSystem()` and `temperatureUnit()` of `nsh` and Rust's `hour_cycle()`,
`first_day_of_week()`, `measurement_system()` and `temperature_unit()` (the prelude) give them, reactively and a memo
each: `Environment::regional` (`platform::RegionalPreferences`, with `HourCycle`, `Weekday`, `MeasurementSystem` and
`TemperatureUnit`), which each platform writes. `RegionalPreferences::of` gives a region's own, from CLDR 48's data,
and `RegionalPreferences::parse` reads a platform locale identifier's region and its Unicode extension keywords (`hc`,
`fw`, `ms`, `mu`, `rg`) in BCP 47's form and ICU's.

The PR is the core and every platform, a commit each:

- **GPUI** (first): its dev client's events are boxed in its messages; the test environment's regional preferences
  make them larger than clippy's `large_enum_variant` allows on Windows.
- **Core**: the types above, `Environment::regional`, the four accessors in Rust and `nsh`. The web reports the
  browser's hour cycle and first day of the week for its language and its likely region's units. `nsh/test`'s `open`
  sets each (the locale's region's otherwise; pinned on real platforms) and `app.setRegionalPreferences` changes them
  in headless runs. The engine boxes the environment in its messages: with the preferences it is the largest by far
  (`large_enum_variant` on wasm32), and it changes rarely.
- **iOS** and **macOS**: Settings' own: the hours of `NSDateFormatter`'s format for the `j` template (which 24-hour
  time changes), `NSCalendar`'s first weekday, `NSLocale`'s measurement system and the `AppleTemperatureUnit` default,
  over the region's; published again on `NSCurrentLocaleDidChangeNotification` (on iOS the locale now follows the region
  format live too).
- **Android**: the system's format locale's tag, whatever the embedding app's language (Android 14 keeps the user's
  first day and temperature unit as its Unicode extension, `en-US-u-fw-mon-mu-celsius`), and the 24-hour setting
  (`DateFormat.is24HourFormat`); switching it sends `ACTION_TIME_CHANGED`, which the time zone's receiver now takes
  too.
- **Windows**: the regional format with the user's changes (`GetLocaleInfoEx`: `LOCALE_STIMEFORMAT`'s hour letter,
  `LOCALE_IFIRSTDAYOFWEEK`, `LOCALE_IMEASURE`), over the region's; the temperature unit is the region's. Read again on
  `WM_SETTINGCHANGE`.
- **Linux**: the POSIX locales' regions (`LC_TIME`'s times and weeks, `LC_MEASUREMENT`'s units); GNOME's
  `clock-format` sets the hour cycle where the region's disagrees, live, where the schema is installed.
- **GPUI**: as the native backends read them on each desktop.

Docs: the four accessors in `website/modules/nsh.md`, "Regional preferences" in
`website/docs/device-and-environment.md`, `open`'s options and `setRegionalPreferences` in
`website/docs/testing-and-trace.md` (each with its zh-CN page), `docs/dev/elements.md` and the platforms' pages.

## Public API

- `nsh`: `hourCycle: Accessor<"h11" | "h12" | "h23" | "h24">`, `firstDayOfWeek: Accessor<1 | 2 | 3 | 4 | 5 | 6 | 7>`
  (1 is Monday), `measurementSystem: Accessor<"metric" | "us" | "uk">`, `temperatureUnit: Accessor<"celsius" |
  "fahrenheit">`.
- `nsh/test`: `open`'s `hourCycle?`, `firstDayOfWeek?`, `measurementSystem?`, `temperatureUnit?` (the locale's
  region's by default); `App.setRegionalPreferences(preferences: { hourCycle?, firstDayOfWeek?, measurementSystem?,
  temperatureUnit? }): Promise<void>` (headless runs; the others stay).
- `nsh-core` (`platform`): `RegionalPreferences { hour_cycle, first_day_of_week, measurement_system, temperature_unit }`
  (`EN_US`, `of(locale)`, `parse(identifier)`; `Default` is en-US's); `HourCycle` (`H11`, `H12`, `H23`, `H24`; `name`,
  `parse`, `of_pattern`, `twelve_hour`); `Weekday` (`number`, `from_number`); `MeasurementSystem` (`Metric`, `Us`, `Uk`;
  `name`, `parse`); `TemperatureUnit` (`Celsius`, `Fahrenheit`; `name`, `parse`); `Locale::language`,
  `Locale::region`; `Environment::regional`.
- `nsh-core` (`element`, and the prelude): `hour_cycle()`, `first_day_of_week()`, `measurement_system()`,
  `temperature_unit()`, reactive; `Environment::regional`.
- `nsh-engine`: the types re-exported; `EnvironmentPins::hour_cycle`, `first_day_of_week`, `measurement_system`,
  `temperature_unit` (each an `Option`).
- `nsh-dev`: `TestEnvironment::hour_cycle`, `first_day_of_week`, `measurement_system`, `temperature_unit` (each an
  `Option`, serialized only when set).

## How to Test

- `cargo test -p nsh-core platform`: `regional_preferences_are_the_regions_unless_the_locale_sets_others`.
- `cargo test -p nsh-engine --test engine rust_platform`: `components_follow_the_regional_preferences` (a change of
  one preference reruns only its readers).
- `cargo test -p nsh-testing --features bundle --test testing`: "the regional preferences follow the system's".
- `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown --features compiler -- -D warnings`: the boxed
  environment.
- Linux and GPUI: `regional_preferences_follow_the_posix_locales` (`cargo test -p nsh-platform-linux` on Linux,
  `(cd crates/gpui && cargo test)`).
- Compat: `tests/compat/cases/modules` shows `Regional: h12, week from 7, us, fahrenheit` and claims the four exports.
- On a device: switch 24-hour time, the first day of the week and the temperature unit in the system's settings while
  the app shows them.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst and Android runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
