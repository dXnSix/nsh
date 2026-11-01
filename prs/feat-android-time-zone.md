# android: Report the device's time zone

> **Upstream (checked 2026-11-01):** in the prepared time zone PR (not opened), this platform's commit.

Branch: `feat/android-time-zone` on `github.com/dXnSix/nsh`, stacked on `feat/time-zone` (`core: Tell apps the
device's time zone`, #50). One commit, `a30e0c36`: `android: Report the device's time zone`. Opened as a pull request into
the fork's `main`; once #50 merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`timeZone()` (`Environment::time_zone`) read `UTC` on Android whatever the device's zone.

### Changes

- **`NshView.java`**: the view keeps the system's zone (`java.util.TimeZone.getDefault().getID()`, read when it is
  attached) and sends it with the environment, a new `timeZone` argument of `Native.environment`.
- It follows `Intent.ACTION_TIMEZONE_CHANGED` (the user's setting, or a new zone with automatic time zones), a
  receiver registered while the view is attached: the broadcast names the new zone (`EXTRA_TIMEZONE`), before the
  process's default zone may have caught up. The broadcast is the system's alone, so the receiver needs no export
  flag; an embedding app's own default zone (`TimeZone.setDefault`) is left alone.
- **`host.rs`**: `Native.environment` takes the zone's ID (`TimeZone::parse`; IDs that are no zone read `UTC`).

## Public API

None: `timeZone()` / `time_zone()` (#50) now report Android's zone. `Native.environment` is the runtime's own.

## Tests

- `cargo xtask clippy-android`: clean.
- Android 35 emulator (arm64), `nsh test --target android` with a probe app showing `timeZone()`: it shows the
  emulator's zone (`Europe/Zagreb`); `open`'s `timeZone: "Asia/Tokyo"` holds over it; and with
  `adb shell cmd alarm set-timezone Asia/Tokyo` while the app runs, it shows `Asia/Tokyo`.
