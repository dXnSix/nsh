---
title: nsh/device
description: The window size, color scheme, font scale, locale, preferred languages, regional preferences, time zone, whether the app is in use, platform, pixel ratio, layout direction and opening URLs outside the app.
order: 6
---

# nsh/device

The device API tells the app about the device and its environment. Rust code calls the functions of `nsh_core::ui::prelude` (`window()`, `dark()`, `font_scale()`, `locale()`, `languages()`, `hour_cycle()`, `first_day_of_week()`, `measurement_system()`, `temperature_unit()`, `time_zone()`, `app_state()`, `os()`, `idiom()`, `pixel_ratio()`, `set_color_scheme()`, ...); TSX imports `nsh/device`. The window size, the color scheme, the font scale, the pixel ratio, the locale, the preferred languages, the regional preferences, the time zone and the app state are reactive: read them in a binding (a closure passed to an element in Rust, JSX in TSX), `computed` or `effect`, and what reads them updates when the platform reports a change (rotation, a window resize, switching to dark mode, changing the text size, the language or the region's settings, moving to another time zone, moving to a screen of another density, leaving the app and coming back).

```rust tabs
use nsh_core::{
    platform::{AppState, Appearance, HourCycle, Idiom, MeasurementSystem, Os, TemperatureUnit, TimeZone, Weekday},
    ui::prelude::*, // window, dark, font_scale, locale, languages, hour_cycle, first_day_of_week, measurement_system, temperature_unit, time_zone, app_state, os, idiom, pixel_ratio, set_color_scheme, ...
};
```

```tsx tabs
import { appState, colorScheme, firstDayOfWeek, fontScale, hourCycle, languages, locale, measurementSystem, openUrl, pixelRatio, platform, setColorScheme, temperatureUnit, timeZone, windowSize } from "nsh/device";
```

See [Device and environment](../docs/device-and-environment.md) for the guide.

## windowSize

```ts
const windowSize: Accessor<Size>;
```

The size of the surface in points: the window, or the embedded `NshAppView` / `NshView`. Reactive. Rust: `window() -> Size<Points>`.

```rust tabs
let wide = move || window().width >= px(600.0);

view().style_when(wide, |s| s.flex_row())
```

```tsx tabs
const wide = () => windowSize().width >= 600;

<View style={{ flexDirection: wide() ? "row" : "column" }}>{/* ... */}</View>
```

## colorScheme

```ts
const colorScheme: Accessor<"light" | "dark">;
```

The appearance shown: the system's, unless the app set its own (`nsh.json` `"appearance"`, [`setColorScheme`](#setcolorscheme)). Reactive. Rust: `dark() -> bool`; theme colors (`AdaptiveColor`) and the `.dark(…)` style variant follow it without reading it. Tailwind's `dark:` variant follows it too (see [Tailwind](../docs/tailwind.md)).

```rust tabs
view().flex_1().bg(move || if dark() { black() } else { white() })
```

```tsx tabs
const dark = () => colorScheme() === "dark";

<View style={{ flex: 1, backgroundColor: dark() ? "#000" : "#fff" }} />
```

## fontScale

```ts
const fontScale: Accessor<number>;
```

The user's preferred text size relative to the default (`1` is the default size; Dynamic Type on iOS, font size on Android), or the text size the embedding app sets in its place. Reactive. Rust: `font_scale() -> f32`. Text already scales with it unless `allowFontScaling={false}`; read it when other sizes should follow, such as icons next to text.

## locale

```ts
const locale: Accessor<string>;
```

The user's locale as a BCP 47 tag (`"en-US"`, `"zh-Hans-CN"`), or the locale the embedding app sets in its place. Reactive. Rust: `locale() -> Locale` (`locale().as_str()` is the tag). `Intl` formats for it by default; read it to choose the strings of your own translations ([Device and environment](../docs/device-and-environment.md#language-and-region)).

## languages

```ts
const languages: Accessor<readonly string[]>;
```

The user's preferred languages as BCP 47 tags, most preferred first (`["de-DE", "en-US"]`): the system's list, with the embedding app's own language first when it sets one. Never empty. Reactive: it changes when the user changes the list. Rust: `languages() -> Languages` (`iter()`, `first()`, `as_slice()`, each a `Locale`). Show the app's text in the first of them it has a translation for: the others are its fallbacks ([Device and environment](../docs/device-and-environment.md#preferred-languages)). `locale()` is the region format, which can differ (English text, German dates). Headless tests have `[locale]` unless `open` sets `languages`.

## hourCycle

```ts
const hourCycle: Accessor<"h11" | "h12" | "h23" | "h24">;
```

Whether the user has 12- or 24-hour time, as `Intl` names its hour cycles: `"h12"` (`12:30 PM`), `"h23"` (`00:30`), and the rarer `"h11"` (`0:30 PM`) and `"h24"` (`24:30`). The system's setting, else the region's. Reactive. Rust: `hour_cycle() -> HourCycle` (`twelve_hour()` for AM and PM). `Intl` writes times in the locale's own hour cycle: pass `hourCycle()` as its `hourCycle` option to write them as the user reads them ([Device and environment](../docs/device-and-environment.md#regional-preferences)). Headless tests have the locale's region's unless `open` sets `hourCycle`.

## firstDayOfWeek

```ts
const firstDayOfWeek: Accessor<1 | 2 | 3 | 4 | 5 | 6 | 7>;
```

The day the user's weeks start on, as ISO 8601 numbers days (and `Intl.Locale`'s `firstDay`): 1 for Monday to 7 for Sunday. A calendar's first column. The system's setting, else the region's (7 in the US, 1 in Germany, 6 in Egypt). Reactive. Rust: `first_day_of_week() -> Weekday` (`number()`). Headless tests have the locale's region's unless `open` sets `firstDayOfWeek`.

## measurementSystem

```ts
const measurementSystem: Accessor<"metric" | "us" | "uk">;
```

The units the user measures lengths, weights and volumes in: `"metric"`, `"us"` (feet, miles, pounds, gallons) or `"uk"` (metric, with miles and pints). The system's setting, else the region's. Reactive. Rust: `measurement_system() -> MeasurementSystem`. Headless tests have the locale's region's unless `open` sets `measurementSystem`.

## temperatureUnit

```ts
const temperatureUnit: Accessor<"celsius" | "fahrenheit">;
```

The unit the user reads temperatures in: `"celsius"` or `"fahrenheit"`. The system's setting, else the region's (Fahrenheit in the US and a few others). Reactive. Rust: `temperature_unit() -> TemperatureUnit`. Headless tests have the locale's region's unless `open` sets `temperatureUnit`.

## timeZone

```ts
const timeZone: Accessor<string>;
```

The device's time zone as the system names it: an IANA name (`"Europe/Berlin"`, `"America/New_York"`), `"UTC"`, or an offset zone where the system has one (`"GMT+01:00"` on Android). Reactive: it changes with the system's setting, as when the user travels. Rust: `time_zone() -> TimeZone` (`time_zone().as_str()` is the name). `Intl` writes dates in it by default, and `Date`'s local times follow the system's zone; read the name to show it or to store it with a date ([Device and environment](../docs/device-and-environment.md#time-zone)). Headless tests are in `"UTC"` unless `open` sets `timeZone`.

## appState

```ts
const appState: Accessor<"active" | "inactive" | "background">;
```

Whether the app is in use, in React Native's `AppState` terms: `"active"` in the foreground and taking input, `"inactive"` while visible without the focus (another app or window has it, or the system shows something over it, such as the app switcher or an incoming call), `"background"` while not visible (in the background, minimized, a hidden browser tab). An app shown inside a native app follows that app's. Reactive. Rust: `app_state() -> AppState` (`AppState::Active`, `Inactive`, `Background`).

Read it in an effect to pause work nobody sees and resume it when the app comes back:

```rust tabs
effect(move || {
    if app_state() == AppState::Background {
        feed.pause();
    } else {
        feed.resume();
    }
});
```

```tsx tabs
effect(() => {
  if (appState() === "background") feed.pause();
  else feed.resume();
});
```

## setColorScheme

```ts
function setColorScheme(scheme: "light" | "dark" | "system"): void;
```

Shows the app in `scheme` whatever the system's appearance, at once: `colorScheme()`, `dark:` classes, the components' theme and the platform's controls (alerts, the status bar's default style) follow it; `"system"` follows the system again. `nsh.json` `"appearance"` sets it from the start. Rust: `set_color_scheme(Appearance::Dark)` (`Appearance::Light`, `Appearance::System`).

## platform

```ts
const platform: Platform;

interface Platform {
  readonly os: "ios" | "android" | "macos" | "windows" | "linux" | "web";
  readonly version: string;
  readonly idiom: "phone" | "pad" | "mac" | "tv" | "desktop";
  readonly catalyst: boolean;
  select<T>(spec: PlatformSelectSpec<T>): T | undefined;
}
```

The platform the app runs on, constant while it runs.

| Member | Rust | Description |
|---|---|---|
| `os` | `os() -> Os` (`os().name()` is the TS string) | The operating system. iOS, the iOS simulator and Mac Catalyst report `"ios"`, browsers `"web"`. |
| `version` | `os_version() -> (u16, u16)` | iOS and desktops: the OS version (`"17.5"`); Android: the API level (`"35"`); the web: `"0.0"`. Rust gives the major and minor version, or the API level and 0 on Android. |
| `idiom` | `idiom() -> Idiom`; `is_desktop()` for `"desktop"` | The kind of device: `"phone"`, `"pad"` (an iPad, an Android tablet), `"mac"` (Mac Catalyst in the Mac idiom), `"tv"` (Apple TV, Android TV) or `"desktop"`, where desktop behavior applies: macOS, Windows, Linux, and browsers with a mouse and a wide window. |
| `catalyst` | `idiom() == Idiom::Mac` | Mac Catalyst in the Mac idiom. |
| `select(spec)` | `match os() { … }` | The value for this platform (see below). |

```rust tabs
let columns = if is_desktop() { 3 } else { 1 };
```

```tsx tabs
const columns = platform.idiom === "desktop" ? 3 : 1;
```

### platform.select

```ts
interface PlatformSelectSpec<T> {
  ios?: T;
  android?: T;
  macos?: T;
  windows?: T;
  linux?: T;
  web?: T;
  mobile?: T;   // iOS, Android, and phone and tablet browsers
  desktop?: T;  // macOS, Windows, Linux, and desktop browsers
  native?: T;   // every platform but the web
  default?: T;
}
```

Returns the value for the current OS (`ios`, `android`, `web`, ...), else `mobile` / `desktop`, else `native` (every platform but the web), else `default`, else `undefined`. In a browser, `mobile` or `desktop` follows the idiom. Rust code matches on `os()` (and `is_desktop()` for `mobile` / `desktop`).

```rust tabs
use nsh_core::platform::Os;

let card = view().p_4();
let card = match os() {
    Os::Ios => card.shadow_color(black()).shadow_opacity(0.15).shadow_radius(px(8.0)),
    Os::Android => card.elevation(px(4.0)),
    _ => card,
};
```

```tsx tabs
const shadow = platform.select({
  ios: { shadowColor: "#000", shadowOpacity: 0.15, shadowRadius: 8 },
  android: { elevation: 4 },
});
```

> [!TIP]
> For props that only one platform has, prefer the `ios={{…}}` / `android={{…}}` props of TSX elements and components to branching on `platform` (see [Platform-specific code](../docs/platform-specific.md)).

## pixelRatio

```ts
const pixelRatio: Accessor<number>;
```

Device pixels per point (e.g. `3` on most iPhones). Reactive: it changes when the window moves to a screen of another density (on Mac Catalyst, the desktops, or an external display) and when Android's display size setting changes the density. Rust: `pixel_ratio() -> f32`, and `round_to_nearest_pixel(points)` for a length on whole device pixels; in TSX that length is `Math.round(x * pixelRatio()) / pixelRatio()`.

```rust tabs
let hairline = move || px(1.0 / pixel_ratio());
```

```tsx tabs
const hairline = () => 1 / pixelRatio();
```

## openUrl

```ts
function openUrl(url: string): Promise<boolean>;
```

Opens `url` outside the app: a web page in the browser, a `mailto:` link in the mail app, a `tel:` link in the phone app, and any scheme another app registered (`maps:`, a host app's own `lb://` links). It resolves to whether something opened it: `false` when no app handles the scheme. Rust: `open_url(url)` of `nsh_core::ui::prelude`, an answer to await in a task (or to drop).

```rust tabs
button()
    .child(text("Contact us"))
    .on_press(|| {
        spawn(async {
            if !open_url("mailto:help@example.com").await {
                log::warn!("no mail app");
            }
        });
    })
```

```tsx tabs
<Button
  onPress={async () => {
    if (!(await openUrl("mailto:help@example.com"))) {
      console.warn("no mail app");
    }
  }}
>
  <Text>Contact us</Text>
</Button>
```

- The URL is absolute (it has a scheme). URLs that would run code or reach the app's own data, `javascript:`, `data:`, `blob:`, `file:`, `about:` and `asset:`, throw a `TypeError`, as do relative addresses.
- An app that declares capabilities needs `openUrl`: `"openUrl": true` for every scheme, or `"openUrl": { "schemes": ["https", "mailto"] }` for those listed; other URLs throw a `TypeError` (see [Sandbox and capabilities](../docs/sandbox-and-capabilities.md)).
- The embedding app sees the URL first and can route it through its own router instead (Swift `NshAppView.onOpenURL`, Java `NshView.setOpenUrlHandler`, Rust `AppView::host().handle_open_url`; see [iOS](../docs/ios.md#links-the-app-opens) and [Android](../docs/android.md#links-the-app-opens)). The promise then resolves to `true`.
- Each platform opens it with its own opener: `UIApplication.open` on iOS (the host app's application when embedded), an `ACTION_VIEW` intent on Android, `NSWorkspace` on macOS, `ShellExecuteW` on Windows, `GtkUriLauncher` on Linux, GPUI's `open_url`, a new tab on the web. `nsh test` records the URLs instead of opening them.
- A [`TextView`](../elements/text-view.md) opens its links this way with `links="open"`.

## Layout direction

The layout direction follows the user's language: in a right-to-left language (Arabic, Hebrew, ...) the app lays out right to left, rows run from the right, and `left` / `right` style properties (`marginLeft`, `paddingRight`, ...) mean start and end. Changes apply at once, with no restart.

A view's `direction` style (`"ltr"`, `"rtl"` or `"inherit"`, the default) sets the direction of its own subtree; in Rust, `.direction(LayoutDirection::..)`.

```rust tabs
view().direction(LayoutDirection::Rtl).flex_row().gap_2().child(text("First")).child(text("Second"))
```

```tsx tabs
<View style={{ direction: "rtl", flexDirection: "row", gap: 8 }}>
  <Text>First</Text>
  <Text>Second</Text>
</View>
```

## Related

- Safe area insets (Rust `safe_area()`; TSX `safeArea()`, `safeAreaInsets()`) and the software keyboard (Rust `keyboard_height()`; TSX `keyboardFrame()`) are part of [`nsh/base`](./nsh-base.md#safearea-and-safeareainsets).
- Screen reader and reduced motion settings are in [`nsh/accessibility`](./nsh-accessibility.md).
