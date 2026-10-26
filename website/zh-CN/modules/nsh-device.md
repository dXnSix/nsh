---
title: nsh/device
description: 窗口尺寸、配色方案、字体缩放、区域设置、偏好语言、地区偏好、时区、应用是否正在使用、平台信息、像素密度、布局方向，以及在应用之外打开 URL。
order: 6
---

# nsh/device

设备 API 向应用提供设备及其环境的信息。Rust 代码调用 `nsh_core::ui::prelude` 中的函数（`window()`、`dark()`、`font_scale()`、`locale()`、`languages()`、`hour_cycle()`、`first_day_of_week()`、`measurement_system()`、`temperature_unit()`、`time_zone()`、`app_state()`、`os()`、`idiom()`、`pixel_ratio()`、`set_color_scheme()` 等）；TSX 导入 `nsh/device`。窗口尺寸、配色方案、字体缩放、像素密度、区域设置、偏好语言、地区偏好、时区和应用状态是响应式的：在绑定（Rust 中传给元素的闭包，TSX 中的 JSX）、`computed` 或 `effect` 中读取它们，平台报告变化（旋转、窗口缩放、切换深色模式、修改文字大小、语言或地区设置、移到另一个时区、移到另一种像素密度的屏幕、离开应用再回来）时，读取它们的地方会随之更新。

```rust tabs
use nsh_core::{
    platform::{AppState, Appearance, HourCycle, Idiom, MeasurementSystem, Os, TemperatureUnit, TimeZone, Weekday},
    ui::prelude::*, // window、dark、font_scale、locale、languages、hour_cycle、first_day_of_week、measurement_system、temperature_unit、time_zone、app_state、os、idiom、pixel_ratio、set_color_scheme……
};
```

```tsx tabs
import { appState, colorScheme, firstDayOfWeek, fontScale, hourCycle, languages, locale, measurementSystem, openUrl, pixelRatio, platform, setColorScheme, temperatureUnit, timeZone, windowSize } from "nsh/device";
```

教程见[设备与环境](../docs/device-and-environment.md)。

## windowSize

```ts
const windowSize: Accessor<Size>;
```

界面的尺寸（点）：窗口，或嵌入时的 `NshAppView` / `NshView`。响应式。Rust：`window() -> Size<Points>`。

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

显示的外观：系统的外观，除非应用设置了自己的（`nsh.json` 的 `"appearance"`、[`setColorScheme`](#setcolorscheme)）。响应式。Rust：`dark() -> bool`；主题颜色（`AdaptiveColor`）和 `.dark(…)` 样式变体无需读取它就会跟随。Tailwind 的 `dark:` 变体也跟随它（见 [Tailwind](../docs/tailwind.md)）。

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

用户偏好的文字大小相对于默认大小的比例（`1` 为默认；iOS 上是动态字体，Android 上是字体大小设置），或嵌入它的应用设置的文字大小。响应式。Rust：`font_scale() -> f32`。除非设置了 `allowFontScaling={false}`，文本本身已经会随之缩放；当其他尺寸（例如文字旁边的图标）也需要跟随时再读取它。

## locale

```ts
const locale: Accessor<string>;
```

用户的区域设置，BCP 47 标签（`"en-US"`、`"zh-Hans-CN"`），或嵌入它的应用设置的区域设置。响应式。Rust：`locale() -> Locale`（`locale().as_str()` 即标签）。`Intl` 默认按它格式化；读取它来选择你自己的翻译文案（[设备与环境](../docs/device-and-environment.md#语言和地区)）。

## languages

```ts
const languages: Accessor<readonly string[]>;
```

用户的偏好语言，BCP 47 标签，按偏好从高到低排列（`["de-DE", "en-US"]`）：系统的列表；嵌入它的应用设置了自己的语言时，那个语言排在最前。从不为空。响应式：用户修改列表时变化。Rust：`languages() -> Languages`（`iter()`、`first()`、`as_slice()`，每项是一个 `Locale`）。用其中第一个有翻译的语言显示应用的文字，其余是它的后备（[设备与环境](../docs/device-and-environment.md#偏好语言)）。`locale()` 是区域格式，可以不同（英文文字、德国日期）。除非 `open` 设置了 `languages`，headless 测试中它是 `[locale]`。

## hourCycle

```ts
const hourCycle: Accessor<"h11" | "h12" | "h23" | "h24">;
```

用户使用 12 小时制还是 24 小时制，用 `Intl` 对小时周期的命名：`"h12"`（`12:30 PM`）、`"h23"`（`00:30`），以及较少见的 `"h11"`（`0:30 PM`）和 `"h24"`（`24:30`）。是系统的设置，否则是所在地区的。响应式。Rust：`hour_cycle() -> HourCycle`（`twelve_hour()` 表示有上午和下午）。`Intl` 按区域设置自己的小时周期书写时间：把 `hourCycle()` 作为它的 `hourCycle` 选项传入，就能按用户的习惯书写（[设备与环境](../docs/device-and-environment.md#地区偏好)）。除非 `open` 设置了 `hourCycle`，headless 测试中它是区域设置所在地区的。

## firstDayOfWeek

```ts
const firstDayOfWeek: Accessor<1 | 2 | 3 | 4 | 5 | 6 | 7>;
```

用户每周从哪一天开始，按 ISO 8601（以及 `Intl.Locale` 的 `firstDay`）给星期编号：1 是星期一，7 是星期日。即日历的第一列。是系统的设置，否则是所在地区的（美国是 7，德国是 1，埃及是 6）。响应式。Rust：`first_day_of_week() -> Weekday`（`number()`）。除非 `open` 设置了 `firstDayOfWeek`，headless 测试中它是区域设置所在地区的。

## measurementSystem

```ts
const measurementSystem: Accessor<"metric" | "us" | "uk">;
```

用户度量长度、重量和容积所用的单位：`"metric"`（公制）、`"us"`（英尺、英里、磅、加仑）或 `"uk"`（公制，但距离用英里、容量用品脱）。是系统的设置，否则是所在地区的。响应式。Rust：`measurement_system() -> MeasurementSystem`。除非 `open` 设置了 `measurementSystem`，headless 测试中它是区域设置所在地区的。

## temperatureUnit

```ts
const temperatureUnit: Accessor<"celsius" | "fahrenheit">;
```

用户读温度所用的单位：`"celsius"`（摄氏度）或 `"fahrenheit"`（华氏度）。是系统的设置，否则是所在地区的（美国及少数几个地区是华氏度）。响应式。Rust：`temperature_unit() -> TemperatureUnit`。除非 `open` 设置了 `temperatureUnit`，headless 测试中它是区域设置所在地区的。

## timeZone

```ts
const timeZone: Accessor<string>;
```

设备的时区，即系统给它的名称：IANA 名称（`"Europe/Berlin"`、`"America/New_York"`）、`"UTC"`，或系统使用偏移量时区时的名称（Android 上的 `"GMT+01:00"`）。响应式：随系统设置变化，例如用户出行时。Rust：`time_zone() -> TimeZone`（`time_zone().as_str()` 即名称）。`Intl` 默认按它书写日期，`Date` 的本地时间跟随系统时区；读取这个名称来显示它，或和日期一起保存（[设备与环境](../docs/device-and-environment.md#时区)）。除非 `open` 设置了 `timeZone`，headless 测试中它是 `"UTC"`。

## appState

```ts
const appState: Accessor<"active" | "inactive" | "background">;
```

应用是否正在使用，与 React Native 的 `AppState` 含义相同：`"active"` 表示在前台并接收输入；`"inactive"` 表示仍可见但没有焦点（焦点在另一个应用或窗口上，或系统在它上面显示了内容，例如应用切换器或来电）；`"background"` 表示不可见（在后台、已最小化、浏览器中隐藏的标签页）。显示在原生应用中的应用跟随该应用的状态。响应式。Rust：`app_state() -> AppState`（`AppState::Active`、`Inactive`、`Background`）。

在 effect 中读取它，即可暂停无人可见的工作，并在应用回来时恢复：

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

无论系统外观如何，立即以 `scheme` 显示应用：`colorScheme()`、`dark:` 类、组件的主题和平台控件（警告框、状态栏的默认样式）都随之变化；`"system"` 重新跟随系统。`nsh.json` 的 `"appearance"` 从启动时就设置它。Rust：`set_color_scheme(Appearance::Dark)`（`Appearance::Light`、`Appearance::System`）。

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

应用所运行的平台，运行期间不变。

| 成员 | Rust | 说明 |
|---|---|---|
| `os` | `os() -> Os`（`os().name()` 即 TS 的字符串） | 操作系统。iOS、iOS 模拟器和 Mac Catalyst 都报告 `"ios"`，浏览器报告 `"web"`。 |
| `version` | `os_version() -> (u16, u16)` | iOS 和桌面系统：系统版本（`"17.5"`）；Android：API 级别（`"35"`）；Web：`"0.0"`。Rust 给出主版本和次版本号，在 Android 上是 API 级别和 0。 |
| `idiom` | `idiom() -> Idiom`；`"desktop"` 用 `is_desktop()` | 设备类型：`"phone"`、`"pad"`（iPad、Android 平板）、`"mac"`（以 Mac 形态运行的 Mac Catalyst）、`"tv"`（Apple TV、Android TV）或 `"desktop"`，即适用桌面行为：macOS、Windows、Linux，以及有鼠标且窗口较宽的浏览器。 |
| `catalyst` | `idiom() == Idiom::Mac` | 以 Mac 形态运行的 Mac Catalyst。 |
| `select(spec)` | `match os() { … }` | 当前平台对应的值（见下文）。 |

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
  mobile?: T;   // iOS、Android，以及手机和平板上的浏览器
  desktop?: T;  // macOS、Windows、Linux，以及桌面浏览器
  native?: T;   // 除 Web 外的所有平台
  default?: T;
}
```

依次返回当前操作系统（`ios`、`android`、`web` 等）对应的值，否则 `mobile` / `desktop`，否则 `native`（除 Web 外的所有平台），否则 `default`，都没有则返回 `undefined`。在浏览器中，`mobile` 还是 `desktop` 由设备类型决定。Rust 代码对 `os()` 做 `match`（`mobile` / `desktop` 用 `is_desktop()`）。

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
> 对于只有某个平台才有的属性，优先使用 TSX 元素和组件的 `ios={{…}}` / `android={{…}}` 属性，而不是按 `platform` 分支（见[平台专有代码](../docs/platform-specific.md)）。

## pixelRatio

```ts
const pixelRatio: Accessor<number>;
```

每个点对应的设备像素数（大多数 iPhone 上为 `3`）。响应式：窗口移动到另一种像素密度的屏幕上时（Mac Catalyst、桌面系统或外接显示器），以及 Android 的显示大小设置改变像素密度时，它会变化。Rust：`pixel_ratio() -> f32`，取整到设备像素的长度用 `round_to_nearest_pixel(points)`；在 TSX 中这个长度是 `Math.round(x * pixelRatio()) / pixelRatio()`。

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

在应用之外打开 `url`：网页在浏览器中打开，`mailto:` 链接在邮件应用中打开，`tel:` 链接在电话应用中打开，其他应用注册的 scheme（`maps:`、宿主应用自己的 `lb://` 链接）交给对应的应用。结果表示是否有应用打开了它：没有应用处理该 scheme 时为 `false`。Rust：`nsh_core::ui::prelude` 的 `open_url(url)`，返回可在任务中等待（或直接丢弃）的回答。

```rust tabs
button()
    .child(text("联系我们"))
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
  <Text>联系我们</Text>
</Button>
```

- URL 必须是绝对地址（带 scheme）。会执行代码或访问应用自身数据的 URL，即 `javascript:`、`data:`、`blob:`、`file:`、`about:` 和 `asset:`，以及相对地址，会抛出 `TypeError`。
- 声明了能力的应用需要 `openUrl`：`"openUrl": true` 允许所有 scheme，`"openUrl": { "schemes": ["https", "mailto"] }` 只允许列出的 scheme；其他 URL 抛出 `TypeError`（见[沙箱与能力](../docs/sandbox-and-capabilities.md)）。
- 嵌入它的应用先看到这个 URL，可以改由自己的路由处理（Swift `NshAppView.onOpenURL`、Java `NshView.setOpenUrlHandler`、Rust `AppView::host().handle_open_url`；见 [iOS](../docs/ios.md#应用打开的链接) 和 [Android](../docs/android.md#应用打开的链接)），此时结果为 `true`。
- 各平台用自己的方式打开：iOS 用 `UIApplication.open`（嵌入时是宿主应用的 application），Android 用 `ACTION_VIEW` Intent，macOS 用 `NSWorkspace`，Windows 用 `ShellExecuteW`，Linux 用 `GtkUriLauncher`，GPUI 用它的 `open_url`，Web 在新标签页中打开。`nsh test` 记录这些 URL，而不真正打开。
- [`TextView`](../elements/text-view.md) 设置 `links="open"` 后，以同样的方式打开其中的链接。

## 布局方向

布局方向跟随用户的语言：在从右到左书写的语言（阿拉伯语、希伯来语等）中，应用从右到左布局，行从右侧开始排列，`left` / `right` 样式属性（`marginLeft`、`paddingRight` 等）表示起始端和结束端。修改即时生效，无需重启。

视图的 `direction` 样式（`"ltr"`、`"rtl"` 或默认的 `"inherit"`）设置其自身子树的方向；Rust 中为 `.direction(LayoutDirection::..)`。

```rust tabs
view().direction(LayoutDirection::Rtl).flex_row().gap_2().child(text("First")).child(text("Second"))
```

```tsx tabs
<View style={{ direction: "rtl", flexDirection: "row", gap: 8 }}>
  <Text>First</Text>
  <Text>Second</Text>
</View>
```

## 相关内容

- 安全区域（Rust 的 `safe_area()`；TSX 的 `safeArea()`、`safeAreaInsets()`）和软键盘（Rust 的 `keyboard_height()`；TSX 的 `keyboardFrame()`）属于 [`nsh/base`](./nsh-base.md#safearea-and-safeareainsets)。
- 屏幕阅读器和减弱动态效果的设置在 [`nsh/accessibility`](./nsh-accessibility.md) 中。
