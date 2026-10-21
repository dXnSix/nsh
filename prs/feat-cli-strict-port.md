# cli: Fail instead of moving to another port with --strict-port

Branch: `feat/cli-strict-port` on `github.com/dXnSix/nsh`. It follows upstream by merging `upstream/main` in, never by
rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened as a pull request into the fork's `main`, Gamify's
integration line, and merged there with a merge commit. Not opened upstream; the owner
decides and posts there (upstream squash merges).

## Summary

`nsh run` and `nsh dev` try the next nine ports when the one asked for is taken. A process manager that starts `nsh`
on a port it chose (.NET Aspire, a container, a reverse proxy) then health-checks and links the wrong port while the
dev server listens elsewhere, and nothing says so.

`--strict-port` (or `NSH_STRICT_PORT=1`) makes the command stop with an error naming the port:

```text
error: start dev server: port 8790 is taken (--strict-port): stop what uses it or choose another --port
```

The server keeps its port for the whole session: a change to an app's Rust code rebuilds and restarts the session
in the same process, and the strict port holds across that restart (proved with Gamify's hot-reload spec). Without the flag nothing changes.

## Public API

### `nsh` CLI

- `nsh run --strict-port`, `nsh dev --strict-port`: use exactly `--port`; fail when it is taken. Environment:
  `NSH_STRICT_PORT` (`1`/`true`).

### `nsh-cli` (crate-internal)

- `dev::Port { number: u16, strict: bool }` replaces the bare port in `DevSession::start` and `run::run`.

## Breaking Changes

None (additive flag; default behaviour unchanged).

## Docs

- `website/docs/cli.md` and `website/zh-CN/docs/cli.md`: the flag in `nsh run` and `nsh dev`.
- `docs/dev/tooling.md`: the dev server's port.

## Validation

- `cargo test -p nsh-cli strict_port`: 4 new tests (a taken port fails naming it, and only an address-in-use error is
  reported as taken; without the flag the next free port
  is used; a free port is used as is; the flag and its environment variable on `run` and `dev`).
- `cargo +nightly fmt --all`, `cargo clippy -p nsh-cli -p nsh-dev --all-targets -- -D warnings`: clean.
- After the merge of `upstream/main` `a8ff77c`, which added `--limits` to `nsh run` and `nsh dev`, both flags are
  taken together (`DevSession::start(entry, port, release, limits)` with `Port`), and the CLI pages list both;
  `cargo test -p nsh-cli -p nsh-dev` passes.
