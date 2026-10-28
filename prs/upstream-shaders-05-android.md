# Upstream PR: android: Draw shader effects, transitions and page shaders

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/05-android`, on `pr/shaders/02-page-dialog-shaders` (pushed to the fork, `dXnSix/nsh`, 2026-10-28). Two commits: the backend and its `docs/dev/shaders.md` notes.
- Source: the fork's `74f138df4` and `4ae00db7a` on `sync/upstream-478737b1` (the fork-only CI commit `eab3f8a07` left out).
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/05-android`).
- The gallery test's dialog half needs `pr/android/02-test-dialog-touches` (its own draft,
  [`upstream-android-test-dialog-touches.md`](upstream-android-test-dialog-touches.md)): without it the test agent taps
  the page under a `Dialog`. That fix is independent of shaders and can go first.

- Checked locally on `7dba85d4`: `cargo xtask clippy-android` (both feature sets), `javac` of the Java sources against `android-36`, nightly rustfmt. Not run on the emulator here; the same code (with the dialog-touches fix) passed `shaders.test.tsx` 3 of 3 on the `nsh-multires-35` AVD on the fork's sync branch.

The body to paste starts below the line.

---

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): Android (API 29 and later; Android 9 shows the children as they are) draws
`ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition` and pages' and dialogs' `transitionShader`
(#[01 PR], #[02 PR]) on `Shader`'s renderer.

- **Pictures** (`Pictures.java`): a drawing recorded into a `RenderNode`, rendered by a `HardwareRenderer` into an
  `ImageReader` the CPU reads, its RGBA rows handed to `libnsh` (`Native.shaderInput`), which drops one equal to the
  last of its input.
- **`ShaderEffect`**: the children are pictured when one of them invalidates (or the core marks them,
  `SHADER_INPUT`), and the frame shows in `dispatchDraw` where they were, overflow included; they still take touches
  and stay accessible. `sample_backdrop` pictures what lies behind the view (`Backdrop`'s recording) before every
  window frame.
- **`ShaderTransition`**: its batch pictures what shows as `from` before the rest of it applies
  (`SHADER_TRANSITION`), holds that picture until the first frame, and pictures the children as `to` while it runs.
- **`ScreenEffect` and `ScreenTransition`**: the same over the whole `NshView` (dialogs, in their own windows, are not
  part of it).
- **`transitionShader`** (`NAV_SHADER`): a push, pop, presentation or dismissal pictures the lower and the upper side
  and plays the shader over the stack or the dialog's window (`NavTransition.java`); its own clock moves
  `nsh.progress` from 0 to 1 on a push or a presentation, from 1 to 0 on a pop or a dismissal.

On the GL backend (the emulator's), wgpu-hal logs `Fake map` at error level on every frame read back; it is harmless
(GLES has no `glGetBufferSubData`).

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| Android | [the gallery's Shaders page without the shaders] | [the same page with them] |

(Screenshots to take on an emulator with this branch and #[10 PR]'s demo.)

## How to Test

- `ANDROID_NDK_HOME=<NDK 27> cargo xtask clippy-android` (both feature sets).
- `javac` of `crates/nsh-platform-android/java` against `android-36`'s `android.jar`.
- With #[10 PR] and the dialog-touches fix: `nsh test examples/gallery/tests/shaders.test.tsx --target android` on an
  emulator (API 35).

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and the Android clippy runs (clean).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #[01 PR] and #[02 PR].
- [ ] Ran the related `examples/gallery` demos on Android (not run on this base; the fork's sync branch ran `shaders.test.tsx` 3 of 3 on an API 35 emulator).
