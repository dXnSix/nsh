# core: Let views hear the mouse's other buttons with onAuxPress

> **Upstream (checked 2026-11-09):** prepared as [upstream-mouse-buttons](upstream-mouse-buttons.md)
> (`pr/mouse-buttons/01-core`, local), not opened: the owner decides.

Branch: `feat/mouse-buttons` on `github.com/dXnSix/nsh`, cut from `71aeb06a` (the newest `upstream/main` commit the
fork's `main` holds). Six commits, the core first, then one per platform: `f123432c` `core: Let views hear the
mouse's other buttons with onAuxPress`, `bbbaf443` `macos: …`, `96001d4f` `windows: …`, `fa4cdb4b` `linux: …`,
`471c0b84` `gpui: …`, `d7437d3b` `web: …` (head). Local, not pushed yet: to open as a pull request into the fork's
`main` and merge there with a merge commit. Not opened upstream: the owner decides and posts there.

Merges cleanly with `feat/app-paths`, `feat/will-terminate`, `feat/locale-direction` and `feat/high-contrast`.
Conflicts with `feat/wheel-events` in 22 files, the two features' additions side by side (keep both): the
`events!` list and `Native.java` (both append at 76: whichever merges second takes 77, in both places), `input.rs`
(its `use crate::{…}` line and the tests), `builder.rs`, `js/host.rs`, `nsh-testing`'s `driver.rs`, `module.rs`,
`screen.rs` and `tests/testing/main.rs`, `nsh-platform-web/tests/text/events.rs`; the docs where both extend the same
line or add a block at the same place: `docs/dev/elements.md`, `components.md`, the Input rows of `macos.md`,
`windows.md`, `linux.md` and `gpui.md`, `website/elements/index.md`, `view.md` and `website/docs/testing-and-trace.md`
(each with its zh-CN page).

## Summary

RayX's input map binds actions to pointer input: a middle click, the back and forward buttons, a secondary click.
NSH's presses are the primary button's (and a finger's and the keyboard's), and nothing told a view that another
button pressed it.

`onAuxPress` (Rust `.on_aux_press`) is GPUI's `on_aux_click` and the DOM's `auxclick`: a mouse button other than the
primary one was pressed and released over the view. Its payload is the mouse events' `MouseEvent`, whose
`pressedButton` is the button (`"right"`, `"middle"`, `"back"`, `"forward"`), with where it came up and the modifier
keys.

### Design

- **An event of its own, the payload the mouse events already have.** `MouseEvent` carries the button
  (`pressedButton`, `MouseButton`, GPUI's names) and is `#[non_exhaustive]`. `PressEvent` is not: `nsh-compat`'s
  `Device::press` builds it with a struct literal, so a `button` field would break the compat build CI requires.
  Presses also keep their meaning: a new button never reaches an existing `onPress`, which would otherwise fire on
  right and middle clicks.
- **The DOM's numbering where JS sees it**: `MouseButton`'s names stand for the DOM's `button` 0 (`left`, the
  primary), 1 (`middle`), 2 (`right`, the secondary), 3 (`back`), 4 (`forward`); the web maps those numbers, the docs
  list them. JS keeps one form of a button, the names `pressedButton` already uses.
- **Decided natively, from what is mounted** (`nsh_core::input::AuxPress`, every backend's): as the button goes down,
  the nearest view around the one under the pointer that listens takes the press, unless a text input is nearer (it
  takes its buttons: GTK's middle-click paste); the view hears it when the same button comes up over it or a view
  inside it. `disabled` removes the listener, as it does a press's.
- **What the buttons do natively stays**: the secondary button still asks for `onContextMenu` as it goes down (a
  right click is both), the back button still goes back (`onBackPress`, `popOnBack`), Control-click on macOS stays a
  context menu only.

### Changes

- **`nsh-core`**: `EventKind::AuxPress` (`onAuxPress`, appended); `input::AuxPress<V>` (`down`, `up`);
  `.on_aux_press` on every element; `disabled` drops it (`DISABLED_OFF`); `MouseEvent` / `MouseButton` docs (the
  DOM's numbers); the `HostProps` row and the TS docs of `MouseEvent` and `MouseButton`.
- **`nsh test`**: `press(selector, { button })` (`PressOptions`, `nsh/test`'s `MouseButton`): another button than the
  primary one presses the nearest `onAuxPress` listener (headless, a desktop or web `platform`; `"right"` asks for a
  context menu first, as `rightClick` does); `Driver::aux_press` is headless only for now; `Screen::events`.
- **macOS**: `rightMouseDown:` / `otherMouseDown:` and their ups (`buttonNumber` 2, 3, 4).
- **Windows**: `WM_RBUTTON*`, `WM_MBUTTON*`, `WM_XBUTTON*` (`XBUTTON1` back, `XBUTTON2` forward); raw items (web
  views) keep the buttons pressed on them.
- **Linux**: the root's capture-phase `EventControllerLegacy` (GDK button events 2, 3, 8, 9; the existing input path,
  not a `GestureClick`); their native handling goes on (the back button's `GestureClick`).
- **GPUI**: `MouseDownEvent` / `MouseUpEvent`'s `Right`, `Middle`, `Navigate(Back | Forward)`.
- **Web**: the DOM's `mousedown` / `mouseup` `button` 1–4 (`nsh.js` `auxDown` / `auxUp`; browsers raise `auxclick`
  for some of the buttons only); `event.rs` decodes `onAuxPress` as a `MouseEvent`.
- **iOS, Android, the terminal**: report no other buttons. iOS' presses come from tap recognizers of the primary
  button; on Android a mouse's secondary and middle buttons go down as the touch stream (`ACTION_DOWN`) and press
  views as the primary one does, reporting no button (passing `MotionEvent.getButtonState()` would change the JNI
  press call; left for a follow-up).
- **`Native.java`**: `AUX_PRESS = 76` (the order of `EventKind`, kept in sync).
- **Compat**: `tests/compat/cases/events/events-aux-press.tsx` (`covers [events] onAuxPress`).
- **Docs** (en and zh-CN): "Mouse buttons" in `website/elements/index.md`, the `View` page's row,
  `press(selector, { button })` in `website/docs/testing-and-trace.md`; `docs/dev/elements.md`, `api-naming.md`
  ("Press, not click"), `components.md`, and the platforms' pages (`macos.md`, `windows.md`, `linux.md`, `gpui.md`,
  `web.md`).

## Public API

- TSX `nsh`: `onAuxPress?: (e: MouseEvent) => void` on every element; `MouseEvent.pressedButton` is the button that
  pressed for it.
- `nsh/test`: `App.press(selector, options?: PressOptions)`; `PressOptions { button?: MouseButton }`;
  `MouseButton = "left" | "right" | "middle" | "back" | "forward"`.
- `nsh-core`: `EventKind::AuxPress`; `Interactive::on_aux_press(listener: impl IntoListener<MouseEvent, M>)`;
  `input::AuxPress<V> { view, button }` with `AuxPress::down(button, chain) -> Option<AuxPress<V>>` and
  `AuxPress::up(&self, button, chain) -> bool`.
- `nsh-testing`: `Headless::aux_press(&mut self, selector: &str, button: MouseButton) -> Result<(), String>`.

Not breaking: no existing type gains a field; `EventKind` gains a variant at the end, as the others did.

## Tests

- New: `cargo test -p nsh-core --lib aux_presses` (the nearest listener, the button that comes up, a text input
  nearer, the primary button); `cargo test -p nsh-engine --test engine aux_press` (the mask, `disabled`, each button
  and its positions reach the handler); `cargo test -p nsh-testing --features bundle --test testing
  a_press_with_another` (four TSX tests in `tests/testing/buttons`: middle, back and forward with Shift held, the
  secondary button's context menu first, a text input taking its buttons, a phone rejecting; it fails with a wrong
  expectation); `cargo test -p nsh-platform-web --test text an_aux_press`; `node --test
  crates/nsh-platform-web/web/mouse.test.mjs`'s "the other buttons press the nearest onAuxPress listener once they
  come up over it" (with `nsh.test.mjs` and `scroll.test.mjs`: 88 pass).
- Compat (on `pr/mouse-buttons/01-core`): `NSH_COMPAT_APP=events cargo xtask compat` passes and fails as it should
  with `expect Aux: none middle` changed (`step 5 … not shown`).
- Existing: `cargo test -p nsh-js declarations` (18).
- Clippy `-D warnings`: `nsh-core`, `nsh-engine`, `nsh-testing` (`bundle`), `nsh-platform-tui` (all targets); the
  macOS crate (all targets, `dev`); GPUI's `nsh-platform-gpui` (`dev`) on macOS; `nsh-platform-web` for wasm32;
  `nsh-platform-ios` for `aarch64-apple-ios`; `cargo xtask clippy-android`. The Android Java runtime compiled with
  `javac --release 17` against android-35. Nightly rustfmt `--check` on the changed files.
- Not run: the Windows and Linux crates (CI builds them; their changes call only the backends' own helpers, no new
  system API, so no scratch crate could check them: Windows' are the twin of GPUI's, which compiled), devices,
  simulators and desktop windows with a real mouse (`press(…, { button })` runs headless only), the website's tests.
