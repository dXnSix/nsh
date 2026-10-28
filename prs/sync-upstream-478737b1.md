# Merge upstream/main 478737b1

Brings the fork's main up to upstream/main 478737b1 (126 commits) and moves the fork's shader effects and
transitions onto upstream's new `Shader` (#517), so NSH keeps one GPU pipeline: upstream's. Merge with a merge
commit. Branch `sync/upstream-478737b1`; the next fork tag is `fork-0.3.0-2`.

## Before the merge

- `cli: Drop --strict-port, which upstream's port takeover replaces`: upstream's #428 keeps the requested port and
  replaces the previous development session on it, so `--strict-port` (and `NSH_STRICT_PORT`) has nothing left to do.
  Consumers that pass it (Gamify's `xtask web` and browser specs) drop it with their next NSH bump.
- `core: Remove the fork's shader pipeline, which upstream's Shader replaces`: upstream added a `Shader` element
  drawn by wgpu with its own `nsh-shader` crate. The fork's pipeline (#19, #20, #21, #22, #26: the WGSL profile and its
  MSL/HLSL/GLSL/AGSL writers, `nsh-metal`, the per-platform renderers, its `Shader`, `ShaderEffect`,
  `ShaderTransition`, `ScreenEffect`, `ScreenTransition`, `createShader`, `shaderSupport`) goes; files only it
  changed are upstream's again, the fork's other features stay.

## The merge

- What it brings: Shader (#517), Select, OtpInput, NumberInput, DatePicker, Calendar, ColorPicker, Combobox, Toggle
  and ToggleGroup, RichEditor tables, plot marks and Sankey, the port takeover (#428), the incremental web dev runtime
  (#429), Android display density changes (#431, the fork's own feature as upstream merged it), every web app on the
  browser's JavaScript engine (#603), one event object for every event (#558), listeners with `on(event, l)` (#612),
  file dialogs, window and menu APIs for Rust views (#629), screen color picking, the animation graph's speed-ups
  (#588, #589, #590: the fork's #88, #89, #90 as upstream merged them), fixes.
- It also fixes the Linux CI's intermittent SIGSEGV after the text tests: the fork's `text.rs` added app fonts to
  Fontconfig's shared configuration while other font maps read it; upstream gives each map a configuration of its
  own (`set_font_files`).
- Conflicts: the fork's environment accessors and font functions next to upstream's in `nsh_core::prelude` and the
  crate root; the density remount (#431) is upstream's and its tests replace the fork's copy; iOS surface: upstream's
  keyboard duration in milliseconds and screen color with the fork's app state; web: the fork's app state and time
  zone next to upstream's screen color; macOS decor: upstream's shape without the shadow
  fields, the fork's visible border sides; Android: upstream's `setDensity`; docs: both sides' words.

## After the merge: effects and transitions on upstream's Shader

- `core: Add ShaderEffect, ShaderTransition, ScreenEffect and ScreenTransition on Shader`: `sample_content`,
  `sample_backdrop`, `sample_from`, `sample_to`, `nsh.progress`, `nsh.overflow` (`maxSampleOffset`) in the WGSL
  contract; the four elements (TSX from `nsh`, Rust builders); roles on `ShaderLayer`; `MountItem::ShaderTransition`
  and `ShaderInput`; `nsh_engine::shader` pictures, transitions and overflow.
- `core: Let pages and dialogs come and go through a shader`: `transitionShader` on `NavPage` and `Dialog`,
  `MountItem::NavShader`.
- Upstream links naga into a web runtime only for apps that use shaders (`nsh_feature_shader`): the bundler counts
  the four elements' tags and a page's `animation: { shader }` as shader use, the built-in cross-fade and page
  shaders compile through the runtime's compiler.
- `engine: Let a shader transition run backwards`: `ShaderView::start_transition_backwards`, for pages that leave
  and dialogs that close.
- The platforms, one commit each: macOS, iOS, Android (API 29+), web, Windows, Linux draw effects, transitions,
  screen effects and transitions, backdrops and page/dialog shaders from pictures of the views
  (`ShaderView::set_input`); GPUI draws fills only (effects show their children, transitions change at once). On
  every platform a page or dialog shader has the lower side as `from` and the upper as `to`, and runs backwards on
  pop and dismiss. Screen effects on iOS and Android leave dialogs out (UIKit presents them above the root, Android
  shows them in windows of their own).
- `gallery: Show shader effects and transitions on the Shaders page`, with `tests/shaders.test.tsx`;
  `ci: Run the shader tests on macOS, iOS, Android and the web`.
- `android: Send test touches to the shown dialog's window`: an older test agent bug the shader test found: a
  `Dialog` is a window of its own, and every test touch went to the activity's window, so no dialog button took a
  press in an Android test.
- Known: on emulators wgpu picks the GL backend, whose readback logs "Fake map" (wgpu-hal `gles`) for every shader
  frame; harmless.
- `ci: Run the controls tests on Android again`: the swiftshader crash does not happen on this base (arm64,
  `-gpu swiftshader_indirect`); five newer controls tests fail on Android for other reasons (presses right after
  typing, paste, a desktop Tab order test), so `examples.sh` runs the 19 that pass by name.
- `docs: Describe shader effects, transitions and page shaders`.

## iOS test fixes

The iOS job's timeouts and dropped keys, fixed in the test agent:

- `ios: Type each key in tests once the one before it landed`: keys typed every 80 ms were dropped by a slow
  simulator's keyboard ("hello" became "HELO"); each key now waits until the focused input's text changed (at most
  600 ms).
- `ios: Let test steps settle while a focused input's caret blinks`: UIKit's endless loops repeat `FLT_MAX` times,
  which the quiet check took for an animation that ends, so every step with a focused input waited the full 2 s;
  such loops now count as endless (the iOS list's tests: 570 s instead of 712 s).
- `ios: Scroll in tests with a quicker finger`: a scroll gesture took about 550 ms and ate much of the 30 s of tests
  that ran out on the runners; the finger now moves at a quick swipe's pace (about 340 ms), still resting before it
  lifts.
- `ci: Run form and most of controls in the iOS simulator`: form's typing works again; the synced
  `controls.test.tsx` has 24 tests, five fail on iOS every time (slider, number input, OTP Clear, PIN Change PIN,
  Tab order), so `examples.sh` runs the 19 that pass by name, as Android's list does.
- Known: stalls inside `open` on the runner (timeouts with no screen) have no known cause yet; a few iOS flakes may
  remain.

## Validation

Local (macOS host, Rust 1.99, `CARGO_INCREMENTAL=0`):

- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: pass.
- The fork CI's platform checks: macOS clippy with `dev`, iOS clippy for Mac Catalyst with `embed,dev`, the iOS
  simulator host build, `cargo xtask clippy-android`, web clippy for wasm32 with `compiler`: pass. Windows and
  Linux: clippy against their targets passed with stubbed system libraries (no link); CI builds them for real.
- `cargo +nightly fmt --all --check`: only five upstream files (lines upstream committed) differ; left as upstream's.
- `examples.sh macos` (every listed gallery test, rust-exports, gallery-rs): pass.
- `shaders.test.tsx`: macOS, iOS simulator, Android emulator (arm64, API 35) and web (headless Chrome) pass;
  added to `examples.sh` for all four.
- Screenshots of every kind on each platform that draws them (macOS, iOS simulator, Android emulator, web).
- No `nsh-macos-host` crash reports after the fix of the views kept in a thread-local (an exit-time abort while
  the port was in progress).

## CI on the fork

The first runs on the merged code (Windows, Linux, macOS and iOS passed as they were):

- `ci: Run form's Input demo and disabled field on Android`: the synced form tests' sign-up form and search field
  set the viewport, which an emulator keeps at its device's size.
- `ci: Run the Android emulator without guest Vulkan`: on the x86_64 runner wgpu picked the emulator's SwiftShader
  Vulkan; the shader tests stalled after a SPIR-V error and the emulator went away. With GLES, as on an emulator on
  a Mac, they pass.
- `ci: Leave the tests that move the clock out of the web's list`: on the web `app.wait()` and
  `app.clock.runFor()` never return (dock's locked layout, form's request and search, timers, toasts);
  error-boundary's failing binding expects QuickJS's message.
- `ci: Run only text-view's Text View demo on Android`: a press on the chat's featured cards starts no reply on
  Android (its chips do).

Known, not fixed here (each its own fork PR next):

- The web's test clock: upstream's own `Rust application` web job fails dock's locked layout test the same way on
  its main, so that check stays red on this PR.
- The chat's featured cards take no press on Android.
