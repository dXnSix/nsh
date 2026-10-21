# list: Let sticky headers stick below a bar the content scrolls under

Draft of an upstream issue for `longbridge/nsh`, drafted for the owner before the PR
([`upstream-list-sticky-offset.md`](../prs/upstream-list-sticky-offset.md)). Posted as [longbridge/nsh#279](https://github.com/longbridge/nsh/issues/279) (2026-10-21).

---

**Title:** `list: Let sticky headers stick a distance from the top (stickyHeaderOffset)`

## The problem

`stickyHeaderIndices` pins headers at the very top of a `List`. Many screens have something over the top of the list
that the content scrolls under: a translucent navigation bar, a floating search field or filter chips, a large title
that collapses. The headers belong below it, as CSS `position: sticky; top: 56px` keeps them on the web; at the top of
the list they hide under the bar.

An app cannot get this today:

- Padding the list (`contentStyle={{ paddingTop: 56 }}`) moves the first rows below the bar, but the headers still pin
  at the top, under it.
- Laying the list out below the bar (a `marginTop`, or the bar outside the list) keeps the headers visible, but the
  content no longer scrolls under the bar, which is what the translucent bar is for.
- Pinning the headers in JS from `onScroll` runs a frame late and crosses to JS on every scroll frame.

React Native has no prop for it either; FlashList v2 has `stickyHeaderConfig.offset` for this case.

## Proposal

A `List` prop and the same on Rust components' lists:

- TSX: `stickyHeaderOffset?: number` (points, default 0): how far from the top (from the left in horizontal lists)
  sticky items stick, as CSS `position: sticky`'s `top`. The content keeps scrolling under the space above them; the
  next sticky item pushes the pinned one out where it reaches that one's bottom edge; a negative offset leaves part of
  a header out of view.
- Rust: `VirtualList::sticky_header_offset(offset: Points) -> Self`.

```tsx
<View style={{ flex: 1 }}>
  <List each={rows} stickyHeaderIndices={headers} stickyHeaderOffset={56} contentStyle={{ paddingTop: 56 }} style={{ flex: 1 }}>
    {(row) => <Row row={row()} />}
  </List>
  <BlurredBar style={{ position: "absolute", top: 0, left: 0, right: 0, height: 56 }} />
</View>
```

## How it fits

The core computes sticky travel, and every platform shows a sticky cell `clamp(offset - start, 0, end - start)` past
its frame (`MountItem::StickyCells`). The core would count the offset where it picks the pinned header and subtract it
from the ranges it sends, so `start` and `end` become the scroll offsets between which a cell moves with the scroll.
Without the prop every range stays what it is, and no platform needs a change: they already pin from the ranges. The
web, once it pins with `position: sticky` (#[web sticky issue]), derives the cell's inset from the same ranges.

The prop would be recorded in `api/` with a `props` fixture case that checks the ranges a list with an offset sends.

## Questions

- The name: `stickyHeaderOffset` sits next to `stickyHeaderIndices`; `stickyHeaderInset` would also read well.
- Whether a Rust component that places an app's rows (an `Items` prop) should take it from the app too. The proposal
  keeps it on `List` and `VirtualList` only: such a component decides where its own headers stick.
