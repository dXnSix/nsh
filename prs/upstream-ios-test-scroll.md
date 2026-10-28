# Upstream PR: ios: Scroll in tests with a quicker finger

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/ios/03-test-scroll`, one commit on `upstream/main` `ba289971`. Independent of the
  other two iOS test fixes (`pr/ios/01-test-typing`, `02-test-caret-settle`, `03-test-scroll`): each applies on its own
  and they merge in any order.
- Source: the fork's `b1b1eb1d3` on `sync/upstream-478737b1` (fork PR #91), found while running the gallery's tests on the
  iOS simulator and the fork's iOS CI runner.
- Checked locally on `ba289971`: the Catalyst clippy run (`nsh-platform-ios/embed`, `dev`, `-D warnings`) and nightly
  rustfmt on the changed files. The simulator runs are the fork's, on the sync branch (below).
- Open it from `dXnSix:pr/ios/03-test-scroll` into `longbridge/nsh:main`, title (upstream squash-merges): `ios: Scroll in tests with a quicker finger`.

The body to paste starts below the line.

---

## Description

A test's scroll was a finger moving 20 pt a frame, eased out, then resting 120 ms: 28 trips to the main thread for a
third of the screen, about 550 ms a gesture, and pressing a row far down the gallery's index took six of them. On CI
runners, where the main thread answers slowly, the scrolling alone was a large part of the tests that ran out of their
30 s (the animations', the timers').

The finger now moves 40 pt a frame on average, a quick swipe's pace, still resting before it lifts so the scroll view
keeps no velocity: a gesture takes about 340 ms, and the same rows come into view with the same number of gestures.

Apps are not affected: only `nsh test`'s iOS agent changes.

## How to Test

- `cargo clippy` as above (Catalyst).
- On a simulator: any gallery test that presses a row far down the index
  (`nsh test examples/gallery/tests/<file> --target ios`): the same rows come into view in the same number of
  gestures, each about 340 ms instead of 550 ms.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and clippy (Catalyst run).
- [x] Changes to what apps use: none (the test agent only).
- [x] Ran the related `examples/gallery` tests on the iOS simulator (with the fork's sync branch).
