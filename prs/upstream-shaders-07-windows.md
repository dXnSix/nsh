# Upstream PR: windows: Draw shader effects, transitions and page shaders

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/07-windows`, on `pr/shaders/02-page-dialog-shaders`. One commit (backend, `docs/dev/shaders.md`, `docs/dev/windows.md`).
- Source: the fork's `62193eca1` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/07-windows`).
- Not compiled on the Mac this was prepared on (no MSVC): the Windows workflow is its check.

- Checked: nightly rustfmt locally (this Mac cannot build Windows code); the fork's Windows workflow on this branch at
  `ba289971` passed (clippy and tests): https://github.com/dXnSix/nsh/actions/runs/40755878661

- 2026-10-29: rebased on the new 02 (`a8575e071`), no conflicts; nightly rustfmt locally. The fork's Windows workflow
  decides.
- 2026-10-31: rebased on the new 02 (`40a7d2a22`), no conflicts; it differs from the previous head only by 01's
  padding commit and 02's docs line (previous head kept locally as `old-pre-pad/07-windows`).

- 2026-11-01: #682 (01) was merged (squashed, `8274f6b05`), so this branch is rebuilt on `upstream/main`
  `13d87a11` (`git cherry-pick` of its own commits, no conflicts; old head `40a7d2a22`, new head `4bb2209a0`). Fixed for 01's
  typed `progress`: `with_progress(Percentage::ZERO, …)`, as on Android (found by reading; Windows builds only in
  its workflow).

The body to paste starts below the line.

---

**Depends on:** #682 and #[02 PR] (the elements and pages' and dialogs' `transitionShader` it draws); merge it after
both. Independent of the other platform PRs.

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): Windows draws `ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition`
and pages' and dialogs' `transitionShader` (#682, #[02 PR]) as `nsh_engine::shader` frames in Direct2D bitmaps.

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
- [x] Passed `cargo +nightly fmt` on the changed files, and the Windows workflow's clippy and tests (on the fork).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #682 and #[02 PR].
- [ ] Ran the related `examples/gallery` demos on Windows.
