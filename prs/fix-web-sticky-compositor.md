# web: Keep sticky list headers pinned in step with the browser's scrolling

Branch: `fix/web-sticky-compositor` on `github.com/dXnSix/nsh`, cut from `fix/web-sticky-reset` (`53d2a401`, fork PR #12,
on `upstream/main` `5f8daece`), so upstream it stacks on that fix. It follows upstream by merging `upstream/main` in,
never by rebasing. Two commits: `f93a187e` (web) and `324ac1a1` (core). Merged into the fork's `main` as fork PR #13
(merge commit `c9e8445d`, in `six-gamify-0.1.1-7`). Not opened upstream; the owner decides and posts there (upstream
squash merges).

## Summary

On the web, a sticky header of a `List` (`stickyHeaderIndices`) does not stay on its place while the list scrolls: it
sits below the top (scrolling up; above it, scrolling down) by the distance the list moves in one frame, and slides back
as the scrolling slows down, which looks like an animation. In frames captured during a trackpad-style scroll gesture
(Chromium, CDP screencast), the pinned header was drawn 13–14 px below the top at 800 px/s and 20 px at 1200 px/s:
exactly one 60 Hz frame of travel. A mouse wheel's scroll did not show it.

The web pinned a sticky cell by setting its `translate` from the scroll view's `scroll` events. The browser scrolls on its
compositor, ahead of those events, so each frame showed the content at the new offset and the header at the previous
one. The native backends pin in their synchronous scroll callbacks and do not have this.

Each sticky cell now sits in a track, an absolutely positioned box spanning its range of travel (from its frame to the
`end` of its range plus its own size), and the browser's `position: sticky` keeps it pinned inside, the next header
pushing it out where the track ends. A sticky cell's frame places its track; the last one's track ends with the
content (`ScrollContent`), so it never adds scrollable space; cells that stop being sticky leave their tracks for their
frames (the reset of the previous fix), and removed cells take their tracks along. No scroll handler moves anything any
more. Tracks pass pointer events through; the cell in one takes them as before.

The core keeps one more cell rendered: the sticky row before the pinned one. In a section longer than the overscan,
that header was bound only once it became the pinned one, so scrolling back up past the pinned header's start showed
one frame without it (on every platform; seen in the captured frames). The platforms place it pushed out above the
pinned header, out of sight until the list scrolls back into its section.

## Public API

None.

## Breaking Changes

None. Sticky headers behave on the web as documented and as on the native platforms.

## Docs

`docs/dev/scrolling-and-lists.md`: how the web keeps sticky cells pinned.

## Validation

- Captured frames (Gamify's offer list, CDP screencast during `Input.synthesizeScrollGesture` touch scrolls up): before,
  the header was off its place in 236 of 258 frames at 800 px/s (13–14 px) and in 199 of 220 at 1200 px/s (up to 28 px);
  after, in 1 of 259 (1 px) and 1 of 225 (the one frame the previous header was missing, which the core change fixes).
- No running animation touches the headers (`document.getAnimations()` per frame: only the scroll bar's fade).
- Gamify's per-frame tiling probe (rows below the pinned header tile the list, through startup, scrolling, nav,
  filters and collapses): 0 faulty frames of 3,191; a pinned header in its track takes a click (it collapses).
- `cargo test -p nsh-core --lib core::list` (16 tests, `the_sticky_item_before_the_pinned_one_stays_rendered` new),
  `cargo test -p nsh-engine --test engine rust_lists`, `cargo +nightly fmt --all`, `cargo clippy -p nsh-core
  --all-targets`: clean.

## Other platforms

The native backends already pin in their scroll callbacks; they get the core change (one more bound cell near a sticky
section's start) and nothing else.
