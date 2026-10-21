# list: The previous sticky header can be missing when a list scrolls back into its section

Draft of an upstream issue for `longbridge/nsh`, drafted for the owner with its PR
([`upstream-sticky-row-before-pinned.md`](../prs/upstream-sticky-row-before-pinned.md)). Posted as [longbridge/nsh#278](https://github.com/longbridge/nsh/issues/278) (2026-10-21), in the bug template's sections.

---

**Title:** `list: The previous sticky header is not rendered until it is pinned, so scrolling back up can show a frame without it`

## What happens

In a `List` with `stickyHeaderIndices` whose sections are longer than its overscan (the list renders the viewport plus
one viewport each way), scroll down into a section, so that its header is pinned, and then back up past the
start of that section. The previous section's header should be pinned at the top at once, with the section's own
header pushed down below it. For a moment the top shows the previous section's rows without any header.

On the web it shows in captured frames (Chromium, CDP screencast during touch scrolls), once the web pins sticky
cells in step with its scrolling (#[web sticky issue]; before that, its one-frame lag hides it): in a horizontal list
with sections of 8 cells of 80 points, every scroll back over the sections has one frame with a 6-point gap at the
list's start where the previous header belongs, and an app's offer list with long sections of league rows showed the
same scrolling up. How long it lasts depends on how long binding the header's cell takes against the platform's
frame: on the web the binding runs on the page's main thread right after the scroll event; on the native platforms it
crosses from the main thread to the engine thread and back.

## Why

`ListState::update_window` binds the items in the render window and the active sticky item (`active_sticky`: the last
sticky item that starts before the viewport's start). The previous sticky item is neither while its section is longer
than the overscan: its header is far above the window. When the scroll offset crosses back over the pinned header's
start, the previous item becomes the active one and is bound in that round, and its cell renders a round later, while
the platform already draws the new offset.

## Proposal

Keep the sticky item before the active one bound too. Every platform already places a sticky cell from its range of
travel, so the extra cell sits pushed out above the pinned header, out of sight, until the list scrolls back into its
section; then it is there in the first frame. It costs one more bound cell (the header's) in lists with sticky items,
none in others.
