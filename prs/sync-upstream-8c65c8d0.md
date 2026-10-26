# Merge upstream/main 8c65c8d0

Brings the fork's main up to upstream/main 8c65c8d0 (17 commits), before the next feature series. Merge with a merge
commit.

## What it brings

- **0.3.0** (#496): the workspace's version. The next fork tag is `fork-0.3.0-1`.
- **One home per API** (#488): `nsh/device`, `nsh/animated`, `nsh/accessibility` and the other modules merged into
  `nsh`; `nsh/ios` and `nsh/android` into `nsh/mobile`; Rust's `nsh_core::ui` merged into `nsh_core::element`, the
  prelude `nsh_core::prelude` only; `Input` moved to `nsh/base`; CSS `env()` instead of `SafeAreaView` and
  `KeyboardAvoidingView`. The Compat workflow is off upstream until the API freezes.
- `RichEditor`, `Slider`, `Bar`, `Pie` and `Arc` in `nsh/base`, the listbox and option roles, focus stops without
  presses, and fixes on iOS, macOS and the web.

## Conflicts

- **The fork's environment accessors** (`appState`, `timeZone`, `languages`, `hourCycle`, `firstDayOfWeek`,
  `measurementSystem`, `temperatureUnit`) are `nsh`'s now, as the rest of `nsh/device` is: `nsh-js`'s device exports
  moved with it, and `nsh_core::prelude` gets their Rust functions (`app_state()`, `time_zone()`, `languages()`,
  `hour_cycle()`, `first_day_of_week()`, `measurement_system()`, `temperature_unit()`) and the fork's shader builders
  (`shader()`, `shader_effect()`, `shader_transition()`, `screen_effect()`, `screen_transition()` and their views),
  which the old `ui::prelude` had.
- **`nsh-js` `modules.rs`**: `Input` left `nsh` for `nsh/base`; the fork's `Shader`, `ShaderEffect`,
  `ShaderTransition`, `ScreenEffect`, `ScreenTransition`, `createShader` and `shaderSupport` stay in `nsh`.
- **`js/host.rs`**: upstream's new `on::VIEW` took bit 13, so the fork's shader elements' bits move to 14–18.
- **The platforms' module lists**: upstream's `selectable_view` (was `selection_area`) and the fork's `shader`.
- **macOS**: `system.rs`'s imports are both sides'; upstream removed `system::surface_resized` (with toasts leaving
  for `nsh/base`), the fork's `screen::resized` stays.
- **`Cargo.toml`**: 0.3.0 for every crate, the fork's `nsh-shader` and `nsh-metal` too.
- **Tests**: the engine's environment tests (app state, languages, regional preferences, time zone) take upstream's
  imports; the harness's environment app and the compat case `mod-device` import the fork's accessors from `nsh`, and
  the compat script covers them as `[nsh]`.
- **Docs**: prose both sides changed is merged word by word; the `nsh/device` page's sections the fork added (the
  seven accessors) move to the `nsh` page's Device section, in English and Chinese, and its intro lists them.

## On top of the conflicts

The fork's own code and docs that named the old paths: `crate::ui` in `nsh-core`'s `platform.rs`, `shader.rs` and
`ops.rs`; `nsh_core::ui` in an engine test; `nsh/device` and `nsh_core::ui::prelude` in the device guide, the shader
element page and `docs/dev/shaders.md`; `nsh/animated` in the shader demos, tests and docs (`nsh` now).

## Validation

On macOS (Rust 1.99): `cargo fmt --all --check`; clippy `-D warnings` for `nsh-core`, `nsh-engine`, `nsh-js`,
`nsh-dev`, `nsh-testing`, `nsh-base`, the macOS crates, iOS (Mac Catalyst), Android (`cargo xtask clippy-android`) and
the web (wasm32); `cargo test` of `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-base`, `nsh-testing` and GPUI's
(`--features shot`). Five of upstream's new `RichEditor` tests (`mod+b`, `mod+a`, copy) fail on this Mac on
upstream/main itself as on this branch; upstream's CI runs them on Linux. Linux and Windows are this pull request's
CI.
