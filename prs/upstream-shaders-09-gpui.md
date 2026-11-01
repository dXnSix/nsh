# Upstream PR: gpui: Draw only shader fills: effects show their children, transitions change at once

Opened 2026-11-01 as [longbridge/nsh#770](https://github.com/longbridge/nsh/pull/770), at the owner's decision (all nine together, the platform PRs and the gallery's marked as depending on #763); the body
below the line is the draft it was posted from, with the PR numbers filled in.

- Branch: `pr/shaders/09-gpui`, on `pr/shaders/02-page-dialog-shaders`. One commit (`mount.rs`, `docs/dev/gpui.md`, `docs/dev/shaders.md`).
- Source: the fork's `cd4e3807f` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/09-gpui`).

- Checked locally on `ba289971`: the GPUI workspace's clippy (`nsh-platform-gpui/dev`, all targets) and `cargo test` (207 + 2 pass), nightly rustfmt. Both runs rewrite `crates/gpui/Cargo.lock` (`nsh-base` gained `chrono` upstream without the GPUI lock): upstream's, not this PR's; the lock is left as `main` has it.

- 2026-10-29: rebased on the new 02 (`fa00d7818`), no conflicts; nightly rustfmt locally. The fork's GPUI workflow
  decides.
- 2026-10-31: rebased on the new 02 (`4c0b595de`), no conflicts; it differs from the previous head only by 01's
  padding commit and 02's docs line (previous head kept locally as `old-pre-pad/09-gpui`).

- 2026-11-01: #682 (01) was merged (squashed, `8274f6b05`), so this branch is rebuilt on `upstream/main`
  `13d87a11` (`git cherry-pick` of its own commits, no conflicts; old head `4c0b595de`, new head `85942ff15`).

- 2026-11-01, later: rebuilt again on `upstream/main` `e4e464f4` (23 commits past `a5036af4`), no conflicts of its own; head `6d4278e5c`.

The body to paste starts below the line.

---

**Depends on:** #682 and #[02 PR] (the layer roles and `transitionShader` it leaves out); merge it after both.
Independent of the other platform PRs.

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs). GPUI keeps drawing `Shader` fills only: the layer of an effect, a transition or a
screen layer (#682) is not drawn as a fill under the children (`ShaderRole::Fill` only). An effect's or a
transition's children show as they are and a transition changes them at once, a screen effect or transition draws
nothing, and pages and dialogs with `transitionShader` (#[02 PR]) come and go as they would without it.

## Screenshot

No visible change for apps without the new elements.

## How to Test

- `(cd crates/gpui && cargo clippy --all-targets --features nsh-platform-gpui/dev -- -D warnings)`
- `(cd crates/gpui && cargo test)`

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and the GPUI workspace's clippy (clean).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`): no new API here; the elements' cases came with #682 and #[02 PR].
- [ ] No gallery change on GPUI.
