# Upstream PR: gallery: Show shader effects and transitions on the Shaders page

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/10-gallery-docs`, on `pr/shaders/02-page-dialog-shaders`. Two commits:
  the gallery's Shaders page with its test, and the website's Shader page in English and Chinese.
- Source: the fork's `26fb47163` and the `website/` part of `deac47d86` on `sync/upstream-478737b1`.
- It needs only 01 and 02 to build; its test shows the shaders on a platform once that platform's PR (03–08) is in.
  Open it last, after rebasing onto `upstream/main`
  (`git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/10-gallery-docs`).

- Checked locally on `ba289971`: `nsh test examples/gallery/tests/shaders.test.tsx` headless, 2 of 2; on macOS with 03 merged, 2 of 2.

The body to paste starts below the line.

---

**Depends on:** #682 and #[02 PR] to build; its demo shows the shaders on a platform once that platform's PR is
merged (#[03 PR] to #[08 PR]), so merge it last.

## Description

The gallery and the docs for the shader series (#682 elements, #[02 PR] page and dialog shaders, the platform
PRs):

- **Gallery** (`examples/gallery/demos/Shaders.tsx`): a `ShaderEffect` that pixelates a tile (drag across it, or the
  Pixelate and Sharpen buttons animate `progress`), a `ShaderTransition` that wipes between slides, a `ScreenEffect`
  toggle (the whole surface in two inks), and a page and a dialog that come and go through an iris
  (`transitionShader`). `examples/gallery/tests/shaders.test.tsx` presses through them: the controls keep working
  under the frames, the transition's new children show once it ran, the page and the dialog come and go.
- **Website** (`website/elements/shader.md`, `website/zh-CN/elements/shader.md`): a section "Effects, transitions and
  the screen": the four `sample_` functions, `nsh.progress` and `nsh.overflow`, `ShaderEffect`, `ShaderTransition`,
  `ScreenEffect`, `ScreenTransition` and `transitionShader`, with examples, their props and the Rust methods.

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| iOS      | [the Shaders page today] | [the page with the effect, transition, screen and page cards] |
| Android  | [the Shaders page today] | [the page with the effect, transition, screen and page cards] |

## How to Test

- `nsh test examples/gallery/tests/shaders.test.tsx` (headless), and with the platform PRs `--target macos`, `ios`,
  `android`, `web`.
- The website: `cargo xtask web`, then the Shader page.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] No Rust changes; the gallery's TSX type-checks as the gallery's build does (the gallery bundles and its headless tests pass).
- [x] Changes to what apps use: none here.
- [ ] Ran the related `examples/gallery` demos on iOS and Android (ran on macOS (with 03) and headless; iOS, Android and web on the fork's sync branch).
