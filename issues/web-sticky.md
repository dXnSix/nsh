# web: Sticky list headers trail the scroll, and recycled headers leave rows out of place

Draft of an upstream issue for `longbridge/nsh`, drafted for the owner with the PR
([`upstream-web-sticky.md`](../prs/upstream-web-sticky.md)). Posted as [longbridge/nsh#277](https://github.com/longbridge/nsh/issues/277) (2026-10-21), in the bug template's sections. A related core change, the previous sticky row
kept rendered, has an issue of its own ([`sticky-row-before-pinned.md`](sticky-row-before-pinned.md)).

---

**Title:** `web: Sticky list headers trail the scroll, and recycled ones draw rows over each other`

## What happens

On the web (`nsh run --target web`, `nsh package --target web`), a `List` with `stickyHeaderIndices`:

1. **Draws rows over each other and leaves gaps** once it has been scrolled past a few sections: a row of an earlier
   section shows up in a later one, over the row that belongs there, or its place stays empty.
2. **Lets the pinned header trail touch and touchpad scrolling.** While a finger or a touchpad moves the list, the
   pinned header sits below the top by the distance the list moves in one frame, with rows showing above it, and
   slides back as the scrolling slows, which looks like an animation.
3. **Draws the pinned header over the views after the list**, such as a translucent bar laid over the top of the list
   that the content scrolls under. Natively the bar covers the header.

The other platforms pin sticky cells correctly.

## How to reproduce

A list with long sections shows both (`nsh run . --target web`, then scroll down a few thousand points and drag up
and down with a touchscreen, touch emulation or a touchpad):

```tsx
import { mount, Text, View } from "nsh";
import { List } from "nsh/list";

type Row = { header: boolean; label: string };
const rows: Row[] = [];
const headers: number[] = [];
for (const letter of "ABCDEFGHIJKLMNOPQRSTUVWXYZ") {
  headers.push(rows.length);
  rows.push({ header: true, label: letter });
  for (let i = 1; i <= 30; i++) rows.push({ header: false, label: `${letter} ${i}` });
}

function App() {
  return (
    <List testID="list" each={rows} stickyHeaderIndices={headers} estimatedItemSize={44} style={{ flex: 1 }}>
      {(row) => (
        <If
          when={row().header}
          else={
            <View style={{ height: 44, justifyContent: "center", paddingLeft: 16, backgroundColor: "#ffffff" }}>
              <Text>{row().label}</Text>
            </View>
          }
        >
          <View style={{ height: 32, justifyContent: "center", paddingLeft: 16, backgroundColor: "#d0006f" }}>
            <Text style={{ color: "#ffffff", fontWeight: "700" }}>{row().label}</Text>
          </View>
        </If>
      )}
    </List>
  );
}

mount(App);
```

Measured in Chromium (Playwright at 420 × 800; the painted frames come from a CDP screencast during
`Input.synthesizeScrollGesture` touch scrolls, three up and three down):

| Touch scroll speed | Frames with the pinned header off its place | How far |
|---|---|---|
| 800 px/s | 74 of 490 | 13–14 px in most |
| 1200 px/s | 47 of 417 | 20 px in most |

Horizontal lists and refreshable ones (`onRefresh`) show the same. Scrolled in 60 steps of 600 px, the rows overlapped
64 times. The gallery's Lists › Sections demo shows the trailing faintly (2 of 177 frames, 5–6 px): its list is short
and fits in the render window, so its headers are never recycled.

## Why

`nsh.js` keeps the ranges of `MountItem::StickyCells` (`op_sticky`), and `updateSticky`, called from the scroll view's
`scroll` events, sets each cell's `translate` to `clamp(offset - start, 0, end - start)`, with `nsh-stuck`
(`z-index: 5`) while it is shifted.

1. It only updates the cells the core lists. When a header leaves the render window, its cell is recycled for another
   item and the next `StickyCells` no longer lists it, so it keeps its last `translate` (a pushed-out header's is
   nearly the length of its section) and `nsh-stuck`, whatever row it shows next. The native backends put such cells
   back: `scroll::sticky` on macOS, Windows and Linux ("Cells no longer sticky go back to their frames"), the identity
   transform on iOS, `setTranslationY(0)` on Android.
2. The browser scrolls on its compositor, and `scroll` events reach the page after it has drawn the content at the new
   offset, so a shift set from them is a frame late. The native backends pin in their synchronous scroll callbacks;
   the desktop idiom's scroll bar on the web avoids the lag with a sticky `.nsh-bar`.
3. `nsh-stuck`'s `z-index: 5` is the page's, since no view makes a stacking context; the native backends bring sticky
   cells to the front of their list only.

## Proposed fix

Let the browser pin sticky cells with `position: sticky`, which moves them in step with its scrolling:

- Each sticky cell sits in a track, an absolutely positioned box from its frame along its range of travel, in which the
  cell is `position: sticky` with its frame's distance from the range's start as its inset. The next sticky cell's
  track starts where it pushes the cell out; the last one's ends with the content (`ScrollContent`), so it adds no
  scrollable space.
- The track has no breadth across the list, so it takes no pointer events and the cell takes those any cell takes. It
  sits where the cells are (in the content layer of a refreshable or zoomable list) and keeps the cell above the
  others (`z-index: 5`, as `nsh-stuck` did); a list with sticky cells isolates that z-index, so they stay under the
  views after the list.
- Cells that leave the sticky set go back to their frames, removed cells take their tracks along, and sticky cells skip
  layout animations (the browser moves them). No scroll handler moves anything.
