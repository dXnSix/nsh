# list: Let sticky headers stick a distance from the top

> **Upstream (checked 2026-11-01):** opened and merged as longbridge/nsh#285 (2026-10-21).

Branch: `feat/list-sticky-offset` on `github.com/dXnSix/nsh`, cut from `fix/web-sticky-compositor`, whose web tracks it
builds on; upstream it stacks on that fix. One commit, `7b09c216`; merged into the fork's `main` as fork PR #14 (merge
commit `df084a10`, in `six-gamify-0.1.1-7`). It follows upstream by merging `upstream/main` in, never by rebasing. Not opened upstream; the owner decides and posts there (upstream squash merges).

## Summary

`stickyHeaderIndices` pins headers at the very top of a list. Under a translucent bar (a navigation bar the content
scrolls under, a floating search field), they belong below it, as CSS `position: sticky; top: 56px` does on the web.

`stickyHeaderOffset` (TSX, a number of points, default 0) and `VirtualList::sticky_header_offset` (Rust components) set
that distance. The content keeps scrolling under the space above the headers; the next header pushes the pinned one
out where it reaches its bottom edge; a negative offset leaves part of a header out of view.

The core takes the offset into account where it decides which header is pinned (`active_sticky`: the last one that
reached the offset) and subtracts it from the travel ranges it sends (`StickyCell { start, end }` are now documented as
the scroll offsets between which a cell moves with the scroll). Every platform already shows a sticky cell
`clamp(offset - start, 0, end - start)` past its frame, so the native backends need no change, and the web's
`position: sticky` track (the previous fix) sticks the cell at its frame's start less the range's start: the offset.

## Public API

- TSX: `List`'s `stickyHeaderOffset?: number` (`ListHostProps`). Not in `ListItemsProps`: a Rust component that lays
  out an app's rows decides where its headers stick (`sticky_header_offset`), and the alias stays as recorded.
- Rust: `VirtualList::sticky_header_offset(self, offset: Points) -> Self`; `Prop::StickyOffset(Points)`.
- `api/types/nsh/types.d.ts` records it (two lines); `api/apps/props-0.1.1.2` is the `props` fixture with the
  `props-list-sticky-offset` case, whose script checks the ranges the platform gets and covers the prop.

## Breaking Changes

None: without the prop, every range is what it was.

## Docs

`website/docs/lists.md` and `website/zh-CN/docs/lists.md` (the prop's row, a paragraph in "Sticky headers");
`docs/dev/scrolling-and-lists.md`, `docs/dev/components.md`.

## Validation

- `cargo test -p nsh-core --lib core::list`: `sticky_items_stick_at_their_offset` (ranges, the pinned item at the
  offset, back to 0, negative).
- `NSH_API_APP=props cargo test -p nsh-engine --test engine compat::apps_still_run`: the new case's step
  (`stickyHeaderIndices start=-24 end=56, start=96 end=Infinity` for items 0 and 3 of 40-point rows at offset 24) passes,
  and fails with a wrong value; `cargo test -p nsh-engine --test engine compat::` (20 tests), `cargo xtask api`.
- `cargo test -p nsh-core --lib` (442), `cargo test -p nsh-js --lib` (169), `cargo +nightly fmt --all`,
  `cargo clippy -p nsh-core -p nsh-js --all-targets`.

## Other platforms

None needed: the offset reaches them through the ranges they already use.
