# ios: Keep the Catalyst clippy run clean over AppSource's variant sizes

Branch: `feat/ios-app-source-lint` on `github.com/dXnSix/nsh`, cut from the fork's `main` `7d4d7fc` (upstream
`a8ff77c` plus the fork's merged branches), merged into the fork's `main` as fork PR #9 (`83ad831`). Its commit
`fed40c67` named #169 as the cause; fork PR #10 (`feat/ios-app-source-lint-message`, merged as `2a068a2`) reverted it
and applied the same change again as `8941b5a`, whose message names #145. Not opened upstream; the owner decides and
posts there (upstream squash merges; the change applies to upstream `main` `a8ff77c` as is, as the single commit
`8941b5a`).

## Summary

The Catalyst clippy run that `AGENTS.md` lists for platform code,

```sh
cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features nsh-platform-ios/embed,nsh-platform-ios/dev
```

fails with `-D warnings` on upstream `main`: `clippy::large_enum_variant` on `nsh_platform_ios::AppSource`
(`crates/nsh-platform-ios/src/app.rs`), whose `Bundle` variant is much larger than `Root` since #145 grew a bundle's
policy with the process and file grants. #145 added the same `allow` to the other backends' `AppSource` (macOS, Linux,
Windows, GPUI) and to `nsh-engine`, but not to iOS; no CI workflow runs this command, so it went unnoticed.

An `AppSource` is built once per launch and handed to `run_app`, so its size costs nothing. Boxing the large variant
would change the public enum (every `AppSource::Bundle(bundle)` caller). The enum allows the lint with its reason, as the
macOS, Linux and Windows `AppSource` already do.

## Public API

None.

## Breaking Changes

None.

## Validation

- The Catalyst clippy run above with `-D warnings`: clean (it failed on `main` before).
- `cargo clippy -p nsh-platform-ios --target aarch64-apple-ios-macabi -- -D warnings` (without `dev`): clean.
- `cargo +nightly fmt --all --check`: clean.
