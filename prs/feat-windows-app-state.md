# windows: Report whether the app is in use

> **Upstream (checked 2026-11-01):** in longbridge/nsh#747 (open), this platform's commit.

Branch: `feat/windows-app-state` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `1def3acc` (it builds on
`core: Tell apps whether they are in use`, #37). One commit, `96b44695`: `windows: Report whether the app is in use`.
Opened as a pull request into the fork's `main` and merged there with a merge commit. Not opened upstream: the owner
decides and posts there, after the core change it builds on.

## Summary

`appState()` (`Environment::app_state`) read `"active"` on Windows whatever the app's state.

### Changes

- **`system.rs`**: the environment carries the app's state: `background` while the window (an embedded surface's root
  window) is minimized (`IsIconic`) or hidden (`IsWindowVisible`), `active` while the app is the active one, `inactive`
  otherwise.
- Whether the app is the active one comes from `WM_ACTIVATEAPP`'s flag, which Windows sends to the top-level windows of
  the thread being activated or deactivated. Before the first one, and for an embedded surface (`AppView` in another
  app's window, whose host's windows get them), it is whether the foreground window is the process's.
- **`window.rs`**: `WM_ACTIVATEAPP` and a top-level window's `WM_WINDOWPOSCHANGED` (shown, hidden, minimized or
  restored; before default handling sends `WM_SIZE`) update it. The environment's other updates read it again too.
- **Docs**: the Environment row of `docs/dev/windows.md` and the module doc of `system.rs`.

## Public API

None: `appState()` / `app_state()` (#37) now report Windows's state.

## Tests

- `cargo fmt -p nsh-platform-windows --check`: clean.
- Not built or run on Windows by hand: the fork's CI (the Windows workflow: clippy, tests and the examples' tests in
  Windows windows) checks it.
- Not run in an embedding app.
