# Upstream PR: gpui: Show a canvas's latest frame when it changes on every frame

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/gpui/01-canvas-one-job`, one commit on `upstream/main` `ba289971` (rebased 2026-10-28 from its
  2026-10-25 base; it applied without conflicts, as no upstream commit since touched the file).
- Source: the fork's `21595bca` (fork PR #38).
- Checked locally on `ba289971`: the GPUI workspace's clippy (`nsh-platform-gpui/dev`, all targets, `-D warnings`) and `cargo test` (the new `a_canvas_changing_on_every_frame_keeps_one_job_in_flight` among them), nightly rustfmt.
- Open it from `dXnSix:pr/gpui/01-canvas-one-job` into `longbridge/nsh:main`, title (upstream squash-merges): `gpui: Show a canvas's latest frame when it changes on every frame`.

The body to paste starts below the line.

---

## Description

On GPUI a canvas whose animated values move the whole drawing (no area bounds them) is rasterized again on a worker on
every frame. Each frame queued another job while the last one was still being made, so on a busy machine or a 2x
display the two workers fell behind and delivered frames ever older: an animation held at a moment showed another
moment for a while, catching up only as the queue drained.

A canvas now keeps one job in flight (`CanvasState::ask`). While a worker makes a version, frames that change the
drawing only mark it stale; when the job lands (`landed`), the canvas draws again and asks for the latest version if it
is not the one that landed. A rasterization that lands late (an older version) frees nothing. Canvases that change
less often than a job takes behave as before.

## Screenshot

No still difference: what changes is how far the shown frame lags behind the animation.

## How to Test

- `(cd crates/gpui && cargo test -p nsh-platform-gpui a_canvas_changing_on_every_frame_keeps_one_job_in_flight)`: ten
  changes while a job runs queue nothing; the landing asks for the latest; a late landing frees nothing.
- `(cd crates/gpui && cargo clippy --all-targets --features nsh-platform-gpui/dev -- -D warnings)`, `cargo test`.
- In an app on GPUI: a `Canvas` drawn from an animated value that loops quickly (a rotating shape), on a 2x display.
  Before, pausing the animation leaves the canvas moving on for a moment; after, it stops where the value stopped.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed file and the GPUI workspace's clippy and tests.
- [x] Changes to what apps use: none (GPUI's canvas rasterization only).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: not applicable (GPUI only).
