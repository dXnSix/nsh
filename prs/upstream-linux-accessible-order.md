# Upstream PR: linux: Use NshView's accessible order, which GtkWidget's took the place of

**Not to be opened: upstream fixed it itself.** LeeDashan's #1176 (`f627013e7`, `linux: Install NshView's reading
order for screen readers`, merged 2026-11-07 04:06 UTC) makes the same one-line change: `type Interfaces =
(gtk::Accessible, gtk::AccessibleRange)`. Only the comment differs. Checked against `upstream/main` `c015f2c5e`
(2026-11-07). The fork's version remains below the line as a record.

- Branch: `fix/linux-accessible-order`, one commit (`657993edd`) on `upstream/main` `7571948ee` (#1069).
- Fork PR: dXnSix/nsh#157, merged 2026-11-07 (`1d64f049d`, which is tag `fork-0.4.0-1`).
- Found by: the `Linux` workflow on the sync to `7571948ee` (dXnSix/nsh#152, run
  https://github.com/dXnSix/nsh/actions/runs/42311010258). Its second test run, under `xvfb-run`, failed
  `view::tests::a_scroll_views_content_is_read_in_screen_order` (119 passed, 1 failed). The first run, without a
  display, passed the test only because `gtk_test::run` runs nothing there. Upstream's own `Linux / check` has the
  same xvfb pass. #1176 says #1122's CI never reached it, because of a clippy error that #1166 fixed.
- Checked: the `Linux` workflow passed on the fork PR (https://github.com/dXnSix/nsh/actions/runs/42317684647) and on
  the fork's `main` at `1d64f049d` (https://github.com/dXnSix/nsh/actions/runs/42321466422). Not built locally
  (macOS, no GTK).
- Next sync: `crates/nsh-platform-linux/src/view.rs` conflicts with `f627013e7` on the comment above `Interfaces`
  (`git merge-tree` of the fork's `main` with `c015f2c5e`). Take upstream's.
- Title: `linux: Use NshView's accessible order, which GtkWidget's took the place of`.

---

## Description

#1122 gave `NshView` an `AccessibleImpl` (`first_accessible_child`, `next_accessible_sibling`). It reads a scroll
view's content, and a list's cells in its parent's viewport, in screen order. But `NshView` listed only
`gtk::AccessibleRange` in its `Interfaces`, so GObject never installed that impl. GTK kept `GtkWidget`'s
implementation of `gtk::Accessible`, which follows the widgets' order.

`view::tests::a_scroll_views_content_is_read_in_screen_order` fails wherever GTK opens a display. The Linux workflow's
second test run (under `xvfb-run`) fails it every time. The first run, without a display, passes only because
`gtk_test::run` then runs nothing. The panic does not show, because it happens on the shared GTK thread.

`NshView` now lists `gtk::Accessible` too, which re-implements it over `GtkWidget`'s. gtk4-rs's `AccessibleImpl`
defaults call the parent's implementation for the other entries (bounds, platform state, AT context, accessible
parent).

## How to Test

- The Linux workflow's "Test" step: `dbus-run-session -- xvfb-run -a cargo test -p nsh-platform-linux --features
  webview`. `a_scroll_views_content_is_read_in_screen_order` passes; on `main` it fails.
- With Orca: a scroll view whose children were added out of screen order is read top to bottom.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt`; the Linux workflow builds, lints and tests it.
- [x] Changes to what apps use: screen readers on Linux read scroll views in screen order, as #1122 meant.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: Linux only.
