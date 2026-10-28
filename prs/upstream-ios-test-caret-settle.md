# Upstream PR: ios: Let test steps settle while a focused input's caret blinks

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/ios/02-test-caret-settle`, one commit on `upstream/main` `ba289971`. Independent of the
  other two iOS test fixes (`pr/ios/01-test-typing`, `02-test-caret-settle`, `03-test-scroll`): each applies on its own
  and they merge in any order.
- Source: the fork's `c60c01322` on `sync/upstream-478737b1` (fork PR #91), found while running the gallery's tests on the
  iOS simulator and the fork's iOS CI runner.
- Checked locally on `ba289971`: the Catalyst clippy run (`nsh-platform-ios/embed`, `dev`, `-D warnings`) and nightly
  rustfmt on the changed files. The simulator runs are the fork's, on the sync branch (below).
- Open it from `dXnSix:pr/ios/02-test-caret-settle` into `longbridge/nsh:main`, title (upstream squash-merges): `ios: Let test steps settle while a focused input's caret blinks`.

The body to paste starts below the line.

---

## Description

A test step waits until the app is quiet (no animation that ends by itself running), 2 s at most. The walk of the
layers took an animation for one that ends when its repeat count was finite, but UIKit's loops repeat `FLT_MAX` times,
Core Animation's way to say forever: a focused input's caret blinks so, and every step taken while an input had the
focus waited the full 2 s. In the gallery's tests on the simulator 80 steps did, and those with inputs ran out of their
30 s on CI runners (the sign-in code, the PIN, the cart's quantities, the list search).

A repeat count of `FLT_MAX`, an infinite duration or an infinite repeat duration now counts as a loop, which no step
waits for.

Apps are not affected: only `nsh test`'s iOS agent changes.

## How to Test

- `cargo clippy` as above (Catalyst).
- On a simulator: the iOS list's gallery tests took 570 s instead of 712 s, the list search's 6 s instead of 24 s
  (`nsh test examples/gallery/tests/<file> --target ios`).

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and clippy (Catalyst run).
- [x] Changes to what apps use: none (the test agent only).
- [x] Ran the related `examples/gallery` tests on the iOS simulator (with the fork's sync branch).
