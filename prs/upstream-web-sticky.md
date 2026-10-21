# Upstream PR: web: Keep sticky list headers pinned and their recycled cells in place

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#280](https://github.com/longbridge/nsh/pull/280) (2026-10-21) with its issue
([`../issues/web-sticky.md`](../issues/web-sticky.md), [#277](https://github.com/longbridge/nsh/issues/277)), from `pr/web-sticky` rebased onto
`upstream/main` `91e5878e` (`a64569ee`). The body below is the draft; the posted one names the issues and links the
screenshots from the fork's `pr-assets` branch (`9924c5d0`).

- Branch: `pr/web-sticky`, one commit, `3b9b3bc0`, on `upstream/main` `d5839d7a` (2026-10-21, rebased from `681738a3`
  over 37 upstream commits; nothing conflicted); pushed to the fork
  (`dXnSix/nsh`). Open the PR from `dXnSix:pr/web-sticky` into `longbridge/nsh:main`.
- PR title (upstream squash-merges it): `web: Keep sticky list headers pinned and their recycled cells in place`
- Posting order: this issue and PR now; the `stickyHeaderOffset` issue now as well
  ([`../issues/list-sticky-offset.md`](../issues/list-sticky-offset.md)), its PR only after this one is merged
  ([`upstream-list-sticky-offset.md`](upstream-list-sticky-offset.md)). The core change that keeps the previous
  sticky row rendered is a PR of its own ([`upstream-sticky-row-before-pinned.md`](upstream-sticky-row-before-pinned.md)).
- Images for the Screenshot table, in `Six.Gamify/artifacts-temp/nsh-upstream-sticky/evidence/` (upload them when
  posting): `web-overlap-before-10.png` / `web-overlap-after-10.png`, `web-lag-before.png` / `web-lag-after.png`,
  `web-bar-before.png` / `web-bar-after.png`. The lab app they come from is `artifacts-temp/nsh-upstream-sticky/lab/`,
  the Playwright harnesses `artifacts-temp/nsh-upstream-sticky/harness/`.
- It ports the fork's `53d2a401` and `f93a187e` (fork PRs #12 and #13) and fixes what the fork's version got wrong, all
  found in review and confirmed in the browser: a track of a refreshable or zoomable list went outside its content
  layer (the pinned header stayed put while a pull or a refresh moved the rows down 56 points), tracked cells took
  pointer events inside an inert list (`pointer-events: auto`), in a right-to-left page a horizontal list's tracked cell
  was laid out at its track's far end (out of view), and pinned headers covered the views after the list (also on
  upstream). The fork has these fixes too, from `fix/web-sticky-hardening`
  ([`fix-web-sticky-hardening.md`](fix-web-sticky-hardening.md)).

The body to paste starts below the line.

---

Closes #[issue number]

## Description

On the web, a `List` with `stickyHeaderIndices` drew rows over each other and left gaps once it was scrolled past a few
sections, and its pinned header trailed touch and touchpad scrolling: it sat below the top by the distance the list
moves in a frame (13–14 px at 800 px/s, 20 px at 1200 px/s in Chromium) and slid back as the scrolling slowed. A pinned
header also covered the views after the list, such as a translucent bar the content scrolls under. The other platforms
pin sticky cells correctly.

Causes, all in `nsh.js`:

- `op_sticky` updated only the cells the core listed. A header recycled for another item kept its last `translate` (a
  pushed-out header's is nearly the length of its section) and `nsh-stuck`, whatever row it showed next. The native
  backends put cells that leave the sticky set back on their frames.
- The `translate` was set from the scroll view's `scroll` events, which the browser sends after it has drawn the
  content at the new offset on its compositor.
- `nsh-stuck`'s `z-index: 5` was the page's: no view makes a stacking context. The native backends bring sticky cells
  to the front of their list only.

The fix lets the browser pin sticky cells with `position: sticky`, as the desktop idiom's scroll bar already does with
its sticky `.nsh-bar`. Each sticky cell sits in a track, a box from its frame along its range of travel
(`StickyCells`' `end - start` plus its size), in which the cell is `position: sticky` at its frame's distance from the
range's start. The next sticky cell's track starts where it pushes the cell out; the last one's ends with the content
(`ScrollContent`), so it adds no scrollable space. A track has no breadth across the list, so it takes no pointer
events and its cell takes the ones any cell takes; it sits where the cells are (in the content layer of a refreshable
or zoomable list); its `z-index: 5` keeps the cell above the other cells, and the list isolates it (`nsh-sticky`), so
it no longer covers the views after the list. Cells that leave the sticky set go back to their frames, removed cells
take their tracks along, sticky cells skip layout animations (the browser moves them), and no scroll handler moves
anything.

Tests: five cases in `crates/nsh-platform-web/web/scroll.test.mjs` beside the scroll indicator ones (tracks along the
travel and the list's isolation, the last track and the content size, cells that stop being sticky and removed ones,
horizontal lists, content layers); they fail on `main`'s `nsh.js`. Docs: `docs/dev/web.md`,
`docs/dev/scrolling-and-lists.md`.

## Screenshot

The list from the issue, in a test app with a row of buttons above it (and, in the last row, a translucent 56-point bar
laid over the list's top), Chromium at 420 × 800.

| Platform | Before | After |
| -------- | ------ | ----- |
| Web: scrolled 6000 points down | [web-overlap-before-10.png: row E 28 missing, its cell elsewhere] | [web-overlap-after-10.png] |
| Web: a frame of a touch scroll up at 800 px/s | [web-lag-before.png: header G 13 px below the top, row G 17 above it] | [web-lag-after.png] |
| Web: a translucent bar over the list (a later sibling) | [web-bar-before.png: the pinned header over the bar] | [web-bar-after.png: the bar over the header, as on iOS] |

iOS and Android are unaffected: they pin sticky cells in their scroll callbacks, put cells that leave the sticky set
back on their frames and keep them in their list. This PR changes only `nsh.js`.

## Performance

The core's output is unchanged: the change is in `nsh.js`, so `nsh test --trace` gives the same ops and mount items. In
the browser, scroll events no longer run any sticky work; a sticky cell's new frame or the list's new content size
re-places its track (a few style writes per sticky cell).

## How to Test

- `node --test crates/nsh-platform-web/web/scroll.test.mjs` (8 tests; the 5 new ones fail on `main`'s `nsh.js`).
- In a browser: `nsh run <app> --target web` with the list from the issue. Scroll down a few thousand points: every row
  is in its place. Drag up and down with a touchscreen, Chromium's touch emulation or a touchpad: the pinned header
  stays at the top. The gallery's Lists › Sections demo shows the same on a short list.
- What I measured, in Chromium with Playwright:
  - painted frames from a CDP screencast during three `Input.synthesizeScrollGesture` touch scrolls up and three
    down, and where the pinned header is drawn in each:

    | | Before | After |
    |---|---|---|
    | vertical, 800 px/s | 74 of 490 frames off, 13–14 px in most | 0 of 492 |
    | vertical, 1200 px/s | 47 of 417 off, 20 px in most | 0 of 417 |
    | horizontal, 800 px/s | 10 of 354 off, 13–14 px in most | 1 of 355, 6 px (*) |
    | horizontal, 1200 px/s | 58 of 303 off | 1 of 303, 6 px (*) |
    | refreshable (`onRefresh`), 800 px/s | 75 of 494 off | 0 of 494 |
    | refreshable (`onRefresh`), 1200 px/s | 49 of 417 off | 0 of 417 |
    | the gallery's Lists › Sections, 800 px/s | 2 of 177 off, 5–6 px | 0 of 181 |

    (*) Not the lag: scrolling back past a header's start, the previous section's header is not rendered yet for one
    frame. That is the core's doing, on every platform; #[core PR] fixes it (0 frames with it).

  - the rows after each of 60 scrolls of 600 px: 64 overlaps before, none after;
  - a click on a pinned header reaches it, a click on a row under its track reaches the row, and with the list made
    inert the point under the pinned header hits nothing in the list;
  - while a refreshable list refreshes, the pinned header moves down with the rows (56 points), as before;
  - a horizontal list in a right-to-left page shows its first header at the left, as before (horizontal scroll views
    in right-to-left pages do not scroll on the web, before or after).
- Only Chromium was run; WebKit and Firefox were not.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test --workspace`, `cargo +nightly fmt --all` and clippy (including the Catalyst and Android runs for platform code). On `main` (`d5839d7a`) itself, `cargo +nightly fmt --all --check` reports three files this PR does not touch (`nsh-core/src/lib.rs`, `nsh-js/src/modules.rs`, `nsh-platform-linux/src/input.rs`) and clippy one warning (`nsh-windows-host/src/lib.rs`, items after a test module); the files this PR changes are clean.
- [x] Changes to what apps use are additive and recorded in `api/` (`cargo xtask api`); the fixture apps in `api/apps` are untouched. Nothing apps use changes; `compat::` passes with `api/` untouched.
- [ ] Ran the related `examples/gallery` demos on iOS and Android (if the change is visible or platform-specific). The change is web-only (`nsh.js`) and ran in the gallery on the web; iOS and Android run none of it.
