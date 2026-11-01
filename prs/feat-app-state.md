# core: Tell apps whether they are in use

> **Upstream (checked 2026-11-01):** in longbridge/nsh#747 (open), the core's commit.

Branch: `feat/app-state` on `github.com/dXnSix/nsh`, cut from `eb76a17d` (the latest `upstream/main` commit the fork's
`main` holds). One commit, `8dbbb5e7`: `core: Tell apps whether they are in use`. Opened as a pull request into the
fork's `main` and merged there with a merge commit. Not opened upstream: the owner decides and posts there (upstream
squash merges). The first of a series: the native platforms report the state in pull requests of their own (mobile,
desktop, GPUI).

## Summary

Apps had no way to know that the user left them: a live feed, polling or a video kept running while nobody saw it, and an
app could not save or pause anything as it went to the background.

`appState()` of `nsh/device` and Rust's `ui::app_state()` say it, reactively, in React Native's `AppState` terms:
`"active"` in the foreground, `"inactive"` while visible without the focus (another app or window has it, or the system
shows something over it), `"background"` while not visible.

### Changes

- **Core**: `platform::AppState` (`Active`, default; `Inactive`; `Background`; `name()`), `Environment::app_state` and
  the UI environment's `app_state`; `ui::app_state()` (in the prelude) reads it with a memo of its own, so a change of
  another part runs nothing that read only this one.
- **Engine**: hands it to the core with the rest of the environment; `nsh_engine::AppState` re-exported.
- **JS** (`nsh-js` `env.rs`): the environment's `AppState` part, and `appState` in `nsh/device`
  (`Accessor<"active" | "inactive" | "background">`).
- **Web**: the page reports `appState` with its environment: `background` while `document.visibilityState` is
  `hidden`, else `active` with the focus and `inactive` without; it follows `visibilitychange`, `focus` and `blur`.
- **Testing**: `nsh/test`'s `app.setAppState(state)` (headless; a real platform's agent answers that it runs headless
  only), as `setColorScheme` sets the color scheme.
- **Docs**: `appState` in `website/modules/nsh-device.md`, "In the background" in
  `website/docs/device-and-environment.md`, `setAppState` in `website/docs/testing-and-trace.md` (and their zh-CN pages),
  `docs/dev/elements.md` and `docs/dev/components.md`.

The platforms write `Environment::app_state` as the system reports it, as they write the color scheme; an embedded
surface follows the embedding app's. iOS, Android, macOS, Windows, Linux and GPUI report it in the next pull requests;
until then their apps read `"active"`.

## Public API

- `nsh_core::platform::AppState` (`Active`, `Inactive`, `Background`), `AppState::name(self) -> &'static str`;
  re-exported as `nsh_engine::AppState` and in `nsh_core::ui`.
- `nsh_core::platform::Environment::app_state: AppState` and `nsh_core::ui::Environment::app_state: AppState` (new
  fields; both structs are built with `..Default::default()` everywhere in the workspace).
- `nsh_core::ui::app_state() -> AppState` (also in `nsh_core::ui::prelude`): reactive.
- JS `appState: Accessor<"active" | "inactive" | "background">` in `nsh/device`.
- `nsh/test`: `App.setAppState(state: "active" | "inactive" | "background"): Promise<void>`.

## Tests

- `components_follow_whether_the_app_is_in_use` in `crates/nsh-engine/tests/engine/rust_platform.rs`
  (`cargo test -p nsh-engine --test engine rust_platform`): an effect reading `app_state()` runs once per change
  (inactive, background, active) and not when another part (the font scale) changes.
- `crates/nsh-testing/tests/testing/environment`: "the app state follows the system's" (`setAppState` through
  inactive, background and active) and `appState()` in the defaults test
  (`cargo test -p nsh-testing --features bundle --test testing`).
- Compat: `App state: active` in `tests/compat/cases/modules/mod-device.tsx` and `covers [nsh/device] appState` in its
  script. `NSH_COMPAT_APP=modules cargo test checkout_apps_run` in `nsh-compat` runs the app through and holds the
  claim; the run then reports only the `events` app's `covers [props] pasteBehavior`, which `nsh-compat`'s latest commit
  (#13) expects of a newer NSH than this base.
- `cargo test -p nsh-core -p nsh-engine -p nsh-js -p nsh-testing --features nsh-testing/bundle`: passed.
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-core -p nsh-engine -p nsh-js -p nsh-testing --all-targets` and
  `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown`: clean; `web/nsh.js` parses.
- Not run in a browser: the page's part (`visibilitychange`, `focus`, `blur`) is checked by review only.
