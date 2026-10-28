# Upstream PR: android: Build again: pass over the design tokens command

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#681](https://github.com/longbridge/nsh/pull/681) (2026-10-28) with the body below the line.

- Branch: `pr/android/03-command-tokens`, one commit on `upstream/main` `ba289971`.
- Checked locally on `ba289971`: `cargo xtask clippy-android` (both feature sets) fails without it (E0004) and passes
  with it; nightly rustfmt on `encode.rs`.
- Open it from `dXnSix:pr/android/03-command-tokens` into `longbridge/nsh:main`, title (upstream squash-merges):
  `android: Build again: pass over the design tokens command`.

The body to paste starts below the line.

---

## Description

#674 added `Command::Tokens`, which the core sends on the root when the app's design tokens change, for the platforms'
own menus to take their colors from (Linux colors its GTK menus with it). Android's encoder (`encode.rs`) matches
every `Command` without a catch-all, so `nsh-platform-android` stopped compiling on `main`:

```
error[E0004]: non-exhaustive patterns: `nsh_core::Command::Tokens(_)` not covered
   --> crates/nsh-platform-android/src/encode.rs:520:23
```

Android has no menus of its own to color, so the command joins the others Android passes over: it is encoded as `-1`,
which `Mounter.command` ignores (`default: break`). Nothing else changes.

## How to Test

- `cargo xtask clippy-android` (with `ANDROID_NDK_HOME` set): on `main` it fails with the error above; with this
  change both feature sets pass `-D warnings`.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed file and clippy (both Android runs).
- [x] Changes to what apps use: none.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: a build fix; no behavior changes.
