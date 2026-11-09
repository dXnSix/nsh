# Upstream PR: core: Tell apps the app is about to terminate

Not opened: prepared 2026-11-09 for the owner to post. The body to paste starts below the line.

- Branch: `pr/will-terminate/01-core` (local, not pushed, tracking nothing), nine commits on `upstream/main`
  `27fecbbe` (#982), the fork's `feat/will-terminate` cherry-picked: `eedefc94` core, `68861271` macos, `205a8354` ios,
  `d70b1fa3` android, `ea010c57` windows, `468a5bef` linux, `40a99caf` gpui, `e0c82407` web, `9c161b64` tui (head).
  One conflict, the prelude's list beside upstream's `app_url` / `set_app_url` (#982) and `haptic` (#1328): upstream's
  list kept with `on_terminate` added, then nightly rustfmt over it.
- One pull request with a commit per platform, as #747 (`lifecycle()`) was: each platform's part is a few lines on
  the core's `EngineControl::will_terminate_and_wait`, and none means anything without the core.
- Size: 52 files, +1109 / −47 (code +655 / −39, tests +308, docs +146 / −8).
- Open it from `dXnSix:pr/will-terminate/01-core` into `longbridge/nsh:main`, title (upstream squash-merges):
  `core: Tell apps the app is about to terminate`.
- Compatibility case: `tests/compat/cases/modules/mod-lifecycle.tsx` (`covers [nsh] Lifecycle`): `on("terminate")`
  returns a function, nothing arrives while the app runs, the listener stops.
- Compat: `NSH_COMPAT_APP=modules cargo xtask compat` passes (`checkout_apps_run`, 32 tests), and fails as it should
  with the case's `expect Terminated: no` changed to another text (not committed).
- Checked locally on `27fecbbe` (Rust 1.99.1, macOS): the new tests and the engine groups below, the unit and
  declaration tests, clippy `-D warnings` of `nsh-core` and `nsh-engine` (all targets), the macOS crates, the terminal
  crates, `nsh-platform-ios` for `aarch64-apple-ios`, `cargo xtask clippy-android`, `nsh-engine` and `nsh-platform-web`
  for wasm32; `javac` of the Android runtime; the web's JS tests (`nsh.test.mjs` 75, the browser runtime's 50); nightly
  rustfmt `--check` on the changed files. The Windows and Linux calls type-checked in scratch crates (`windows` 0.62.2
  for `x86_64-pc-windows-msvc`; gio / glib 0.22.10 for Linux) ([feat-will-terminate](feat-will-terminate.md)).
- Not run: GPUI's workspace (not built here), the Windows and Linux crates as a whole (CI builds them), the Catalyst
  target, devices, simulators and a real quit on any desktop.
- Conflicts with `pr/app-paths/01-core` (`prelude.rs`, `nsh-engine/src/lib.rs`): whichever merges second is rebased.

---

**Depends on:** no other PR; it applies to `main` as it is.

## Description

`lifecycle()` says whether the app is in use, but nothing tells an app that it is about to end: it cannot write a
document, or the changes it keeps in memory, as the user quits, closes its window or the system ends it.

`on_terminate(f)` (`nsh_core::prelude`) and `Lifecycle.on("terminate", f)` (`nsh`) run `f` once, as the app is about
to terminate:

```tsx
import { Lifecycle, Storage } from "nsh";

Lifecycle.on("terminate", () => {
  Storage.setItem("draft", draft());
});
```

```rust
on_terminate(move || {
    let _ = Storage::set_item("draft", draft.get_untracked());
});
```

The platform calls `EngineControl::will_terminate_and_wait(TERMINATE_TIMEOUT)` (two seconds) and waits. The engine
runs the Rust listeners in the order they were added, each under its owner (removed when the owner goes), then what
they started as a flush runs it: their tasks, the JS listeners (posted by a task of the `Lifecycle` module, which waits
for the end) and the promise reactions that follow at once, in rounds while that queues more. Then it writes the app's
storage out: `StorageBackend` gains `flush`, a no-op by default, which `FileStorage` answers once its writer thread has
written what it was handed; a process that ends without dropping the engine (AppKit's `exit` after `-terminate:`,
iOS's) could otherwise lose the last writes. Told again, the engine runs nothing. `LocalEngine::will_terminate` handles
it in place, for the web and for tests.

**Why an event, not a fourth `lifecycle()` value.** `platform::Lifecycle` is not `#[non_exhaustive]`, so a new variant
breaks every exhaustive `match` outside `nsh-core`; JS code compares `lifecycle()` with `"background"` or `"active"`,
and would read the end as a state it does not mean, at the moment it should save; and a state reaches code through
effects, which run in a later turn, after the platform's callback returned and possibly after the process went. So
`lifecycle()` keeps its three values and the end is a listener, after the existing patterns: Rust's `on_<event>`
(`on_back_press`), TSX's `on(event, l)` returning `() => void` (`UrlScheme.on("open")`), on a `Lifecycle` object beside
`lifecycle()`.

Each platform, a commit each:

- **macOS**: the standalone app's `stop_running`, where every way it ends comes (Quit, its window closed,
  `quit_app()`, a restart, a logout), before the engine stops. An embedded surface observes
  `NSApplicationWillTerminateNotification` with no queue, so the block runs in AppKit's post and AppKit, which exits
  right after, waits for it.
- **iOS**: each surface observes `UIApplicationWillTerminateNotification` (UIKit posts it for an app running in the
  background) and `UISceneDidDisconnectNotification` of its own window's scene (a window closed on an iPad or under
  Catalyst, a scene the system releases), with no queue.
- **Android**: the process gets no reliable signal. `NshView`'s `ActivityLifecycleCallbacks`, which already follow
  its activity for `lifecycle()`, call it when the activity pauses, stops or is destroyed while `isFinishing()`
  (the earliest point, before its views are destroyed); `AppActivity` and `DevActivity` before `destroy()`, for an
  activity finished while stopped. A configuration change is no end. JNI `Native.willTerminate`.
- **Windows**: a top-level surface window's `WM_DESTROY`, before its engine stops (the loop ends after the last one;
  `run_app` asks once more after the loop), and `WM_ENDSESSION` with a true `wParam` (the user signs out, Windows shuts
  down or restarts; `WM_QUERYENDSESSION` may still be cancelled). An embedded surface hears its root window's through
  the subclass that already passes `WM_ACTIVATEAPP` on.
- **Linux**: the standalone app's main window destroyed (Ctrl+Q and `Application.quit()` close it), and the thread's
  default `GApplication`'s `shutdown` (an embedding app's), whose handlers run before GTK destroys the windows left.
- **GPUI**: `Context::on_app_quit`, registered by every `AppView` (the listeners run in the callback itself, before
  GPUI polls the futures for its `SHUTDOWN_TIMEOUT`), and the window the view owns closing (`own_window`).
- **Web**: `pagehide` without `persisted`: the tab closes, or another page or a reload replaces this one. Not
  `beforeunload`, which may be cancelled and keeps pages out of the back-forward cache.
- **Terminal**: `run_app` before it stops the engine, however its loop ends (Ctrl+C, `Application.quit()`, the host's
  stop signals), through the new `AppView::will_terminate`.

The system also ends apps without a word (one suspended in the background, Android's background processes, phone
browsers' discarded tabs), so the docs present `terminate` as the last save and say to save as changes are made and on
`background`.

Docs: `Lifecycle.on` in `website/modules/nsh.md`, "When the app terminates" (with the platforms' table) in
`website/docs/device-and-environment.md`, the `nsh` row of `website/modules/index.md` (each with its zh-CN page);
`docs/dev/elements.md`, `components.md` and the platforms' pages.

## Public API

- `nsh`:
  - `Lifecycle.on(event: "terminate", listener: () => void): () => void`: `listener` runs once, synchronously, as the
    app is about to terminate; the function returned stops it, as does the end of the component that added it.
- `nsh-core`:
  - `element::on_terminate(f: impl FnOnce() + 'static)` (also in `prelude`): runs `f` once as the app is about to
    terminate, while the current owner lives.
  - `Core::will_terminate(&mut self)`: runs the listeners once (the engine calls it).
  - `storage::StorageBackend::flush(&self)`: returns once the changes handed to `write` are kept; provided, does
    nothing by default.
- `nsh-engine`:
  - `EngineControl::will_terminate(&self, done: impl FnOnce() + Send + 'static)`: the app is about to terminate; `done`
    runs after the listeners, what they started and the storage's flush.
  - `EngineControl::will_terminate_and_wait(&self, timeout: Duration) -> bool` (not in WebAssembly): the same, waiting
    at most `timeout`; whether it finished in time.
  - `LocalEngine::will_terminate(&mut self)`: the same, handled before it returns.
  - `TERMINATE_TIMEOUT: Duration`: what NSH's platforms wait, two seconds.
  - `FileStorage` implements `StorageBackend::flush`.
- `nsh-platform-tui`:
  - `AppView::will_terminate(&self)`: for an embedder's own loop, before `stop`.

No breaking change: `StorageBackend::flush` has a default, and `lifecycle()` and `platform::Lifecycle` are unchanged.

## How to Test

- `cargo test -p nsh-engine --test engine terminate`: Rust listeners run once in order, with a task one starts and
  without a removed component's, `storage.json` is written before the engine answers and before it stops, a second
  call runs nothing, a stopped engine answers at once (`Engine` and `LocalEngine`); JS listeners run once, without a
  stopped one, with an `await Storage.setItem` continuation.
- `cargo test -p nsh-engine --lib storage`: `a_flush_returns_once_the_changes_handed_over_are_in_the_file`.
- `node --test crates/nsh-platform-web/web/nsh.test.mjs`: a page that goes for good runs the listeners, one kept in the
  back-forward cache does not.
- Compat: `tests/compat/cases/modules`' `mod-lifecycle` claims `covers [nsh] Lifecycle` (`NSH_COMPAT_APP=modules
  cargo xtask compat`).
- The engine's `storage`, `shutdown`, `rust_platform` and `url_scheme` groups; `cargo test -p nsh-core --lib runtime`
  (every export documented); `cargo test -p nsh-js declarations`.
- On a device or desktop: an app whose listener writes to `Storage`, quit (Command-Q, the window's close button,
  Ctrl+Q, a closed tab, the back button on Android's last screen), then started again shows what it wrote.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the iOS, Android and wasm32 runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no demo changes; the engine's tests run both
  engines, and no device run was made.
