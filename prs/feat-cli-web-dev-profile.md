# cli: Build an app's development web runtime incrementally

> **Upstream (checked 2026-11-01):** opened and merged as longbridge/nsh#429 (2026-10-27).

Branch: `feat/cli-web-dev-profile` on `github.com/dXnSix/nsh`. It follows upstream by merging `upstream/main` in, never by
rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened as a pull request into the fork's `main`, Gamify's
integration line, and merged there with a merge commit. Not opened upstream; the owner
decides and posts there (upstream squash merges).

## Summary

`nsh run --target web` builds an app's own web runtime (an app with `nsh.json` `"rust"`) again on every change to
the app's Rust code. It used the release `web` profile (`opt-level = "z"`, fat LTO, one codegen unit), so every edit
re-optimized the whole module.

The generated workspace now has a `web-dev` profile, used for `WebKind::Dev`:

```toml
[profile.web-dev]
inherits = "dev"
opt-level = 1
debug = false
debug-assertions = false
overflow-checks = false
panic = "abort"
[profile.web-dev.package."*"]
opt-level = "s"
[profile.web-dev.package.rquickjs-sys]
opt-level = 3
[profile.web-dev.package."<each app crate>"]
opt-level = 1
```

It panics as `web` does and has no debug assertions, so the page behaves as a published one. The generated manifest
also gives `rquickjs-sys` and `taffy` `opt-level = 3` in `dev`, as nsh's own root manifest does, for the debug hosts.
Release runtimes (`nsh package --target web`) keep the `web` profile byte for byte.

## Measurements

`examples/rust-exports` (TSX entry, one app crate), wasm32 runtime built with `cargo build -p <app>-web --profile …`
after a full build, then one edit of the app crate (a new constant) three times; 2026-10-19, Apple Silicon Mac,
Rust 1.99.0.

| Profile | Full build | One edit (3 runs) | Module size |
|---|---:|---:|---:|
| `web` (before) | 34.1 s | 7.96 / 7.69 / 7.81 s | 3,725,569 B |
| `web-dev` (after) | 35.7 s | 0.41 / 0.29 / 0.31 s | 6,488,631 B |

## Public API

None (generated build configuration of apps with Rust crates).

## Breaking Changes

None.

## Docs

- `docs/dev/web.md` ("Apps with Rust components"), `docs/dev/tooling.md`.
- `website/docs/web.md` and `website/zh-CN/docs/web.md`: the development runtime builds incrementally.

## Validation

- `cargo test -p nsh-cli web_dev_profile`: 2 new tests (the development profile's settings, per-app-crate
  overrides and the dev overrides; the release `web` profile unchanged).
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-cli --all-targets`: clean.
- The measurement above with the CLI built from the branch.
- After the merge of `upstream/main` `a8ff77c`, whose QuickJS crate is `quickjs-jit-sys`, the generated `web-dev` and
  `dev` profiles raise that crate instead of `rquickjs-sys` (`4c48d8e`); `cargo test -p nsh-cli rust_app`: 19 tests
  pass.
