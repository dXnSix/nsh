# Upstream PR: macos: Draw shader effects, transitions and page shaders

Opened 2026-11-01 as [longbridge/nsh#764](https://github.com/longbridge/nsh/pull/764), at the owner's decision (all nine together, the platform PRs and the gallery's marked as depending on #763); the body
below the line is the draft it was posted from, with the PR numbers filled in.

- Branch: `pr/shaders/03-macos`, on `pr/shaders/02-page-dialog-shaders`. Two commits: the backend and its `docs/dev` notes (`docs/dev/macos.md`, `docs/dev/shaders.md`). Independent of
  the iOS PR; whichever of the two is merged second resolves the shared table of `docs/dev/shaders.md` (both rows).
- Source: the fork's `fb5f2e195` and the macOS part of `1d2f5ab68` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/03-macos`).

- Checked locally on `ba289971`: the macOS clippy run (dev, all targets) and `cargo test -p nsh-platform-macos`; nightly rustfmt; with 10 merged, `nsh test examples/gallery/tests/shaders.test.tsx --target macos`: 2 of 2 pass.

- 2026-10-29: rebased on the new 02 (`56b736662`), no conflicts. Checked locally: the macOS clippy run (dev, all
  targets) and `cargo test -p nsh-platform-macos`; nightly rustfmt.
- 2026-10-31: rebased on the new 02 (`b36b53205`), no conflicts; it differs from the previous head only by 01's
  padding commit and 02's docs line (previous head kept locally as `old-pre-pad/03-macos`).

- 2026-11-01: #682 (01) was merged (squashed, `8274f6b05`), so this branch is rebuilt on `upstream/main`
  `13d87a11` (`git cherry-pick` of its own commits, no conflicts; old head `b36b53205`, new head `ee9bc0de5`).

- 2026-11-01, later: rebuilt again on `upstream/main` `e4e464f4` (23 commits past `a5036af4`), no conflicts of its own; head `7f134503e`.

## Review (2026-11-02)

huacnlee, two findings, both held: a `ScreenEffect` / `ScreenTransition` left its layer and the root's mask when its
view was deleted, and `set` masked children without a renderer. Fix `e3d3b8c01` (fork `31b070bff`): `deleted` takes
the screen layer off the root and `sync_mask` unmasks it unless another screen shader hides it, `clear` does the same
when the app stops; `ShaderState::hides` needs a renderer, so without a GPU device nothing is masked, transitions start
no run and pages / dialogs keep their own `animation` (`Nav::shader`, `Nav::start`). Found on the way: the decoration's
clipping reset the same container mask on every reshape; the mask now goes through `decor::hide_children`. Fork CI
macos green (#107), Rust application green on `tmp/shaders-03-macos-review`, macOS clippy (dev) and the backend's tests
locally. Pushed after the 02 merge (`9ded3bc03`); replied `issuecomment-6572825379`.

Later on 2026-11-02: the upstream web check failed on main's own menu bar hover test (#791, fixed by #795 three
minutes after the run started) and compat hit nsh-compat's 5 s first-screen flake. Merged `upstream/main` `ca3dbd60`
(no conflicts; #783's border lines in `decor.rs` sit apart from `hide_children`): fork Rust application, Compat and CI
paths green on `tmp/shaders-03-macos-review`, macOS clippy (dev) and its tests locally. Pushed, head `45170546f`.

The body to paste starts below the line.

---

**Depends on:** #682 and #[02 PR] (the elements and pages' and dialogs' `transitionShader` it draws); merge it after
both. Independent of the other platform PRs (the iOS PR shares a table of `docs/dev/shaders.md` with it).

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs): macOS draws `ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition`
and pages' and dialogs' `transitionShader` (#682, #[02 PR]) on `Shader`'s renderer.

- **Pictures.** `renderInContext:` of the children's container (`NshView::ensure_container`) without its mask, at the
  window's backing scale; a screen's is all the root shows but the frames over it; the backdrop is the root with the
  view's layer hidden. They are taken on the display frame after the core marks them (`MountItem::ShaderInput`) or a
  view in them changed (`NshView::changed`, `moved`, `redraw_rect`), at most once per frame.
- **Effects and transitions.** An empty `mask` hides the children's container while the frames show over it (AppKit
  still hit-tests the views), in a sublayer at `zPosition` 1e6 that reaches `overflow` points beyond the box; a
  transition pictures its children (or its running frame) as `from` when it starts and keeps taking `to`. Screen
  layers are the root's.
- **Pages and dialogs.** The motion is drawn through a layer over the stack (the root for a dialog) from pictures of
  it without and with the upper side (a page or dialog that left: its last image), the views at rest under it until it
  ends (`navigation::Motion::Shader`); popping and dismissing run backwards.

`renderInContext:` draws no video or web page, nor backdrop blur or glass: those show transparent in a picture.

## Screenshot

| Platform | Before | After |
| -------- | ------ | ----- |
| macOS | [the gallery's Shaders page: the effect and transition cards show their children plainly] | [the same page: pixelated and sharpened cards, a wipe between seasons, the iris page] |

(Screenshots to take from `examples/gallery` with this branch and #[10 PR]'s demo.)

## How to Test

- `cargo clippy -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app --all-targets --features nsh-platform-macos/dev -- -D warnings`
- `cargo test -p nsh-platform-macos`
- With #[10 PR]: `nsh test examples/gallery/tests/shaders.test.tsx --target macos` (both tests: the effect and transition
  controls keep their presses under the frames; a page and a dialog come and go through their shader).

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and clippy for macOS (clean).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #682 and #[02 PR].
- [x] Ran the related `examples/gallery` demos: the Shaders page on macOS (ran: `shaders.test.tsx` on macOS, 2 of 2).

## Review 2 (2026-11-02, huacnlee's agents): follow-ups after the series lands

The maintainer's review asked for one contract across the platforms (precedence over `animation: "none"`, reduce
motion, interruption), a Rust builder and per-platform perf fixes. The user settled the contract (the shader replaces
`animation`; reduce motion skips the shader; a change that starts while one runs ends that one first). The
maintainer then pushed its own builder to #763 (`2c5d96585`) and said the rest comes as follow-up PRs after the series
lands; the user chose that. Nothing was pushed to the `pr/shaders/*` branches. Replied on #763
(`issuecomment-6582820956`). The fixes are on fork branches, listed with their commits in the shared note
`side-session-notes/local_3fb0899a-…/shaders-followups.md`; open them as follow-up PRs, core first, once the series is
merged.
