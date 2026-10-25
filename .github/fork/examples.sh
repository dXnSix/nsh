#!/usr/bin/env bash
# The examples' tests on a real platform (`nsh test --target <target>`), for
# the platforms upstream's workflows do not cover (macOS, iOS, Android, the
# web). Usage: examples.sh <target> (in a checkout, after `cargo build -p
# nsh-cli`).
#
# Each platform runs the gallery's test files it passes, measured by running
# every one of them there and on the runners; a file (or a test of it) is
# left out for what the platform does not do or does not report yet, or
# for what does not end within its 30 s on a runner (named below), and
# comes back once it does. Every listed test runs, and the failures are
# listed at the end. A file that fails runs once more before it counts:
# the runners' small windows and slow frames make tests that pass at a desk
# miss now and then (scrolling a control into view, a drag); one that passes
# the second time is reported as flaky (a warning on the run).
set -uo pipefail
target="$1"
# "<file> <filter>": tests run one by one, by a part of their name, from a
# file that runs only in part.
filtered=()
case "$target" in
  # Left out: context-menu (passes on a Mac, but did not end on the
  # runner: the menu's tracking loop), context-menu-press, overlays (touch
  # screens' menus and tooltips), controls (the segmented control is not a
  # tab list), menus (a row's menu, submenus), text-view (a streamed reply's
  # code), picker (`picks()` runs headless only). On the runner: animations,
  # network's second request and toasts' pointer (13.6 s of waits) do not
  # end in 30 s, and the window is 390 × 653 (environment's rotation expects
  # 844).
  macos)
    gallery=(device dock error-boundary form gestures hover links list-search mention navigation
      photos press resizable shortcuts storage timers)
    filtered=("environment appearance" "network loads the posts" "network shows why"
      "toasts a toast shows" "toasts its action" "toasts its close button" "toasts at most three"
      "toasts a swipe")
    ;;
  # Left out: context-menu (the menu is not reported), controls (the
  # segmented control is not a tab list), links, text-view (span positions
  # are not reported), menus (a row's menu), overlays (tooltips), picker
  # (`picks()` runs headless only).
  web)
    gallery=(animations context-menu-press device dock environment error-boundary form gestures
      hover list-search mention navigation network photos press resizable shortcuts storage timers
      toasts)
    ;;
  # Left out: context-menu, dock, hover, overlays, resizable, shortcuts,
  # toasts (a mouse, a keyboard or a desktop's windows), picker (the
  # system's photo picker stays open over the tests after it); environment
  # runs its appearance only (its rotation expects a 390 × 844 window).
  # On the runner, animations does not end in 30 s.
  android)
    gallery=(context-menu-press controls device error-boundary form gestures links
      list-search mention menus navigation network photos press storage text-view timers)
    filtered=("environment appearance")
    ;;
  # Left out: context-menu, dock, hover, overlays, resizable, toasts (a
  # mouse or a desktop's windows), picker (the system's photo picker);
  # environment runs its appearance only (its rotation expects a 390 × 844
  # window, the newest simulators are bigger).
  ios)
    gallery=(animations context-menu-press controls device error-boundary form gestures links
      list-search mention menus navigation network photos press shortcuts storage text-view
      timers)
    filtered=("environment appearance")
    ;;
  *)
    echo "examples.sh: no tests for target $target" >&2
    exit 2
    ;;
esac

failed=()
flaky=()
run() {
  ./target/debug/nsh test "$@" --target "$target" && return
  if ./target/debug/nsh test "$@" --target "$target"; then
    flaky+=("$*")
  else
    failed+=("$*")
  fi
}
run examples/counter
for file in "${gallery[@]}"; do
  run "examples/gallery/tests/$file.test.tsx"
done
# (Guarded: Bash 3.2, macOS's, takes an empty array for an unset one.)
for entry in ${filtered[@]+"${filtered[@]}"}; do
  read -r file filter <<< "$entry"
  run "examples/gallery/tests/$file.test.tsx" --filter "$filter"
done
# Apps with Rust crates, in hosts built with them: a TSX app and a Rust app
# (its root).
run examples/rust-exports
run examples/gallery-rs/App.test.tsx

for entry in ${flaky[@]+"${flaky[@]}"}; do
  echo "::warning title=Flaky test::$entry failed, then passed when run again"
done
if ((${#failed[@]})); then
  printf 'failed: %s\n' "${failed[@]}" >&2
  exit 1
fi
