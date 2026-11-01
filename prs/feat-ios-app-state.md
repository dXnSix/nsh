# ios: Report whether the app is in use

> **Upstream (checked 2026-11-01):** in longbridge/nsh#747 (open), this platform's commit.

Branch: `feat/ios-app-state` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `1def3acc` (it builds on
`core: Tell apps whether they are in use`, #37). One commit, `78b6b24c`: `ios: Report whether the app is in use`. Opened
as a pull request into the fork's `main` and merged there with a merge commit. Not opened upstream: the owner decides
and posts there, after the core change it builds on.

## Summary

`appState()` (`Environment::app_state`) read `"active"` on iOS whatever the app's state, and the iOS backend did not
build since #37 added the field: iOS writes its `Environment` out in full, with no `..Default::default()`. The fork's
`main` and `fork-0.2.0-5` have that break; this fixes it.

### Changes

- **`RootView`** (`crates/nsh-platform-ios/src/surface.rs`) follows UIApplication's `didBecomeActive`,
  `willResignActive`, `didEnterBackground` and `willEnterForeground` notifications (`observe_app_state`, next to the
  keyboard's observers and removed with them) and keeps the state in its ivars: `active`, `inactive` (resigning active,
  or back in the foreground before it is active again) and `background`. It publishes the environment on each.
- The state comes from the notification, not from `applicationState`, which still says active during
  `willResignActive`. A surface starts with the state `applicationState` reports when it is made.
- An embedded surface (`NshAppView`) follows the embedding app's state: the notifications are the application's.
- **Docs**: iOS in the `appState()` paragraph of `docs/dev/elements.md`.

## Public API

None: `appState()` / `app_state()` (#37) now report iOS's state.

## Tests

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features
  nsh-platform-ios/embed,nsh-platform-ios/dev`: clean; `cargo build -p nsh-ios-host --target aarch64-apple-ios-sim`:
  builds.
- iOS Simulator (iPhone 17 Pro Max, iOS 26.5), `nsh run <probe> --target ios`, a probe app that logs
  `appState=${appState()}` in an effect and shows it:
  - launch: `inactive`, `active`;
  - `xcrun simctl launch booted com.apple.Preferences` (Settings comes to the front): `inactive`, `background`;
  - `xcrun simctl launch booted com.longbridge.nsh.host` (back): `inactive`, `active`.
- Not run on a device or in an embedding app.
