"""Which of Fork CI's jobs a pull request needs (`fork-ci.yml`, job `changes`).

Each job runs only when the pull request changes what it builds. The lists
of paths are upstream's: the `paths` of the workflow each job's commands
come from (`core.yml` for `core`, `linux.yml` for `linux`, and so on), read
from the checkout, so they follow upstream's as it keeps them (its
`.github/ci/paths.py` checks them against the dependency graph). The jobs
with no upstream workflow (`android`, `web`, `tizen`, and `apple`, the
local check of `.github/fork/apple.sh`) add their crates to the shared
ones. A change to the fork's own CI files runs every job, and so does a
list that cannot be read. The local checks (`apple`: macOS and iOS,
`emulator`: Android's examples' tests) follow the scripts in
`.github/fork/`, not the workflows, which they do not run.

Usage: changed.py <file with the changed paths, one a line> | --all
Prints `<job>=true|false` lines for `$GITHUB_OUTPUT`.
"""

import fnmatch
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

# The fork's own CI: a change here is checked by every job of the gate, and
# a change to its scripts by the local checks too.
SCRIPTS = [".github/fork/**"]
EVERYTHING = SCRIPTS + [".github/workflows/fork-*.yml"]
LOCAL = {"apple", "emulator"}


def upstream(name):
    """The `paths` of an upstream workflow's `pull_request` event."""
    text = (ROOT / ".github/workflows" / name).read_text()
    block = re.search(r"\n  pull_request:\n(.*?)\n  [a-z_]+:", text, re.S)
    paths = re.findall(r'^\s+- "([^"]+)"$', block.group(1), re.M) if block else []
    if not paths:
        raise ValueError(f"{name}: no pull_request paths")
    return paths


def areas():
    core = upstream("core.yml")
    gpui = upstream("gpui.yml") + core
    rust_app = upstream("rust-app.yml")
    android = core + ["crates/nsh-platform-android/**", "crates/xtask/**"]
    return {
        "core": core,
        "linux": upstream("linux.yml"),
        "windows": upstream("windows.yml"),
        "gpui": gpui,
        "tui": upstream("tui.yml"),
        "examples": upstream("examples.yml"),
        "rust-app": rust_app,
        # The examples' tests without a platform, as Rust application's
        # `web` job runs them.
        "headless": rust_app,
        "android": android,
        "emulator": android,
        "web": core + ["crates/nsh-platform-web/**", "crates/nsh-cli/**"],
        "tizen": gpui,
        "apple": core + [
            "crates/nsh-platform-apple/**", "crates/nsh-platform-macos/**", "crates/nsh-platform-ios/**",
            "crates/nsh-macos-app/**", "crates/nsh-macos-host/**",
            "crates/nsh-ios-app/**", "crates/nsh-ios-host/**",
            "crates/nsh-cli/**", "examples/gallery-rs/**", "examples/rust-exports/**",
        ],
    }


def matches(path, globs):
    # GitHub's `**` spans directories; fnmatch's `*` does too.
    return any(fnmatch.fnmatch(path, glob) for glob in globs)


def main():
    try:
        jobs = areas()
    except (OSError, ValueError) as error:
        print(f"changed.py: {error}: every job runs", file=sys.stderr)
        jobs, files = dict.fromkeys(NAMES, []), None
    else:
        files = None if sys.argv[1] == "--all" else [
            line for line in pathlib.Path(sys.argv[1]).read_text().splitlines() if line
        ]
    # GitHub lists at most 3000 files of a pull request: a longer list is cut.
    if files is not None and len(files) >= 3000:
        files = None
    for name in NAMES:
        if files is None:
            run = True
        else:
            ci = SCRIPTS if name in LOCAL else EVERYTHING
            run = any(matches(f, ci) or matches(f, jobs[name]) for f in files)
        print(f"{name}={'true' if run else 'false'}")


NAMES = ["core", "linux", "windows", "gpui", "tui", "examples", "rust-app", "headless", "android", "web",
         "tizen", "apple", "emulator"]

main()
