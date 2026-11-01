# ios: Report the user's preferred languages

> **Upstream (checked 2026-11-01):** in the prepared languages PR (not opened), this platform's commit.

Branch: `feat/ios-languages` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the user's
preferred languages`, #63). One commit, `8d5bffba`: `ios: Report the user's preferred languages`. Opened as a pull request
into the fork's `main`; once #63 merges, it is its own commit only. Not opened upstream: the owner decides and posts
there.

## Summary

`languages()` (`Environment::languages`) read `[en-US]` on iOS whatever the user's list.

### Changes

- **`surface.rs`**: the environment carries `NSLocale.preferredLanguages`, Settings' list (with the app's own
  language first when the user chose one for it in the app's settings), read as the environment is published; the
  region format's locale when the list names none.
- iOS restarts apps when the user changes the list (or an app's own language), so nothing follows it while the app
  runs.

## Public API

None: `languages()` (#63) now reports iOS's list.

## Tests

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features
  nsh-platform-ios/embed,nsh-platform-ios/dev -- -D warnings`: clean.
- iPhone 17 Pro Max simulator (iOS 26), its `AppleLanguages` set to `de-DE, en-US` (and put back after),
  `nsh test --target ios` with a probe app showing `languages()`: it shows `de-DE,en-US`, and `open`'s
  `languages: ["ja-JP", "fr"]` holds over it.
