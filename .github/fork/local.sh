#!/usr/bin/env bash
# macOS, iOS and Android, checked on a Mac instead of on GitHub's runners:
# its macOS runners are slow and often not there, and its Android emulator
# goes away under load. The gate (`fork-ci.yml`) has no macOS job and no
# emulator: this script is the fork's check of those platforms for a pull
# request, and for a `pr/*` branch before it goes upstream. GitHub's
# runners check them on `main` at night and after a sync (`fork-full.yml`).
#
# Usage, in the checkout to check (a worktree, at its root or below):
#
#   bash <path>/.github/fork/local.sh [macos] [ios] [android] [gpui] [--all] [--quick] [--no-status]
#
# The script may come from another checkout: a `pr/*` branch, cut from
# upstream, has no `.github/fork/`, and is checked with `main`'s script
# (it reads `examples.sh` and `changed.py` beside itself).
#
# - `macos`: clippy and tests of the macOS backend, hosts and the Apple
#   crate, then the examples' tests in macOS windows (`examples.sh macos`).
#   The windows open on this Mac's screen; their input goes to the views,
#   not through the mouse and the keyboard.
# - `ios`: clippy for Mac Catalyst, the simulator's development host, then
#   the examples' tests in an iOS simulator (`examples.sh ios`).
# - `android`: clippy for Android, the development runtime for the
#   emulator's ABI, then the examples' tests in an Android emulator
#   (`examples.sh android`): the device `adb devices` lists first, else the
#   AVD `nsh-ci-35` (`NSH_ANDROID_AVD` names another), started without a
#   window and stopped afterwards. It is CI's device: Android 15 (API 35,
#   `google_apis`), 320 x 640 at 160 dpi, a 6 GB data partition.
# - `gpui`: upstream's `gpui.yml` steps for macOS, in `crates/gpui`. Only
#   when named.
# - No part named: the parts the change needs, by the paths it touches
#   since it left `main` (`changed.py`, as the gate picks its jobs).
#   `--all`: `macos`, `ios` and `android`.
# - `--quick`: no examples' tests (lints, builds and unit tests only).
#
# The result is reported to GitHub as commit statuses on the checked
# commit, which a pull request of that commit shows with its checks:
# `apple (local)` for `macos` and `ios`, `android (local)` for `android`.
# Only on dXnSix/nsh, only when the commit is there and the checkout has no
# uncommitted change, and a pass only after the whole of it ran (`macos`
# and `ios`, or `android`, without `--quick`). `--no-status` reports
# nothing.
#
# One run at a time on a machine: a second one stops at once.
set -uo pipefail

REPO=dXnSix/nsh
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

parts=()
quick=false
status=true
all=false
for arg in "$@"; do
  case "$arg" in
    macos | ios | android | gpui) parts+=("$arg") ;;
    --all) all=true ;;
    --quick) quick=true ;;
    --no-status) status=false ;;
    *)
      echo "local.sh: unknown argument $arg" >&2
      exit 2
      ;;
  esac
done

if [[ "$(uname -s)" != Darwin ]]; then
  echo "local.sh: runs on a Mac" >&2
  exit 2
fi
root="$(git rev-parse --show-toplevel 2> /dev/null)" || {
  echo "local.sh: not in a checkout" >&2
  exit 2
}
cd "$root"
if [[ ! -d crates/nsh-platform-macos ]]; then
  echo "local.sh: $root is not an NSH checkout" >&2
  exit 2
fi

# No part named: what the change needs. Everything when that cannot be
# told (no `origin/main`, no `changed.py`).
if [[ "$all" == true ]]; then
  parts=(macos ios android)
elif ((${#parts[@]} == 0)); then
  needs="apple=true emulator=true"
  if base="$(git merge-base HEAD origin/main 2> /dev/null)" && [[ -f "$here/changed.py" ]]; then
    files="$(mktemp)"
    git diff --name-only "$base" HEAD > "$files"
    needs="$(python3 "$here/changed.py" "$files" 2> /dev/null | tr '\n' ' ')" || needs="apple=true emulator=true"
    rm -f "$files"
  fi
  [[ "$needs" == *"apple=true"* ]] && parts+=(macos ios)
  [[ "$needs" == *"emulator=true"* ]] && parts+=(android)
  if ((${#parts[@]} == 0)); then
    echo "local.sh: nothing to check: the change touches nothing macOS, iOS or Android build (--all runs them anyway)"
    exit 0
  fi
  echo "local.sh: the change needs: ${parts[*]}"
fi
has() {
  local part
  for part in "${parts[@]}"; do [[ "$part" == "$1" ]] && return 0; done
  return 1
}

lock="${TMPDIR:-/tmp}/nsh-local.lock"
if ! mkdir "$lock" 2> /dev/null; then
  other="$(cat "$lock/pid" 2> /dev/null || true)"
  if [[ -n "$other" ]] && kill -0 "$other" 2> /dev/null; then
    echo "local.sh: another run is going (pid $other, $(cat "$lock/root" 2> /dev/null))" >&2
    exit 3
  fi
  # A run that was killed left its lock behind.
  rm -rf "$lock"
  mkdir "$lock"
fi
echo $$ > "$lock/pid"
echo "$root" > "$lock/root"

# The emulator this run started, stopped when it ends.
started_emulator=""
cleanup() {
  if [[ -n "$started_emulator" ]]; then
    "$adb" -s "$started_emulator" emu kill > /dev/null 2>&1 || true
  fi
  rm -rf "$lock"
}
trap cleanup EXIT

sha="$(git rev-parse HEAD)"
dirty="$(git status --porcelain --untracked-files=no)"
started=$SECONDS
# The failed steps of each status.
failed_apple=()
failed_android=()
group=apple

# Runs a step, and goes on after a failure: one run lists everything that
# fails.
step() {
  local name="$1" from=$SECONDS
  shift
  printf '\n== %s\n' "$name"
  if "$@"; then
    printf -- '-- %s: passed (%ss)\n' "$name" "$((SECONDS - from))"
  else
    printf -- '-- %s: FAILED (%ss)\n' "$name" "$((SECONDS - from))"
    if [[ "$group" == android ]]; then failed_android+=("$name"); else failed_apple+=("$name"); fi
    return 1
  fi
}

examples() {
  if [[ "$quick" == true ]]; then
    echo "skipped (--quick): the examples' tests on $1"
    return 0
  fi
  step "build the command line" cargo build -p nsh-cli || return 1
  step "the examples' tests on $1" bash "$here/examples.sh" "$1"
}

# The device the Android tests run on: `nsh test --target android` takes
# the first one `adb devices` lists.
android_device() {
  local serial avd="${NSH_ANDROID_AVD:-nsh-ci-35}" waited=0
  serial="$("$adb" devices | awk '$2 == "device" { print $1; exit }')"
  if [[ -z "$serial" ]]; then
    if ! "$sdk/emulator/emulator" -list-avds 2> /dev/null | grep -qx "$avd"; then
      echo "no Android device is connected and there is no AVD named $avd (NSH_ANDROID_AVD names another)" >&2
      return 1
    fi
    echo "starting the emulator $avd"
    # CI's options: no window, no guest Vulkan (wgpu would pick it).
    "$sdk/emulator/emulator" -avd "$avd" -no-window -gpu swiftshader_indirect -feature -Vulkan \
      -no-snapshot -no-audio -no-boot-anim > "$work/emulator.log" 2>&1 &
    while [[ -z "$serial" ]] && ((waited < 120)); do
      sleep 2
      waited=$((waited + 2))
      serial="$("$adb" devices | awk '$1 ~ /^emulator-/ { print $1; exit }')"
    done
    [[ -n "$serial" ]] || {
      echo "the emulator $avd did not start (see $work/emulator.log)" >&2
      return 1
    }
    started_emulator="$serial"
    until [[ "$("$adb" -s "$serial" shell getprop sys.boot_completed 2> /dev/null | tr -d '\r')" == 1 ]]; do
      sleep 2
      waited=$((waited + 2))
      ((waited < 300)) || {
        echo "the emulator $avd did not boot in 5 minutes (see $work/emulator.log)" >&2
        return 1
      }
    done
  fi
  # As CI's emulator: no system animations.
  for scale in window_animation_scale transition_animation_scale animator_duration_scale; do
    "$adb" -s "$serial" shell settings put global "$scale" 0.0
  done
  abi="$("$adb" -s "$serial" shell getprop ro.product.cpu.abi | tr -d '\r')"
  echo "device $serial ($abi)"
  [[ -n "$abi" ]]
}

# The emulator's ABI only, stripped, as CI builds it: the development
# runtime at debug is larger than an emulator's storage.
android_runtime() {
  local runtime="$work/nsh-runtime/android/dev" strip
  cargo xtask android --dev --abi "$abi" || return 1
  rm -rf "$work/nsh-runtime"
  mkdir -p "$runtime"
  cp -R target/android/debug-dev/. "$runtime/" || return 1
  strip="$(ls "$sdk"/ndk/*/toolchains/llvm/prebuilt/*/bin/llvm-strip 2> /dev/null | sort | tail -1)"
  [[ -n "$strip" ]] || {
    echo "no llvm-strip in the SDK's NDKs" >&2
    return 1
  }
  "$strip" --strip-debug "$runtime/lib/$abi/libnsh.so"
}

for part in "${parts[@]}"; do
  case "$part" in
    macos)
      group=apple
      step "macOS clippy" cargo clippy --all-targets \
        -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app \
        --features nsh-platform-macos/dev -- -D warnings
      step "macOS clippy without optional features" cargo clippy -p nsh-macos-app \
        --no-default-features --features js -- -D warnings
      step "macOS tests" cargo test -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app -p nsh-platform-apple
      examples macos
      ;;
    ios)
      group=apple
      # `nsh-platform-ios` is empty off iOS: Catalyst checks it on the Mac.
      step "iOS clippy (Mac Catalyst)" cargo clippy \
        -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app \
        --target aarch64-apple-ios-macabi \
        --features nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings
      if step "the iOS simulator's development host" cargo build -p nsh-ios-host --target aarch64-apple-ios-sim; then
        examples ios
      fi
      ;;
    android)
      group=android
      sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
      adb="$sdk/platform-tools/adb"
      work="$root/target/local-check"
      mkdir -p "$work"
      # `examples.sh` looks for adb there when the emulator goes away.
      export ANDROID_HOME="$sdk"
      step "Android clippy" cargo xtask clippy-android
      if [[ "$quick" == true ]]; then
        echo "skipped (--quick): the examples' tests on android"
      elif step "an Android device" android_device \
        && step "the Android development runtime ($abi)" android_runtime; then
        export NSH_RUNTIME="$work/nsh-runtime"
        examples android
        unset NSH_RUNTIME
      fi
      ;;
    gpui)
      group=apple
      gpui() { (cd crates/gpui && "$@"); }
      step "GPUI clippy" gpui cargo clippy --all-targets --features nsh-platform-gpui/dev -- -D warnings
      step "GPUI clippy without optional features" gpui cargo clippy -p nsh-platform-gpui \
        --no-default-features --features js -- -D warnings
      step "GPUI tests" gpui cargo test
      step "GPUI check (runtime-shaders)" gpui cargo check -p nsh-gpui-host --features nsh-platform-gpui/runtime-shaders
      ;;
  esac
done

minutes=$(((SECONDS - started + 30) / 60))
total=$((${#failed_apple[@]} + ${#failed_android[@]}))
echo
if ((total)); then
  echo "local.sh: FAILED on ${sha:0:9} in $minutes min:"
  ((${#failed_apple[@]})) && printf '  %s\n' "${failed_apple[@]}"
  ((${#failed_android[@]})) && printf '  %s\n' "${failed_android[@]}"
else
  echo "local.sh: passed on ${sha:0:9} in $minutes min (${parts[*]}$([[ "$quick" == true ]] && echo ', quick'))"
fi

# report <context> <whole: true|false> <what ran> <failed steps...>
report() {
  local context="$1" whole="$2" ran="$3" state description
  shift 3
  if (($#)); then
    state=failure
    description="$ran: $# failed ($1)"
  else
    state=success
    description="$ran: passed, $minutes min for the run"
    if [[ "$whole" == false ]]; then
      echo "no \"$context\" status: a pass is reported only after the whole of it ran, without --quick"
      return 0
    fi
  fi
  if gh api "repos/$REPO/statuses/$sha" -f state="$state" -f context="$context" \
    -f description="${description:0:140}" --silent; then
    echo "status \"$context\" on $REPO@${sha:0:9}: $state"
  else
    echo "no \"$context\" status: GitHub refused it"
  fi
}
if [[ "$status" == true ]]; then
  if [[ -n "$dirty" ]]; then
    echo "no status: the checkout has uncommitted changes"
  elif ! command -v gh > /dev/null || ! gh api "repos/$REPO/commits/$sha" --silent 2> /dev/null; then
    echo "no status: ${sha:0:9} is not on $REPO (push the branch, then run again)"
  else
    if has macos || has ios; then
      whole=false
      has macos && has ios && [[ "$quick" == false ]] && whole=true
      ran="$(has macos && printf 'macos ')$(has ios && printf 'ios')"
      report "apple (local)" "$whole" "${ran% }" ${failed_apple[@]+"${failed_apple[@]}"}
    fi
    if has android; then
      whole=true
      [[ "$quick" == true ]] && whole=false
      report "android (local)" "$whole" android ${failed_android[@]+"${failed_android[@]}"}
    fi
  fi
fi
((total == 0))
