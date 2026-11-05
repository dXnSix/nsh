# Upstream PR: android: Give the file picker its own command code

Not sent: upstream's longbridge/nsh#994 (`e1af8c6c`, 2026-11-05) made the same change (`PickFiles` 45, notifications 44); dXnSix/nsh#136 closed. The draft body is below the line.

- Branch: `fix/android-pick-files-code`, one commit (`e1933795`) on `upstream/main` `88987268` (#946).
- Found by the fork's CI: Fork CI's Android job failed javac on every pull request based on `88987268`
  (`Mounter.java:2175: error: duplicate case label`).
- Checked locally: `cargo xtask android --dev` (libnsh.so and the Java runtime, javac included) fails on `88987268`
  and passes with the change; `cargo xtask clippy-android` with `fix/updater-lints` applied (on its own it stops at
  #922's lint first).
- Fork PR dXnSix/nsh#136 (head `tmp/android-pick-files-code-fork-ci`: the branch plus Fork CI's files and #922's lint
  allowances).
- Title: `android: Give the file picker its own command code`.

---

## Description

Since #946, the Android runtime does not build: notifications (#931) and the file picker (#946) both took command
code 44, so `Mounter.java`'s command switch has two `case 44:` labels and javac refuses it
(`Mounter.java:2175: error: duplicate case label`). `encode.rs` likewise sends 44 for both `Command::Notification`
and `Command::PickFiles`.

The file picker, which merged second, takes 45, the next code free across the encoder: `encode.rs` sends it for
`Command::PickFiles` and the Mounter reads it there. Notifications keep 44.

## How to Test

- `cargo xtask android --dev`: fails on `main` (javac's duplicate case label), passes with this change.
- On a device or emulator: `Picker.pickFiles` opens the system's document picker, and a notification still shows.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Built the Android runtime (`cargo xtask android --dev`) and ran `cargo xtask clippy-android`.
- [x] Changes to what apps use: none; the encoder and the Mounter change together, so no app sees the code.
- [ ] Ran the related `examples/gallery` demos on Android: the fork's CI runs the gallery's tests on an emulator.
