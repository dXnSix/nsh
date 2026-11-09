# Upstream PR: core: Tell apps whether the user asked for more contrast

Not opened: prepared 2026-11-09 for the owner to post. The body to paste starts below the line.

- Branch: `pr/high-contrast/01-core` (local, not pushed, tracking nothing), eight commits on `upstream/main` `84b2cf91`
  (#1357), the fork's `feat/high-contrast` cherry-picked: `7410c0be` core, `955c45b9` macos, `4e7ecf56` ios,
  `45f6ebb2` android, `98e0dafe` windows, `31d4e30b` linux, `ddc040e7` gpui, `5ba39998` web (head). Two conflicts in
  the core commit, resolved in upstream's favour: the prelude's list beside #982's `app_url` / `set_app_url` and
  #1328's `haptic` (upstream's list with `high_contrast` added, then nightly rustfmt over it), and the `nsh` row of
  `docs/dev/js-modules.md` beside #982's `UrlScheme.url` (upstream's row with `highContrast()` added).
- One pull request with a commit per platform, as #747 (`lifecycle()`) was: each platform's part is a few lines on
  the core's `Environment::high_contrast`, and none means anything without the core.
- Size: 65 files, +676 / −103 (code +289 / −61, tests +236 / −3, docs +151 / −39).
- Open it from `dXnSix:pr/high-contrast/01-core` into `longbridge/nsh:main`, title (upstream squash-merges):
  `core: Tell apps whether the user asked for more contrast`.
- Compatibility case: `tests/compat/cases/modules/mod-contrast.tsx` (`covers [nsh] highContrast`): the accessor reads
  `false` headless, and a panel's `contrast-more:bg-black` leaves its `bg-white`.
- Compat: `NSH_COMPAT_APP=modules cargo xtask compat` passes (`checkout_apps_run`, 32 tests), and fails as it should
  with the panel's expected background changed to `#000000ff` (`step 49 style id:contrast-panel backgroundColor
  #000000ff: backgroundColor is #ffffffff`; not committed).
- Checked locally on `84b2cf91` (Rust 1.99.1, macOS): the new engine group, unit test and JS tests, the test runner's
  environment test, the engine's `rust_platform` group, `cargo test -p nsh-js declarations`, the bootstrap's
  `--check`; clippy `-D warnings` of `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-dev` and `nsh-testing` (all targets),
  the macOS crates, `nsh-platform-ios` for `aarch64-apple-ios`, `nsh-platform-web` and `nsh-engine` for wasm32, GPUI's
  `nsh-platform-gpui` on macOS; nightly rustfmt `--check` on the changed files. On the fork's base (`71aeb06a`) also
  the Catalyst target, `cargo xtask clippy-android`, `javac` of the Android runtime, and the Windows and Linux calls in
  scratch crates (`windows` 0.62.2 for `x86_64-pc-windows-msvc`; gio / glib 0.22.10 and `zbus` 5.19 for Linux)
  ([feat-high-contrast](feat-high-contrast.md)).
- Not run: the Windows and Linux crates as a whole and GPUI off macOS (CI builds them), devices, simulators and desktop
  windows with the setting on (turning it on changes the machine's accessibility settings), the website's tests.
- Conflicts with `pr/app-paths/01-core` (`prelude.rs`, `nsh-engine/src/lib.rs`) and `pr/will-terminate/01-core`
  (`prelude.rs`, `nsh.test.mjs`, the Environment rows of `docs/dev/gpui.md`, `linux.md` and `windows.md`, the device
  page and the modules index in both languages): additions side by side, so whichever merges second is rebased. Clean
  with `pr/locale-direction/01-core`.
- Left for the owner: Android reads the secure setting `high_text_contrast_enabled` with a `ContentObserver`, as React
  Native does, because `AccessibilityManager`'s getter and listener for it are not in android-35; if NSH compiles
  against a newer SDK that makes them public, they could replace the observer on the devices that have them.

---

**Depends on:** no other PR; it applies to `main` as it is.

## Description

NSH tells apps about the accessibility settings they adapt to (a screen reader, less motion, the text size, the
appearance), but not whether the user asked for more contrast. Apps answer that setting by drawing borders, dividers
and secondary text in stronger colors, and NSH's colors are the app's own, so nothing does it for them.

`high_contrast()` (`nsh_core::prelude`) and `highContrast()` (`nsh`, beside `reduceMotion()`) say whether the user
asked for more contrast, reactive like the other settings, and the Tailwind variant `contrast-more:` applies while
they do:

```tsx
import { highContrast, Text, View } from "nsh";

<View class="border border-zinc-200 contrast-more:border-zinc-900">
  <Text class="text-zinc-500 contrast-more:text-zinc-900">Updated just now</Text>
</View>
<View style={{ height: 1, backgroundColor: highContrast() ? "#18181b" : "#e4e4e7" }} />
```

```rust
view()
    .border_1()
    .border_color(ZINC.shade(200))
    .style_when(high_contrast, |s| s.border_color(ZINC.shade(900)))
```

The core gets it as `Environment::high_contrast` and carries it into components' environment and the classes' (the
variant is Tailwind v4's `@media (prefers-contrast: more)`, ranked after `motion-reduce:` and before the breakpoints;
classes that use it are styled again when it changes, `deps::CONTRAST`). `nsh test`'s `open` takes `highContrast`
(false), pinned on a real platform as `reducedMotion` is.

**The name.** A state, as `reduceMotion()` is Apple's Reduce Motion. The platforms mean one thing by three names:
Apple's Increase Contrast, "high contrast" on Windows (`SPI_GETHIGHCONTRAST`, the contrast themes), Android (High
contrast text) and GNOME (`high-contrast`), and the web's `prefers-contrast: more`. "High contrast" is the most common
and Flutter's cross-platform name (`MediaQueryData.highContrast`); `increaseContrast()` would read as a command. The
variant keeps Tailwind's name; `contrast-less:` stays unknown, as no platform reports a wish for less contrast.

Each platform, a commit each, reads the setting at start and follows its changes:

- **macOS**: `NSWorkspace.accessibilityDisplayShouldIncreaseContrast`, followed through the workspace's display options
  notification, which the backend already observes for reduce motion.
- **iOS**: `UIAccessibilityDarkerSystemColorsEnabled()` (Swift's `isDarkerSystemColorsEnabled`, Increase Contrast),
  followed through `UIAccessibilityDarkerSystemColorsStatusDidChangeNotification`.
- **Android**: Settings > Accessibility > High contrast text, the secure setting `high_text_contrast_enabled` (React
  Native's `AccessibilityInfo` reads the same), followed by a `ContentObserver` on its URI while `NshView` is attached;
  `AccessibilityManager`'s getter and listener for it are not in the SDK NSH builds against. The JNI environment's
  values gain one.
- **Windows**: `SPI_GETHIGHCONTRAST`'s `HCF_HIGHCONTRASTON` (a contrast theme on), re-read on `WM_SETTINGCHANGE` with
  the other settings.
- **Linux**: the settings portal the backend follows for the color scheme: `org.freedesktop.appearance contrast` (1:
  higher), else GNOME's `org.gnome.desktop.a11y.interface high-contrast`, which GNOME's portal serves too (the source
  before `contrast`), followed through `SettingChanged`. The portal rather than gsettings, as for the color scheme: it
  answers inside a sandbox.
- **GPUI**: as each native backend reads it (macOS from the universal access `increaseContrast` preference, as GPUI
  reads reduce motion and VoiceOver), read again as the window's appearance changes or it becomes the active one.
- **Web**: `matchMedia("(prefers-contrast: more)")` and its `change` event; browsers derive it from the system's
  setting.
- **Terminal**: false.

Docs: "More contrast" in `website/docs/accessibility.md`, `highContrast` in `website/modules/nsh.md`, `contrast-more:`
in the Tailwind guide and reference, `open`'s `highContrast` in `website/docs/testing-and-trace.md`, the device page,
`windows.md` and `web.md` (each with its zh-CN page); `docs/dev/` (`elements.md`, `styles.md`, `components.md`,
`api-naming.md`, `js-modules.md`, `native-pipeline.md`, `tooling.md`, `tui.md` and the platforms' pages).

## Public API

- `nsh`:
  - `highContrast: Accessor<boolean>`: whether the user asked for more contrast (Increase Contrast, high contrast text,
    a contrast theme, `prefers-contrast: more`); reactive.
- Tailwind:
  - `contrast-more:`: applies while the user asks for more contrast (`@media (prefers-contrast: more)`).
- `nsh/test`:
  - `OpenOptions.highContrast?: boolean`: the user's contrast setting the app starts with (`false`); pinned on a real
    platform.
- `nsh-core`:
  - `element::high_contrast() -> bool` (also in `prelude`): the same as `highContrast()`; reading subscribes.
  - `platform::Environment::high_contrast: bool`: what the platform reports.
  - `element::Environment::high_contrast: bool`: what components read.
  - `tailwind::ClassEnvironment::high_contrast: bool`: what `contrast-more:` matches.
  - `tailwind::deps::CONTRAST: u16`: classes that depend on it.
- `nsh-engine`:
  - `EnvironmentPins::high_contrast: Option<bool>`: holds the setting whatever the platform reports.
- `nsh-dev`:
  - `protocol::TestEnvironment::high_contrast: Option<bool>`: a test's pinned setting, for its host.
- `nsh-testing`:
  - `OpenOptions::high_contrast: Option<bool>`: `open`'s option, in Rust.

## Breaking Changes

`platform::Environment`, `element::Environment` and `tailwind::ClassEnvironment` are not `#[non_exhaustive]`: a
struct literal that lists every field outside `nsh-core` needs the new one (literals with `..Default::default()` are
unchanged).

```diff
 let env = ClassEnvironment {
     dark: true,
     platform: Platform::Android,
     reduce_motion: false,
+    high_contrast: false,
 };
```

## How to Test

- `cargo test -p nsh-engine --test engine high_contrast`: a TSX app's `highContrast()` and a `contrast-more:`
  background follow the platform's reports, on and off; an app starts with the setting the platform reports; a Rust
  component's `high_contrast()` follows. The TSX ones fail without the engine's class environment line.
- `cargo test -p nsh-core --lib contrast_more`: the variant on and off, over `motion-reduce:`, under `md:`.
- `cargo test -p nsh-testing --features bundle --test testing open_sets_the_environment`: `open`'s `highContrast` and
  its default.
- `node --test crates/nsh-platform-web/web/nsh.test.mjs crates/nsh-platform-web/web/browser/environment.test.mjs`: the
  page follows `prefers-contrast: more` and reports its changes; the browser runtime serves `highContrast` from the
  engine's environment.
- Compat: `tests/compat/cases/modules`' `mod-contrast` claims `covers [nsh] highContrast` (`NSH_COMPAT_APP=modules
  cargo xtask compat`).
- On a device or desktop: an app with `contrast-more:` classes, with the setting turned on and off while it runs
  (Increase Contrast on macOS and iOS, High contrast text on Android, a contrast theme on Windows, High Contrast in
  GNOME, the browser on those systems).

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the iOS, Android and wasm32 runs
  included (Windows, Linux and GPUI off macOS on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no demo changes; the engine's tests run TSX and Rust
  apps, and no device run was made.
