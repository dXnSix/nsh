# Upstream PR: web: Open keyboard context menus at their first item in Chromium on Linux

**Not to be opened: the fork's CI shows that upstream does not need it.** The fork found the failure only on a fork
`main` that did not yet have upstream's own fix: huacnlee's `1137dd527` (`web: Recognize keyboard context menus from
Chromium`, landed in #1011 `2beadec8e` and #1010, 2026-11-06). That fix takes a `contextmenu` with `button === -1` as
the keyboard's. In every Fork CI run on a base with that fix, the gallery's Shift+F10 test passed in Linux's Chrome
without this change. The sync to `7571948ee` passed it too, before this fix existed. So the premise below (Linux's
Chromium raises Shift+F10's menu with `button: 0`) is not confirmed. `upstream/main` `c015f2c5e` (2026-11-07) has no
key-based detection, and the change applies to it cleanly (`git apply --check`). Open it only if the test fails on
upstream's code.

An open upstream PR covers it as well: honsunrise's longbridge/nsh#1098 (`web: Handle desktop context-menu keyboard
requests`, opened 2026-11-06, mergeable, head `bd30b584` on `beeb9fc77`; checked 2026-11-07). Its `keyDown` opens
the focused view's menu for Shift+F10 and the Menu key itself (`contextMenu(e, true)`: no pointer, at the
bottom-start corner) and prevents the browser's default. Whatever `button` Chromium gives the `contextmenu`, the
keyboard's opening then no longer depends on it. It also adds `context-menu-press.test.tsx --filter Shift+F10` to the
Chrome step of upstream's `Rust application` workflow. If #1098 merges, the fork's #154 conflicts with it in
`nsh.js` (`contextMenu`'s signature and its `pointerType` line) at the next sync. Take #1098's and drop `menuKey` and
`MENU_KEY_MS`. Related: `upstream-web-context-menu-keys.md` (longbridge/nsh#1018, the earlier key-based fix, closed
in favor of `1137dd527`).

- Branch: `fix/web-keyboard-context-menu`, one commit (`8789efedb`) on `upstream/main` `7571948ee` (#1069).
- Fork PR: dXnSix/nsh#154, merged 2026-11-07 (`01d9ba2fa`), in tag `fork-0.4.0-1` (`1d64f049d`).
- Found by: Fork CI's `web` job (`nsh test --target web`, Linux Chrome) in run
  https://github.com/dXnSix/nsh/actions/runs/42310977359 on `fork/ci-pull-request-types` `cbb9918af`. That branch was
  cut from the fork's `main` `c20743010`, which had no `button === -1`. "Shift+F10 on the focused trigger opens the
  menu at its first item" failed twice there: `` `text:chose: menu-solo` showing never happened in 5000 ms; on screen:
  ["solo", "tapped: solo", "chose: none", "menu-solo"] ``.
- Passed without it: the same test in Fork CI's `web` job on the sync branch `a9d09e8e7`, which has `7571948ee` and
  `button === -1` but not this fix (https://github.com/dXnSix/nsh/actions/runs/42311010274, 893 ms). It also passed in
  nine Fork CI runs on 2026-11-06 and 2026-11-07 on branches that have #1011 and not this fix (`tmp/*-fork-ci` and
  `sync/upstream-033f00b5f`; for example
  https://github.com/dXnSix/nsh/actions/runs/42235365721 and
  https://github.com/dXnSix/nsh/actions/runs/42346050489).
- Checked with it: Fork CI's `web` job on the fork PR passed the test (822 ms;
  https://github.com/dXnSix/nsh/actions/runs/42313534952, where the gallery's other tests failed on #1111's missing
  `term` gate, `upstream-web-terminal-part.md`). It also passed on the fork's `main` at `1d64f049d`
  (https://github.com/dXnSix/nsh/actions/runs/42321466305). `node --test crates/nsh-platform-web/web/nsh.test.mjs`:
  66 passed.
- Next sync: `crates/nsh-platform-web/web/nsh.js` merges cleanly (`git merge-tree` of the fork's `main` with
  `c015f2c5e`). Whether the fork keeps dXnSix/nsh#154 is the owner's call.
- Title: `web: Open keyboard context menus at their first item in Chromium on Linux`.

---

## Description

Chromium on Linux raises Shift+F10's `contextmenu` as a mouse event (`pointerType: "mouse"`, `button: 0`). The player
looks for an empty pointer type or `button: -1`. `ContextMenu` then treats the opening as a pointer's and leaves the
keyboard focus on its trigger, so Space presses the trigger instead of choosing the first item. The gallery's
`context-menu-press` test ("Shift+F10 on the focused trigger opens the menu at its first item") fails with `nsh test
--target web` in Linux's Chrome. On screen: `["solo", "tapped: solo", "chose: none", "menu-solo"]`.

The player now notes when Shift+F10 (alone) or the Menu key is pressed or released (`menuKey`, capture listeners on
`window`; Windows raises the Menu key's menu on its release). It takes a `contextmenu` raised within 500 ms of that
(`MENU_KEY_MS`) as the keyboard's: `pointerType: null`, placed at the view's bottom-start corner. Each key asks for
one menu, so a right click after it is the mouse's again. `docs/dev/elements.md` says so.

## Public API

- Behavior: in Chrome on Linux, `onContextMenu` for Shift+F10 or the Menu key has `pointerType: null` and the view's
  bottom-start corner. A `ContextMenu` opened this way focuses its first enabled item, as #978 describes for the web.

## How to Test

- `node --test crates/nsh-platform-web/web/nsh.test.mjs`: "a context menu right after Shift+F10 or the Menu key has
  no pointer, whatever Chromium names". After Shift+F10 alone or the Menu key, a `"mouse"` `contextmenu` with
  button 0 has no pointer and opens at the bottom-start corner. After F10 alone, Ctrl+Shift+F10, a key more than
  500 ms earlier, or no key, it is the mouse's. So is a second menu after one key.
- `nsh test examples/gallery/tests/context-menu-press.test.tsx --target web` in Linux's Chrome: "Shift+F10 on the
  focused trigger opens the menu at its first item" passes.
- In Chrome on macOS, where Chromium raises no menu for Shift+F10, dispatch the events Linux's Chromium raises on the
  `context-menu` fixture: Tab, then a `keydown` of Shift+F10 and a `contextmenu` with `pointerType: "mouse"`,
  `button: 0`. `menu-solo` takes the focus, and Space chooses it. A real right click still opens the menu with no item
  focused.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed the web runtime's Node tests (`nsh.test.mjs`); no Rust changes.
- [x] Changes to what apps use keep what released versions shipped (a keyboard opening's `pointerType: null`, which
  `tests/compat/cases/events` checks).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: web only.
