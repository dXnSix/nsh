# web: Put list cells back on their frames when they stop being sticky

Branch: `fix/web-sticky-reset` on `github.com/dXnSix/nsh`, cut from `upstream/main` `5f8daece` (the upstream commit the
fork's `main` holds). It follows upstream by merging `upstream/main` in, never by rebasing. Merged into the fork's
`main`, Gamify's integration line, as fork PR #12 (merge commit `75c488e3`, in `six-gamify-0.1.1-6`). One commit,
`53d2a401`. Not opened upstream; the owner decides and posts there (upstream squash merges).

## Summary

On the web, a `List` with `stickyHeaderIndices` draws rows over each other and leaves gaps once it was scrolled past a
few sections: a row shows up a section further down, over the row that belongs there, and its own place stays empty.
It also happens without scrolling by hand when the list moves itself (rows inserted above the visible ones).

A sticky cell is moved with a `translate` along the scroll and raised by `nsh-stuck` (`z-index: 5`). When its header
leaves the render window, the cell is recycled for another item and the core sends `StickyCells` without it.
`op_sticky` only updated the cells still listed, so the recycled cell kept its last shift (a pushed-out header's is
the length of its section) and stayed above its siblings, whatever row it showed next.

Cells that leave the sticky set now drop their shift and `nsh-stuck`, as the macOS, Windows and Linux backends
(`scroll::sticky`: "Cells no longer sticky go back to their frames"), iOS (identity transform) and Android
(`setTranslationY(0)`) already do.

## Public API

None.

## Breaking Changes

None.

## Docs

None: it makes sticky headers behave on the web as documented and as on the other platforms.

## Validation

- Gamify's browser spec `sportsbook rows` (Playwright, the packaged web app): on the event list (league headers
  sticky, 40 pt, between 72 pt match rows), scrolled in 600 px steps through prematch and live, the rows below the
  pinned header must tile the list, each starting where the one above it ends. Before the change (`six-gamify-0.1.1-5`)
  it fails 3,000 px down the prematch list, two rows over each other (`event-live-baseball-894 at 544,
  event-live-baseball-647 ends at 576`); after it, every step passes.
- In the running app, clearing `translate` and `nsh-stuck` on the cells outside the list's `_stickyCells` by hand
  put every row back where it belongs; the change does the same in `op_sticky`.
- `node --check` (as a module): the runtime parses.

## Other platforms

The native backends already reset cells that leave the sticky set; only the web was missing it.
