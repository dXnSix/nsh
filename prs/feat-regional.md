# core: Tell apps the user's regional preferences

Branch: `feat/regional` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the user's
preferred languages`, #63), which touches the same lines. Two commits: `34ca38b4` `core: Tell apps the user's regional
preferences`, and `fd1a4f34` `gpui: Box the dev client's events` (the test setup's new fields made GPUI's dev
message cross clippy's `large_enum_variant` limit on Windows, where paths are larger). Opened as a pull request into the fork's `main`; once #63 merges, it is its own commit only. Not opened
upstream: the owner decides and posts there.

## Summary

Apps knew the user's locale but not what the systems keep beside it of how the user reads times, weeks and
measurements: 12- or 24-hour time, the day weeks start on, the measurement system and the temperature unit (Language &
Region on Apple platforms, Android's regional preferences, Windows' region settings). A clock, a calendar or a weather
app should show them as the user set them, not as the locale's region would: a user in Germany with 12-hour time, an
American whose weeks start on Monday.

### Changes

- **`nsh-core`**: `platform::RegionalPreferences` (`hour_cycle`, `first_day_of_week`, `measurement_system`,
  `temperature_unit`), with `HourCycle` (`Intl`'s `h11`, `h12`, `h23`, `h24`), `Weekday` (ISO 8601's numbers, 1 Monday
  to 7 Sunday), `MeasurementSystem` (`metric`, `us`, `uk`) and `TemperatureUnit` (`celsius`, `fahrenheit`).
  - `RegionalPreferences::of(locale)`: the region's own, from CLDR 48's `timeData`, `weekData`, `measurementData` and
    `unitPreferenceData` (a few sorted region lists, with the language exceptions CLDR has: `fr-CA` is 24-hour while
    Canada is 12-hour); the world's without a region.
  - `RegionalPreferences::parse(identifier)`: what a platform locale identifier names, its region's (or its `rg`
    keyword's) with its Unicode extension keywords `hc`, `fw`, `ms` and `mu` over them, in BCP 47's form
    (`en-US-u-fw-mon-mu-celsius`, how Android 14 keeps its regional preferences) or ICU's (`en_US@hours=h23;rg=gbzzzz`).
    The native platforms' changes use it.
  - `HourCycle::of_pattern`: the cycle of a date pattern's hour field (`h`, `H`, `K`, `k` outside quoted text), as
    Apple's date formats and Windows' time formats write them.
  - `Locale::language()` and `Locale::region()`.
  - `Environment::regional` (default `EN_US`: 12-hour, Sunday, US units, Fahrenheit) and the reactive
    `ui::ambient::hour_cycle()`, `first_day_of_week()`, `measurement_system()` and `temperature_unit()` (in the
    prelude), a memo each: a change reruns only the readers of what changed.
- **`nsh-engine`**: re-exports the types, copies them into the components' environment, and `EnvironmentPins` keeps
  each a test sets whatever the platform reports.
- **`nsh-js`**: `hourCycle`, `firstDayOfWeek`, `measurementSystem` and `temperatureUnit` of `nsh/device`, four parts of
  the environment.
- **`nsh-testing`, `nsh-dev`**: `open`'s `hourCycle`, `firstDayOfWeek`, `measurementSystem` and `temperatureUnit`
  (headless: the locale's region's otherwise; pinned on real platforms through `TestEnvironment`), and
  `app.setRegionalPreferences({ … })` (headless), which changes the ones it names. A day outside 1 to 7 is refused.
- **`nsh-platform-web`**: browsers tell no user settings of these. The page reports the browser's hour cycle
  (`Intl.DateTimeFormat`'s `resolvedOptions().hourCycle`) and first day of the week (`Intl.Locale`'s `getWeekInfo()`,
  or `weekInfo` before it) for its language, and the language with its likely region (`Intl.Locale`'s `maximize()`,
  `de` is `de-Latn-DE`), whose units it takes.
- **`nsh-platform-ios`**: its environment literal gets `EN_US`. The native platforms report the system's settings in
  changes of their own; until then their apps read `en-US`'s.
- **Docs** (en and zh-CN): the four accessors in `nsh/device`, a "Regional preferences" section of Device and
  environment, `open`'s four options and `app.setRegionalPreferences` in Testing; `docs/dev/elements.md`,
  `docs/dev/components.md`.
- **Compat**: `mod-device` shows them (`h12, week from 7, us, fahrenheit` headless); its script covers the four.

`Intl` still writes times in the locale's own hour cycle; the docs show passing `hourCycle()` as its `hourCycle`
option. Making it `Intl`'s default, as the time zone is, can follow.

## Public API

- Rust: `nsh_core::platform::{RegionalPreferences, HourCycle, Weekday, MeasurementSystem, TemperatureUnit}`
  (`RegionalPreferences::EN_US`, `of`, `parse`; `HourCycle::name`, `parse`, `of_pattern`, `twelve_hour`; `Weekday::number`,
  `from_number`; `MeasurementSystem::name`, `parse`; `TemperatureUnit::name`, `parse`), `Locale::language`,
  `Locale::region`, `Environment::regional`, `ui::Environment::regional`, `ui::ambient::{hour_cycle,
  first_day_of_week, measurement_system, temperature_unit}` (prelude), their `nsh_engine` re-exports,
  `EnvironmentPins::{hour_cycle, first_day_of_week, measurement_system, temperature_unit}`,
  `nsh_dev::protocol::TestEnvironment`'s four fields.
- TSX: `hourCycle`, `firstDayOfWeek`, `measurementSystem` and `temperatureUnit` of `nsh/device`; `nsh/test`'s
  `open({ hourCycle, firstDayOfWeek, measurementSystem, temperatureUnit })` and `app.setRegionalPreferences(…)`.

A struct literal that lists every field of `Environment` (as iOS's does) needs the new field.

## Tests

- `platform::tests::regional_preferences_are_the_regions_unless_the_locale_sets_others` (`nsh-core`): CLDR's
  defaults (the US, Germany, a language exception, Saturday and Friday weeks, the UK's units, Liberia's Celsius), and
  identifiers with keywords in both forms, `rg`, and values it does not know.
  `HourCycle::of_pattern` reads `h:mm a`, `HH 'Uhr'` and a quoted `h`.
- `platform::tests::locales_parse_platform_identifiers`: `language()` and `region()`.
- `rust_platform::components_follow_the_regional_preferences` (`nsh-engine`): a component shows a time, a temperature
  and the week as the preferences say and follows them; an effect that reads the hour cycle alone does not run again
  when only the temperature unit changes.
- `tests/testing/environment` (`nsh-testing`): `open`'s options over the locale's region's (`de_DE` with `h12` and
  `fahrenheit`), the `en-US` default, `en_GB`'s, `setRegionalPreferences`, and the errors for a day outside 1 to 7.
- `NSH_COMPAT_APP=modules cargo xtask compat`: compatible with every recorded release.
