# The NSH fork

> This lives on the fork's orphan branch `private/fork-docs` (no shared history with upstream; a `pre-push` hook
> refuses to send `private/*` to `upstream`), with the fork's maintenance script, [scripts/sync-main.sh](scripts/sync-main.sh),
> and its recorded conflict resolutions. To run it, check this branch out next to the fork's checkout for the
> time it takes (`git -C ../nsh worktree add ../nsh-fork-docs private/fork-docs`, then
> `git -C ../nsh worktree remove ../nsh-fork-docs`); the commands below run from that checkout, with `../nsh` the
> fork's.

Gamify takes NSH from the fork `github.com/dXnSix/nsh` (`upstream` is `longbridge/nsh`), only through git tags. The
fork's `main` is Gamify's integration line: upstream's `main` plus one pull request per branch (`feat/*` for a
feature, `fix/*` for a fix, `fork/*` for a fork-only change), each merged with a merge commit. Nothing is pushed to
upstream or opened there: the drafts in [prs/](prs/) and [issues/](issues/) are for the owner to post.

## Branches

Each topic is a branch in its own worktree under `../nsh-wt/<topic>`: `feat/<topic>` for a feature, `fix/<topic>` for a
fix of NSH's behaviour as it is. Both follow NSH's `AGENTS.md` and `CONTRIBUTING.md` (one topic, `<area>: <Sentence>`
subjects, docs in English and zh-CN, `cargo +nightly fmt` and clippy clean, no Gamify or AgentX files). A branch never
rebases: AgentX records name its commits.

`fork/<topic>` is for a change only the fork needs, which never goes upstream and so gets no draft in [prs/](prs/): today
`fork/release` (worktree `../nsh-wt/release`), which adds `.github/workflows/fork-release.yml` and nothing else. It
follows the same subject and tooling rules and joins `main` through a pull request like the others. A `fork/*` branch
adds files of its own rather than editing upstream's, so an upstream sync never conflicts with it.

## Bringing in upstream

```bash
scripts/sync-main.sh branch ../nsh feat/<topic> --push   # or fix/<topic>
scripts/sync-main.sh upstream ../nsh --pr
```

`branch` merges the newest `upstream/main` into a topic branch. `upstream` cuts `sync/upstream-<sha>` from the fork's
`main`, merges `upstream/main` into it and opens its pull request. Conflict resolutions are replayed from
`scripts/rr-cache/` (git rerere); after resolving a new one, commit the merge, run
`scripts/sync-main.sh save-resolutions ../nsh` and commit `scripts/rr-cache/` on this branch.

When the merged upstream range changes `.github/workflows/release.yml`, `upstream` warns: `fork-release.yml` copies the
steps of its `dist` job, so carry the change over on `fork/release` (a new tool, a new runner) before the next tag.
`scripts/tests/sync-main-release-warning.sh` tests the warning (TAP, throwaway repositories).

## A branch into main

1. Open its pull request with its draft as the body: `gh pr create --repo dXnSix/nsh --base main --head <feat|fix>/<topic>`.
2. Wait for the fork's GitHub Actions. When the pull request conflicts with `main` (a branch merged before it touched
   the same files), merge `origin/main` into the branch and rerun its focused tests.
3. Merge with a merge commit, never squash: `gh pr merge <n> --repo dXnSix/nsh --merge`.

## Tags

```bash
scripts/sync-main.sh tag ../nsh fork-<nsh version>-<n>
```

Tags are `fork-<nsh version>-<n>` on the fork's `main`, never `v*` (the fork's copies of upstream's release
workflows run on those) and never named after a consumer; Cargo versions stay upstream's. The older `gamify-*` and
`six-gamify-*` tags were deleted on 2026-10-21, so Gamify commits that pin them no longer build from a fresh clone.

A pushed tag now produces a release: `.github/workflows/fork-release.yml` (from `fork/release`) checks that the tag is
`fork-<workspace version>-<n>` on `main`, builds NSH's distribution (`cargo xtask dist`) on `macos-arm64`,
`linux-x86_64`, `linux-arm64` and `windows-x86_64`, and publishes the GitHub release `fork-<nsh version>-<n>` with
`nsh-<platform>.tar.gz` (`.zip` on Windows), a `.sha256` and an attestation for each. It takes about an hour. Those
four platforms are required; `windows-arm64` is attached when it builds and never holds the release back. An existing
tag is released by running the workflow by hand with its `tag`
(`gh workflow run fork-release.yml --repo dXnSix/nsh -f tag=fork-<nsh version>-<n>`).

After tagging, move every pin in Gamify (`.cargo/config.toml`, then `cargo update` writes `Cargo.lock`) and run
`rayx setup` there: rayx reads the new pin from the lock and installs that tag's release, or builds it from source
while the release is still running.
