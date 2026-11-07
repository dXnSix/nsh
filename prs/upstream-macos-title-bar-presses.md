# Upstream PR: macos: Pass test hosts' presses under a hidden title bar to the app

**Not opened (the user's call, 2026-11-07): upstream no longer has the bug.** Upstream's #1196
(`e12c6f8b8`, `macos: Center the traffic lights in a hidden title bar without a toolbar`, merged 2026-11-07) removed
the `NSToolbar` that a hidden title bar with the app's bar height used to get, and the macOS backend attaches no
toolbar anywhere since (`git grep setToolbar upstream/main -- crates/nsh-platform-macos/src`: nothing). The fork's
#159 lost presses to that toolbar's `NSToolbarView`, so the fix has nothing to do on upstream's `main`.

- Probe: `tmp/macos-title-bar-probe` (`662ce951`) is upstream's `main` `dd41b7dd` without the fix, plus the fixture and
  a log-only `sendEvent:` on `NshWindow`. Run https://github.com/dXnSix/nsh/actions/runs/42375706337 used the
  `macos-15` runner (ProductVersion 15.8.2). Every background press hit the content: under the hidden title bar (window
  points (117, 822) in the fixture, (174, 828) on the dock drawer's Hide), AppKit's hit view was `NshView > NSView >
  NSThemeFrame` (`first_mouse=true`, `in_content=true`). `TitleBar.test.tsx`: 1 passed. `dock.test.tsx --filter "a
  dock is a drawer"`: 1 passed.
- For comparison: on the fork (before #1196), dXnSix/nsh#159's probe on the same runner hit `NSToolbarView >
  NSTitlebarView > NSTitlebarContainerView` (`acceptsFirstMouse: NO`).
- Port, kept for the record: `fix/macos-background-title-bar-clicks-upstream` (`0354255c`, local, not pushed) on
  `dd41b7dd`. Upstream's #1137 made every window an `NshWindow` / `NshPanel` (`window.rs`), so the fork's `TestWindow`
  (`app.rs`) became `sendEvent:` on those two classes, gated on upstream's `in_background()`. The fixture moved to
  upstream's `currentWindow().set({ titleBar })`, because #1137 removed `Window.setTitleBar`. Locally on macOS 26.6:
  clippy `-D warnings`, fmt, `cargo test` of the macOS crates, the host fixtures (11/11) and `dock.test.tsx` (11/11)
  pass. On upstream the override never triggers.
- Fork CI twin dXnSix/nsh#161 (`ca0335fc`): cancelled once the probe showed the fix is not needed, then closed; the
  branches `tmp/macos-title-bar-probe` and `tmp/macos-background-title-bar-clicks-upstream-fork-ci` are deleted. Checks that had
  finished passed: CI paths, Core, Examples, Rust application (Linux, Windows), TUI (Linux), upstream dispatch.
- For the fork's next sync past `e12c6f8b8` / `d06c99edd`: `app.rs`'s `TestWindow` conflicts with upstream's
  `window::new_window`. Take upstream's `app.rs`; the `sendEvent:` override is then dead code and can go. Keep or drop
  `tests/host/TitleBar.*` (on upstream it needs `currentWindow().set`, as on this branch). dXnSix/nsh#160 (the dock
  drawer back in the fork's macOS runs) stays right either way.
- Title it would have had: `macos: Pass test hosts' presses under a hidden title bar to the app`.

The ported body follows as a record. It assumes the bug exists, which the probe disproved for upstream's `main`.

---

## Description

Since #907, `nsh test --target macos` runs its hosts in the background: the app's activation policy is `Prohibited`,
so AppKit never makes its windows key. A window that is not key gets a left press only where the view hit accepts
the first mouse (every `NshView` does); anywhere else the press only makes the window key. A hidden title bar
(`Window.setTitleBar({ style: "hidden", height })`) puts the content under a transparent title bar, which lies over
the content's top. On macOS 15, HIT_VIEW is what `hitTest:` finds across the bar's whole width, and it does not
accept the first mouse, so a press on the app's own views in that strip never arrives. A key window hands such a
press on to the content under it, and macOS 26 hit-tests the strip to the content.

On GitHub's `macos-15` runners this fails the gallery's `dock.test.tsx` "a dock is a drawer, presented from the
toolbar and dismissed from its strip": in the phone-sized window the drawer's header, with its Hide button, sits
under the gallery's hidden title bar, and the press on Hide leaves the drawer shown. It passes at a desk on macOS 26
and with `--foreground`. Upstream's CI runs no macOS gallery tests, so it shows only in macOS 15 runs.

`NshWindow` and `NshPanel` (`window.rs`) now override `sendEvent:`. In a background app, a left press that AppKit would
hand to a view outside the content view that does not accept the first mouse goes to the content's view under the
pointer instead, and the press's drags and release follow it there, as in the key window. The press still arrives
through the application's queue, so it is `NSApp.currentEvent` while it is handled; `windowControl` close presses
(`window::asked`), modal windows (`window::blocked`) and text fields read it. Every other event goes to AppKit's own
dispatch, and apps that are not in the background (`nsh run`, release runners, embedding apps) are unchanged.

`agent::mouse` (#1161) already hands a background press into a web view to the `WKWebView` directly. This change does
not go there because a direct `mouseDown:` would leave `NSApp.currentEvent` at the previous event.

A host fixture test (`crates/nsh-platform-macos/tests/host/TitleBar.test.tsx`) presses a button under a hidden title
bar. `docs/dev/pitfalls.md` records the AppKit behavior, `docs/dev/tooling.md` the windows' part, and the macOS page
(English and zh-CN) says clicks reach the app's views under a hidden title bar too.

## How to Test

- HOW_TO_TEST

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt --all` and clippy for the macOS crates (`-D warnings`).
- [x] Changes to what apps use: none (test hosts' input only).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: macOS only.
