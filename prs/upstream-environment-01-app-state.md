# Upstream PR: core: Tell apps whether they are in use

Opened 2026-10-31 as [longbridge/nsh#747](https://github.com/longbridge/nsh/pull/747), at the owner's
decision; the body below the line is the one posted.

- Branch: `pr/environment/01-app-state` (`ceba797ee`), seven commits on `pr/environment/00-non-exhaustive`: the
  core's, then one per platform (iOS, Android, macOS, Windows, Linux, GPUI). It replaces the per-platform branches
  `pr/app-state/01-core` to `07-gpui`; every file in it is the same as in those branches, except
  `docs/dev/elements.md`, where the iOS and Android sentences are joined.
  The literals that build an environment outside `nsh-core` (iOS's root view, the engine's `ui_environment` and its
  tests, `nsh-testing`'s) set the new field by assignment, as `pr/environment/00-non-exhaustive` builds them; the
  branch was rebuilt on it 2026-10-30 (the previous heads are `old-pre-nonexh/01-app-state` locally).
- Size of its own commits: 38 files, +723 / −62 (code +493 / −38, tests +106 / −10, docs +124 / −14).
- Open it from `dXnSix:pr/environment/01-app-state` into `longbridge/nsh:main`, title (upstream squash-merges):
  `core: Tell apps whether they are in use`.
- The first of the environment series ([`upstream-environment-and-fonts.md`](upstream-environment-and-fonts.md)).
- Checked: see "Checked" in the series' index.

---

**Depends on:** #744 (`core: Mark the environments non-exhaustive, so new fields break no code`): the new
field is set the way that PR builds environments outside `nsh-core`. Until it is merged this PR shows its commit too;
the app state's own are the last seven. The rest of the environment series stacks on this one: the time zone, the
preferred languages and the regional preferences, each a PR of the core and every platform.

## Description

Apps have no way to know that the user left them: a live feed, polling or a video keeps running while nobody sees it,
and an app cannot save or pause anything as it goes to the background.

`appState()` of `nsh` and Rust's `app_state()` (the prelude) say it, reactively, in React Native's `AppState` terms:
`"active"` in the foreground, `"inactive"` while visible without the focus (another app or window has it, or the
system shows something over it), `"background"` while not visible. It is `Environment::app_state`, which each platform
writes as the system reports the app's state, as it writes the color scheme; an embedded surface follows the
embedding app's.

The PR is the core and every platform, a commit each, so each platform can be reviewed on its own:

- **Core**: `platform::AppState` and `Environment::app_state` (the platform's environment and the components'), read by
  `app_state()` with a memo of its own, so a change of another part of the environment reruns nothing that read only
  this one; `appState` in `nsh`. The web page reports it from `document.visibilityState` and the focus, following
  `visibilitychange`, `focus` and `blur`. `nsh/test`'s `app.setAppState(state)` plays the system in headless runs.
- **iOS**: UIApplication's `didBecomeActive`, `willResignActive`, `didEnterBackground` and `willEnterForeground`
  notifications. The state comes from the notification, as `applicationState` still says active during
  `willResignActive`; a surface starts with the state `applicationState` reports when it is made.
- **Android**: `NshView` follows its activity through the application's `ActivityLifecycleCallbacks` while it is
  attached: resumed is `active`, paused (or started again on its way back) `inactive`, stopped `background`. The state
  is one more value of `Native.environment`.
- **macOS**: NSApplication's `didBecomeActive`, `didResignActive`, `didHide`, `didUnhide` and
  `didChangeOcclusionState`: background while none of the app shows (hidden, minimized or covered), active while it is
  the active app, inactive otherwise.
- **Windows**: background while the window (an embedded surface's root window) is minimized or hidden, active while
  the app is the active one, inactive otherwise; followed on `WM_ACTIVATEAPP` and `WM_WINDOWPOSCHANGED`.
- **Linux**: GDK's toplevel states: background while the surface's window is not shown (unmapped, minimized or
  suspended), active while one of the app's windows has the focus, inactive otherwise.
- **GPUI**: the windows' activation and visibility: background while the surface's window is not presented, active
  while a window showing a surface is the active one.

Docs: `appState` in `website/modules/nsh.md`, "In the background" in `website/docs/device-and-environment.md`,
`setAppState` in `website/docs/testing-and-trace.md` (each with its zh-CN page), `docs/dev/elements.md` and the
platforms' pages.

## Public API

- `nsh`: `appState: Accessor<"active" | "inactive" | "background">`, whether the app is in use.
- `nsh/test`: `App.setAppState(state: "active" | "inactive" | "background"): Promise<void>`, the system's change in a
  headless run (a real platform's run answers that it runs headless only).
- `nsh-core`: `platform::AppState` (`Active`, the default; `Inactive`; `Background`) with
  `AppState::name(self) -> &'static str`; `platform::Environment::app_state: AppState` and
  `element::Environment::app_state: AppState` (new fields with defaults); `element::app_state() -> AppState` (also in
  the prelude), reactive.
- `nsh-engine`: `AppState` re-exported.

## How to Test

- `cargo test -p nsh-engine --test engine rust_platform`: `components_follow_whether_the_app_is_in_use` (an effect
  reading `app_state()` runs once per change and not when the font scale changes).
- `cargo test -p nsh-testing --features bundle --test testing`: the environment app's "the app state follows the
  system's" (`setAppState` through inactive, background and active) and `appState()` among the defaults.
- `(cd crates/gpui && cargo test)`: `the_app_s_state_follows_its_window_s_activation_and_visibility`.
- `node --test crates/nsh-platform-web/web/*.test.mjs`.
- Compat: `tests/compat/cases/modules` shows `App state: active` and claims `covers [nsh] appState`.
- On a device: an app with `<Text>{appState()}</Text>` and an effect that logs it, sent to the background, brought
  back, and covered by the app switcher or another window.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst and Android runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
