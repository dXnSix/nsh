# Upstream PR: web: Draw shader effects, transitions and page shaders

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/06-web`, on `pr/shaders/02-page-dialog-shaders`. One commit (backend, `docs/dev/shaders.md`, `docs/dev/web.md`).
- Source: the fork's `ab266134a` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/06-web`).

- Checked locally on `ba289971`: the wasm32 clippy run (`compiler`), `node --test` of `nsh.test.mjs`, `scroll.test.mjs`, `mouse.test.mjs`, `local-time.test.mjs` (43 of 43), nightly rustfmt. Not run in Chromium here; the same code passed `shaders.test.tsx --target web` on the fork's sync branch.

The body to paste starts below the line.

---

**Depends on:** #682 and #[02 PR] (the elements and pages' and dialogs' `transitionShader` it draws); merge it after
both. Independent of the other platform PRs.

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): the web backend draws `ShaderEffect`, `ShaderTransition`, `ScreenEffect`,
`ScreenTransition` and pages' and dialogs' `transitionShader` (#682, #[02 PR]) on its shared WebGL 2 context.

- The layer's JSON carries its role, overflow, progress, progress slot and the program's pictures (sampler uniform
  names and inputs); `MountItem::ShaderTransition`, `ShaderInput` and `NavShader` are the ops `sht`, `shi` and `shn`.
- **Pictures** are the DOM painted into a 2D canvas (`paintTree`: backgrounds, rounded corners, uniform borders,
  fitted images, canvases, other shaders' pictures, text runs, clipping and opacity; no rotations, gradients or
  shadows; cross-origin images without CORS are left out so the canvas stays readable), covering the box and the
  overflow, uploaded premultiplied to the program's samplers.
- **Effects**: the output canvas is over the children, which stay at opacity 0 and hit-testable under it; they are
  pictured again on `shi`, DOM mutations, scroll and load, at most once per frame. A backdrop is the surface painted up
  to the view.
- **Transitions**: `sht` comes first in its batch, so the children as they show become `sample_from`; the children
  after the batch are `sample_to`, and `nsh.progress` runs over the duration on animation frames. Screen effects and
  transitions do the same over the root.
- **Pages and dialogs** run on an overlay over the stack or the root; dialogs shown in the top layer
  (`dialogs: "viewport"`) keep their own motion.

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| Web (Chromium) | [the gallery's Shaders page without the shaders] | [the same page with them] |

## How to Test

- `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown --features compiler -- -D warnings`
- `node --test crates/nsh-platform-web/web/` (the backend's own tests).
- With #[10 PR]: `nsh test examples/gallery/tests/shaders.test.tsx --target web`.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and the wasm32 clippy run (clean).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #682 and #[02 PR].
- [ ] Ran the related `examples/gallery` demos on the web (not run on this base; the fork's sync branch ran `shaders.test.tsx` on the web).
