# cli: Take nsh's crates from the git repository the app's crates name

> **Superseded (2026-10-20).** Upstream's #213 ("cli: Run Git-backed apps in the native desktop host") supports git
> dependencies on nsh's crates its own way, from the checkout Cargo resolves. The fork's main took upstream's
> implementation when it merged `upstream/main` `5f8daece` (fork PR #11), and the branch `feat/cli-git-source` is deleted.
> Nothing here is to be posted upstream.

Branch: `feat/cli-git-source` on `github.com/dXnSix/nsh`. It follows upstream by merging `upstream/main` in, never by
rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened as a pull request into the fork's `main`, Gamify's
integration line, and merged there with a merge commit. Not opened upstream; the owner
decides and posts there (upstream squash merges).

## Summary

An app with Rust crates of its own (`nsh.json` `"rust"`) can take nsh's crates from crates.io or from a checkout by
path. A git dependency (`nsh-core = { git = "…", tag = "…" }`: a fork, or a change that is in the repository but not
yet on crates.io) was read as crates.io: the generated host workspaces asked for `nsh-core = "=0.1.1"` from crates.io
while the app's crates took the repository's, and the build failed later on two `nsh-core`s whose component and
registry types differ.

The CLI now reads each dependency's `source` from `cargo metadata --no-deps` and classifies
`git+<url>?tag|branch|rev=<value>` as a git source (`rust_app::GitSource`):

- the generated manifests name that repository and reference for every nsh crate (`{ git = "…", tag = "…" }`);
- the workspaces' `[patch.crates-io]` send the app's crates.io dependencies on nsh to it, as for a checkout;
- the web runtime's `nsh.js` is copied from the fetched `nsh-platform-web`, as from crates.io;
- once Cargo has fetched the repository (`cargo metadata` of the generated workspace), its `nsh-core` must be the
  running `nsh`'s version, else the command says which `nsh` to install
  (`cargo install --git <url> … --locked nsh-cli`).

Git mixed with crates.io or a path, or two references of one repository, stop with both places named. crates.io and
path apps build exactly as before.

## Public API

### `nsh` CLI

- Apps whose crates depend on nsh's crates through git (`nsh-core = { git = "<url>", tag | branch | rev = "<value>" }`)
  build their hosts and runtimes from that repository and reference. Previously such apps failed to build.

No Rust or JS API changes.

## Breaking Changes

None.

## Docs

- `website/docs/rust.md` and `website/zh-CN/docs/rust.md`: new section "Where NSH's crates come from" (crates.io,
  path, git).
- `docs/dev/tooling.md`: the git source in "Apps with Rust components of their own".
- `AGENTS.md`: the `nsh-cli` layout line.

## Validation

- `cargo test -p nsh-cli git_source`: 5 new tests (parsing Cargo's sources; classification for tag, branch and rev
  without fetching; mixed sources naming both places; every generated manifest and both `[patch.crates-io]` tables
  naming the repository (`describe` and `test` included); the version check and the checkout directory from resolved
  packages, including another commit of the same repository).
- `cargo test -p nsh-cli rust_app`: all 22 `rust_app` tests pass.
- After the merge of `upstream/main` `a8ff77c` (#189 replaced the `trace` runner with `nsh test`'s, and
  `nsh-ui-mobile` with `nsh-base`), the tests and the Rust page name `nsh-base` and the `test` runner
  (`a3fdfb2`).
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-cli --all-targets`: clean.
- Not yet proved end to end: the version check and the checkout lookup are tested with `cargo metadata`-shaped
  package data, not against a fetched repository. Gamify's integration sprint (the fork's `gamify` branch and its first
  `gamify-*` tag) builds an app from a git tag through this path; this section is updated with that result.
