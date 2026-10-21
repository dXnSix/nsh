# Upstream PR: list: Let sticky headers stick a distance from the top

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#285](https://github.com/longbridge/nsh/pull/285)
(2026-10-21; issue [#279](https://github.com/longbridge/nsh/issues/279)) from `pr/list-sticky-offset` (`1da55210`), rebuilt on
`upstream/main` `91e5878e` without #280: it needs no web change (`nsh.js` already applies the ranges), so the offset's
web test and its `docs/dev/web.md` sentence stay out of it. Its test sits apart from #281's, so the two merge in either
order. The plan below was the first one: the issue
([`../issues/list-sticky-offset.md`](../issues/list-sticky-offset.md)) goes up now, with the web sticky issue and PR, so
the API's name and meaning get discussed; this PR only after the web sticky PR
([`upstream-web-sticky.md`](upstream-web-sticky.md)) is merged, rebased to one commit.

- Branch: `pr/list-sticky-offset`, pushed to the fork (`dXnSix/nsh`): the web sticky PR's commit `3b9b3bc0` and
  `39e37f06`, on `upstream/main` `d5839d7a` (rebuilt there: upstream had meanwhile kept `props-0.1.1.2` and `-0.1.1.3`,
  so the fixture is now `props-0.1.1.4`, and the list docs gained a Rust column). It does not need the core change of
  [`upstream-sticky-row-before-pinned.md`](upstream-sticky-row-before-pinned.md).
- Before opening it: rebase onto `upstream/main` once the web PR is merged (upstream squash-merges, so drop
  `3b9b3bc0` with `git rebase --onto upstream/main 3b9b3bc0 pr/list-sticky-offset`); resolve `docs/dev/web.md` and
  `scroll.test.mjs` if the merged version differs; if `api/apps` got a newer `props` fixture meanwhile, rebuild this
  one (`cargo xtask api --keep api/cases/props`), append the steps below again and run `cargo xtask api`; rerun the
  checks. If the API discussion in the issue changes the name or the meaning, change them before opening.
- Open it from `dXnSix:pr/list-sticky-offset` into `longbridge/nsh:main`, title (squash):
  `list: Let sticky headers stick a distance from the top`
- Images for the Screenshot table, in `Six.Gamify/artifacts-temp/nsh-upstream-sticky/evidence/`:
  `web-offset-before.png` / `web-offset-after.png`, `ios-offset-before.png` / `ios-offset-after.png`. They and the
  offset's frame counts were taken with builds that also had the core change (since split out), which does not touch
  the offset; the checks and the trace numbers are of this branch.
- It ports the fork's `7b09c216` (fork PR #14) onto the new base: the `props` fixture was rebuilt with this base's
  compiler (`cargo xtask api --keep api/cases/props`, `api/apps/props-0.1.1.4`) and its steps appended at the end
  of the script, as upstream's #222 did, instead of after the list cases; a web test of the offset was added.

The body to paste starts below the line.

---

Closes #[issue number]

## Description

`stickyHeaderIndices` pins headers at the very top of a `List`. Under a translucent bar the content scrolls under (a
navigation bar, a floating search field), they belong below it, as CSS `position: sticky; top: 56px` keeps them on the
web. Padding the list or laying it out below the bar does not do it: the headers still pin at the top, or the content
no longer scrolls under the bar.

`stickyHeaderOffset` (TSX, points, default 0) and `VirtualList::sticky_header_offset` set that distance. The content
keeps scrolling under the space above the headers, the next header pushes the pinned one out where it reaches its
bottom edge, and a negative offset leaves part of a header out of view. As in CSS, a header laid out above the offset
(the first one, without padding above it) is shifted down to it from the start.

The core does it all. It counts the offset where it picks the pinned header (`ListState::active_sticky`) and subtracts
it from the travel ranges it sends (`sticky_range`); `StickyCell`'s `start` and `end` are now documented as the scroll
offsets between which a cell moves with the scroll. Every platform already shows a sticky cell
`clamp(offset - start, 0, end - start)` past its frame, so no backend changes, and the web's sticky track (#[web sticky
PR]) sticks the cell at its frame's distance from its range's start: the offset. Without the prop every range is what
it was.

Tests: `sticky_items_stick_at_their_offset` (ranges, the pinned item at the offset, back to 0, negative); a case of the
`props` fixture (`api/cases/props/props-list-sticky-offset.tsx`, kept as `api/apps/props-0.1.1.4`) whose script checks
the ranges a list with an offset of 24 sends and covers the prop; a web test that such a cell sticks 24 points down.
Docs: `website/docs/lists.md` and `website/zh-CN/docs/lists.md` (the prop's row, a paragraph in "Sticky headers"),
`docs/dev/scrolling-and-lists.md`, `docs/dev/components.md`, `docs/dev/web.md`.

## Screenshot

A list of 26 sections of 30 rows under a translucent 56-point bar, with `contentStyle={{ paddingTop: 56 }}`, scrolled a
few hundred points (500 on the web): the issue's example, in a test app with a row of buttons above it.

| Platform | Before | After |
| -------- | ------ | ----- |
| Web (Chromium) | [web-offset-before.png: without `stickyHeaderOffset`, header A pinned under the bar] | [web-offset-after.png: `stickyHeaderOffset={56}`, header A below the bar, rows under it] |
| iOS (iPhone 17 Pro Max simulator, iOS 26.5) | [ios-offset-before.png] | [ios-offset-after.png] |

Android gets the same ranges and pins them with the same formula (`ScrollBox.pinSticky`); it was not run.

## Public API

- `nsh/list`: `List` prop `stickyHeaderOffset?: number` (`ListHostProps`), points, default 0: how far from the start
  of the viewport (the top, or the left of a horizontal list) sticky items stick, as CSS `position: sticky`'s `top`;
  the content scrolls under the space before them; negative leaves part of a header out of view.
- `nsh-core` (`ui`): `VirtualList::sticky_header_offset(self, offset: Points) -> Self`: the same for Rust components'
  lists.
- `nsh-core` (`ops`): `Prop::StickyOffset(Points)`, the host prop behind both (`#[prop(js = "stickyHeaderOffset",
  default = Points(0.0), on = LIST)]`).
- `nsh-core` (`mount`): `StickyCell { start, end }` keeps its fields and types; they are now documented as the scroll
  offsets between which the cell moves with the scroll (its position less the list's sticky offset), which is what
  they were without an offset.

`ListItemsProps` (the props of a Rust component's `Items`) does not get it: such a component decides where its own
headers stick, and its recorded alias stays as it is. `api/types/nsh/types.d.ts` records the prop (two lines).

## Performance

Without the prop nothing changes. `nsh test --trace` (debug, headless, three interleaved runs), a list of 26 sections
of 30 rows scrolled 9000 points down and 4000 up: 304 commits, 1424 ops / 1436 mount items and 246 nodes before and
after, median commits 815–825 µs before and 824–835 µs after (noise); `examples/counter` (56 ops / 49 items) and
`examples/gallery/Bench.tsx` scrolled 3000 points (2270 ops / 1923 items) the same. With an offset the core does one
more addition per sticky row.

## How to Test

- `cargo test -p nsh-core --lib core::list`
- `NSH_API_APP=props cargo test -p nsh-engine --test engine compat::apps_still_run`: the new case's step expects
  `stickyHeaderIndices start=-24 end=56, start=96 end=Infinity` (items 0 and 3 of 40-point rows, offset 24); it fails
  with the ranges of a list without the offset. `cargo test -p nsh-engine --test engine compat::` (21 tests) and
  `cargo xtask api` (no change).
- `node --test crates/nsh-platform-web/web/scroll.test.mjs` (9 tests).
- In an app: a `List` with `stickyHeaderIndices`, `stickyHeaderOffset={56}` and `contentStyle={{ paddingTop: 56 }}`
  under an absolutely positioned translucent bar 56 points tall. Scroll: each header stops below the bar, rows go on
  under it, and the next header pushes the pinned one out from there.
- What I checked: in Chromium (Playwright), the pinned header's box sits 56 points down at rest and at 500, 1380, 1400
  and 3000 points, the next one pushes it out as above, and in the painted frames of three touch scrolls up and three
  down (CDP screencast) it is drawn at 56 points in 494 of 494 frames at 800 px/s and 417 of 417 at 1200 px/s; on the
  iOS 26.5 simulator the same list shows the header below the bar while scrolled.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test --workspace`, `cargo +nightly fmt --all` and clippy (including the Catalyst and Android runs for platform code). On `main` (`d5839d7a`) itself, `cargo +nightly fmt --all --check` reports three files this PR does not touch (`nsh-core/src/lib.rs`, `nsh-js/src/modules.rs`, `nsh-platform-linux/src/input.rs`) and clippy one warning (`nsh-windows-host/src/lib.rs`, items after a test module); the files this PR changes are clean.
- [x] Changes to what apps use are additive and recorded in `api/` (`cargo xtask api`); the fixture apps in `api/apps` are untouched. The new prop is recorded in `api/types`, the new fixture `api/apps/props-0.1.1.4` covers it (`uncovered.txt` unchanged), and the existing fixtures are untouched.
- [ ] Ran the related `examples/gallery` demos on iOS and Android (if the change is visible or platform-specific). The gallery has no demo of the new prop; the list above ran on the web and the iOS 26.5 simulator, and the gallery's Lists › Sections demo (which does not set it) on the iOS simulator. Android was not run.
