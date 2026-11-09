# Upstream PR: core: Let views hear the mouse's other buttons with onAuxPress

Not opened: prepared 2026-11-09 for the owner to post. The body to paste starts below the line.

- Branch: `pr/mouse-buttons/01-core` (local, not pushed, tracking nothing), six commits on `upstream/main` `02a9e978`
  (#1362), the fork's `feat/mouse-buttons` cherry-picked: `dbac1b2a` core, `cb9418a8` macos, `94e672e1` windows,
  `ba781c64` linux, `9274afa1` gpui, `90966dae` web (head). Two conflicts, resolved in upstream's favour: the
  test runner's `tests/testing/main.rs` beside #1327's `a_press_reaches_a_view_across_a_scroll_view_s_fold` (both
  kept, ours after it), and the Input row of `docs/dev/macos.md` after #1326 rewrote the Scroll views row beside it
  (upstream's rows with this change's clause added).
- One pull request with a commit per platform, as #747 (`lifecycle()`) was: each platform's part is a few lines on
  the core's `input::AuxPress`, and none means anything without the core.
- Size: 40 files, +978 / −44 (code +524 / −36, tests +357, docs +97 / −8).
- Open it from `dXnSix:pr/mouse-buttons/01-core` into `longbridge/nsh:main`, title (upstream squash-merges):
  `core: Let views hear the mouse's other buttons with onAuxPress`.
- Compatibility case: `tests/compat/cases/events/events-aux-press.tsx` (`covers [events] onAuxPress`): the middle
  button over the view and the back button over a view inside it reach the listener with their button.
- Compat: `NSH_COMPAT_APP=events cargo xtask compat` passes, and fails as it should with the case's `expect Aux:
  none middle` changed to `right` (`step 5 \`expect Aux: none right\`: not shown`; not committed).
- Checked locally on `02a9e978` (Rust 1.99.1, macOS): the new unit, engine, test runner and web tests, `cargo test -p
  nsh-js declarations`, the web runtime's JS tests (94 pass); clippy `-D warnings` of `nsh-core`, `nsh-engine`,
  `nsh-testing` (`bundle`) and the macOS crate (all targets), GPUI's `nsh-platform-gpui` (all targets, `dev`) on
  macOS, `nsh-platform-web` for wasm32, `nsh-platform-ios` for `aarch64-apple-ios`; nightly rustfmt `--check` on the
  changed files. On the fork's base (`71aeb06a`) also `nsh-platform-tui`, `cargo xtask clippy-android` and `javac`
  of the Android runtime ([feat-mouse-buttons](feat-mouse-buttons.md)).
- Not run: the Windows and Linux crates (CI builds them; Windows' change is the twin of GPUI's), devices, simulators,
  desktop windows with a real mouse, the website's tests.
- Conflicts with `pr/wheel-events/01-core` in 22 files, the two features' additions side by side; both append an
  `EventKind` at 76, so whichever merges second takes 77 (and `Native.java`'s constant with it). With
  `pr/app-paths/01-core` merge-tree reports `prelude.rs` and `nsh-engine/src/lib.rs`, which this branch does not
  touch: that branch is on an older `main` (`d96fc35c`). Clean with `pr/will-terminate/01-core`,
  `pr/locale-direction/01-core` and `pr/high-contrast/01-core`.
- Left for the owner: (1) JS sees GPUI's button names, as `pressedButton` already does, with the DOM's numbers in
  the docs; a numeric `button` beside them would be a second form of one thing. (2) The back button still goes back
  natively as `onAuxPress` hears it; an app that binds it to its own back keeps `popOnBack` / `onBackPress` for that,
  or a later change could let a listener take it. (3) Android reports no button: a mouse's secondary and middle
  buttons press views there as Android's touch stream delivers them; passing `MotionEvent.getButtonState()` would
  change the JNI press call.

---

**Depends on:** no other PR; it applies to `main` as it is.

## Description

NSH's presses are the primary button's (and a finger's and the keyboard's), so a view cannot tell a middle click, the
back and forward buttons or a secondary click from nothing: an app cannot close a tab on the middle button, or bind the
side buttons of a mouse to actions of its own.

`onAuxPress` (`.on_aux_press` in Rust) is GPUI's `on_aux_click` and the DOM's `auxclick`: a mouse button other than
the primary one was pressed and released over the view. Its payload is the mouse events' `MouseEvent`: `pressedButton`
is the button (`"right"`, `"middle"`, `"back"` or `"forward"`, GPUI's names for the DOM's `button` 2, 1, 3 and 4),
`position` and `pagePosition` where it came up, and the `modifiers`:

```tsx
<View onPress={() => select(tab)} onAuxPress={(e) => e.pressedButton === "middle" && close(tab)}>
  <Text>{title}</Text>
</View>
```

```rust
view()
    .on_press(move || select(tab))
    .on_aux_press(move |e: &MouseEvent| {
        if e.pressed_button == Some(MouseButton::Middle) {
            close(tab);
        }
    })
```

`PressEvent` keeps its fields and presses keep the primary button, so no existing handler hears a new button: a right
or middle click never reaches an `onPress`. The platform decides on the main thread from the mounted event masks, as
for `onContextMenu` (`nsh_core::input::AuxPress`, shared by the desktop backends and the headless driver): as the
button goes down, the nearest view around the one under the pointer that listens takes the press, unless a text
input is nearer, which takes its buttons itself; the view hears it when the same button comes up over it or a view
inside it. `disabled` removes the listener as it removes a press's. What the buttons do natively stays: the secondary
one also asks for `onContextMenu` as it goes down, and the back button still goes back.

- **macOS**: `rightMouseDown:` / `otherMouseDown:` and their ups (`buttonNumber` 2, 3 and 4); Control-click stays a
  context menu.
- **Windows**: `WM_RBUTTON*`, `WM_MBUTTON*` and `WM_XBUTTON*` (`XBUTTON1` back, `XBUTTON2` forward).
- **Linux**: the root's GDK button events 2, 3, 8 and 9 (the capture-phase controller every pointer event goes
  through); their native handling goes on (the back button's `GestureClick`, a text field's middle-click paste).
- **GPUI**: `MouseDownEvent` / `MouseUpEvent`'s `Right`, `Middle` and `Navigate(Back | Forward)`.
- **Web**: the DOM's `mousedown` / `mouseup` `button` 1 to 4 (`nsh.js` `auxDown` / `auxUp`): browsers raise
  `auxclick` for some of these buttons only.
- iOS, Android and the terminal report no other buttons.

`nsh/test`'s `press(selector, { button })` presses with another button, headless on a desktop or web platform:
`"right"` asks for a context menu first, as `rightClick` does.

Docs: "Mouse buttons" in `website/elements/index.md`, the `View` page's row and `press`'s `button` in
`website/docs/testing-and-trace.md` (each with its zh-CN page); `docs/dev/elements.md`, `api-naming.md`,
`components.md` and the platforms' pages.

## Public API

- `nsh`:
  - `onAuxPress?: (e: MouseEvent) => void` on every element: a mouse button other than the primary one was pressed
    and released over the view; `e.pressedButton` is the button.
- `nsh/test`:
  - `App.press(selector: string, options?: PressOptions): Promise<void>`: presses with `options.button`.
  - `PressOptions { button?: MouseButton }`: the mouse button (`"left"`, a tap's, by default).
  - `MouseButton = "left" | "right" | "middle" | "back" | "forward"`: `nsh`'s, for the options.
- `nsh-core`:
  - `EventKind::AuxPress`: the event (`onAuxPress`), appended.
  - `Interactive::on_aux_press(self, listener: impl IntoListener<MouseEvent, M>) -> Self`: its Rust listener.
  - `input::AuxPress<V> { view: V, button: MouseButton }`: a press of another button in progress.
  - `input::AuxPress::down(button, chain: impl IntoIterator<Item = (V, EventMask, bool)>) -> Option<AuxPress<V>>`:
    the view that takes the press (the nearest listener, unless a text input is nearer).
  - `input::AuxPress::up(&self, button, chain: impl IntoIterator<Item = V>) -> bool`: whether the button coming up
    there completes it.
- `nsh-testing`:
  - `Headless::aux_press(&mut self, selector: &str, button: MouseButton) -> Result<(), String>`: the step, in Rust.

## How to Test

- `cargo test -p nsh-core --lib aux_presses`: the nearest listener takes the press, the same button completes it over
  the view or inside it, a text input nearer takes it, the primary button presses instead.
- `cargo test -p nsh-engine --test engine aux_press`: the handler is in the event mask, a `disabled` view mounts none,
  and each button reaches the handler with its positions and modifiers.
- `cargo test -p nsh-testing --features bundle --test testing a_press_with_another`: `press(selector, { button })`
  with the middle, back and forward buttons (Shift held), the secondary button's context menu first, a text input,
  and a phone rejecting.
- `cargo test -p nsh-platform-web --test text an_aux_press` and `node --test
  crates/nsh-platform-web/web/mouse.test.mjs`: the page's `mousedown` / `mouseup` of the DOM's buttons 1 to 4 press
  the nearest listener once the same button comes up over it, not past a field.
- Compat: `tests/compat/cases/events`' `events-aux-press` claims `covers [events] onAuxPress` (`NSH_COMPAT_APP=events
  cargo xtask compat`).
- On a desktop: a view with `onAuxPress`, clicked with the middle, secondary, back and forward buttons on macOS,
  Windows, Linux, GPUI and in a browser.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the iOS, Android and wasm32 runs
  included (Windows and Linux on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no demo changes, and iOS and Android report no
  other buttons; no device run was made.
