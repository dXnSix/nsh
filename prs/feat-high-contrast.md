# core: Tell apps whether the user asked for more contrast

> **Upstream (checked 2026-11-09):** prepared as [upstream-high-contrast](upstream-high-contrast.md)
> (`pr/high-contrast/01-core`, local), not opened: the owner decides.

Branch: `feat/high-contrast` on `github.com/dXnSix/nsh`, cut from `71aeb06a` (the newest `upstream/main` commit the
fork's `main` holds). Eight commits, the core first, then one per platform, as #747 (`lifecycle()`) was: `e682d0ce`
`core: Tell apps whether the user asked for more contrast`, `4ca0aeb9` `macos: …`, `516a9e7f` `ios: …`, `56a90c95`
`android: …`, `06355a5f` `windows: …`, `f0be3595` `linux: …`, `708c4df5` `gpui: …`, `41a9802f` `web: …` (head). Local,
not pushed yet: to open as a pull request into the fork's `main` and merge there with a merge commit. Not opened
upstream: the owner decides and posts there.

Merges cleanly with `feat/locale-direction`. Conflicts, all of them additions side by side (keep both):

- `feat/app-paths`: `prelude.rs` only (both add names to the same `pub use` list): take both names, then nightly
  rustfmt over the list.
- `feat/will-terminate`: `prelude.rs` (as above, with `on_terminate`); `nsh.test.mjs` (both append a test at the end);
  the Environment rows of `docs/dev/gpui.md`, `linux.md` and `windows.md`; the table of
  `website/docs/device-and-environment.md` and its zh-CN page (the `on_terminate` row above the accessibility row);
  the `nsh` row of `website/modules/index.md` and its zh-CN page.
- The fork's `main`: `prelude.rs`; the `nsh` row of `docs/dev/js-modules.md`; the sentence of `open`'s pinned values
  in `website/docs/testing-and-trace.md` and its zh-CN page (the location feature's `location` beside `highContrast`).

## Summary

NSH tells apps about the accessibility settings they adapt to (a screen reader, less motion, the text size, the
appearance), but not whether the user asked for more contrast. RayX's platform snapshot (`PlatformSnapshot`, beside its
`reduced_motion`) needs it, and apps use it to draw borders, dividers and secondary text in stronger colors.

`high_contrast()` (Rust, the prelude) and `highContrast()` (`nsh`, beside `reduceMotion()`) read the new
`Environment::high_contrast`, reactive like the other settings; the Tailwind variant `contrast-more:` applies while it
holds. Each platform reads the setting at start and follows its changes.

### The name

`highContrast`, a state, as `reduceMotion()` is Apple's Reduce Motion. The platforms mean one thing by three names:
Apple's Increase Contrast (macOS, iOS), "high contrast" on Windows (`SPI_GETHIGHCONTRAST`, the contrast themes),
Android (High contrast text) and GNOME (`high-contrast`), and the web's `prefers-contrast: more`. "High contrast" is
the most common of them and Flutter's cross-platform name (`MediaQueryData.highContrast`); `increaseContrast()` would
read as a command and `prefersMoreContrast()` takes a web-only shape. The variant keeps Tailwind's name,
`contrast-more:`. `contrast-less:` stays unknown: no platform reports a wish for less contrast, and a boolean cannot
say it.

### Changes

- **`nsh-core`**: `platform::Environment::high_contrast` and `element::Environment::high_contrast`;
  `element::high_contrast()` (the prelude); `tailwind::ClassEnvironment::high_contrast`, the `contrast-more:` variant
  (Tailwind v4's `@media (prefers-contrast: more)`, ranked after `motion-reduce:` and before the breakpoints) and
  `tailwind::deps::CONTRAST`, so classes that use it are styled again when it changes.
- **`nsh-engine`**: the core's environment and class environment carry it; `EnvironmentPins::high_contrast`.
- **`nsh-js`**: `highContrast` in `ACCESSIBILITY_EXPORTS` (`nsh/accessibility`, which `nsh` serves), a part of the
  environment's signals. The browser runtime (`browser/environment.js`, `bootstrap.js` regenerated) serves the same.
- **`nsh test`**: `open`'s `highContrast` (false), pinned on a real platform as `reducedMotion` is
  (`TestEnvironment::high_contrast`, `EnvironmentPins`).
- **macOS**: `NSWorkspace.accessibilityDisplayShouldIncreaseContrast`, followed through
  `NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification`, which the backend already observes.
- **iOS**: `UIAccessibilityDarkerSystemColorsEnabled()` (Swift's `isDarkerSystemColorsEnabled`), followed through
  `UIAccessibilityDarkerSystemColorsStatusDidChangeNotification` beside the reduce motion and VoiceOver ones.
- **Android**: the secure setting `high_text_contrast_enabled` (Settings > Accessibility > High contrast text), read
  with the environment and followed by a `ContentObserver` on its URI, registered while `NshView` is attached (as the
  TalkBack listener is). Not `AccessibilityManager`: its getter and listener for the setting are not in the SDK the
  runtime builds against (android-35); React Native's `AccessibilityInfo` reads the same setting. The JNI
  environment's values gain a sixteenth.
- **Windows**: `SPI_GETHIGHCONTRAST`'s `HCF_HIGHCONTRASTON` (a contrast theme on), re-read on `WM_SETTINGCHANGE` with
  the other settings.
- **Linux**: through the settings portal the backend already follows for the color scheme: `org.freedesktop.appearance
  contrast` (1: higher), the desktop-neutral key, else GNOME's `org.gnome.desktop.a11y.interface high-contrast`, which
  GNOME's portal serves too (the source before `contrast`); both read at start and followed through `SettingChanged`.
  The portal, not gsettings: it answers inside a sandbox, as for the color scheme. Off without a portal.
- **GPUI**: as each native backend reads it: Windows' `SPI_GETHIGHCONTRAST`, macOS' universal access
  `increaseContrast` preference (as it reads reduce motion and VoiceOver), Linux's portal keys over `zbus`. On Windows
  and macOS GPUI reports no setting changes: the settings are read again as the window's appearance changes or it
  becomes the active one.
- **Web**: `matchMedia("(prefers-contrast: more)")` and its `change` event (`highContrast` in `nsh_environment`'s
  JSON).
- **Terminal**: false (a terminal reports no such setting).
- **Compat**: `tests/compat/cases/modules/mod-contrast.tsx` (`covers [nsh] highContrast`).
- **Docs** (en and zh-CN): "More contrast" in `website/docs/accessibility.md`; `highContrast` in
  `website/modules/nsh.md`; `contrast-more:` in `website/docs/tailwind.md` and `website/modules/tailwind-reference.md`;
  `open`'s `highContrast` in `website/docs/testing-and-trace.md`; the device page's table and its Windows, Linux and
  browser paragraphs, `website/docs/windows.md`, `website/docs/web.md`, `website/modules/index.md`;
  `docs/dev/elements.md`, `styles.md`, `components.md`, `api-naming.md` (the React Native row), `js-modules.md`,
  `native-pipeline.md`, `tooling.md`, `tui.md`, and the platforms' pages (`macos.md`, `windows.md`, `linux.md`,
  `gpui.md`, `web.md`).

## Public API

- TSX `nsh`: `highContrast: Accessor<boolean>`; Tailwind `contrast-more:`.
- `nsh/test`: `OpenOptions.highContrast?: boolean` (false).
- `nsh-core`: `element::high_contrast() -> bool` (also in the prelude); `platform::Environment::high_contrast: bool`;
  `element::Environment::high_contrast: bool`; `tailwind::ClassEnvironment::high_contrast: bool`;
  `tailwind::deps::CONTRAST: u16`.
- `nsh-engine`: `EnvironmentPins::high_contrast: Option<bool>`.
- `nsh-dev`: `protocol::TestEnvironment::high_contrast: Option<bool>`.
- `nsh-testing`: `OpenOptions::high_contrast: Option<bool>`.

Breaking for Rust only: `platform::Environment`, `element::Environment` and `tailwind::ClassEnvironment` are not
`#[non_exhaustive]`, so a struct literal that lists every field needs the new one (or `..Default::default()`).

## Tests

- New: `cargo test -p nsh-engine --test engine high_contrast` (3): `a_tsx_app_follows_the_users_contrast_setting`
  (`highContrast()` and a `contrast-more:` background follow the platform's reports, on and off),
  `an_app_starts_with_the_contrast_the_platform_reports`, `a_rust_component_follows_the_users_contrast_setting`. The
  two TSX ones fail with the class environment's line left out. `cargo test -p nsh-core --lib contrast_more`
  (`contrast_more_follows_the_users_contrast_setting`: on and off, over `motion-reduce:`, under `md:`). The test
  runner's `open_sets_the_environment_and_routes_answer_requests` (`cargo test -p nsh-testing --features bundle --test
  testing`) checks `highContrast: true` and the default (it fails with a wrong expectation); `node --test
  crates/nsh-platform-web/web/nsh.test.mjs`'s "the page reports whether the user asked for more contrast, and its
  changes" (69 pass); `browser/environment.test.mjs`'s "the contrast setting is the engine's" (6 pass).
- Compat (on `pr/high-contrast/01-core`, [upstream-high-contrast](upstream-high-contrast.md)):
  `NSH_COMPAT_APP=modules cargo xtask compat` passes with `mod-contrast`'s steps, and fails with the panel's expected
  background changed.
- Existing: `cargo test -p nsh-core --lib` `tailwind` (56), `runtime` (31, every export documented), `unit_types` (2),
  `motion` (10); the engine's `rust_platform` (14) and `components::runtime_modules`; `cargo test -p nsh-engine --lib`
  `pinned_values_hold_whatever_the_platform_reports` (with `high_contrast`) and `public_api_is_typed`; `cargo test -p
  nsh-js declarations` (18) and `--lib env`; the test runner's `open_rejects_the_options_a_target_cannot_honor`; the
  browser runtime's JS tests (49 pass, the rest need Wasm); `cargo run -p nsh-bundler --example browser_bootstrap --
  --check`.
- Clippy `-D warnings`: `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-dev`, `nsh-testing` (`bundle`) (all targets); the
  macOS crates (all targets, `dev`); `nsh-platform-ios` for `aarch64-apple-ios` and `aarch64-apple-ios-macabi`
  (`embed`, `dev`); `cargo xtask clippy-android`; `nsh-platform-web` for wasm32 (default features, none, `compiler`)
  and `nsh-engine` for wasm32 without features; the GPUI workspace's `nsh-platform-gpui` (all targets, `dev`) on macOS,
  with `cargo test -p nsh-platform-gpui --lib system` (15). The Android Java runtime compiled with `javac --release
  17` against android-35. Nightly rustfmt `--check` on the changed files.
- Type-checked on macOS in scratch crates (deleted after): the Windows backend's `high_contrast()` (the same imports:
  `HIGHCONTRASTW`, `HCF_HIGHCONTRASTON`, `SPI_GETHIGHCONTRAST`) with `windows` 0.62.2 and GPUI's own FFI version,
  `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings`; the Linux backend's `watch_portal` and
  `portal_setting` with gio / glib 0.22.10 for `aarch64-unknown-linux-gnu` (`SYSTEM_DEPS_*_NO_PKG_CONFIG`), and
  GPUI's Linux `Desktop`, `Change` and `change` with `zbus` 5.19: clean.
- Not run: the Windows and Linux crates as a whole (CI builds them), GPUI on Windows and Linux, any device, simulator
  or desktop window with the setting turned on (that changes the machine's accessibility settings), the website's
  tests (no `node_modules` here).
