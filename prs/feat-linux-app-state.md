# linux: Report whether the app is in use

Branch: `feat/linux-app-state` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `1def3acc` (it builds on
`core: Tell apps whether they are in use`, #37). One commit, `ed9e3c2e`: `linux: Report whether the app is in use`.
Opened as a pull request into the fork's `main` and merged there with a merge commit. Not opened upstream: the owner
decides and posts there, after the core change it builds on.

## Summary

`appState()` (`Environment::app_state`) read `"active"` on Linux whatever the app's state.

### Changes

- **`system.rs`**: the environment carries the app's state from GDK's toplevel states (`GdkToplevelState`):
  `background` while the surface's window is not shown (the root unmapped, the window `MINIMIZED`, or `SUSPENDED`: on
  another workspace or covered, where the compositor reports it), `active` while one of the app's windows
  (`gtk_window_list_toplevels`, an embedding app's windows too) is `FOCUSED`, `inactive` otherwise.
- It follows the root's `map` and `unmap` and its toplevel's `state` (connected on each realize, with the scale's
  watcher). A change in one surface's window updates every surface of the thread, as whether the app is the active one
  depends on all of its windows. The environment's other updates read it again too.
- `GDK_TOPLEVEL_STATE_FOCUSED` rather than `GtkWindow:is-active`: GTK derives the second from the first in its own
  handler, and reading GDK's does not depend on which handler runs first.
- **Docs**: the Environment row of `docs/dev/linux.md` and the module doc of `system.rs`.

## Public API

None: `appState()` / `app_state()` (#37) now report Linux's state.

## Tests

- `cargo fmt -p nsh-platform-linux --check`: clean.
- Not built or run on Linux by hand: the fork's CI (the Linux workflow: clippy, tests and the examples' tests in GTK
  windows on a virtual display) checks it.
- Not run in an embedding app.
