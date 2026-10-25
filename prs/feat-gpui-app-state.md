# gpui: Report whether the app is in use

Branch: `feat/gpui-app-state` on `github.com/dXnSix/nsh`, cut from the fork's `main` at `1def3acc` (it builds on
`core: Tell apps whether they are in use`, #37). One commit, `c0f864c2`: `gpui: Report whether the app is in use`. Opened as a
pull request into the fork's `main` and merged there with a merge commit. Not opened upstream: the owner decides and
posts there, after the core change it builds on.

## Summary

`appState()` (`Environment::app_state`) read `"active"` on the GPUI backend whatever the app's state.

### Changes

- **`system.rs`**: the environment carries the app's state from GPUI's window activation and visibility:
  `background` while the surface's window is not presented (`WindowVisibility::Hidden`: minimized, covered or on
  another desktop, as far as each platform reports it, see GPUI's `WindowVisibility`), `active` while one of the
  windows showing a surface is the active one, `inactive` otherwise.
- Each watched surface records whether its window is active and visible, from `observe_window_activation` and
  `observe_window_visibility` (registered with the appearance's observer); a change in one updates every surface, as
  whether the app is the active one depends on all of their windows. The environment's other updates read it too.
- An embedded surface follows the embedding app's window it is drawn in; the embedding app's other windows (those
  showing no surface) are not looked at.
- **Docs**: the Environment row of `docs/dev/gpui.md` and the module doc of `system.rs`.

## Public API

None: `appState()` / `app_state()` (#37) now report the GPUI backend's state.

## Tests

- New `the_app_s_state_follows_its_window_s_activation_and_visibility` in the tests in windows of GPUI's test
  platform (`src/tests.rs`, feature `shot`): a field showing `appState()` reads `active` in an active window,
  `inactive` once it is deactivated, `background` while it is hidden, `inactive` when it shows again, `active` when it
  is activated.
- Checked to fail when the state never becomes `inactive`: the field reads `active`, not `inactive`.
- `cargo test -p nsh-platform-gpui --features shot` (198 passed) and
  `cargo clippy --all-targets --features nsh-platform-gpui/dev -- -D warnings` in `crates/gpui`, on macOS: clean.
