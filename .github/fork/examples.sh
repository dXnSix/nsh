#!/usr/bin/env bash
# The examples' tests on a real platform (`nsh test --target <target>`): the
# list upstream's Linux and Windows workflows run, for the platforms they do
# not cover (macOS, iOS, Android, the web). Usage: examples.sh <target>
# (in a checkout, after `cargo build -p nsh-cli`).
set -euo pipefail
target="$1"
run() { ./target/debug/nsh test "$@" --target "$target"; }
run examples/counter
for file in context-menu device dock form gestures hover links navigation network photos resizable shortcuts storage timers; do
  run "examples/gallery/tests/$file.test.tsx"
done
run examples/gallery/tests/environment.test.tsx --filter appearance
# Apps with Rust crates, in hosts built with them: a TSX app and a Rust app
# (its root).
run examples/rust-exports
run examples/gallery-rs/App.test.tsx
