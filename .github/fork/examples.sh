#!/usr/bin/env bash
# The examples' tests on a real platform (`nsh test --target <target>`), for
# the platforms upstream's workflows do not cover (macOS, iOS, Android, the
# web, Windows on Arm). Usage: examples.sh <target> (in a checkout, after `cargo build -p
# nsh-cli`).
#
# Each platform runs the gallery's test files it passes, measured by running
# every one of them there and on the runners; a file (or a test of it) is
# left out for what the platform does not do or does not report yet, or
# for what does not end within its 30 s on a runner (named below), and
# comes back once it does. Every listed test runs, and the failures are
# listed at the end (an Android emulator that goes away ends the run at the
# tests that lost it). A file that fails runs once more before it counts:
# the runners' small windows and slow frames make tests that pass at a desk
# miss now and then (scrolling a control into view, a drag); one that passes
# the second time is reported as flaky (a warning on the run). A listed file
# the branch does not have is skipped, with a notice.
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
  # environment's rotation expects an 844 pt tall window. Toasts' close
  # button, back and dismissToast: since upstream's #1305 (the toast
  # stack's springs run until they settle) its third toast does not show
  # within the test's 5 s on the runner.
  macos)
    gallery=(device dock error-boundary form gestures hover links list-search mention navigation
      photos press resizable shaders shortcuts storage timers)
    filtered=("environment appearance" "toasts a toast shows" "toasts an undo restores"
      "toasts at most three" "toasts a swipe")
    ;;
  # Left out: context-menu (the menu is not reported), controls (the
  # segmented control is not a tab list), links, text-view (span positions
  # are not reported), menus (a row's menu), overlays (tooltips), picker
  # (`picks()` runs headless only). Error-boundary's failing binding expects
  # QuickJS's message; the browser words it its own way. Context-menu-press
  # runs its tap and its Shift+F10: the test's held finger is the mouse in
  # the browser, and a held mouse button opens no menu since upstream's
  # #1555. Toasts runs all but three tests: since upstream's #1305 (the
  # toast stack's springs run until they settle) a toast outlasts the time
  # the tests give it on the runner in "a toast shows", "at most three" and
  # "on a desktop, the pointer" (all three pass on a Mac).
  web)
    gallery=(animations device dock environment form gestures hover list-search
      mention navigation network photos press resizable shaders shortcuts storage timers)
    filtered=("error-boundary a failing listener"
      "context-menu-press a tap presses" "context-menu-press Shift+F10"
      "toasts an undo restores" "toasts its close button" "toasts a swipe"
      "toasts an error's retry" "toasts a loading toast counts" "toasts a failed upload")
    ;;
  # Left out: context-menu, dock, hover, overlays, resizable, shortcuts,
  # toasts (a mouse, a keyboard or a desktop's windows), picker (the
  # system's photo picker stays open over the tests after it); environment
  # runs its appearance only (its rotation expects a 390 × 844 window).
  # Form runs its Input demo and its disabled field: the sign-up form and the
  # search field set the viewport, which an emulator keeps at its device's
  # size. Controls runs all but six tests: its number input's lock, a code's
  # Clear and a PIN's Change PIN (presses right after typing, which do not
  # land on the emulator), its pasted codes (Android's tests do not paste
  # yet), its Tab order (a desktop's, which the device does not follow) and
  # the gallery's toggle (below).
  # On the runner, animations does not end in 30 s, and about one run in
  # five neither does the gallery's toggle: it opens the Styles page, whose
  # progressive blur (five stacked backdrop blurs, since upstream's #930)
  # the emulator draws in software on the runner's four cores. The emulator
  # stalls on that page, then goes away, and every test after it fails (a
  # Mac's emulator, drawing in software too, passes it). Text-view runs its
  # Text View demo: a press on the chat's featured cards starts no reply on
  # Android (its chips do), with the test's clock and without.
  # Context-menu-press runs its tap: its Shift+F10 opens the app as the web,
  # which `--target android` refuses, and since the sync with upstream
  # `6a4f1de19` the menu its held finger opens has chosen its item before
  # the test's press on it, which finds no menu (iOS too). Menus runs all
  # but three tests (undo and redo, a choice of one in a submenu, a row's
  # menu changes the list): since that sync a press on an item of a menu
  # that has just opened lands on the item next to it on the emulator (Copy
  # for Undo Save, Duplicate for Delete); they pass on iOS.
  android)
    gallery=(device error-boundary gestures links list-search mention
      navigation network photos press shaders storage timers)
    filtered=("environment appearance" "form the Input demo" "form a disabled field"
      "text-view the Text View demo"
      "menus submenus three levels" "menus settings are checkable" "menus a choice of one closes"
      "menus a row's context menu" "menus the placement and alignment" "menus a long menu scrolls"
      "menus a command that takes a while"
      "context-menu-press a tap presses"
      "controls a switch turns" "controls a quick setting" "controls check" "controls a radio picks"
      "controls choice cards and sized" "controls tabs show" "controls collapsible"
      "controls accordion" "controls progress bar" "controls uploads" "controls stepped setup"
      "controls slider" "controls equalizer" "controls cart's quantities"
      "controls segmented control")
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
  # its Tab order (a desktop's). Context-menu-press runs its tap: its
  # Shift+F10 opens the app as the web, which `--target ios` refuses, and
  # since the sync with upstream `6a4f1de19` the menu its held finger opens
  # has chosen its item before the test's press on it, which finds no menu.
  ios)
    gallery=(animations device error-boundary gestures links list-search
      mention menus navigation network photos press shaders storage text-view timers)
    filtered=("environment appearance" "form the Input demo" "form a disabled field"
      "context-menu-press a tap presses"
      "controls switch" "controls check" "controls a radio picks" "controls choice cards and sized"
      "controls tabs show" "controls collapsible" "controls accordion" "controls progress bar"
      "controls uploads" "controls stepped setup" "controls equalizer" "controls cart's quantities"
      "controls a sign-in code" "controls segmented control")
    ;;
  # Windows on Arm (the `windows-arm64` job): the files upstream's
  # `windows.yml` runs on x64.
  windows)
    gallery=(context-menu device dock form gestures hover links navigation network photos
      resizable shortcuts storage timers)
    filtered=("environment appearance")
    ;;
  *)
    echo "examples.sh: no tests for target $target" >&2
    exit 2
    ;;
esac

failed=()
flaky=()
# The tests that lost the emulator, once it is gone.
lost=""
# Whether the Android emulator is gone: `adb devices` lists no device, which
# is where `nsh test` looks for one.
emulator_gone() {
  [[ "$target" == android ]] || return 1
  local adb="${ANDROID_HOME:+$ANDROID_HOME/platform-tools/}adb"
  command -v "$adb" > /dev/null || return 1
  ! "$adb" devices 2> /dev/null | grep -q $'\tdevice$'
}
run() {
  # Without its emulator every test only fails: the run ends at the tests
  # that lost it, and says so.
  [[ -n "$lost" ]] && return
  # The lists follow the fork's main; a pull request cut from upstream
  # before a gallery test landed there has no file for it yet.
  if [[ "$1" == *.test.tsx && ! -e "$1" ]]; then
    echo "::notice title=Test file not on this branch::$1"
    return
  fi
  ./target/debug/nsh test "$@" --target "$target" && return
  if ./target/debug/nsh test "$@" --target "$target"; then
    flaky+=("$*")
  else
    failed+=("$*")
    if emulator_gone; then
      lost="$*"
      echo "::error title=The emulator went away::during $lost; the tests after it did not run"
    fi
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
# The macOS host's own check of a press under a hidden title bar: on the
# runners' macOS 15 the title bar's toolbar view is what AppKit hits there.
if [[ "$target" == macos ]]; then
  run crates/nsh-platform-macos/tests/host/TitleBar.test.tsx
fi
# Apps with Rust crates, in hosts built with them: a TSX app and a Rust app
# (its root). The TSX app's `modules.test.tsx` runs headless only (`open`'s
# `modules`), so its `App.test.tsx` is named, as in upstream's workflows.
run examples/rust-exports/App.test.tsx
run examples/gallery-rs/App.test.tsx

for entry in ${flaky[@]+"${flaky[@]}"}; do
  echo "::warning title=Flaky test::$entry failed, then passed when run again"
done
if ((${#failed[@]})); then
  printf 'failed: %s\n' "${failed[@]}" >&2
  exit 1
fi
