# Upstream PR: linux: Draw shader effects, transitions and page shaders

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/08-linux`, on `pr/shaders/02-page-dialog-shaders`. One commit (backend, `docs/dev/shaders.md`, `docs/dev/linux.md`).
- Source: the fork's `285734297` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/08-linux`).
- Not compiled on the Mac this was prepared on (no GTK): the Linux workflow is its check.

- Checked locally: nightly rustfmt only (no GTK on this Mac); the code applied without conflicts. The Linux workflow compiles and tests it.

The body to paste starts below the line.

---

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): Linux draws `ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition` and
pages' and dialogs' `transitionShader` (#[01 PR], #[02 PR]) as `nsh_engine::shader` frames in `GdkMemoryTexture`s.

- Effects and running transitions draw their last frame instead of their children; whenever GTK draws one again, its
  children's render node is rendered by the window's renderer into a texture of the box and overflow
  (`gsk::Renderer::render_texture`) and downloaded, and given to the shader when its pixels changed.
- A transition's `from` is that picture taken when it starts (or its last frame, if one runs), shown until the first
  frame. Screen layers are the root's.
- A backdrop is the surface's views drawn anew up to the view after the frame clock paints, again when the core marks
  it or anything on the UI thread changes (`NshView::changed`; a shader's own frame only `redraw`s).
- Pages and dialogs: pictures of the host drawn anew without and with the upper side, through a `NavPainter` (the
  layer with one more slot for `nsh.progress`) in a layer over the stack or the root.

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| Linux | [the gallery's Shaders page without the shaders] | [the same page with them] |

## How to Test

- On Linux: `cargo clippy -p nsh-platform-linux -p nsh-linux-host -p nsh-linux-app --all-targets -- -D warnings`,
  `cargo test -p nsh-platform-linux`.
- With #[10 PR]: the gallery's Shaders page.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [ ] Passed `cargo +nightly fmt` on the changed files; clippy and tests run on Linux only (the Linux workflow).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #[01 PR] and #[02 PR].
- [ ] Ran the related `examples/gallery` demos on Linux.
