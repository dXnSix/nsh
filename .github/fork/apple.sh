#!/usr/bin/env bash
# macOS and iOS, checked on a Mac instead of on GitHub's macOS runners
# (slow, and often not there). The gate (`fork-ci.yml`) has no macOS job:
# this script is the fork's macOS and iOS check for a pull request, and for
# a `pr/*` branch before it goes upstream. GitHub's macOS runners check
# `main` at night and after a sync (`fork-full.yml`).
#
# Usage, in the checkout to check (a worktree, at its root or below):
#
#   bash <path>/.github/fork/apple.sh [macos] [ios] [gpui] [--quick] [--no-status]
#
# The script may come from another checkout: a `pr/*` branch, cut from
# upstream, has no `.github/fork/`, and is checked with `main`'s script
# (it reads `examples.sh` beside itself).
#
# - `macos`: clippy and tests of the macOS backend, hosts and the Apple
#   crate, then the examples' tests in macOS windows (`examples.sh macos`).
#   The windows open on this Mac's screen; their input goes to the views,
#   not through the mouse and the keyboard.
# - `ios`: clippy for Mac Catalyst, the simulator's development host, then
#   the examples' tests in an iOS simulator (`examples.sh ios`).
# - `gpui`: upstream's `gpui.yml` steps for macOS, in `crates/gpui`. Only
#   when named.
# - No part named: `macos` and `ios`.
# - `--quick`: no examples' tests (lints, builds and unit tests only).
#
# The result is reported to GitHub as the commit status `apple (local)` on
# the checked commit, which a pull request of that commit shows with its
# checks. Only on dXnSix/nsh, only when the commit is there and the checkout
# has no uncommitted change, and a pass only after a whole run (`macos` and
# `ios`, without `--quick`). `--no-status` reports nothing.
#
# One run at a time on a machine: a second one stops at once.
set -uo pipefail

REPO=dXnSix/nsh
CONTEXT="apple (local)"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

parts=()
quick=false
status=true
for arg in "$@"; do
  case "$arg" in
    macos | ios | gpui) parts+=("$arg") ;;
    --quick) quick=true ;;
    --no-status) status=false ;;
    *)
      echo "apple.sh: unknown argument $arg" >&2
      exit 2
      ;;
  esac
done
whole=false
if ((${#parts[@]} == 0)); then
  parts=(macos ios)
  [[ "$quick" == false ]] && whole=true
fi

if [[ "$(uname -s)" != Darwin ]]; then
  echo "apple.sh: runs on a Mac" >&2
  exit 2
fi
root="$(git rev-parse --show-toplevel 2> /dev/null)" || {
  echo "apple.sh: not in a checkout" >&2
  exit 2
}
cd "$root"
if [[ ! -d crates/nsh-platform-macos ]]; then
  echo "apple.sh: $root is not an NSH checkout" >&2
  exit 2
fi

lock="${TMPDIR:-/tmp}/nsh-apple.lock"
if ! mkdir "$lock" 2> /dev/null; then
  other="$(cat "$lock/pid" 2> /dev/null || true)"
  if [[ -n "$other" ]] && kill -0 "$other" 2> /dev/null; then
    echo "apple.sh: another run is going (pid $other, $(cat "$lock/root" 2> /dev/null))" >&2
    exit 3
  fi
  # A run that was killed left its lock behind.
  rm -rf "$lock"
  mkdir "$lock"
fi
echo $$ > "$lock/pid"
echo "$root" > "$lock/root"
trap 'rm -rf "$lock"' EXIT

sha="$(git rev-parse HEAD)"
dirty="$(git status --porcelain --untracked-files=no)"
started=$SECONDS
failed=()

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
    failed+=("$name")
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

for part in "${parts[@]}"; do
  case "$part" in
    macos)
      step "macOS clippy" cargo clippy --all-targets \
        -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app \
        --features nsh-platform-macos/dev -- -D warnings
      step "macOS clippy without optional features" cargo clippy -p nsh-macos-app \
        --no-default-features --features js -- -D warnings
      step "macOS tests" cargo test -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app -p nsh-platform-apple
      examples macos
      ;;
    ios)
      # `nsh-platform-ios` is empty off iOS: Catalyst checks it on the Mac.
      step "iOS clippy (Mac Catalyst)" cargo clippy \
        -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app \
        --target aarch64-apple-ios-macabi \
        --features nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings
      if step "the iOS simulator's development host" cargo build -p nsh-ios-host --target aarch64-apple-ios-sim; then
        examples ios
      fi
      ;;
    gpui)
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
echo
if ((${#failed[@]})); then
  echo "apple.sh: FAILED on ${sha:0:9} in $minutes min:"
  printf '  %s\n' "${failed[@]}"
  state=failure
  description="${parts[*]}: ${#failed[@]} failed (${failed[0]})"
else
  echo "apple.sh: passed on ${sha:0:9} in $minutes min (${parts[*]}$([[ "$quick" == true ]] && echo ', quick'))"
  state=success
  description="${parts[*]}: passed in $minutes min"
fi

report() {
  [[ "$status" == true ]] || return 0
  if [[ -n "$dirty" ]]; then
    echo "no status: the checkout has uncommitted changes"
    return 0
  fi
  if [[ "$state" == success && "$whole" == false ]]; then
    echo "no status: a pass is reported only after a whole run (macos and ios, without --quick)"
    return 0
  fi
  if ! command -v gh > /dev/null || ! gh api "repos/$REPO/commits/$sha" --silent 2> /dev/null; then
    echo "no status: ${sha:0:9} is not on $REPO (push the branch, then run again or report by hand)"
    return 0
  fi
  if gh api "repos/$REPO/statuses/$sha" -f state="$state" -f context="$CONTEXT" \
    -f description="${description:0:140}" --silent; then
    echo "status \"$CONTEXT\" on $REPO@${sha:0:9}: $state"
  else
    echo "no status: GitHub refused it"
  fi
}
report
[[ "$state" == success ]]
