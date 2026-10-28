# Upstream PR: android: Send test touches to the shown dialog's window

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#675](https://github.com/longbridge/nsh/pull/675)
(2026-10-28) with the body below the line.

- Branch: `pr/android/02-test-dialog-touches`, one commit on `upstream/main` `ba289971`.
- Source: the fork's `88cbf9491` on `sync/upstream-478737b1`, found while running the shader gallery test on Android.
  Independent of the shader series; the Android shader PR's gallery test needs it for its dialog half.

- Checked locally on `ba289971`: `javac` of `crates/nsh-platform-android/java` against `android-36`'s `android.jar`.
  `cargo xtask clippy-android` fails on `upstream/main` itself at `ba289971`: #674 added `Command::Tokens` and
  `nsh-platform-android/src/encode.rs` does not match it (E0004). The Rust this PR builds is upstream's; on `7dba85d4`
  the same commit passed clippy-android. The fix was found and checked on the `nsh-multires-35` emulator with the
  fork's sync branch (`shaders.test.tsx` 3 of 3; its dialog half fails without it).
- Open it from `dXnSix:pr/android/02-test-dialog-touches` into `longbridge/nsh:main`, title (upstream squash-merges): `android: Send test touches to the shown dialog's window`.

The body to paste starts below the line.

---

## Description

A `Dialog` on Android is a window of its own (`ModalHost`), but the test agent (`TestAgent.target`) sent every touch to
the activity's window unless a *system* dialog showed. So `nsh test --target android` pressing a button inside an NSH
dialog tapped the page under it: the dialog's handler never ran, and a test that closes a dialog by its button failed
on Android only.

Touches now go to the topmost shown `Dialog`'s window when there is one (`Mounter.topModalWindow()`, from
`ModalHost.window()`), else to the activity's window as before. Apps are not affected: only the test agent changes.

## How to Test

- `javac` of `crates/nsh-platform-android/java` against `android-36`'s `android.jar`; `cargo xtask clippy-android`.
- On an emulator (API 35): a test that opens a `Dialog` and presses a button in it, e.g. the gallery's dialog tests,
  `nsh test examples/gallery/tests/<file> --target android`. Before: the press lands on the page under the dialog;
  after: on the dialog's button.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `javac` against `android-36`. `cargo xtask clippy-android` on today's `main` stops at `encode.rs`, whose
  `Command` match lacks `Tokens` (from #674), outside this PR; on `7dba85d4` it passed with this change.
- [x] Changes to what apps use: none (the test agent only).
- [x] Ran a gallery test that closes a `Dialog` by its button on an API 35 emulator: it passed 3 of 3 with this change;
  without it the press landed on the page under the dialog.
