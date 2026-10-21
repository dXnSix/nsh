# The NSH fork

> This lives on the fork's orphan branch `private/fork-docs` (no shared history with upstream; a `pre-push` hook
> refuses to send `private/*` to `upstream`). `scripts/nsh-fork/` below is in the Gamify repository.

Gamify takes NSH from the fork `github.com/dXnSix/nsh` (`upstream` is `longbridge/nsh`), only through git tags. The
fork's `main` is Gamify's integration line: upstream's `main` plus one pull request per topic branch (`feat/*` for a
feature, `fix/*` for a fix), each merged with a merge commit. Nothing is pushed to upstream or opened there: the drafts
in [prs/](prs/) and [issues/](issues/) are for the owner to post.

## Branches

Each topic is a branch in its own worktree under `../nsh-wt/<topic>`: `feat/<topic>` for a feature, `fix/<topic>` for a
fix of NSH's behaviour as it is. Both follow NSH's `AGENTS.md` and `CONTRIBUTING.md` (one topic, `<area>: <Sentence>`
subjects, docs in English and zh-CN, `cargo +nightly fmt` and clippy clean, no Gamify or AgentX files). A branch never
rebases: AgentX records name its commits.

## Bringing in upstream

```bash
scripts/nsh-fork/sync-main.sh branch ../nsh feat/<topic> --push   # or fix/<topic>
scripts/nsh-fork/sync-main.sh upstream ../nsh --pr
```

`branch` merges the newest `upstream/main` into a topic branch. `upstream` cuts `sync/upstream-<sha>` from the fork's
`main`, merges `upstream/main` into it and opens its pull request. Conflict resolutions are replayed from
`scripts/nsh-fork/rr-cache/` (git rerere); after resolving a new one, commit the merge and run
`sync-main.sh save-resolutions ../nsh`.

## A branch into main

1. Open its pull request with its draft as the body: `gh pr create --repo dXnSix/nsh --base main --head <feat|fix>/<topic>`.
2. Wait for the fork's GitHub Actions. When the pull request conflicts with `main` (a branch merged before it touched
   the same files), merge `origin/main` into the branch and rerun its focused tests.
3. Merge with a merge commit, never squash: `gh pr merge <n> --repo dXnSix/nsh --merge`.

## Tags

```bash
scripts/nsh-fork/sync-main.sh tag ../nsh fork-<nsh version>-<n>
```

Tags are `fork-<nsh version>-<n>` on the fork's `main`, never `v*` (the fork's copies of upstream's release
workflows run on those) and never named after a consumer; Cargo versions stay upstream's. The older `gamify-*` and
`six-gamify-*` tags were deleted on 2026-10-21, so Gamify commits that pin them no longer build from a fresh clone.
After tagging, move every pin in Gamify (`cargo xtask nsh verify`
names them) and reinstall the CLI:

```bash
cargo install --git https://github.com/dXnSix/nsh --tag fork-<nsh version>-<n> --locked nsh-cli
```
