# Upstream PR: updater: Pass clippy on iOS, Android and the web

Opened as longbridge/nsh#1002 (2026-11-05, head `a7e0d412`; fork CI dXnSix/nsh#135; clippy for the web, Catalyst and Android passes on its merge with `aafa450c`, fails on `aafa450c` alone). The body is below the line.

- Branch: `fix/updater-lints`, one commit (`a7e0d412`) on `upstream/main` `90a7efd3` (#951).
- Found by the fork's CI: Fork CI's iOS, Android and web jobs failed clippy on every pull request based on `90a7efd3`.
  The fork's upstream PRs 1–6 (`upstream-prs.md`) carry the same two allowances in their fork-only CI commit until
  this merges.
- Checked locally: nightly rustfmt; clippy with `-D warnings` for the web backend (`--target wasm32-unknown-unknown
  --features compiler`), Mac Catalyst (`nsh-platform-ios` with `embed,dev`), `cargo xtask clippy-android`, macOS
  (`nsh-platform-macos` and its hosts) and `nsh-core` / `nsh-engine` on the host.
- Title: `updater: Pass clippy on iOS, Android and the web`.
- Fork PR dXnSix/nsh#135 (head `tmp/updater-lints-fork-ci`: the branch plus Fork CI's files and the Compat fix, no lint
  allowances of its own).

---

## Description

Since #922, clippy with `-D warnings` fails for the iOS, Android and web builds, which the repository's workflows do
not lint:

- `crates/nsh-core/src/runtime/updater.rs`: `check`'s `let mut update` is only mutated where an update can be
  installed (Linux, macOS, Windows), so iOS and Android report `unused_mut`.
- `crates/nsh-engine/src/script.rs`: `SystemConfig::updater` is only read by QuickJS's host (`set_updater`); the
  browser runtime's `Script` hands the other fields to its `System` and has no updater (nothing updates on the web,
  `checkForUpdate` resolves `null` there), so the wasm32 build reports `dead_code`.

Both are allowed where they apply (`cfg_attr`), with a comment on why. Nothing changes in what the code does.

## How to Test

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features
  nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`: fails on `main` (`unused_mut`), passes with this change.
- `cargo xtask clippy-android`: the same.
- `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown --features compiler -- -D warnings`: fails on
  `main` (`dead_code`), passes with this change.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` and clippy for the crates it touches, on the Catalyst, Android, web and macOS
  targets.
- [x] Changes to what apps use: none.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: a lint fix, no change in behavior.
