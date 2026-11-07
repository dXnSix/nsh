# Upstream PR: web: Export the Terminal part from the browser runtime

**Not to be opened: upstream fixed it itself.** huacnlee's #1152 (`5af4b5686`, `web: Register Terminal in the browser
runtimes`, merged 2026-11-07 01:04 UTC) adds `term => nsh_part_term` to `base_parts!`. It also adds a compile-time
check: `base_parts!` asserts `covers_every_part` against `nsh_base::parts::ALL` under `base-full`. That check covers
what the fork's test checks. Checked against `upstream/main` `c015f2c5e` (2026-11-07). The fork's version remains
below the line as a record.

- Branch: `fix/web-terminal-part`, one commit (`e40cb6ae1`) on `upstream/main` `7571948ee` (#1069).
- Fork PR: dXnSix/nsh#156, merged 2026-11-07 (`fd8b00af1`), in tag `fork-0.4.0-1` (`1d64f049d`).
- Found by: Fork CI's `web` job on the sync to `7571948ee` (dXnSix/nsh#152, run
  https://github.com/dXnSix/nsh/actions/runs/42311010274). It ran every gallery test in Linux's Chrome
  (`nsh test --target web`), and 110 failed with nothing on screen (`no pressable view matches role:button[Storage];
  on screen: []`). Bisected to #1111 (`c8e044b42`, `term: Add Terminal, …`) with `storage.test.tsx`: it passes on
  `897fd9c2d` and fails on `c8e044b42`. Upstream's `Rust application` / `web` job runs `storage.test.tsx` in Chrome
  too, so upstream's CI may have caught this on `main` after #1111. #1152 merged about 12 hours later.
- Checked: Fork CI's `web` job passed with no failure on the fork PR
  (https://github.com/dXnSix/nsh/actions/runs/42317497338) and on the fork's `main` at `1d64f049d`
  (https://github.com/dXnSix/nsh/actions/runs/42321466305). Locally: `cargo test -p nsh-cli --bin nsh
  every_part_of_nsh_base_has_its_gate_in_the_web_runtime` passes, and fails without the `base_parts!` line; `nsh test
  examples/gallery/tests/storage.test.tsx --target web`: 2 passed (2 failed before).
- Next sync: `crates/nsh-platform-web/src/lib.rs` merges cleanly with `5af4b5686` (both add the same line), according
  to `git merge-tree` of the fork's `main` with `c015f2c5e`. The fork's `nsh-cli` test is then redundant with
  upstream's assertion; dropping it in the sync is optional.
- Title: `web: Export the Terminal part from the browser runtime`.

---

## Description

#1111 added `nsh/base`'s `term` part (`Terminal`). The CLI gives every part of `nsh_base::parts::ALL` a gate,
`nsh_part_<name>`. The page calls the gates of the parts its app uses (`linked_gates_of`), or all of them for an app
that imports `nsh/base` as a whole. `nsh-platform-web`'s `base_parts!` had no `term`. The gallery's page therefore
called an export that the runtime does not have, and stopped before its first frame:

- every gallery test run with `nsh test --target web` found nothing on screen (`no pressable view matches
  role:button[Storage]; on screen: []`);
- in `nsh run --target web`, the Terminal demo failed with `TypeError: Component is not a function`.

`base_parts!` now has `term => nsh_part_term`, and `nsh-cli` checks that it lists every part of
`nsh_base::parts::ALL`.

## How to Test

- `cargo test -p nsh-cli --bin nsh every_part_of_nsh_base_has_its_gate_in_the_web_runtime`: passes. Without the
  `base_parts!` line it fails with "nsh-platform-web's base_parts! has no `term => nsh_part_term`".
- `nsh test examples/gallery/tests/storage.test.tsx --target web`: 2 passed (on `main`: 2 failed, nothing on screen).
- `nsh run examples/gallery --target web`: the Terminal demo opens.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo +nightly fmt` and clippy for `nsh-cli` and `nsh-platform-web`.
- [x] Changes to what apps use: `Terminal` works on the web, as #1111 meant it to.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: web only.
