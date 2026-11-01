# ios: Report the device's time zone

> **Upstream (checked 2026-11-01):** in the prepared time zone PR (not opened), this platform's commit.

Branch: `feat/ios-time-zone` on `github.com/dXnSix/nsh`, stacked on `feat/time-zone` (`core: Tell apps the device's
time zone`, #50). One commit, `f302d0e5`: `ios: Report the device's time zone`. Opened as a pull request into the fork's
`main`; once #50 merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`timeZone()` (`Environment::time_zone`) read `UTC` on iOS whatever the device's zone.

### Changes

- **`surface.rs`**: the environment carries the system's zone, `NSTimeZone.systemTimeZone`'s IANA name
  (`Europe/Berlin`), read whenever the environment is published.
- It follows `NSSystemTimeZoneDidChangeNotification` (the user changed the setting, or the device moved to another
  zone with automatic time zones on): Foundation caches the system's zone until `resetSystemTimeZone`, so the
  observer resets it and publishes the environment again.
- **Workspace**: `objc2-foundation`'s `NSTimeZone` feature.

## Public API

None: `timeZone()` / `time_zone()` (#50) now report iOS's zone.

## Tests

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features
  nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`: clean.
- iPhone 17 Pro Max simulator (iOS 26), `nsh test --target ios` with a probe app showing `timeZone()`: it shows the
  Mac's zone (`Europe/Zagreb`, which the simulator takes), and `open`'s `timeZone: "Asia/Tokyo"` holds over it.
- Not run: a change of the zone while the app runs (the simulator takes the Mac's zone when it boots).
