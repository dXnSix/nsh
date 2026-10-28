# Upstream PR: gpui: Draw only shader fills: effects show their children, transitions change at once

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/09-gpui`, on `pr/shaders/02-page-dialog-shaders`. One commit (`mount.rs`, `docs/dev/gpui.md`, `docs/dev/shaders.md`).
- Source: the fork's `cd4e3807f` on `sync/upstream-478737b1`.
- Open it after the PRs it sits on are merged; rebase onto `upstream/main` then (upstream squash-merges:
  `git rebase --onto upstream/main pr/shaders/02-page-dialog-shaders pr/shaders/09-gpui`).

- Checked locally on `ba289971`: the GPUI workspace's clippy (`nsh-platform-gpui/dev`, all targets) and `cargo test` (207 + 2 pass), nightly rustfmt. Both runs rewrite `crates/gpui/Cargo.lock` (`nsh-base` gained `chrono` upstream without the GPUI lock): upstream's, not this PR's; the lock is left as `main` has it.

The body to paste starts below the line.

---

## Description

Part of the shader series (01 elements, 02 page and dialog shaders, then one PR per platform, 10 gallery and docs). GPUI keeps drawing `Shader` fills only: the layer of an effect, a transition or a
screen layer (#[01 PR]) is not drawn as a fill under the children (`ShaderRole::Fill` only). An effect's or a
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
  (`cargo xtask compat`): no new API here; the elements' cases came with #[01 PR] and #[02 PR].
- [ ] No gallery change on GPUI.
