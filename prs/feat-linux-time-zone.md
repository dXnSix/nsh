# linux: Report the device's time zone

> **Upstream (checked 2026-11-01):** in the prepared time zone PR (not opened), this platform's commit.

Branch: `feat/linux-time-zone` on `github.com/dXnSix/nsh`, stacked on `feat/time-zone` (`core: Tell apps the
device's time zone`, #50). One commit, `3e4d8140`: `linux: Report the device's time zone`. Opened as a pull request into the
fork's `main`; once #50 merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`timeZone()` (`Environment::time_zone`) read `UTC` on Linux whatever the system's zone.

### Changes

- **`system.rs`**: the environment carries, in this order:
  - the process's own zone, `TZ`, when it names a zone (`Europe/Berlin`, `:Europe/Berlin`, a zoneinfo path,
    `UTC`; not a POSIX rule such as `CET-1CEST`, as `nsh-js`'s `Intl` decides too): the process's local times follow
    it, whatever the system's;
  - the system's zone as systemd's `timedated` keeps it (`org.freedesktop.timedate1`'s `Timezone` on the system bus),
    followed live: a change of the setting (the desktop's settings, `timedatectl`) comes as a property change, and
    every surface reads its environment again;
  - else the zone `/etc/localtime` links to, else `/etc/timezone`'s (Debian's).
- Without a system bus or `timedated` (containers, a CI's virtual display) the zone comes from `/etc`.
- **Docs**: the Environment row of `docs/dev/linux.md` and the module doc of `system.rs`.

## Public API

None: `timeZone()` / `time_zone()` (#50) now report Linux's zone.

## Tests

- `cargo fmt --all --check`: clean.
- The new functions, in a crate of their own with `gio`/`glib` 0.22.10 (the locked versions), checked by
  `cargo clippy -- -D warnings` on macOS: clean.
- Not built or run on Linux by hand: the fork's CI (the Linux workflow) builds and tests it.
