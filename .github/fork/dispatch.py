"""The fork's pull requests run every upstream workflow, whatever they change.

Upstream's Rust workflows run only when a pull request changes what they
build (their `paths`, or a `dorny/paths-filter` step, as Compat's). The fork
wants each of its pull requests checked on every platform, so this starts,
on the pull request's branch, each workflow its own `pull_request` event did
not run: those whose paths the change does not touch. The ones it touched
already run, and run once. Workflows disabled in the fork (Website,
Packages) and the fork's own (`fork-*.yml`) are left out.

Usage: dispatch.py <base sha> <head branch>  (at the root of a checkout of
the head, with `gh` signed in as the workflow's token, `actions: write`).
The script itself may come from another checkout: a `pr/*` branch, cut from
upstream, has no `.github/fork/`.
"""

import fnmatch
import json
import pathlib
import subprocess
import sys

import yaml

ROOT = pathlib.Path.cwd()


def changed(base):
    out = subprocess.run(
        ["git", "diff", "--name-only", f"{base}...HEAD"],
        check=True,
        capture_output=True,
        text=True,
        cwd=ROOT,
    ).stdout
    return [line for line in out.splitlines() if line]


def patterns(workflow):
    """The paths a workflow's `pull_request` runs for; `None`: every change."""
    on = workflow.get(True) or {}
    event = on.get("pull_request")
    if isinstance(event, dict) and event.get("paths"):
        return event["paths"]
    for job in (workflow.get("jobs") or {}).values():
        for step in job.get("steps", []):
            if step.get("uses", "").startswith("dorny/paths-filter"):
                filters = yaml.safe_load(step["with"]["filters"])
                return [p for listed in filters.values() for p in listed]
    return None


def matches(path, globs):
    # GitHub's `**` spans directories; fnmatch's `*` does too, so a
    # `dir/**` pattern matches every file under `dir`.
    return any(fnmatch.fnmatch(path, glob) for glob in globs)


def active():
    """The workflows the fork runs (its disabled ones are left out)."""
    out = subprocess.run(
        ["gh", "workflow", "list", "--all", "--json", "path,state"],
        check=True,
        capture_output=True,
        text=True,
        cwd=ROOT,
    ).stdout
    return {w["path"] for w in json.loads(out) if w["state"] == "active"}


def main():
    base, head = sys.argv[1], sys.argv[2]
    files = changed(base)
    running = active()
    started = []
    for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
        name = f".github/workflows/{path.name}"
        if path.name.startswith("fork-") or name not in running:
            continue
        workflow = yaml.safe_load(path.read_text())
        on = workflow.get(True) or {}
        if "pull_request" not in on or "workflow_dispatch" not in on:
            continue
        globs = patterns(workflow)
        if globs is None or any(matches(f, globs) for f in files):
            print(f"{path.name}: runs for this pull request already")
            continue
        subprocess.run(
            ["gh", "workflow", "run", path.name, "--ref", head], check=True, cwd=ROOT
        )
        started.append(path.name)
        print(f"{path.name}: started on {head}")
    print(f"started {len(started)} workflow(s)")


main()
