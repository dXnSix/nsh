# web: Keep sticky list cells with their list's layer, events and stacking

Branch: `fix/web-sticky-hardening` on `github.com/dXnSix/nsh`, cut from the fork's `main` (`df084a10`,
`six-gamify-0.1.1-7`). One commit, `41ac75c7`. It brings into the fork what review of the upstream PR changed in the
fork's sticky tracks (`f93a187e`, fork PR #13): the upstream PR's commit `bf9b720a`
([`upstream-web-sticky.md`](upstream-web-sticky.md)) has the same code, so a later sync of `upstream/main` with that PR
merged resolves to it. Fork PR #15, merged into the fork's `main` with a merge commit (`f90a4c89`) after its checks
passed (Rust application: check on macOS, Ubuntu and Windows, and web; Website: build; CI paths; the Core workflow
did not run, as no core crate changed); tagged `six-gamify-0.1.1-8`.

## Summary

The fork's sticky tracks (fork PRs #12 and #13) got four things wrong, all confirmed in Chromium with a lab app
(`Six.Gamify/artifacts-temp/nsh-upstream-sticky/lab/`):

- **Refreshable and zoomable lists.** Their cells live in a content layer that the pull, the refresh spinner's room and
  the zoom move; tracks went into the list itself, outside the layer, so pinned headers stayed put while a pull or a
  refresh moved the rows down 56 points. Tracks now go where the cells are (`el._layer ?? el`), keep their list
  (`track._list`, for `placeTrack`), and `ensureLayer` takes existing tracks into a layer made later.
- **Inert lists.** `.nsh-track > .nsh-v { pointer-events: auto }` overrode what the cell inherits, so a sticky cell
  took clicks inside an inert list or ancestor (`pointerEvents="none"`, a screen during a navigation transition, a view
  animating its removal). A track now has no breadth across the list (width 0 for vertical lists, height 0 for
  horizontal ones), so it takes no pointer events itself without a `pointer-events` rule, and the cell inherits as any
  cell does.
- **Right-to-left pages.** In a horizontal list's track, an in-flow cell narrower than its track was laid out at the
  track's far end in `direction: rtl` (out of view). The cell now has `margin-right: auto`, which puts it at the track's
  left in either direction (and keeps it there in a zero-width track).
- **Stacking.** The tracks' `z-index: 5` (and `nsh-stuck`'s before them, upstream too) was the page's, since no view
  makes a stacking context, so a pinned header painted over the views after the list, such as a translucent bar laid
  over the list's top. A list with sticky cells is now isolated (`nsh-sticky`), as native backends keep sticky cells in
  front within their list only.

And one guard: before the list's content size is known (`ScrollContent` follows `StickyCells` in a batch), the last
track no longer spans `1e9` points (the encoded infinite `end`), it ends at its cell.

## Public API

None.

## Breaking Changes

None.

## Docs

`docs/dev/web.md` ("Scroll views: pull to refresh and zooming"): how the web keeps sticky cells pinned, as in the
upstream PR, with the `stickyHeaderOffset` sentence the fork's `main` already has the prop for.

## Validation

- Chromium (Playwright) on the lab app, with the fork's runtime built from this branch (wasm and `nsh.js` via
  `NSH_RUNTIME`; `nsh run` from a worktree whose `target` is a symlink resolves the main checkout's runtime otherwise),
  the fork's `main` (`df084a10`) against this branch, same engine:

  | | `main` | this branch |
  |---|---|---|
  | a refreshable list refreshing: the pinned header | at 0, under the spinner's room | 56 points down, with the rows |
  | the point under a pinned header, list made inert | hits the sticky cell | hits nothing in the list |
  | a horizontal list in a right-to-left page | no header in view | its first header at the left |
  | `stickyHeaderOffset={56}` under a translucent bar, painted frames off 56 points | 50 of 494 (800 px/s), 18 of 417 (1200 px/s): the header over the bar | 0 of 473, 0 of 417 |
  | painted frames off, vertical / horizontal / refreshable, 800 and 1200 px/s | 0 | 0 |
  | rows overlapping in 60 scrolls of 600 px | 0 | 0 |
  | a click on a pinned header / on a row under its track | reaches it / reaches the row | the same |
- Node tests: the fork has no `crates/nsh-platform-web/web/scroll.test.mjs` yet (upstream added it after the fork's
  base). The upstream branches' six sticky tests (`pr/list-sticky-offset`'s file, `--test-name-pattern
  "sticky|track|cells"`) pass against this `nsh.js`; against `main`'s, four of them fail.
- `cargo +nightly fmt --all --check` (no Rust changed).

## Other platforms

None: the native backends already keep sticky cells in their content, inherit events and stack them within their list.
