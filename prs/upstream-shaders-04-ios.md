# Upstream PR: ios: Draw shader effects, transitions and page shaders

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/04-ios`, on `pr/shaders/02-page-dialog-shaders`. Two commits: the backend and its section of
  `docs/dev/shaders.md`. Independent of the macOS PR; if that one is merged first, this one's table in
  `docs/dev/shaders.md` conflicts with its (keep both rows, and the paragraph says "Both retake ..." for the two).
- Source: the fork's `dac91e56b` and the iOS part of `1d2f5ab68` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/04-ios`).

- Checked locally on `ba289971`: the Catalyst clippy run (embed, dev); nightly rustfmt. Not run on the simulator here; the same code passed `shaders.test.tsx` on the iOS simulator on the fork's `sync/upstream-478737b1`.

- 2026-10-29: rebased on the new 02 (`df0f900bf`), no conflicts. Checked locally: the Catalyst clippy run (embed,
  dev); nightly rustfmt.
- 2026-10-31: rebased on the new 02 (`192c3b012`), no conflicts; it differs from the previous head only by 01's
  padding commit and 02's docs line (previous head kept locally as `old-pre-pad/04-ios`).

The body to paste starts below the line.

---

**Depends on:** #682 and #[02 PR] (the elements and pages' and dialogs' `transitionShader` it draws); merge it after
both. Independent of the other platform PRs (the macOS PR shares a table of `docs/dev/shaders.md` with it).

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): iOS draws `ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition`
and pages' and dialogs' `transitionShader` (#682, #[02 PR]) on `Shader`'s renderer.

- **Pictures.** `renderInContext:` of a view holding the children (`Decor::set_effect`; the view children are added
  to) without its mask, at the screen's scale; the backdrop is the window with the view's layer at opacity 0. They are
  taken again on the display frame after the core marks them or the platform changed a view in them (animated
  values, scrolling, image loads), at most once per frame.
- **Effects and transitions.** An empty `mask` on that view's layer hides the children while the frames show over
  them (UIKit still hit-tests the views), in a sublayer at `zPosition` 1e6 reaching `overflow` points beyond the box.
  Screen layers are on the root view (dialogs, presented by UIKit above it, are outside).
- **Pages and dialogs.** The screens change, and the dialog is presented or dismissed, at once under a layer over the
  window's part where the surface is, from pictures before the change and after it (the first tick); popping and
  dismissing run backwards.

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| iOS | [the gallery's Shaders page without the shaders] | [the same page with them] |

(Screenshots to take on the iOS simulator with this branch and #[10 PR]'s demo.)

## How to Test

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`
- With #[10 PR]: `nsh test examples/gallery/tests/shaders.test.tsx --target ios` on a simulator.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and clippy (Catalyst) (clean).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #682 and #[02 PR].
- [ ] Ran the related `examples/gallery` demos on iOS (not run on this base; the fork's sync branch ran `shaders.test.tsx` on the iOS simulator).
