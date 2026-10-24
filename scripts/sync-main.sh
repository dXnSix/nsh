#!/usr/bin/env bash
# Keeps the NSH fork's `main` (github.com/dXnSix/nsh), Gamify's integration
# line, in step with upstream and tags it. `main` is upstream's `main` plus
# the merge commits of the fork's topic-branch pull requests, so it never
# fast-forwards to upstream again and is never rebuilt: everything reaches it
# through a pull request merged with a merge commit (`gh pr merge --merge`).
# Never pushes to `upstream`, opens nothing on longbridge/nsh, never creates
# `v*` tags (the fork's copies of upstream's release workflows run on them).
#
#   scripts/sync-main.sh upstream <nsh-checkout> [--pr]
#   scripts/sync-main.sh branch <nsh-checkout> <feat/topic|fix/topic> [--push]
#   scripts/sync-main.sh tag <nsh-checkout> fork-<nsh version>-<n>
#   scripts/sync-main.sh save-resolutions <nsh-checkout>
#
# upstream  Merges the newest `upstream/main` into a new `sync/upstream-<sha>`
#           branch cut from `origin/main` and pushes it; `--pr` opens its pull
#           request into the fork's `main`. Merge it with a merge commit. When
#           the merged range changes upstream's `.github/workflows/release.yml`,
#           it warns: the fork's `fork-release.yml` copies its `dist` steps by
#           hand.
# branch    Merges the newest `upstream/main` into a topic branch (`feat/*`
#           for a feature, `fix/*` for a fix; in its own worktree under
#           `../nsh-wt/`), never rebasing it: sealed AgentX records name its
#           commits. `--push` pushes it to `origin`. Warns like `upstream`
#           when the merged range changes upstream's release workflow. Before a
#           branch's pull request merges, merge `origin/main` into it the same
#           way when it overlaps a branch merged before it.
# tag       Tags the fork's `origin/main` (annotated) and pushes the tag. Cargo
#           versions stay upstream's. Older `gamify-*` tags are kept for good.
# save-resolutions
#           After resolving a new conflict by hand and committing the merge,
#           copies the repository's rerere cache next to this script.
#
# Conflict resolutions recorded next to this script (`rr-cache/`) are replayed
# with git rerere. Tags never name a consumer: the older `gamify-*` and
# `six-gamify-*` tags were deleted.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
die() { echo "sync-main: $*" >&2; exit 1; }

[ $# -ge 2 ] || die "usage: $0 upstream|branch|tag|save-resolutions <nsh-checkout> …"
cmd="$1"; checkout="$2"; shift 2
git_() { git -C "$checkout" "$@"; }
common="$(cd "$checkout" && cd "$(git rev-parse --git-common-dir)" && pwd)"

git_ remote get-url upstream >/dev/null || die "no upstream remote"
git_ remote get-url origin | grep -q 'dXnSix/nsh' || die "origin is not github.com/dXnSix/nsh"

replay() {
  mkdir -p "$common/rr-cache"
  if [ -d "$here/rr-cache" ]; then cp -R "$here/rr-cache/." "$common/rr-cache/"; fi
}
merge_in() { # <worktree> <ref>
  if ! git -C "$1" -c rerere.enabled=true -c rerere.autoUpdate=true merge --no-edit --quiet "$2"; then
    left="$(git -C "$1" diff --name-only --diff-filter=U)"
    [ -z "$left" ] || die "merging $2 left conflicts in: $left
resolve them, commit the merge, then run save-resolutions and commit rr-cache/"
    git -C "$1" commit --quiet --no-edit
  fi
}

# Warns when the upstream range just merged (from <base> to upstream/main)
# changes upstream's release workflow, whose `dist` job `fork-release.yml`
# (on `fork/release`) copies.
release_changed() { # <worktree> <base>
  if ! git -C "$1" diff --quiet "$2" upstream/main -- .github/workflows/release.yml; then
    echo "sync-main: warning: this upstream range changes .github/workflows/release.yml;" >&2
    echo "sync-main: copy what its dist job changed into .github/workflows/fork-release.yml on fork/release" >&2
  fi
}

case "$cmd" in
  save-resolutions)
    mkdir -p "$here/rr-cache"
    cp -R "$common/rr-cache/." "$here/rr-cache/"
    echo "sync-main: saved $(ls "$here/rr-cache" | wc -l | tr -d ' ') recorded resolutions"
    ;;
  upstream)
    pr=0; [ "${1:-}" = --pr ] && pr=1
    [ -z "$(git_ status --porcelain)" ] || die "$checkout has uncommitted changes"
    git_ fetch --quiet upstream
    git_ fetch --quiet origin
    if git_ merge-base --is-ancestor upstream/main origin/main; then
      echo "sync-main: origin/main already contains upstream/main"; exit 0
    fi
    branch="sync/upstream-$(git_ rev-parse --short upstream/main)"
    base="$(git_ merge-base origin/main upstream/main)"
    replay
    git_ checkout --quiet -B "$branch" origin/main
    merge_in "$checkout" upstream/main
    git_ push --quiet origin "$branch"
    echo "sync-main: pushed $branch"
    release_changed "$checkout" "$base"
    if [ "$pr" = 1 ]; then
      gh pr create --repo dXnSix/nsh --base main --head "$branch" \
        --title "Merge upstream/main $(git_ rev-parse --short upstream/main)" \
        --body "Brings the fork's main up to upstream/main $(git_ rev-parse --short upstream/main). Merge with a merge commit."
    fi
    ;;
  branch)
    [ $# -ge 1 ] || die "branch needs <feat/topic> or <fix/topic>"
    topic="$1"; push=0; [ "${2:-}" = --push ] && push=1
    case "$topic" in feat/* | fix/*) ;; *) die "feat or fix branches only, not $topic" ;; esac
    worktree="$(git_ worktree list --porcelain | awk -v b="refs/heads/$topic" '/^worktree /{w=$2} $0=="branch "b{print w}')"
    [ -n "$worktree" ] || die "$topic has no worktree"
    [ -z "$(git -C "$worktree" status --porcelain)" ] || die "$worktree has uncommitted changes"
    git_ fetch --quiet upstream
    base="$(git -C "$worktree" merge-base HEAD upstream/main)"
    replay
    merge_in "$worktree" upstream/main
    echo "sync-main: $topic $(git -C "$worktree" log --oneline -1)"
    release_changed "$worktree" "$base"
    [ "$push" = 1 ] && git -C "$worktree" push --quiet origin "$topic"
    ;;
  tag)
    [ $# -ge 1 ] || die "tag needs fork-<nsh version>-<n>"
    tag="$1"
    case "$tag" in fork-*-*) ;; *) die "tags are fork-<nsh version>-<n>, never $tag" ;; esac
    git_ fetch --quiet origin
    git_ tag -a "$tag" -m "Fork integration $tag: the fork's main at $(git_ rev-parse --short origin/main)" origin/main
    git_ push --quiet origin "refs/tags/$tag"
    echo "sync-main: tagged and pushed $tag on $(git_ log --oneline -1 origin/main)"
    ;;
  *) die "unknown command $cmd" ;;
esac
