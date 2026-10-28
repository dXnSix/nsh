# Upstream PR: windows: Draw shader effects, transitions and page shaders

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/07-windows`, on `pr/shaders/02-page-dialog-shaders`. One commit (backend, `docs/dev/shaders.md`, `docs/dev/windows.md`).
- Source: the fork's `62193eca1` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/07-windows`).
- Not compiled on the Mac this was prepared on (no MSVC): the Windows workflow is its check.

- Checked locally: nightly rustfmt only (no MSVC on this Mac); the code applied without conflicts. The Windows workflow compiles and tests it.

The body to paste starts below the line.

---

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): Windows draws `ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition`
and pages' and dialogs' `transitionShader` (#[01 PR], #[02 PR]) as `nsh_engine::shader` frames in Direct2D bitmaps.

- Effects and running transitions (screen ones on the root) draw their last frame instead of their children, which
  are drawn into a target bitmap by a context of their own and read back (`render::read_back`) as the window draws
  them, and given to the shader when their pixels changed.
- A transition's `from` is drawn offscreen when it starts (`render::draw_offscreen`).
- A backdrop is read back from the window's target before the view draws; partial frames draw the view's whole area,
  overflow included (`render::drawn_area`).
- Pages and dialogs: recordings (`ID2D1CommandList`) of the host without and with the upper side, drawn into bitmaps
  and read back once by a hosted `NavItem` over the stack or the root.

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| Windows | [the gallery's Shaders page without the shaders] | [the same page with them] |

## How to Test

- On Windows: `cargo clippy -p nsh-platform-windows -p nsh-windows-host -p nsh-windows-app --all-targets -- -D warnings`,
  `cargo test -p nsh-platform-windows -p nsh-windows-host -p nsh-windows-app`.
- With #[10 PR]: the gallery's Shaders page.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [ ] Passed `cargo +nightly fmt` on the changed files; clippy and tests run on Windows only (the Windows workflow).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #[01 PR] and #[02 PR].
- [ ] Ran the related `examples/gallery` demos on Windows.
