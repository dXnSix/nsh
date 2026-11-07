# Upstream PR: web: Draw shader effects, transitions and page shaders

Opened 2026-11-01 as [longbridge/nsh#767](https://github.com/longbridge/nsh/pull/767), at the owner's decision (all nine together, the platform PRs and the gallery's marked as depending on #763); the body
below the line is the draft it was posted from, with the PR numbers filled in.

- Branch: `pr/shaders/06-web`, on `pr/shaders/02-page-dialog-shaders`. One commit (backend, `docs/dev/shaders.md`, `docs/dev/web.md`).
- Source: the fork's `ab266134a` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/06-web`).

- Checked locally on `ba289971`: the wasm32 clippy run (`compiler`), `node --test` of `nsh.test.mjs`, `scroll.test.mjs`, `mouse.test.mjs`, `local-time.test.mjs` (43 of 43), nightly rustfmt. Not run in Chromium here; the same code passed `shaders.test.tsx --target web` on the fork's sync branch.

- 2026-10-29: rebased on the new 02 (`d6f042595`), no conflicts. Checked locally: the wasm32 clippy run (`compiler`),
  `node --test` of `nsh.test.mjs`, `scroll`, `mouse`, `local-time` (45 pass), nightly rustfmt.
- 2026-10-31: rebased on the new 02 (`32dcc856a`), no conflicts; it differs from the previous head only by 01's
  padding commit and 02's docs line (previous head kept locally as `old-pre-pad/06-web`).

- 2026-11-01: #682 (01) was merged (squashed, `8274f6b05`), so this branch is rebuilt on `upstream/main`
  `13d87a11` (`git cherry-pick` of its own commits, no conflicts; old head `32dcc856a`, new head `19dfc38d2`). Fixed for 01's
  `#[non_exhaustive]` `ShaderRole`: the layer encoder gives a role it does not know the fill's number, 0 (wasm32
  clippy found it).

- 2026-11-01, later: rebuilt again on `upstream/main` `e4e464f4` (23 commits past `a5036af4`), no conflicts of its own; head `51b5ef9ae`.

## Review (2026-11-02)

huacnlee's review (ChatGPT on his behalf, a summary without an inline thread): "[P2] Restore original content when
WebGL rendering is unavailable": `op_sh` added `nsh-fx` before rendering, and `renderShader` returned without WebGL 2
or with a refused program, so the children stayed hidden. It held. Fix `4fd07f2ee` on `pr/shaders/06-web`
(fast-forward, the fork's `2f13b741e` in dXnSix/nsh#107): `shaderDraws(layer)` (WebGL 2 and a program the driver takes)
gates `nsh-fx` (`op_sh` ends a run and clears the canvas through `endTransition` when it cannot draw), `op_sht`,
`shaderNav` (returns false: the page's own motion), `screenShown` and `shaderFrame` (no pictures, no frames). Five
`nsh.test.mjs` tests fail on the old code. Checked: fork CI all green on #107, `tmp/shaders-06-web-fallback` (Rust
application, CI paths), node tests, and Chromium with WebGL 2 off (effect, transition, screen effect, page, dialog show
their content) and on. Replied in a PR comment (`issuecomment-6567282492`).

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

## Review 2 (2026-11-02, huacnlee's agents): follow-ups after the series lands

The maintainer's review asked for one contract across the platforms (precedence over `animation: "none"`, reduce
motion, interruption), a Rust builder and per-platform perf fixes. The user settled the contract (the shader replaces
`animation`; reduce motion skips the shader; a change that starts while one runs ends that one first). The
maintainer then pushed its own builder to #763 (`2c5d96585`) and said the rest comes as follow-up PRs after the series
lands; the user chose that. Nothing was pushed to the `pr/shaders/*` branches. Replied on #763
(`issuecomment-6582820956`). The fixes are on fork branches, listed with their commits in the shared note
`side-session-notes/local_3fb0899a-…/shaders-followups.md`; open them as follow-up PRs, core first, once the series is
merged.
