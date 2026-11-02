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
  # code), picker (`picks()` runs headless only). On the runner: animations
  # and toasts' pointer (13.6 s of waits) do not end in 30 s, network's
  # presses on `fetch()`, far down its page, stall in the window (390 × 653;
  # the request itself is the engine's, run on the other platforms), and
  # environment's rotation expects an 844 pt tall window.
  macos)
    gallery=(device dock error-boundary form gestures hover links list-search mention navigation
      photos press resizable shaders shortcuts storage timers)
    filtered=("environment appearance" "toasts a toast shows" "toasts an undo restores"
      "toasts its close button" "toasts at most three" "toasts a swipe")
    ;;
  # Left out: context-menu (the menu is not reported), controls (the
  # segmented control is not a tab list), links, text-view (span positions
  # are not reported), menus (a row's menu), overlays (tooltips), picker
  # (`picks()` runs headless only). Error-boundary's failing binding expects
  # QuickJS's message; the browser words it its own way.
  web)
    gallery=(animations context-menu-press device dock environment form gestures hover list-search
      mention navigation network photos press resizable shaders shortcuts storage timers toasts)
    filtered=("error-boundary a failing listener")
    ;;
  # Left out: context-menu, dock, hover, overlays, resizable, shortcuts,
  # toasts (a mouse, a keyboard or a desktop's windows), picker (the
  # system's photo picker stays open over the tests after it); environment
  # runs its appearance only (its rotation expects a 390 × 844 window).
  # Form runs its Input demo and its disabled field: the sign-up form and the
  # search field set the viewport, which an emulator keeps at its device's
  # size. Controls runs all but five tests: its number input's lock, a code's
  # Clear and a PIN's Change PIN (presses right after typing, which do not
  # land on the emulator), its pasted codes (Android's tests do not paste
  # yet) and its Tab order (a desktop's, which the device does not follow).
  # On the runner, animations does not end in 30 s. Text-view runs its Text
  # View demo: a press on the chat's featured cards starts no reply on
  # Android (its chips do), with the test's clock and without.
  android)
    gallery=(context-menu-press device error-boundary gestures links list-search mention menus
      navigation network photos press shaders storage timers)
    filtered=("environment appearance" "form the Input demo" "form a disabled field"
      "text-view the Text View demo"
      "controls switch" "controls check" "controls a radio picks" "controls choice cards and sized"
      "controls tabs show" "controls collapsible" "controls accordion" "controls progress bar"
      "controls uploads" "controls stepped setup" "controls slider" "controls equalizer"
      "controls cart's quantities" "controls segmented control")
    ;;
  # Left out: context-menu, dock, hover, overlays, resizable, toasts (a
  # mouse or a desktop's windows), shortcuts (it opens the gallery as macOS,
  # a desktop's platform, which `--target ios` refuses), picker (the
  # system's photo picker);
  # environment runs its appearance only (its rotation expects a 390 × 844
  # window, the newest simulators are bigger). Form runs its Input demo and
  # its disabled field: the sign-up form and the search field set the
  # viewport, which a simulator keeps at its device's size. Controls runs
  # all but five tests: its slider (the volume's drag past its end leaves
  # the demo), its number input's typed seats, a code's Clear and a PIN's
  # Change PIN (the first press after typing only ends the editing), and
  # its Tab order (a desktop's).
  ios)
    gallery=(animations context-menu-press device error-boundary gestures links list-search
      mention menus navigation network photos press shaders storage text-view timers)
    filtered=("environment appearance" "form the Input demo" "form a disabled field"
      "controls switch" "controls check" "controls a radio picks" "controls choice cards and sized"
      "controls tabs show" "controls collapsible" "controls accordion" "controls progress bar"
      "controls uploads" "controls stepped setup" "controls equalizer" "controls cart's quantities"
      "controls a sign-in code" "controls segmented control")
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
