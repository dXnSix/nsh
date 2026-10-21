# android: Follow display density changes while the app runs

Branch: `feat/android-density` on `github.com/dXnSix/nsh`. It follows upstream by merging `upstream/main` in, never by
rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened as a pull request into the fork's `main`, Gamify's
integration line, and merged there with a merge commit. One commit, `7cd3c46`: `android: Follow display density
changes while the app runs`. Not opened upstream; the owner decides and posts there (upstream squash merges).

## Summary

The generated manifest lists `density` in the activity's `android:configChanges`, so Android does not recreate the
activity when the display density changes (`adb shell wm density`, the "Display size" setting, a display with
another density). Instead it calls `onConfigurationChanged`, and NSH did nothing with the new density. Pixels were
converted with the density read at startup in four places: `NshView` (viewport and environment), `Mounter` (every frame,
radius, border, shadow and effect), `CanvasBox` (canvas scale), and the Rust `AndroidTextEngine` (the `StaticLayout`s
it builds). The view keeps its pixel size, so no `onSizeChanged` arrived either. As a result the layout, the text and
`PixelRatio` stayed as they were until a relaunch. iOS, macOS and the web already follow such changes, because they
lay out in points or CSS pixels.

### Changes

- **`NshView.onConfigurationChanged`**: reads the density again. When it changed, it updates its own and the
  `Mounter`'s, publishes the environment (whose `pixel_ratio` is the density), then resizes the viewport to its new size
  in dp. The environment goes first so that JS gets the new window size together with the new ratio
  (`PixelRatio.get()` is not reactive and is read alongside `windowSize()`).
- **`CanvasBox`** reads `mounter.density` when it draws instead of keeping a copy.
- **Host (`host.rs`, `text.rs`)**: the density is a `Density` (`Arc<AtomicU32>`) that the host and its
  `AndroidTextEngine` share. When `Native.environment` brings a new density, the host stores it and calls
  `EngineControl::remount()` after `set_environment`.
- **Engine**: `EngineControl::remount()` sends `Message::Remount`, which calls `Core::remount()`.
- **Core**: `Core::remount()` sends every existing native view again everything a new view gets, but does not create
  it again: frames (it unsets the remembered frame, marks every layout dirty and clears the roots' `laid_out`), view
  styles, events, image, scroll, canvas, inputs, component props and so on. It also measures and lays out all text again
  (`fonts_changed`). To do this, the per-node part of `ensure_created` moves into a new `mount_state(key) -> u32`, which
  `ensure_created` and `remount` both use. `ensure_created` behaves exactly as before.

The app keeps its state. Only the platform's copy of the view tree is refreshed.

### Cost

A density change is rare and user-driven. It costs one full relayout and one batch that touches every view, about what
the first mount costs, minus creating the views.

## Tests

- New group `remount` in `crates/nsh-core/src/core/tests.rs` (`cargo test -p nsh-core remount`). After a remount, the
  batch has no `Create`, `Insert` or `Remove`. It sends the same frames again, the `ViewStyle` of a styled view and the
  `Text` of a text block, and the next commit is empty.
- `cargo test -p nsh-core`: 316 passed. `cargo test -p nsh-engine`: passed.
- `cargo +nightly fmt --check` and `cargo xtask clippy-android` are clean, and so is
  `cargo clippy -p nsh-core -p nsh-engine --all-targets -D warnings`.
- After the merge of `upstream/main` `a8ff77c` (whose Android host also keeps the engine's control for answers the
  sink gives itself), the host keeps both the shared `Density` and that control; `cargo test -p nsh-core remount`,
  `cargo test -p nsh-core`, `cargo test -p nsh-engine`, the clippy runs above and `cargo xtask clippy-android` pass
  again. The emulator proof below was not repeated after the merge.

## Emulator proof (Android 15 arm64, AVD `nsh-multires-35`, density 440)

`cargo run -p nsh-cli -- run <probe> --target android`. The probe app shows
`ratio=${PixelRatio.get()} window=${windowSize()}`, a 100×100 dp box with 24 dp padding, and a tap counter. Bounds
come from `uiautomator dump`:

| `wm density` | Before (upstream/main) | After (this branch) |
| --- | --- | --- |
| 440 at launch | `ratio=2.75 window=393x829`, box `[66,286][341,561]` (275 px) | same |
| 320 | unchanged: `ratio=2.75 window=393x829`, box 275 px | `ratio=2 window=540x1140`, box `[48,208][248,408]` (200 px) |
| 560 | – | `ratio=3.5 window=309x651`, box `[84,364][434,714]` (350 px) |
| reset (440) | – | `ratio=2.75 window=393x829`, box `[66,286][341,561]` |

Across all the changes the same `ActivityRecord` stays resumed, and after three taps at 440 the counter still shows
`taps=3` at 320. The app was not relaunched. Text is laid out and drawn at the new density.

## Not covered

- An embedded `NshView` whose host activity does **not** declare `density` in `configChanges` is recreated by Android
  anyway. That is the host app's choice and works as before.
- Not checked: a scrolled view probably keeps its offset in pixels, which after the change is a different offset in dp.
  The probe app does not scroll.
- The dev-mode overlay pills (`NshView` dev UI) keep their startup size until the next reload.
- Not run on a physical device.
