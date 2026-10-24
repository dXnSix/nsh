#!/usr/bin/env bash
# TAP test: `sync-main.sh upstream` and `sync-main.sh branch` warn when the
# upstream range they merge changes `.github/workflows/release.yml` (whose
# `dist` job the fork's `fork-release.yml` copies), and say nothing about it
# otherwise. Every
# repository is a throwaway one under `mktemp -d`; nothing is pushed anywhere
# else and `--pr` is never passed.
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

# A fork whose main has one commit of its own, and an upstream that then
# changes `changed` (a path relative to the repository root).
# Prints the fork checkout's directory.
setup() { # <name> <changed>
  local dir="$root/$1" changed="$2"
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
  echo "fork" > "$dir/fork-work/FORK.md"
  git -C "$dir/fork-work" add -A
  git -C "$dir/fork-work" commit --quiet -m "Fork change"
  git -C "$dir/fork-work" push --quiet origin main

  # Upstream moves on.
  echo "changed" >> "$dir/seed/$changed"
  git -C "$dir/seed" commit --quiet -am "Upstream change"
  git -C "$dir/seed" push --quiet "$dir/longbridge/nsh.git" main

  git clone --quiet "$dir/dXnSix/nsh.git" "$dir/nsh"
  git -C "$dir/nsh" remote add upstream "$dir/longbridge/nsh.git"
  echo "$dir/nsh"
}

failed=0
echo "1..3"

checkout="$(setup release .github/workflows/release.yml)"
output="$("$script" upstream "$checkout" 2>&1)" || output+=$'\n'"(sync-main.sh exited $?)"
if grep -q '\.github/workflows/release\.yml' <<<"$output" && grep -q 'fork-release\.yml' <<<"$output" \
  && grep -q 'warning' <<<"$output"; then
  echo "ok 1 - a range that changes release.yml warns naming release.yml and fork-release.yml"
else
  echo "not ok 1 - a range that changes release.yml warns naming release.yml and fork-release.yml"
  failed=1
  sed 's/^/# /' <<<"$output"
fi

checkout="$(setup readme README.md)"
output="$("$script" upstream "$checkout" 2>&1)" || output+=$'\n'"(sync-main.sh exited $?)"
if ! grep -q 'release\.yml' <<<"$output" && grep -q 'pushed sync/upstream-' <<<"$output"; then
  echo "ok 2 - a range that leaves release.yml alone says nothing about it"
else
  echo "not ok 2 - a range that leaves release.yml alone says nothing about it"
  failed=1
  sed 's/^/# /' <<<"$output"
fi

# A topic branch in its own worktree takes upstream's range the same way.
checkout="$(setup branch .github/workflows/release.yml)"
git -C "$checkout" worktree add --quiet -b feat/topic "$root/branch/wt-topic" origin/main
output="$("$script" branch "$checkout" feat/topic 2>&1)" || output+=$'\n'"(sync-main.sh exited $?)"
if grep -q '\.github/workflows/release\.yml' <<<"$output" && grep -q 'fork-release\.yml' <<<"$output" \
  && grep -q 'sync-main: feat/topic' <<<"$output"; then
  echo "ok 3 - a topic branch that takes a release.yml change warns too"
else
  echo "not ok 3 - a topic branch that takes a release.yml change warns too"
  failed=1
  sed 's/^/# /' <<<"$output"
fi

exit "$failed"
