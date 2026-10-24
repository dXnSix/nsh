#!/usr/bin/env bash
# TAP test: `sync-main.sh upstream` and `sync-main.sh branch` warn when the
# upstream range they merge changes `.github/workflows/release.yml` (whose
# `dist` job the fork's `fork-release.yml` copies), also when the merge stops
# on a conflict, and say nothing about it otherwise. Every repository is a
# throwaway one under `mktemp -d`; nothing is pushed anywhere else and `--pr`
# is never passed.
set -euo pipefail

script="$(cd "$(dirname "$0")/.." && pwd)/sync-main.sh"
root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT

export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.com
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.com
export GIT_CONFIG_NOSYSTEM=1 HOME="$root/home"
mkdir -p "$HOME"
git config --global init.defaultBranch main
git config --global advice.detachedHead false

# A fork whose main has one commit of its own (it rewrites `README.md` when
# `conflict` is 1, else adds `FORK.md`), and an upstream that then appends to
# each of `changed` (paths relative to the repository root, space-separated).
# Prints the fork checkout's directory.
setup() { # <name> <changed> [conflict]
  local dir="$root/$1" changed="$2" conflict="${3:-0}"
  mkdir -p "$dir/longbridge" "$dir/dXnSix"
  git init --quiet --bare "$dir/longbridge/nsh.git"
  git init --quiet --bare "$dir/dXnSix/nsh.git"

  git init --quiet "$dir/seed"
  mkdir -p "$dir/seed/.github/workflows"
  echo "name: Release" > "$dir/seed/.github/workflows/release.yml"
  echo "nsh" > "$dir/seed/README.md"
  git -C "$dir/seed" add -A
  git -C "$dir/seed" commit --quiet -m "Initial"
  git -C "$dir/seed" push --quiet "$dir/longbridge/nsh.git" main
  git -C "$dir/seed" push --quiet "$dir/dXnSix/nsh.git" main

  # The fork's own pull request.
  git clone --quiet "$dir/dXnSix/nsh.git" "$dir/fork-work"
  if [ "$conflict" = 1 ]; then
    echo "the fork's nsh" > "$dir/fork-work/README.md"
  else
    echo "fork" > "$dir/fork-work/FORK.md"
  fi
  git -C "$dir/fork-work" add -A
  git -C "$dir/fork-work" commit --quiet -m "Fork change"
  git -C "$dir/fork-work" push --quiet origin main

  # Upstream moves on.
  local path
  for path in $changed; do echo "changed" >> "$dir/seed/$path"; done
  git -C "$dir/seed" commit --quiet -am "Upstream change"
  git -C "$dir/seed" push --quiet "$dir/longbridge/nsh.git" main

  git clone --quiet "$dir/dXnSix/nsh.git" "$dir/nsh"
  git -C "$dir/nsh" remote add upstream "$dir/longbridge/nsh.git"
  echo "$dir/nsh"
}

# Runs sync-main.sh, setting `output` (stdout and stderr) and `status`.
sync() {
  status=0
  output="$("$script" "$@" 2>&1)" || status=$?
}

warned() {
  grep -q 'warning' <<<"$output" && grep -q '\.github/workflows/release\.yml' <<<"$output" \
    && grep -q 'fork-release\.yml' <<<"$output"
}

failed=0
report() { # <passed> <description>
  if [ "$1" = 1 ]; then
    echo "ok $n - $2"
  else
    echo "not ok $n - $2"
    failed=1
    sed 's/^/# /' <<<"$output"$'\n'"(sync-main.sh exited $status)"
  fi
  n=$((n + 1))
}
pass() { if "$@"; then echo 1; else echo 0; fi; }
n=1
echo "1..5"

checkout="$(setup release .github/workflows/release.yml)"
sync upstream "$checkout"
report "$(pass eval '[ "$status" = 0 ] && warned')" \
  "a range that changes release.yml warns naming release.yml and fork-release.yml"

checkout="$(setup readme README.md)"
sync upstream "$checkout"
report "$(pass eval '[ "$status" = 0 ] && ! grep -q "release\.yml" <<<"$output" && grep -q "pushed sync/upstream-" <<<"$output"')" \
  "a range that leaves release.yml alone says nothing about it"

# A topic branch in its own worktree takes upstream's range the same way.
checkout="$(setup branch .github/workflows/release.yml)"
git -C "$checkout" worktree add --quiet -b feat/topic "$root/branch/wt-topic" origin/main
sync branch "$checkout" feat/topic
report "$(pass eval '[ "$status" = 0 ] && warned && grep -q "sync-main: feat/topic" <<<"$output"')" \
  "a topic branch that takes a release.yml change warns too"

# A merge that stops on a conflict has warned before it stopped.
checkout="$(setup upstream-conflict ".github/workflows/release.yml README.md" 1)"
sync upstream "$checkout"
report "$(pass eval '[ "$status" = 1 ] && warned && grep -q "left conflicts in: README.md" <<<"$output"')" \
  "upstream warns about release.yml when its merge stops on a conflict"

checkout="$(setup branch-conflict ".github/workflows/release.yml README.md" 1)"
git -C "$checkout" worktree add --quiet -b fix/topic "$root/branch-conflict/wt-topic" origin/main
sync branch "$checkout" fix/topic
report "$(pass eval '[ "$status" = 1 ] && warned && grep -q "left conflicts in: README.md" <<<"$output"')" \
  "branch warns about release.yml when its merge stops on a conflict"

exit "$failed"
