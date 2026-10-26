# android: Report the user's preferred languages

Branch: `feat/android-languages` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the
user's preferred languages`, #63). One commit, `1048166a`: `android: Report the user's preferred languages`. Opened as a
pull request into the fork's `main`; once #63 merges, it is its own commit only. Not opened upstream: the owner
decides and posts there.

## Summary

`languages()` (`Environment::languages`) read `[en-US]` on Android whatever the user's list.

### Changes

- **`NshView.java`**: the environment carries the configuration's `LocaleList` (`getLocales()`, API 24: the
  user's list, with Android 13's per-app language first when the user chose one), after the embedding app's own
  language when it sets one (`setLocale`), as a new `languages` argument of `Native.environment`. A configuration
  change (the user's list) publishes it again, as it does the locale.
- **`host.rs`**: `Native.environment` takes the comma-separated tags (`Languages::parse`: repeats and tags that name
  no language left out; the locale when none is left).

## Public API

None: `languages()` (#63) now reports Android's list. `Native.environment` is the runtime's own.

## Tests

- `cargo xtask clippy-android`: clean.
- Android 35 emulator (arm64), the development host's app languages set to `de-DE, en-US`
  (`adb shell cmd locale set-app-locales`, cleared after), `nsh test --target android` with a probe app showing
  `languages()`: it shows `de-DE,en-US`, and `open`'s `languages: ["ja-JP", "fr"]` holds over it.
