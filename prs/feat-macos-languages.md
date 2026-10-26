# macos: Report the user's preferred languages

Branch: `feat/macos-languages` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the user's
preferred languages`, #63). One commit, `1a184788`: `macos: Report the user's preferred languages`. Opened as a pull
request into the fork's `main`; once #63 merges, it is its own commit only. Not opened upstream: the owner decides
and posts there.

## Summary

`languages()` (`Environment::languages`) read `[en-US]` on macOS whatever the user's list.

### Changes

- **`system.rs`**: the environment carries `NSLocale.preferredLanguages` (System Settings' list, with the app's
  own language first when the user chose one for it), the locale alone when the list names none. It is read again
  with the system's other settings, on the notifications the module already follows
  (`NSCurrentLocaleDidChangeNotification` among them).
- **Docs**: the Environment row of `docs/dev/macos.md` and the module doc of `system.rs`.

## Public API

None: `languages()` (#63) now reports macOS's list.

## Tests

- `cargo clippy --all-targets -p nsh-platform-macos -p nsh-macos-host -p nsh-macos-app --features
  nsh-platform-macos/dev -- -D warnings`: clean.
- macOS 26, `nsh test --target macos` with a probe app showing `languages()`: it shows this Mac's list
  (`en-HR,en`, as `defaults read -g AppleLanguages` says), and `open`'s `languages: ["ja-JP", "fr"]` holds over it.
- Not run: a change of the list while the app runs (it would change the user's own setting).
