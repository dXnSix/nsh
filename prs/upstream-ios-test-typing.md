# Upstream PR: ios: Type each key in tests once the one before it landed

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/ios/01-test-typing`, one commit on `upstream/main` `ba289971`. Independent of the
  other two iOS test fixes (`pr/ios/01-test-typing`, `02-test-caret-settle`, `03-test-scroll`): each applies on its own
  and they merge in any order.
- Source: the fork's `15b818dc4` on `sync/upstream-478737b1` (fork PR #91), found while running the gallery's tests on the
  iOS simulator and the fork's iOS CI runner.
- Checked locally on `ba289971`: the Catalyst clippy run (`nsh-platform-ios/embed`, `dev`, `-D warnings`) and nightly
  rustfmt on the changed files. The simulator runs are the fork's, on the sync branch (below).
- Open it from `dXnSix:pr/ios/01-test-typing` into `longbridge/nsh:main`, title (upstream squash-merges): `ios: Type each key in tests once the one before it landed`.

The body to paste starts below the line.

---

## Description

`nsh test --target ios` typed a key every 80 ms. On a slow simulator the keyboard drops a key typed while it still
handles the previous one, the second of two equal keys most often: `"hello"` typed into the gallery form's uppercasing
input became `"HELO"`, then `"HEO"`. Each key now waits until the focused input's text changed before the next is
typed (`synth::focused_text`: the first responder's `text`), at most 600 ms (`KEY_LANDS`), so an input that refuses a
key (`maxLength`) only delays the next one.

Apps are not affected: only `nsh test`'s iOS agent changes.

## How to Test

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`
- On a simulator: `nsh test examples/gallery/tests/form.test.tsx --target ios`. Before, its typing steps lost repeated
  letters on a loaded machine; after, the typed texts arrive whole.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed files and clippy (Catalyst run).
- [x] Changes to what apps use: none (the test agent only).
- [x] Ran the related `examples/gallery` tests on the iOS simulator (with the fork's sync branch).
