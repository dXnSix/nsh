# Upstream PR: ios: Draw shader effects, transitions and page shaders

Opened 2026-11-01 as [longbridge/nsh#765](https://github.com/longbridge/nsh/pull/765), at the owner's decision (all nine together, the platform PRs and the gallery's marked as depending on #763); the body
below the line is the draft it was posted from, with the PR numbers filled in.

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

- 2026-11-01: #682 (01) was merged (squashed, `8274f6b05`), so this branch is rebuilt on `upstream/main`
  `13d87a11` (`git cherry-pick` of its own commits, no conflicts; old head `192c3b012`, new head `e1b61742a`).

- 2026-11-01, later: rebuilt again on `upstream/main` `e4e464f4` (23 commits past `a5036af4`), no conflicts of its own; head `5e3f94418`. One context change: upstream's #742 added `alert_tint` beside the `nav_shaders` field `Mounter` gains here.

## Review (2026-11-02)

huacnlee: "[P2] Keep native content visible if renderer initialization fails": held. Fix `2765760a5` (fork
`8b57bfd01`): children are masked only with a renderer (`hides`), transitions start no run without one;
`nav_overlay` returns whether it runs, so `reconcile` and dialogs keep UIKit's motion; `sync_mask` keeps the root
masked while any screen shader still draws it. Fork CI ios green (#107), Catalyst clippy (embed, dev) and the
gallery's steps in an iOS simulator locally. Pushed after the 02 merge (`92e4dc9db`); replied
`issuecomment-6572825745`.

Later on 2026-11-02: the upstream web check failed on main's own menu bar hover test (#791, fixed by #795 three
minutes after the run started). Merged `upstream/main` `ca3dbd60` (no conflicts): fork Rust application, Compat and
CI paths green on `tmp/shaders-04-ios-review`, Catalyst clippy locally. Pushed, head `372f309f0`.

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

## Review 2 (2026-11-02, huacnlee's agents): follow-ups after the series lands

The maintainer's review asked for one contract across the platforms (precedence over `animation: "none"`, reduce
motion, interruption), a Rust builder and per-platform perf fixes. The user settled the contract (the shader replaces
`animation`; reduce motion skips the shader; a change that starts while one runs ends that one first). The
maintainer then pushed its own builder to #763 (`2c5d96585`) and said the rest comes as follow-up PRs after the series
lands; the user chose that. Nothing was pushed to the `pr/shaders/*` branches. Replied on #763
(`issuecomment-6582820956`). The fixes are on fork branches, listed with their commits in the shared note
`side-session-notes/local_3fb0899a-…/shaders-followups.md`; open them as follow-up PRs, core first, once the series is
merged.
