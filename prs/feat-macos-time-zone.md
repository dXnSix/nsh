# macos: Report the device's time zone

Branch: `feat/macos-time-zone` on `github.com/dXnSix/nsh`, stacked on `feat/time-zone` (`core: Tell apps the device's
time zone`, #50). One commit, `7a4cfe00`: `macos: Report the device's time zone`. Opened as a pull request into the fork's
`main`; once #50 merges, it is its own commit only. Not opened upstream: the owner decides and posts there.

## Summary

`timeZone()` (`Environment::time_zone`) read `UTC` on macOS whatever the Mac's zone.

### Changes

- **`system.rs`**: the environment carries the system's zone, `NSTimeZone.systemTimeZone`'s IANA name, read with the
  system's other settings.
- It follows `NSSystemTimeZoneDidChangeNotification`, installed with the other settings' observers (once per
  thread): Foundation caches the system's zone until `resetSystemTimeZone`, so the observer resets it and every
  surface reads its environment again. An embedded surface (`AppView`) reads the same, the process's.
- **Workspace**: `objc2-foundation`'s `NSTimeZone` feature (the same line as the iOS change, #51).
- **Docs**: the Environment row of `docs/dev/macos.md` and the module doc of `system.rs`.

## Public API

None: `timeZone()` / `time_zone()` (#50) now report macOS's zone.

## Tests

- `cargo clippy --all-targets -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app --features
  nsh-platform-macos/dev -- -D warnings`: clean.
- macOS 26, `nsh test --target macos` with a probe app showing `timeZone()`: it shows the Mac's zone
  (`Europe/Zagreb`), and `open`'s `timeZone: "Asia/Tokyo"` holds over it.
- Not run: a change of the Mac's zone while the app runs (it needs an administrator).
