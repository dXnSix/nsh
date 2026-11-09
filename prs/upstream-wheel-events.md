# Upstream PR: core: Tell views of the wheel over them with onWheel

Not opened: prepared 2026-11-09 for the owner to post. The body to paste starts below the line.

- Branch: `pr/wheel-events/01-core` (local, not pushed, tracking nothing), six commits on `upstream/main` `02a9e978`
  (#1362), the fork's `feat/wheel-events` cherry-picked: `37e7eea4` core, `090ba8b7` macos, `948fd3e6` windows,
  `c9d8f92c` linux, `ad60cd33` gpui, `c31fb3a8` web (head). Four conflicts, resolved in upstream's favour: the
  export lists of `nsh-core`'s `lib.rs` and `element/mod.rs` beside #1328's `Haptic` names (upstream's lists with
  `WheelEvent` / `WheelScroll` added, then nightly rustfmt), the test runner's `tests/testing/main.rs` beside #1327's
  test (both kept), and the Input row of `docs/dev/macos.md` beside #1326's Scroll views row (upstream's rows with this
  change's clause added).
- One pull request with a commit per platform, as #747 (`lifecycle()`) was: each platform's part routes its wheel
  through the core's `input::wheel_route`, and none means anything without the core. Not split by platform: the core
  commit is 35 files (+940 / −20), each platform's 2 to 7 files of 110 to 170 lines.
- Size: 50 files, +1633 / −103 (code +1111 / −94, tests +394, docs +128 / −9).
- Open it from `dXnSix:pr/wheel-events/01-core` into `longbridge/nsh:main`, title (upstream squash-merges):
  `core: Tell views of the wheel over them with onWheel`.
- Compatibility case: `tests/compat/cases/events/events-wheel.tsx` (`covers [events] onWheel`): a wheel's notches over
  the view and a touchpad over a view inside it reach the listener, `precise` telling them apart. `wheelScroll` has no
  claim: `nsh-compat`'s `values.rs` cannot print the new prop, so no step can check it; a record would need that
  change there first.
- Compat: `NSH_COMPAT_APP=events cargo xtask compat` passes, and fails as it should with the case's `expect Wheel:
  none notches` changed to `touchpad` (`step 277 \`expect Wheel: none touchpad\`: not shown`; not committed).
- Checked locally on `02a9e978` (Rust 1.99.1, macOS): the new unit, engine, test runner, `nsh-js` and web tests, the
  engine's `hover::` group, `cargo test -p nsh-js declarations`, the web runtime's JS tests (94 pass); clippy `-D
  warnings` of `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-testing` (`bundle`), `nsh-platform-tui` and the macOS crate
  (all targets), GPUI's `nsh-platform-gpui` (all targets, `dev`) on macOS, `nsh-platform-web` for wasm32,
  `nsh-platform-ios` for `aarch64-apple-ios`; nightly rustfmt `--check` on the changed files. On the fork's base
  (`71aeb06a`) also `nsh-base`, `cargo xtask clippy-android`, `javac` of the Android runtime and GPUI's input tests
  ([feat-wheel-events](feat-wheel-events.md)).
- Not run: the Windows and Linux crates (CI builds them; Windows' change is the twin of GPUI's), devices, simulators,
  desktop windows with a real wheel or trackpad, the website's tests.
- Conflicts with `pr/mouse-buttons/01-core` in 22 files, the two features' additions side by side; both append an
  `EventKind` at 76, so whichever merges second takes 77 (and `Native.java`'s constant with it). With
  `pr/app-paths/01-core` merge-tree reports `prelude.rs` and `nsh-engine/src/lib.rs` from that branch's older `main`
  (`d96fc35c`), whatever this one adds. Clean with `pr/will-terminate/01-core`, `pr/locale-direction/01-core` and
  `pr/high-contrast/01-core`.
- Left for the owner: (1) the prop's name and words, `wheelScroll: "auto" | "contain" | "none"` (CSS's
  `overscroll-behavior` words; `scrollOnWheel`, after `popOnBack`, was the other candidate); (2) `contain` also keeps
  the wheel from the `onWheel` listeners around the view, as a stopped propagation would; (3) taking the wheel only
  with some modifier keys held (a map that zooms on Ctrl + wheel and lets a plain wheel scroll the page) needs the type
  to widen to an object later; (4) the web's `precise` is the DOM's pixel `deltaMode`, which Chromium and Safari also
  give a mouse's notches; (5) iOS (a scroll pan recognizer) and Android (`ACTION_SCROLL`) report no wheel yet.

---

**Depends on:** no other PR; it applies to `main` as it is (independent of the `onAuxPress` PR, which touches the
same files beside it).

## Description

Only scroll views hear the mouse wheel and a touchpad's two-finger scroll, so an app cannot use them itself: zoom a
canvas on Ctrl + wheel, step a value, switch tabs, pan a map.

`onWheel` (`.on_wheel` in Rust) is GPUI's `on_scroll_wheel` and the DOM's `wheel`. Its `WheelEvent` has `position`
in the view, `pagePosition` in its root view, `delta`, how far it scrolls in points with the DOM's signs (positive `y`
towards the content's end; a wheel's notches converted as the platform's scroll views take them), the `modifiers`
and `precise` (a touchpad's or a high-resolution wheel's deltas, not notches). The views under the pointer that
listen hear it from the inside out, and the scroll views take it as before.

The main thread never waits for JS, so a handler cannot cancel the scroll after the fact. The view says beforehand
where the wheel goes besides its handler, with `wheelScroll` (`.wheel_scroll` in Rust):

```tsx
<Canvas
  wheelScroll="none"
  onWheel={(e) => (e.modifiers.control ? zoomBy(1 - e.delta.y / 500) : panBy(e.delta))}
/>
<NumberField wheelScroll="contain" onWheel={(e) => step(-Math.sign(e.delta.y))} />
```

```rust
view()
    .wheel_scroll(WheelScroll::None)
    .on_wheel(move |e: &WheelEvent| {
        if e.modifiers.ctrl {
            zoom.update(|z| *z *= 1.0 - e.delta.y.0 / 500.0);
        }
    })
```

- `"auto"` (the default): as without it; the handler only hears.
- `"contain"`: the wheel stays in the view: it, when it scrolls, and the scroll views inside it may take it; the
  scroll views and the `onWheel` listeners around it get nothing.
- `"none"`: no scroll view takes it, inside the view or around it; only the listeners of the view and inside it hear
  it.

Ctrl + wheel's pinch (a zoomable scroll view, pinch values) is the wheel's too: `contain` keeps the views around the
view from zooming, `none` every view. A scroll already going on (wheel notches latched within 200 ms, a touchpad
gesture and its momentum) goes on over a view that keeps wheels in and came under the pointer during it: that view
hears none of it and keeps nothing in, as browsers latch a scroll's target. `nsh_core::input::wheel_route` decides
it over plain data for every backend and the headless driver, from the mounted event masks and
`ViewProps::wheel_scroll`; without a listener or a `wheelScroll` along the chain a wheel costs a walk of it. The
engine merges a view's wheels as it merges mouse moves: one per frame at most, the deltas added up while the modifier
keys stay.

- **macOS**: `scrollWheel:`, its gestures and momentum (lines of 10 points, `hasPreciseScrollingDeltas`).
- **Windows**: `WM_MOUSEWHEEL` / `WM_MOUSEHWHEEL` (three lines of 16 points a notch; a partial notch is precise); web
  views and native components under the pointer take the wheel first, as before.
- **Linux**: GDK's scroll events through the root's capture-phase controller (a notch as `input::ZOOM_NOTCH`'s 48
  points, `ScrollUnit::Surface` deltas precise); a wheel kept in no longer goes on to the widgets around the app.
- **GPUI**: `ScrollWheelEvent`s with and without a touch phase (`ScrollDelta::Pixels` precise).
- **Web**: the DOM's `wheel` (`nsh.js` `wheelListeners`; lines of 40 points, pages of the viewport, pixel deltas
  precise); a wheel kept in is `preventDefault`ed, so the page around keeps still.
- iOS, Android and the terminal report no wheel.

`nsh/test`'s `wheel(selector, { x, y, precise })` turns the wheel over a view headless, on a desktop or web platform,
through the same route: the scroll view it reaches moves within its content and hears `onScrollEnd`.

Docs: "Wheel" in `website/elements/index.md`, the `View` page's row and `wheel` in `website/docs/testing-and-trace.md`
(each with its zh-CN page); `docs/dev/elements.md`, `scrolling-and-lists.md`, `api-naming.md`, `components.md` and the
platforms' pages.

## Public API

- `nsh`:
  - `onWheel?: (e: WheelEvent) => void` on every element: the wheel or a touchpad scrolled over the view or a view
    inside it.
  - `wheelScroll?: WheelScroll` on every element: where a wheel over the view goes besides its `onWheel`.
  - `WheelEvent { position: Point; pagePosition: Point; delta: Point; modifiers: Modifiers; precise: boolean }`: the
    payload.
  - `WheelScroll = "auto" | "contain" | "none"`: as without it, only the view and the scroll views inside it, or no
    scroll view.
- `nsh/test`:
  - `App.wheel(selector: string, by: WheelBy): Promise<void>`: turns the wheel over the view.
  - `WheelBy { x?: number; y?: number; precise?: boolean }`: by how many points, and whether as a touchpad.
- `nsh-core`:
  - `EventKind::Wheel`: the event (`onWheel`), appended.
  - `WheelEvent { position, page_position, delta: Point<Points>, modifiers, precise: bool }` (`#[non_exhaustive]`):
    the payload; `WheelEvent::new(..)`, `WheelEvent::merged(self, later) -> WheelEvent` (the engine's merging).
  - `EventPayload::Wheel(WheelEvent)`: its payload variant.
  - `WheelScroll` (`Auto`, `Contain`, `None`; `PropEnum`): the prop's values.
  - `Prop::WheelScroll(WheelScroll)` and `ViewProps::wheel_scroll: WheelScroll`: the prop, as the platforms get it.
  - `Interactive::on_wheel(self, listener: impl IntoListener<WheelEvent, M>) -> Self` and
    `Interactive::wheel_scroll(self, scroll: WheelScroll) -> Self`: the Rust face.
  - `input::WheelLink { listens: bool, scroll: WheelScroll }` (`WheelLink::new(events, scroll)`): a view as the route
    reads it.
  - `input::WheelRoute { listeners: Vec<usize>, reach: usize }` (`reaches(index) -> bool`): where a wheel goes.
  - `input::wheel_route(chain: &[WheelLink], latched: Option<usize>) -> WheelRoute`: the decision.
- `nsh-testing`:
  - `Headless::wheel(&mut self, selector: &str, delta: Point<Points>, precise: bool) -> Result<(), String>`: the step,
    in Rust.

## Breaking Changes

`EventPayload` is not `#[non_exhaustive]`: an exhaustive `match` on it outside `nsh-core` needs the new variant, as
for every payload added before it. `ViewProps` gains a field; literals with `..Default::default()` (every one in
NSH and in `nsh-compat`) are unchanged.

```diff
 match payload {
     EventPayload::Mouse(e) => mouse(e),
+    EventPayload::Wheel(e) => wheel(e),
     // ...
 }
```

## How to Test

- `cargo test -p nsh-core --lib wheels_reach`: the route's listeners from the inside out, `contain`, `none`, a
  latched scroll and the views around the latched one.
- `cargo test -p nsh-engine --test engine wheel::` (and `hover::`): the handler and `wheelScroll` are mounted, the
  event reaches the handler, and fast wheels are merged with their deltas added up, kept apart by the modifier keys.
- `cargo test -p nsh-testing --features bundle --test testing a_wheel_reaches`: `wheel(selector, …)` scrolls a page
  while the root hears it, `none` with Ctrl held takes it, `contain` keeps it from the page, a phone rejects.
- `cargo test -p nsh-js --lib wheel_payload`, `cargo test -p nsh-platform-web --test text a_wheel` and `node --test
  crates/nsh-platform-web/web/scroll.test.mjs`: the payload, the page's decoding, and the web's route (listeners,
  `none`, a latched scroll going on).
- Compat: `tests/compat/cases/events`' `events-wheel` claims `covers [events] onWheel` (`NSH_COMPAT_APP=events cargo
  xtask compat`).
- On a desktop: a canvas with `wheelScroll="none"` and a field with `contain` in a scrolling page, turned with a
  wheel, a touchpad and Ctrl + wheel on macOS, Windows, Linux, GPUI and in a browser.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the iOS, Android and wasm32 runs
  included (Windows and Linux on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`); `wheelScroll`'s check needs `nsh-compat` to read the prop.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no demo changes, and iOS and Android report no
  wheel; no device run was made.
