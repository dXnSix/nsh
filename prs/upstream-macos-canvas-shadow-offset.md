# Upstream PR: macos: Cast a canvas's shadows the way their offsets point

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#680](https://github.com/longbridge/nsh/pull/680)
(2026-10-28) with the body below the line.

- Branch: `pr/macos/02-canvas-shadow-offset`, one commit on `upstream/main` `ba289971` (rebased 2026-10-28 from its
  2026-10-25 base; no conflicts, and no upstream commit since changed the canvas's shadows).
- Source: the fork's `d63fccc0` (fork PR #52).
- Checked locally on `ba289971`: the macOS clippy run (dev, all targets, `-D warnings`) and `cargo test -p nsh-platform-macos` (the new `a_shadow_falls_by_its_offset_in_the_views_points_in_any_context` among them), nightly rustfmt.
- Screenshots to take before opening: the gallery's canvas demo with a shadow on macOS, before (on `main`) and after.
- Open it from `dXnSix:pr/macos/02-canvas-shadow-offset` into `longbridge/nsh:main`, title (upstream squash-merges): `macos: Cast a canvas's shadows the way their offsets point`.

The body to paste starts below the line.

---

## Description

On macOS every canvas shadow fell up instead of down: `shadowOffsetY: 4` cast the shadow 4 points above the shape.

Core Graphics takes shadow offsets and blurs in the context's base space, which no transform reaches. The canvas
negated the offset's y on the assumption that the base space is y up under the view's y-down space, as it is where the
drawer flips the context itself. But a canvas's content layer has flipped contents, so its base space is y down
already, and the negation turned every shadow around.

The drawer now hands the canvas the transform from the view's space to the base space (in points, flipped or not as it
flipped the context), and the canvas maps offsets and blurs through it. The static part kept as a `CGLayer` uses the
layer's own space (its pixels, y up). Other platforms already cast shadows the way the offsets point.

## Screenshot

No screenshots: the new test reads the drawn pixels instead. A 10-point square at (10, 10) with a hard shadow offset
by (20, 15) now has its shadow's middle at (35, 30), in a flipped and an unflipped context alike; before, it was cast
upward.

## How to Test

- `cargo test -p nsh-platform-macos a_shadow_falls_by_its_offset_in_the_views_points_in_any_context`: draws a shape with
  an offset shadow into a flipped and an unflipped context and checks the shadow's pixels sit below and to the right
  of the shape by the offset, in the view's points, in both.
- `cargo clippy -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app --all-targets --features nsh-platform-macos/dev -- -D warnings`
- In an app on macOS: a `Canvas` that sets `shadowColor`, `shadowBlur` and `shadowOffsetY = 4` before filling a
  rectangle: the shadow shows below it, as on iOS, Android and the web.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files, clippy and `cargo test -p nsh-platform-macos`.
- [x] Changes to what apps use: none in API; macOS now draws canvas shadows as the other platforms do.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: not applicable (macOS only).
