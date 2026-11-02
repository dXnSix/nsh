# core: Move what reads the location once when a turn navigates twice

Branch: `fix/location-one-update` on `github.com/dXnSix/nsh`, from `main`. One commit:
`core: Move what reads the location once when a turn navigates twice`. Upstream, the fix is folded into
`pr/location/01-core`'s commit (`core: Tell apps where the user is in them, and let them navigate`), whose PR is not
open yet, so upstream never sees the bug; `pr/location/02-ios` and `03-android` are rebased onto it unchanged.

## Description

With more than one `navigate()` in a turn, `location()` went back through the earlier locations as the turn
committed. Rust's `navigate` sets the location at once and sends `Command::Navigate` on the root; the core then set
the location again for each navigation as it applied the turn's ops (`Core::navigated`). So `navigate("/a");
navigate("/b")` ran what read the location for `/a`, `/b`, `/a` and `/b`, and a router following `location()` in an
effect replayed its moves (an app showing four navigations in one turn ran its effect eight times).

The core now keeps the turn's last navigation and sets the location to it once when the turn commits: Rust's
navigations set it already (nothing runs again), and JS's, which are only ops to the Rust side, move it once to the
last. Setting the location the app is at no longer runs what read it, and a move by the platform (the embedding app,
the web's back and forward) drops an app navigation not committed yet. JS's own `location()` was right already: its
`navigate` sets it once per call, and the engine's re-publish skips the location JS is at.

## How to Test

- `cargo test -p nsh-core --lib navigations_in_one_turn`: a Rust component following `location()` in an effect
  navigates to `/a`, then `/b` on a press: the effect sees `/a`, `/b` (before: `/a`, `/b`, `/a`, `/b`); two JS
  navigation ops in one batch move it once, to the last; the platform setting the location the app is at runs nothing.
- `cargo test --workspace`, `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets`.
- Upstream branch: nsh-core's tests, fmt and clippy in its own target directory.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test --workspace`, `cargo +nightly fmt --all` and clippy.
- [x] Changes to what apps use keep what released versions shipped (a fix of the location's update; no API change).
