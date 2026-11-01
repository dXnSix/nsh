# android: Report whether the app is in use

> **Upstream (checked 2026-11-01):** in longbridge/nsh#747 (open), this platform's commit.

Branch: `feat/android-app-state` on `github.com/dXnSix/nsh`, stacked on `feat/ios-app-state` (#39: both edit the same
sentence of `docs/dev/elements.md`); it builds on `core: Tell apps whether they are in use` (#37). Its own commit:
`713d861f` `android: Report whether the app is in use`. Opened as a pull request into the fork's `main` after #39, and
merged there with a merge commit. Not opened upstream: the owner decides and posts there, after the core change it
builds on.

## Summary

`appState()` (`Environment::app_state`) read `"active"` on Android whatever the app's state.

### Changes

- **`NshView`** (`java/com/longbridge/nsh/NshView.java`) registers an `Application.ActivityLifecycleCallbacks` while it is
  attached (`onAttachedToWindow` / `onDetachedFromWindow`) and follows its own activity (`activity(getContext())`):
  resumed is `active`, paused (or started again on its way back) is `inactive`, stopped is `background`. A change
  publishes the environment.
- **`Native.environment`** carries it as one more value (index 13); **`host.rs`** reads 14 values and sets
  `Environment::app_state`.
- An embedded `NshView` follows the activity it is shown in. A view starts `active`: the state of an activity it is
  attached to while resumed.
- **Docs**: Android in the `appState()` paragraph of `docs/dev/elements.md`.

## Public API

None: `appState()` / `app_state()` (#37) now report Android's state.

## Tests

- `cargo xtask android --dev` builds; `cargo xtask clippy-android` reports the same two findings with and without this
  change (`host.rs:734` redundant closure, `host.rs:793` `chunks_exact`, both in existing code, flagged by clippy 1.99);
  none in what this change adds.
- Android Emulator (AVD `nsh-multires-35`, Android 15 arm64), `nsh run <probe> --target android`, a probe app that logs
  `appState=${appState()}` in an effect and shows it:
  - launch: `active`;
  - Home (`adb shell input keyevent KEYCODE_HOME`): `inactive`, `background`;
  - back to the app (`adb shell am start -n com.longbridge.nsh.host/com.longbridge.nsh.DevActivity`): `inactive`,
    `active`; the screen shows `App state: active`.
  - The emulator's gesture navigation opens no Recents on `KEYCODE_APP_SWITCH`, so that path was not exercised.
  - The development APK with all three ABIs at debug is 2.4 GB, more than the emulator's free space; the run used a
    stripped arm64-only copy of the same runtime build.
- Not run on a device or in an embedding app.
