# Upstream PR: menu: Focus a one-item menu's item when the keyboard opens it

Opened 2026-11-05 from `fix/menu-one-item-keyboard-focus` (the owner's go) as longbridge/nsh#1017. The body is below the line.

- Review (huacnlee, 2026-11-05, commented): the approach is fine; his branches #1006, #1009–#1011 carry another fix for
  the same bug (`254f6507`: `MenuItem` `on_layout` calls the pick), which conflicts in `menu.rs`; pick one. Answered
  (issuecomment-6670918788): keep this PR's. Both focus in the same batch as the items (`nsh test --trace`), but
  `254f6507` never focuses a level whose only entry is a `Submenu` trigger (`right_on_a_submenus_trigger_focuses_its_only_item`
  fails with its `menu.rs`), and it gives every item a layout listener. The body's Chrome line now names #1010's
  `1137dd527` instead of #1018 (closed).

- Branch: `fix/menu-one-item-keyboard-focus`, one commit (`dbc55fc2`) on `upstream/main` `f7afd23f` (#1000). Its
  companion, `web: Open Chrome's context menus from Shift+F10 as keyboard openings`
  (`upstream-web-context-menu-keys.md`), is stacked on it.
- Found by: `examples/gallery/tests/context-menu-press.test.tsx` › "Shift+F10 on the focused trigger opens the menu at
  its first item" (#978's) failing upstream's `Rust application` / `web` job (a headless run of the gallery's tests)
  on #978's own run and on every PR since (longbridge/nsh#1001): "`text:chose: menu-solo` showing never happened in
  5000 ms; on screen: ["solo", "tapped: solo", "chose: none", "menu-solo"]". It fails headless on a Mac too.
- Checked on the fork (dispatches, no fork PR), on `dbc55fc2`: upstream's Rust application passed
  (https://github.com/dXnSix/nsh/actions/runs/42096622808; its `web` job ran the gallery's 294 headless tests with no
  failure, the Shift+F10 test in 416 ms), Core (https://github.com/dXnSix/nsh/actions/runs/42096628638) and Compat
  (https://github.com/dXnSix/nsh/actions/runs/42096896769) passed. Fork CI on it alone
  (https://github.com/dXnSix/nsh/actions/runs/42096618366): `android` passed, `web` still failed the Shift+F10 test in
  Chrome (the companion's cause), `macos` only upstream's typed-key tests; with the companion all pass but macOS's.
- Fork CI `android` and `ios` also failed the test for a reason of the fork's own: the fork's list ran the file whole,
  and its third test opens the app as the web, which mobile targets refuse. Fixed on the fork's side
  (`ci/context-menu-press-touch-only`, `ee8cc52d`).
- Title: `menu: Focus a one-item menu's item when the keyboard opens it`.

---

## Description

A menu opened from the keyboard puts the keyboard focus on an item, as WAI-ARIA has it: its first enabled one
(Return, Space or Down on a `Menu`'s trigger; Shift+F10 or the Menu key on a `ContextMenu`'s, #978; Right, Return or
Space on a `Submenu`'s), or its last (Up). A menu of one item got no focus at all, so the next key went to the
trigger. `context-menu-press.test.tsx` › "Shift+F10 on the focused trigger opens the menu at its first item" opens a
one-item context menu, and its Space presses the trigger ("tapped: solo") instead of choosing the item: it fails the
`Rust application` workflow's `web` job (the gallery's tests, headless) on `main` and on every pull request since
#978.

The menu picks the item in the handler of its order watch (`watch_order`, since #869), once its items are in the
tree's order. The core tells that handler the order only when it differs from the order the items registered in
(#702: "in order already: nothing to tell, no round to run"). A menu's items register last-first, so a menu of
several items is always out of order and its handler runs; a single item is always in order, the handler never runs
and nothing is focused. A submenu's `focus_first` asks for the pick with `invalidate()` the same way.

`OrderWatch` gains `request()`: as `invalidate()`, and the core tells the handler the order at the end of the round
even when it is the list's own. The menu requests the order instead of invalidating it while an opening from the
keyboard waits for its item (items added then, a submenu's `focus_first`). The other watches (the roving focus of
toolbars, tabs, radios and toggle groups; the carousel) only sort in their handlers and keep skipping orders that
are in order already: in `toolbar.test.tsx` and `tree.test.tsx`, 38 of their looks are, and making every
invalidation tell would cost each of those a handler run and a commit round.

In Chrome the test needs one more change, in the web runtime (Chromium's `contextmenu` for the keys names the mouse
as its pointer): #1010's 1137dd527.

## Public API

- `nsh_core::element::OrderWatch::request(&self)` (`#[doc(hidden)]`, as `watch_order` and `OrderWatch` are):
  as `invalidate`, and the handler hears the order even when it is the list's own.
- Behavior: a `Menu`, `Submenu` or `ContextMenu` of one item opened from the keyboard focuses it, as one of several
  focuses its first enabled item.

## How to Test

- `./target/debug/nsh test examples/gallery/tests/context-menu-press.test.tsx`: "Shift+F10 on the focused trigger
  opens the menu at its first item" fails on `main` (5.4 s, the timeout above) and passes with this change (442 ms).
- `cargo test -p nsh-base --test base menu::`: `an_opening_from_the_keyboard_focuses_a_menus_only_item`,
  `right_on_a_submenus_trigger_focuses_its_only_item` and `shift_f10_focuses_a_context_menus_only_item` (the web and
  Linux) fail without the menu's change and pass with it; the other 16 pass either way.
- `cargo test -p nsh-core --lib order_watch`: `a_requested_order_watch_hears_its_order_even_unchanged` (an
  invalidation of an in-order watch tells nothing, a request tells once).
- `cargo test -p nsh-base --test base` (772 passed) and `cargo test -p nsh-core --lib` (1136 passed); clippy with
  `-D warnings` on both crates' targets; `cargo xtask compat`.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test` for `nsh-core` and `nsh-base`, `cargo +nightly fmt` on the files it touches and clippy for
  both (no platform code changes).
- [x] Changes to what apps use keep what released versions shipped (`cargo xtask compat` passes; what apps use keeps
  its shape).
- [ ] Ran the related `examples/gallery` demos on iOS and Android (if the change is visible or platform-specific).
