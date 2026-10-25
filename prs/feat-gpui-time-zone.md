# gpui: Report the device's time zone

Branch: `feat/gpui-time-zone` on `github.com/dXnSix/nsh`, stacked on `feat/time-zone` (`core: Tell apps the device's
time zone`, #50). One commit, `e793e54a`: `gpui: Report the device's time zone`. Opened as a pull request into the fork's
`main`; once #50 merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`timeZone()` (`Environment::time_zone`) read `UTC` on the GPUI backend whatever the system's zone.

### Changes

- **`system.rs`**: the system's settings carry the time zone, as the native backends name it:
  - Windows: the setting's zone (`GetDynamicTimeZoneInformation`'s key name) by its IANA name from the ICU Windows
    ships (`ucal_getTimeZoneIDForWindowsID`, loaded at run time, `UTC` without it), declared as the module's other
    Win32 calls are;
  - macOS and Linux: `TZ` when it names a zone (not a POSIX rule), else the zone `/etc/localtime` links to (which
    macOS keeps current), else `/etc/timezone`'s.
- GPUI reports no change of the zone (nor of most settings): the settings are read again whenever a window becomes
  the active one, as the user comes back from changing it.
- **Docs**: the Environment row of `docs/dev/gpui.md` and the module doc of `system.rs`.

## Public API

None: `timeZone()` / `time_zone()` (#50) now report the GPUI backend's zone.

## Tests

- In `crates/gpui`: `cargo clippy --all-targets --features nsh-platform-gpui/dev -- -D warnings` and
  `cargo test -p nsh-platform-gpui --features shot` (200 passed) on macOS: clean.
- The Windows code, in a crate of its own (with a compile-time check that the structure is Windows' 432 bytes),
  checked by `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings`: clean.
- macOS 26, `nsh test --target gpui` with a probe app showing `timeZone()`: it shows the Mac's zone
  (`Europe/Zagreb`), and `open`'s `timeZone: "Asia/Tokyo"` holds over it.
- Not run: GPUI on Windows or Linux by hand (the fork's CI builds them), a change of the zone while an app runs.
