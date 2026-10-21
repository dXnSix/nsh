# macos: Show a border that a state turns visible, as focus:border-white

Branch: `feat/macos-focus-style` on `github.com/dXnSix/nsh`. It follows upstream by merging `upstream/main` in, never by
rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened as a pull request into the fork's `main`, Gamify's
integration line, and merged there with a merge commit. One commit, `e93abd8`. Not opened upstream; the owner
decides and posts there (upstream squash merges).

## Summary

On macOS a view styled `border-2 border-transparent focus:border-white` never shows its border when it takes the
keyboard focus (the same holds for `hover:` / `active:` borders over a transparent one, and for a Rust element's
`.focus(|s| s.border_color(…))`). The focused state does arrive: `focus:bg-…` on the same view applies.

A transparent border is shaped without a border layer (the layer's `borderWidth` is 0). The `Shape` the decoration
compares to decide whether to rebuild its layers recorded the widths and whether the sides share a color, but not
which sides are visible, so a state that only changes the border's color was applied as a recoloring, and
`set_border_colors` skips a layer whose border width is 0. `Shape` now records the visible sides
(`visible_sides`): a border that turns visible or invisible rebuilds its layers, and a recoloring of a visible
border stays as cheap as before.

Found while verifying Gamify's keyboard focus ring on macOS: Shift+Tab reached a slip's remove button and Space
pressed it, but no ring showed.

## Public API

None.

## Breaking Changes

None.

## Docs

None: it makes `focus:` / `hover:` / `active:` borders behave as documented.

## Validation

- `cargo test -p nsh-platform-macos a_border_turning_visible_reshapes`: a transparent and a white 2 pt border shape
  differently; two visible colors do not (the test fails without the change).
- A scripted host (`nsh run --target macos --script 'tap …; key shift+Tab; shot …'`) of two pressables with
  `border-4 border-transparent focus:border-white focus:bg-sky-600`: before, only the background changed; after,
  the white border follows the focus.
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-platform-macos --all-targets`: clean.

## Other platforms

iOS applies a view's whole decoration again when its style changes and has no keyboard focus ring; its animated
`borderColor` path has the same `borderWidth() > 0` guard and could follow in another change if an animated border
from transparent is needed.
