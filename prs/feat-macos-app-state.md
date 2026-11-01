# macos: Report whether the app is in use

> **Upstream (checked 2026-11-01):** in longbridge/nsh#747 (open), this platform's commit.

Branch: `feat/macos-app-state` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `1def3acc` (it builds on
`core: Tell apps whether they are in use`, #37). One commit, `628bf1e5`: `macos: Report whether the app is in use`.
Opened as a pull request into the fork's `main` and merged there with a merge commit. Not opened upstream: the owner
decides and posts there, after the core change it builds on.

## Summary

`appState()` (`Environment::app_state`) read `"active"` on macOS whatever the app's state.

### Changes

- **`system.rs`**: the environment carries the app's state from `NSApplication`: `background` while none of the app shows
  (`isHidden`, or `occlusionState` without `visible`: its windows minimized or covered), `active` while it is the active
  app (`isActive`), `inactive` otherwise.
- It follows `NSApplication`'s `didBecomeActive`, `didResignActive`, `didHide`, `didUnhide` and
  `didChangeOcclusionState` notifications, installed with the system settings' watchers (once per thread), as the
  environment follows the system's other settings.
- An embedded surface (`AppView` in another AppKit app) follows the embedding app's state, which is the application's.
- **Docs**: the Environment row of `docs/dev/macos.md` and the module doc of `system.rs`.

## Public API

None: `appState()` / `app_state()` (#37) now report macOS's state.

## Tests

- `cargo clippy -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app --all-targets --features nsh-platform-macos/dev`:
  clean.
- macOS 26 (this Mac), `nsh run <probe> --target macos`, a probe app that logs `appState=${appState()}` in an effect and
  shows it; steps through System Events:
  - start, before the window shows and while another app keeps the focus: `background`, `inactive`;
  - frontmost: `active`; Finder activated: `inactive`; the app hidden: `background`; frontmost again: `active`;
  - its window minimized (`AXMinimized`): `background`; restored and frontmost: `active`.
- Not run in an embedding app.
