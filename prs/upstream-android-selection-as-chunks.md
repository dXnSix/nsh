# Upstream PR: android: Read a selection's rectangles six numbers at a time, as clippy asks

Draft of a pull request for `longbridge/nsh` (not opened: the owner decides and posts), with the body below the line.

- Branch: `pr/android/04-selection-as-chunks`, one commit on `upstream/main` `8d790445`: the fork's `56b833b3f` from
  its sync (#91), where `fork-0.3.0-2` carries it.
- Checked locally on `8d790445` (2026-10-30, Rust 1.99.1): `cargo xtask clippy-android` (both feature sets) fails on
  `main` with the lint below and passes with this commit; nightly rustfmt on `selectable_view.rs`. No open upstream
  PR touches the file (#714 to #718 and #721 are Android PRs of other files).
- Open it from `dXnSix:pr/android/04-selection-as-chunks` into `longbridge/nsh:main`, title (upstream
  squash-merges): `android: Read a selection's rectangles six numbers at a time, as clippy asks`.
- It goes first among the Android PRs prepared on the fork: every Android branch cut from `main` (the shader
  series' `05-android`, the environment series' `03-android` branches) passes `clippy-android` only with it.

The body to paste starts below the line.

---

**Depends on:** no other PR; it applies to `main` as it is. The Android PRs prepared on the fork pass
`cargo xtask clippy-android` only once this is merged.

## Description

`cargo xtask clippy-android` fails on `main` with current stable Rust (1.99): clippy's `chunks_exact_to_as_chunks`
flags the selectable view's reading of the rectangles Java sends, which #703 added, and the run treats warnings as
errors:

```
error: using `chunks_exact` with a constant chunk size
   --> crates/nsh-platform-android/src/selectable_view.rs:352:14
    |
352 |             .chunks_exact(6)
    |              ^^^^^^^^^^^^^^^ help: consider using `as_chunks` instead: `as_chunks::<6>().0.iter()`
```

This takes clippy's suggestion: `as_chunks::<6>()` splits the same numbers into the same rectangles (each a
`&[f32; 6]`, so indexing it stays checked at compile time), and the remainder it drops is what `chunks_exact` dropped.
Nothing else changes.

## How to Test

- `cargo xtask clippy-android` (with `ANDROID_NDK_HOME` set): on `main` it fails with the error above; with this
  change both feature sets pass `-D warnings`.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` on the changed file and clippy (both Android runs).
- [x] Changes to what apps use: none.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: a lint fix with the same behavior; the fork's
  Android emulator job ran the gallery's tests with it (#91).
