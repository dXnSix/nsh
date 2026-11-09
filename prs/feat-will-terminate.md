# core: Tell apps the app is about to terminate

> **Upstream (checked 2026-11-09):** prepared as [upstream-will-terminate](upstream-will-terminate.md)
> (`pr/will-terminate/01-core`, local), not opened: the owner decides.

Branch: `feat/will-terminate` on `github.com/dXnSix/nsh`, cut from `71aeb06a` (the newest `upstream/main` commit the
fork's `main` holds). Nine commits, the core first, then one per platform, as #747 (`lifecycle()`) was: `bf3388ba`
`core: Tell apps the app is about to terminate`, `e42ed02a` `macos: …`, `334c2dce` `ios: …`, `1c05456d` `android: …`,
`35d8ea0a` `windows: …`, `c3edf09f` `linux: …`, `2a432137` `gpui: …`, `f58b4328` `web: …`, `cefcabb8` `tui: …` (head).
Local, not pushed yet: to open as a pull request into the fork's `main` and merge there with a merge commit. Not opened
upstream: the owner decides and posts there.

Conflicts with `feat/app-paths` in `prelude.rs` only (both add names to the same `pub use` list): whichever merges
second takes both names, then nightly rustfmt over the list.

## Summary

`lifecycle()` says whether the app is in use; nothing told an app that it is about to end. RayX's suspension service
needs that moment, a "shutting down" state in which the app gets a last, short, synchronous chance to save (flush its
storage, write a document) before the process goes.

`on_terminate(f)` (Rust, the prelude) and `Lifecycle.on("terminate", f)` (`nsh`) run `f` once as the app is about to
terminate. The platform calls `EngineControl::will_terminate_and_wait(TERMINATE_TIMEOUT)` (two seconds) on its main
thread; the engine runs the listeners on its thread, then what they started, then writes the app's storage out, and
answers.

### Design: an event, not a fourth `lifecycle()` value

- `platform::Lifecycle` is not `#[non_exhaustive]`: a `Terminating` variant breaks every exhaustive `match` outside
  `nsh-core` (RayX's mapping of it is one).
- JS compares the accessor's strings (`lifecycle() === "background"`, `!== "active"`): a new value reads as a state the
  code does not mean, at the very moment it should save.
- A state reaches code through effects, which run in a later turn, after the platform's callback returned: the
  process may be gone by then. An event the platform waits for runs before.
- So `lifecycle()` keeps its three values and the end is a listener, after the existing patterns: Rust's
  `on_<event>` (`on_back_press`, owner-scoped, GPUI's naming), TSX's `X.on(event, l)` returning `() => void`
  (`UrlScheme.on("open")`, `Notifications.on("press")`), a new `Lifecycle` object beside `lifecycle()`.

### Changes

- **`nsh-core`**: `element/terminate.rs` keeps the listeners (in the executor, as `back.rs` keeps the back button's);
  `on_terminate(f: impl FnOnce() + 'static)` adds one under the current owner (removed when it goes);
  `Core::will_terminate()` runs them once in the order added, each under its owner (a panic is logged), and wakes the
  tasks waiting for it. `runtime/lifecycle.rs` is `nsh`'s `Lifecycle` module: `on("terminate", listener)` keeps the
  VM's JS listeners and starts one task per VM that awaits the end and posts them (the `UrlScheme` pattern: a module
  function runs outside the reactive graph, so it cannot reach the executor). `StorageBackend::flush(&self)`, a
  provided no-op.
- **`nsh-engine`**: `Message::WillTerminate(done)`, `EngineControl::will_terminate(done)` (calls `done` at once when
  the engine stopped), `will_terminate_and_wait(timeout) -> bool` (native only; never on the engine's thread),
  `LocalEngine::will_terminate()` (in place: the web, tests), `TERMINATE_TIMEOUT`. The driver runs it once: the core's
  listeners, then a flush (tasks, the JS calls they post, promise reactions, in rounds while that queues more), then
  `StorageBackend::flush`. `FileStorage::flush` waits for its writer thread to have written what it was handed (a
  `Job::Flush` marker answered after the burst's write): a process that ends without dropping the engine (AppKit's
  `exit` after `-terminate:`, iOS's) could otherwise lose the last writes, the writer being a thread of its own.
- **macOS**: the standalone app's `stop_running` (Quit, the window closed, `quit_app()`, restart, logout) runs them
  before the engine stops; an embedded surface observes `NSApplicationWillTerminateNotification` with no queue, so the
  block runs in AppKit's post and AppKit, which exits right after, waits.
- **iOS**: each surface observes `UIApplicationWillTerminateNotification` (posted only for an app running in the
  background) and `UISceneDidDisconnectNotification` of its own window's scene, with no queue.
- **Android**: no process signal exists. `NshView`'s `ActivityLifecycleCallbacks` call it when its activity pauses,
  stops or is destroyed while `isFinishing()`; `AppActivity` / `DevActivity` before `destroy()` for an activity
  finished while stopped. New JNI `Native.willTerminate(long)`; `NshView.willTerminate()` is package-private.
- **Windows**: a top-level surface window's `WM_DESTROY` (before its engine stops; the loop ends after the last),
  `run_app` once more after the loop, and `WM_ENDSESSION` with a true `wParam` (an embedded surface hears its root's
  through the existing subclass).
- **Linux**: the standalone main window's destroy (Ctrl+Q and `Application.quit()` close it), and the thread's default
  `GApplication`'s `shutdown`, connected once per application (an embedding app's).
- **GPUI**: `Context::on_app_quit` per `AppView` (the listeners run in the callback, not in GPUI's 200 ms future), and
  `own_window`'s should-close handler once the window may close.
- **Web**: `pagehide` without `persisted` (`Player.pageHidden` → `nsh_will_terminate` → `LocalEngine::will_terminate`).
  Not `beforeunload`: cancellable, and it keeps pages out of the back-forward cache.
- **Terminal**: `run_app` before `view.stop()`, through the new public `AppView::will_terminate()`.
- **Compat**: `tests/compat/cases/modules/mod-lifecycle.tsx` (`covers [nsh] Lifecycle`).
- **Docs** (en and zh-CN): `Lifecycle.on` in `website/modules/nsh.md`, "When the app terminates" (the per-platform
  table, and saving on `background` too) in `website/docs/device-and-environment.md`, the `nsh` row of
  `website/modules/index.md`; `docs/dev/elements.md`, `components.md`, and each platform's page (`macos.md`,
  `windows.md`, `linux.md`, `gpui.md`, `web.md`, `tui.md`).

## Public API

- TSX `nsh`: `Lifecycle.on(event: "terminate", listener: () => void): () => void`.
- `nsh-core`: `element::on_terminate(f: impl FnOnce() + 'static)` (also in the prelude); `Core::will_terminate(&mut
  self)`; `storage::StorageBackend::flush(&self)` (provided, does nothing by default).
- `nsh-engine`: `EngineControl::will_terminate(&self, done: impl FnOnce() + Send + 'static)`,
  `EngineControl::will_terminate_and_wait(&self, timeout: Duration) -> bool` (not in WebAssembly),
  `LocalEngine::will_terminate(&mut self)`, `TERMINATE_TIMEOUT: Duration` (2 s); `FileStorage` implements `flush`.
- `nsh-platform-tui`: `AppView::will_terminate(&self)`.

No breaking change: `StorageBackend`'s new method has a default, `lifecycle()` and `Lifecycle` are unchanged.

## Tests

- New: `cargo test -p nsh-engine --test engine terminate` (3):
  `rust_listeners_run_once_and_the_storage_is_written_before_the_engine_answers` (`Engine`: two listeners in order, a
  task one starts, a removed child's listener not run, `storage.json` written before the engine stops, a second call
  runs nothing, a stopped engine answers at once), `a_local_engine_terminates_when_its_host_says_so`,
  `js_listeners_run_once_with_what_their_promises_settle_at_once` (a stopped listener not run, an `await
  Storage.setItem` continuation run); `cargo test -p nsh-engine --lib storage`'s
  `a_flush_returns_once_the_changes_handed_over_are_in_the_file`; `node --test
  crates/nsh-platform-web/web/nsh.test.mjs`'s "a page that goes for good runs the app's terminate listeners, one kept
  for later does not" (69 pass).
- Compat (on `pr/will-terminate/01-core`, [upstream-will-terminate](upstream-will-terminate.md)):
  `NSH_COMPAT_APP=modules cargo xtask compat` passes, with `mod-lifecycle`'s steps.
- Existing: the engine groups `storage` (7), `shutdown` (2), `rust_platform` (14), `url_scheme` (3), `clock` (6),
  `external_wake` (4), `notifications` (2); `cargo test -p nsh-engine --lib storage` (10) and `public_api_is_typed`;
  `cargo test -p nsh-core --lib runtime` (31, every export documented) and `unit_types` (2); `cargo test -p nsh-js
  declarations` (18); the browser runtime's JS tests (48 pass, the rest need Wasm).
- Clippy `-D warnings`: `nsh-core`, `nsh-engine` (all targets); the macOS crates (all targets, `dev`);
  `nsh-platform-tui` and `nsh-tui-host` (all targets); `nsh-platform-ios` for `aarch64-apple-ios`;
  `cargo xtask clippy-android`; `nsh-engine` for wasm32 without features, `nsh-platform-web` for wasm32 with its
  default features and with none. The Android Java runtime compiled with `javac --release 17` against android-35.
  Nightly rustfmt `--check` on the changed files.
- Type-checked on macOS in scratch crates (deleted after): the Windows messages (`WM_ENDSESSION` in the root subclass
  and the window procedure) with `windows` 0.62.2, `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings`; the
  Linux `GApplication` calls (`Application::default`, `connect_shutdown`, the weak reference) with gio / glib 0.22.10
  for `aarch64-unknown-linux-gnu` (`SYSTEM_DEPS_*_NO_PKG_CONFIG`): clean.
- Not run: GPUI's workspace (not built here: its `on_app_quit` / `on_window_should_close` code is unchecked until CI),
  the Windows and Linux crates as a whole, the Catalyst target, any real quit on a device, simulator or desktop.
