# Upstream PR: web: Open Chrome's context menus from Shift+F10 as keyboard openings

Opened 2026-11-05 from `fix/context-menu-keyboard-focus` (the owner's go), stacked on the menu PR
(`upstream-menu-one-item-keyboard-focus.md`). The body is below the line.

- Branch: `fix/context-menu-keyboard-focus`, `dbc55fc2` (the menu PR's commit) then its own `4316a476`, on
  `upstream/main` `f7afd23f`. Stacked so its head is the commit Fork CI passed, and its checks run with the one-item
  fix the same test needs; the changes are independent (no shared file), so it rebases onto `main` cleanly once the
  menu PR merges.
- Found by: Fork CI's `web` job (`nsh test --target web`, Linux Chrome) on the menu commit alone: the Shift+F10 test
  still failed there ("tapped: solo", the menu open with no item focused). Chromium's
  `EventHandler::ShowNonLocatedContextMenu` gives the keys' event `PointerEventFactory::kMouseId`, so its
  `contextmenu` has `pointerType: "mouse"` (the source read on 2026-11-05).
- Checked: Fork CI on both commits (https://github.com/dXnSix/nsh/actions/runs/42102703021, head
  `tmp/context-menu-keyboard-focus-fork-ci` `a6c37d12`): `web` passed with no failure or flaky test, the Shift+F10
  test in 1.4 s in Linux Chrome; `android` and `ios` passed; `macos` failed only upstream's typed-key files (dock,
  form, gallery-rs), as on every fork PR since #907. `node --test crates/nsh-platform-web/web/nsh.test.mjs`: 63 pass.
  In the desktop app's Chromium on a Mac, against `nsh run --target web` of the fixture, with the events Chromium
  makes for the keys (Shift+F10's `keydown`, then a `"mouse"` `contextmenu` in the same task): `menu-solo` takes the
  focus and Space chooses it; a lone `"mouse"` `contextmenu` (a right click) focuses no item. macOS Chrome raises no
  `contextmenu` for Shift+F10, so `nsh test --target web` on a Mac cannot check it.
- Title: `web: Open Chrome's context menus from Shift+F10 as keyboard openings`.

---

**Depends on:** #1017 (`menu: Focus a one-item menu's item when the keyboard opens it`): the gallery's Shift+F10 test
opens a one-item menu, which needs both. Until it merges this PR shows its commit too; this PR's own is the last
(`crates/nsh-platform-web/web/nsh.js`, its test and two docs pages), which shares no file with it.

## Description

Shift+F10 and the Menu key on a focused `ContextMenu` trigger open the menu with the keyboard focus on its first
enabled item (#978), and a view's `onContextMenu` hears `pointerType: null` and the view's bottom-start corner. In
Chrome and Edge neither happened: the menu opened with no item focused, and the event named the mouse.

The web runtime opens the menu on the browser's own `contextmenu`, and took one with no `pointerType` (`""`) for the
keyboard's. Chromium's for the keys names the mouse: `EventHandler::ShowNonLocatedContextMenu` gives the event the
mouse's id (`PointerEventFactory::kMouseId`), so it has `pointerType: "mouse"`, and `nsh.js` took it for a right
click.

`nsh.js` now notes the keys as Chromium checks them (`WebViewImpl::KeyEventDefault`: Shift+F10 as it is pressed, the
Menu key with no modifier as it is pressed or, on Windows, released) and takes a `contextmenu` in the same task as
theirs for the keyboard's: `pointerType: null`, at the view's bottom-start corner. The note lasts until the next
task, so a right click after them is the mouse's. `docs/dev/elements.md` and `docs/dev/api-naming.md` say so.

## Public API

- Behavior: in Chrome and Edge, `onContextMenu` for Shift+F10 or the Menu key has `pointerType: null` and the view's
  bottom-start corner, and a `ContextMenu` opened so focuses its first enabled item, as #978 describes for the web.

## How to Test

- `node --test crates/nsh-platform-web/web/nsh.test.mjs`: "a context menu the keys ask for names no pointer, though
  Chromium names the mouse" (Shift+F10 pressed and the Menu key pressed or released: no pointer, at the bottom-start
  corner; a right click, and the next task after a key: the mouse's).
- `./target/debug/nsh test examples/gallery/tests/context-menu-press.test.tsx --target web` on Linux or Windows: "Shift+F10
  on the focused trigger opens the menu at its first item" passes (Chrome raises no `contextmenu` for Shift+F10 on
  macOS).
- In Chrome on Linux or Windows: the gallery's Menus demo, Tab to a row, Shift+F10: the menu opens below the row with
  its first item focused.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed the web runtime's Node tests (`nsh.test.mjs`, `mouse.test.mjs`, `scroll.test.mjs`); no Rust changes.
- [x] Changes to what apps use keep what released versions shipped (`tests/compat/cases/events` already checks a
  keyboard opening's `pointerType: null`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android (if the change is visible or platform-specific).
