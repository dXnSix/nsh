#!/usr/bin/env bash
# The macOS crates' tests, as `cargo test -p <crate>...` runs them, less the
# cases of upstream's tests that fail on upstream's own code. Upstream's CI
# runs none of these crates' tests (it has no macOS test job), so the fork
# leaves such a case out instead of patching upstream's file (the ledger's
# fork-ci.md, "Upstream tests the fork leaves out"). Written for macOS's
# bash 3.2.
#
#   bash .github/fork/macos-tests.sh <package> [<package>...]
set -euo pipefail

# The test target with cases left out, and the cases it still runs (its
# arguments; harness = false); none now. draw_on_show's `canvas`, `window`
# and `unhide`, left out from sync #293 and #299, pass since the fork's
# macos-inline-root (2026-11-25).
PARTIAL=
KEPT=

args=()
for p in "$@"; do args+=(-p "$p"); done

# Every test target of the packages but $PARTIAL.
targets=(--lib --bins --examples)
for t in $(cargo metadata --no-deps --format-version 1 | python3 -c '
import json, sys
names = set(sys.argv[1:-1])
for p in json.load(sys.stdin)["packages"]:
    if p["name"] in names:
        for t in p["targets"]:
            if "test" in t["kind"] and t["name"] != sys.argv[-1]:
                print(t["name"])
' "$@" "$PARTIAL"); do
  targets+=(--test "$t")
done
cargo test "${args[@]}" "${targets[@]}"
cargo test "${args[@]}" --doc
if [ -n "$PARTIAL" ]; then
  # shellcheck disable=SC2086 # the cases are words
  cargo test "${args[@]}" --test "$PARTIAL" -- $KEPT
fi
