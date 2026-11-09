# core: Tell views of the wheel over them with onWheel

> **Upstream (checked 2026-11-09):** prepared as [upstream-wheel-events](upstream-wheel-events.md)
> (`pr/wheel-events/01-core`, local), not opened: the owner decides.

Branch: `feat/wheel-events` on `github.com/dXnSix/nsh`, cut from `71aeb06a` (the newest `upstream/main` commit the
fork's `main` holds). Six commits, the core first, then one per platform: `19ff2076` `core: Tell views of the wheel
over them with onWheel`, `5d595123` `macos: Route the wheel through onWheel and wheelScroll`, `07886fa2`
`windows: …`, `0e81d617` `linux: …`, `3cb78e88` `gpui: …`, `733ccd01` `web: …` (head). Local, not pushed yet: to
open as a pull request into the fork's `main` and merge there with a merge commit. Not opened upstream: the owner
decides and posts there.

Independent of `feat/mouse-buttons` (no shared code), but they conflict in 22 files, additions side by side (keep
both): both append an `EventKind` at 76, so whichever merges second takes 77, in `Native.java` too; `input.rs` (its
`use crate::{…}` line and the tests), `builder.rs`, `js/host.rs`, `nsh-testing`'s `driver.rs`, `module.rs`,
`screen.rs` and `tests/testing/main.rs`, `nsh-platform-web/tests/text/events.rs`; the docs where both extend a line
or add a block at one place (`docs/dev/elements.md`, `components.md`, the Input rows of `macos.md`, `windows.md`,
`linux.md` and `gpui.md`, `website/elements/index.md`, `view.md`, `website/docs/testing-and-trace.md` and their zh-CN
pages). Merges cleanly with `feat/app-paths`, `feat/will-terminate`, `feat/locale-direction` and
`feat/high-contrast`.

## Summary

Only scroll views heard the wheel, so an app could not use it itself: zoom a canvas on Ctrl + wheel (RayX's input
map binds it), step a value, switch tabs, pan a map. `onWheel` (Rust `.on_wheel`) tells every view under the pointer
that listens, and `wheelScroll` (Rust `.wheel_scroll`) says beforehand where the wheel goes besides them.

### Design

- **The event**: GPUI's `on_scroll_wheel`, the DOM's `wheel`. `WheelEvent { position, pagePosition, delta, modifiers,
  precise }`: `delta` in points with the DOM's signs (positive `y` towards the content's end), a wheel's notches
  converted as the platform's scroll views take them (macOS' accelerated lines of 10 points, Windows' and GPUI's
  three lines of 16 per notch, Linux's notch as `input::ZOOM_NOTCH`'s 48 points, the web's lines of 40 and pages of
  the viewport; Shift + wheel sideways as macOS reports it); `precise` for a touchpad's or a high-resolution wheel's
  deltas (GPUI's `ScrollDelta::Pixels`). A new `#[non_exhaustive]` type and `EventPayload::Wheel`.
- **Bubbling**: the listeners along the views under the pointer hear it from the inside out, as `onMouseMove`.
- **Stopping it**: the main thread never waits for JS, so a handler cannot cancel the scroll after the fact; the view
  says it beforehand, as `popOnBack` and `onContextMenu`'s mounted listeners do. `wheelScroll`:
  - `"auto"` (the default): as without it, the handler only hears (a passive listener), so nothing changes for any
    app until it opts in;
  - `"contain"`: the wheel stays in the view: it, when it scrolls, and the scroll views inside it may take it; the
    scroll views and the `onWheel` listeners around it get nothing (a stepper in a form; RayX's root input map does
    not hear a wheel its canvas keeps);
  - `"none"`: no scroll view takes it, inside the view or around it; only the listeners of the view and inside it
    hear it (a canvas that pans and zooms itself).
  The words are CSS's `overscroll-behavior`'s for keeping a scroll in; the type can widen later (an object naming,
  say, the modifiers a view takes the wheel with, for a map that zooms on Ctrl + wheel and lets a plain wheel scroll
  the page).
- **Ctrl + wheel's pinch** (a zoomable scroll view or pinch values) is the wheel's too: `contain` keeps the views
  around from zooming, `none` every view.
- **Latching**: a scroll going on (notches within 200 ms, a touchpad gesture and its momentum) goes on over a view
  that keeps wheels in and came under the pointer during it: that view hears none of it and keeps nothing in, as
  browsers latch a scroll's target; a new gesture starts latched to nothing. `input::wheel_route(chain, latched)`
  decides all of it over plain data for every backend and the headless driver.
- **Performance**: without a listener or a `wheelScroll` along the chain a wheel costs a walk of it, no allocation
  past the chain the scroll target already took; the engine merges a view's wheels as it merges mouse moves (one per
  `MOUSE_MOVE_INTERVAL` at most, the deltas added up while the modifier keys and `precise` stay).

### Changes

- **`nsh-core`**: `EventKind::Wheel` (`onWheel`, appended), `WheelEvent` (with `new`, `merged`),
  `EventPayload::Wheel`; `props::WheelScroll` (`PropEnum`), `Prop::WheelScroll` (`wheelScroll`, every element),
  `ViewProps::wheel_scroll`; `input::WheelLink`, `input::WheelRoute`, `input::wheel_route`; `.on_wheel` and
  `.wheel_scroll` on every element; the prelude and the TS declarations (`WheelEvent`, `WheelScroll`).
- **`nsh-engine`**: wheels wait with the mouse moves (`State::mouse_moves` now holds the event kind), a view's merged.
- **`nsh-js`**: the `WheelEvent` object for handlers. The browser runtime (`browser/events.rs`) builds the same.
- **`nsh test`**: `wheel(selector, { x, y, precise })` (`WheelBy`): the listeners hear it through `wheel_route`, the
  scroll view it reaches moves within its content and hears `onScrollEnd`; modifiers from `keyboard.down`; headless,
  on a desktop or web platform (`Driver::wheel` is headless only for now); `Screen::wheel_link`.
- **macOS**: `scrollWheel:` (notches, trackpad gestures, the system's momentum): `wheel_listeners`, `latched_view`;
  `scroll_target`, `gesture_target`, the held view and `wheel_zoom`'s pinch bounded by the route.
- **Windows**: `WM_MOUSEWHEEL` / `WM_MOUSEHWHEEL` (a partial notch is precise), the same bounds; web views and native
  components under the pointer still take the wheel first.
- **Linux**: GDK's scroll events on the root's capture-phase `EventControllerLegacy` (the existing input path, not an
  `EventControllerScroll`; `ScrollUnit::Surface` deltas are precise); a wheel kept in stops propagating to the
  widgets around the app; the test agent's simulated wheels take the same path.
- **GPUI**: `ScrollWheelEvent`s with and without a touch phase, the same bounds, a touchpad's held view too.
- **Web**: the DOM's `wheel` (`nsh.js` `wheelListeners`; pixel deltas are precise, as the DOM does not tell a
  touchpad from a mouse's pixel notches); `ViewProps`' JSON carries `wheelScroll` when it is not `auto`; a wheel kept
  in is `preventDefault`ed, so the page around keeps still; `event.rs` decodes `onWheel`.
- **iOS, Android, the terminal**: report no wheel (iPadOS' scroll pan and Android's `ACTION_SCROLL` would each need
  a new input path).
- **`Native.java`**: `WHEEL = 76`.
- **Compat**: `tests/compat/cases/events/events-wheel.tsx` (`covers [events] onWheel`). `wheelScroll` has no claim:
  `nsh-compat`'s `values.rs` cannot print the new prop, so no step can check it; covering it before a release is
  recorded needs that change in `nsh-compat`.
- **Docs** (en and zh-CN): "Wheel" in `website/elements/index.md`, the `View` page's row, `wheel` in
  `website/docs/testing-and-trace.md`; `docs/dev/elements.md`, `scrolling-and-lists.md`, `api-naming.md`,
  `components.md`, and the platforms' pages (`macos.md`, `windows.md`, `linux.md`, `gpui.md`, `web.md`).

## Public API

- TSX `nsh`: `onWheel?: (e: WheelEvent) => void` and `wheelScroll?: WheelScroll` on every element;
  `WheelEvent { position: Point; pagePosition: Point; delta: Point; modifiers: Modifiers; precise: boolean }`;
  `WheelScroll = "auto" | "contain" | "none"`.
- `nsh/test`: `App.wheel(selector, by: WheelBy)`; `WheelBy { x?: number; y?: number; precise?: boolean }`.
- `nsh-core`: `EventKind::Wheel`; `WheelEvent` (`new`, `merged`); `EventPayload::Wheel(WheelEvent)`; `WheelScroll`;
  `Prop::WheelScroll(WheelScroll)`; `ViewProps::wheel_scroll: WheelScroll`; `Interactive::on_wheel`,
  `Interactive::wheel_scroll`; `input::WheelLink { listens, scroll }` (`new(events, scroll)`), `input::WheelRoute {
  listeners, reach }` (`reaches(index)`), `input::wheel_route(chain: &[WheelLink], latched: Option<usize>) ->
  WheelRoute`.
- `nsh-testing`: `Headless::wheel(&mut self, selector: &str, delta: Point<Points>, precise: bool) -> Result<(),
  String>`.

Breaking for Rust only: `EventPayload` is not `#[non_exhaustive]`, so an exhaustive `match` on it outside `nsh-core`
needs the new variant (as for every payload added before); `ViewProps` literals with `..Default::default()`, as all
in-tree and `nsh-compat`'s are, are unchanged.

## Tests

- New: `cargo test -p nsh-core --lib wheels_reach` (`wheel_route`: the listeners from the inside out, `contain`,
  `none`, a latched scroll, the views around the latched one); `cargo test -p nsh-engine --test engine wheel::` (the
  mask and `wheel_scroll` mounted, the event's fields, wheels merged with their deltas added up and kept apart by the
  modifier keys); `cargo test -p nsh-testing --features bundle --test testing a_wheel_reaches` (four TSX tests in
  `tests/testing/wheel`: the page scrolls and the root hears, `none` with Ctrl held, `contain`, a phone rejecting;
  they fail with wrong expectations); `cargo test -p nsh-js --lib wheel_payload`; `cargo test -p nsh-platform-web
  --test text a_wheel`; `node --test crates/nsh-platform-web/web/scroll.test.mjs`'s "a wheel reaches the onWheel
  listeners, and wheelScroll keeps it from the scroll views around" (with `nsh.test.mjs` and `mouse.test.mjs`: 88
  pass).
- Compat (on `pr/wheel-events/01-core`): `NSH_COMPAT_APP=events cargo xtask compat` passes and fails as it should with
  `expect Wheel: none notches` changed.
- Existing: `cargo test -p nsh-engine --test engine hover::` (2: the mouse moves' merging, now beside the wheels'),
  `cargo test -p nsh-core --lib` `input::` (24), `runtime` (31), `js::` (7), `unit_types` (2); `cargo test -p
  nsh-engine --lib public_api_is_typed`; `cargo test -p nsh-js declarations` (18); GPUI's `cargo test -p
  nsh-platform-gpui --lib input` (39).
- Clippy `-D warnings`: `nsh-core`, `nsh-engine`, `nsh-js`, `nsh-testing` (`bundle`), `nsh-base`, `nsh-platform-tui`
  (all targets); the macOS crate (all targets, `dev`); GPUI's `nsh-platform-gpui` (all targets, `dev`) on macOS;
  `nsh-platform-web` for wasm32 (default features and none; with `dev` only `nsh-testing`'s existing unused
  `failure::covered`); `nsh-platform-ios` for `aarch64-apple-ios`; `cargo xtask clippy-android`. `javac --release
  17` of the Android runtime. Nightly rustfmt `--check` on the changed files.
- Not run: the Windows and Linux crates (CI builds them; their changes call only the backends' own helpers, so no
  scratch crate could check them; Windows' are the twin of GPUI's, which compiled and whose input tests ran), devices,
  simulators and desktop windows with a real wheel or trackpad (`wheel` runs headless only), the website's tests.
