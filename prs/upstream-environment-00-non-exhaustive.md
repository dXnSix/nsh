# Upstream PR: core: Mark the environments non-exhaustive, so new fields break no code

Opened 2026-10-31 as [longbridge/nsh#744](https://github.com/longbridge/nsh/pull/744), at the owner's
decision; the body below the line is the one posted.

- Branch: `pr/environment/00-non-exhaustive` (`f7fb6d42b`), one commit on `upstream/main` `201d780a`.
- Size: 25 files, +300 / −359 (code +140 / −144, tests +160 / −215): two attributes and their doc lines in
  `nsh-core`, the rest the struct literals outside it rewritten (the engine's `components.rs` tests alone are +90 /
  −136).
- Title (upstream squash-merges): `core: Mark the environments non-exhaustive, so new fields break no code`.
- Prepared 2026-10-30 at the owner's decision: the environment series (PRs 1 to 4) adds a field to both environments
  each, and this makes those additions break nothing; the series stacks on it.
- How it was made: a script rewrote every `Environment { … }` literal outside `nsh-core` into `default()` plus
  assignments (a `let` becomes the variable, an argument a `let` before the call, a field of another literal a block);
  then by hand, a test that changed one environment step by step mutates it in place (clippy's redundant
  redefinition), and the web page's fallback is `unwrap_or_else`.

---

**Depends on:** no other PR; it applies to `main` as it is. The environment series (the app state, the time zone, the
preferred languages, the regional preferences) stacks on it: each of those adds a field to both environments.

## Description

`platform::Environment` (what a platform reports) and `element::Environment` (what components read) are plain public
structs, so every field they gain breaks each struct literal outside `nsh-core` that lists the fields in full. Two do
today: iOS's root view writes its environment out in full, and so does the engine's `ui_environment`; an embedding app
with a backend of its own may too. Each field the platforms add (the app's state, the time zone, the user's languages
and regional preferences are on their way) would break them again.

Both structs are `#[non_exhaustive]` now, as the workspace's rules ask of public structs: other crates start from
`Environment::default()` and set the fields they know, which no later field breaks. Every struct literal outside
`nsh-core` becomes that form once:

- the backends' first environments (macOS, Windows, Linux, GPUI, the web page's) and iOS's root view;
- the engine's `ui_environment` and the engine's tests;
- `nsh-testing`'s headless environment.

Inside `nsh-core` nothing changes, and no behavior does: each environment holds the same values as before.

## Public API

- `nsh-core`: `platform::Environment` and `element::Environment` are `#[non_exhaustive]` (their docs say how other
  crates build one).

## Breaking Changes

Rust code outside `nsh-core` can no longer build either environment with a struct literal, `..Default::default()`
included: it starts from the default and assigns. What reads an environment (Rust components, fields, patterns with
`..`) is unchanged, and TSX apps are not concerned; the code concerned is what builds one, a backend or an embedding
app with its own.

```diff
- let environment = Environment {
-     os: Os::MacOs,
-     font_scale: 1.25,
-     ..Default::default()
- };
+ let mut environment = Environment::default();
+ environment.os = Os::MacOs;
+ environment.font_scale = 1.25;
```

## How to Test

- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test -p nsh-core -p nsh-js -p nsh-testing -p
  nsh-engine -p nsh-macros -p nsh-bundler --features nsh-bundler/bundle`: the engine's tests build their environments
  the new way and pass as before.
- The backends: Catalyst clippy (`embed,dev`: iOS's root view), macOS clippy (`dev`) and `cargo test -p
  nsh-platform-macos`, `cargo xtask clippy-android`, wasm32 clippy (`compiler`), `(cd crates/gpui && cargo clippy
  --all-targets --features nsh-platform-gpui/dev && cargo test)`; Windows and Linux on CI.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst and Android runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use: none (Rust code that builds an environment, see Breaking Changes).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: no behavior changes.
