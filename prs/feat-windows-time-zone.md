# windows: Report the device's time zone

> **Upstream (checked 2026-11-01):** in the prepared time zone PR (not opened), this platform's commit.

Branch: `feat/windows-time-zone` on `github.com/dXnSix/nsh`, stacked on `feat/time-zone` (`core: Tell apps the
device's time zone`, #50). One commit, `20e2ac7e`: `windows: Report the device's time zone`. Opened as a pull request into
the fork's `main`; once #50 merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`timeZone()` (`Environment::time_zone`) read `UTC` on Windows whatever the setting.

### Changes

- **`system.rs`**: the environment carries the zone of the system's setting, read with the other settings:
  `GetDynamicTimeZoneInformation` gives its Windows key name (`W. Europe Standard Time`), and the ICU that Windows
  ships maps it to the IANA name (`ucal_getTimeZoneIDForWindowsID`, ICU's default mapping: `Europe/Berlin`).
- ICU is loaded at run time (`icu.dll` since Windows 10 1903, else `icuin.dll`, from System32 only), not linked: an
  app starts where neither is, and its zone reads `UTC` there. `GetDynamicTimeZoneInformation` reads the setting as
  it is now, unlike ICU's own default zone, which a process keeps from its start.
- **`window.rs`**: `WM_TIMECHANGE`, which a change of the zone sends, reads the settings again, as
  `WM_SETTINGCHANGE` does.
- **Workspace**: the `windows` crate's `Win32_System_Time` feature.
- **Docs**: the Environment row of `docs/dev/windows.md` and the module doc of `system.rs`.

## Public API

None: `timeZone()` / `time_zone()` (#50) now report Windows's zone.

## Tests

- `cargo fmt --all --check`: clean.
- The new functions, in a crate of their own with `windows` 0.62 (the same features), checked by
  `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings` on macOS: clean.
- Not built or run on Windows by hand: the fork's CI (the Windows workflow) builds and tests it.
