# gpui: Report the user's preferred languages

Branch: `feat/gpui-languages` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the user's
preferred languages`, #63). One commit, `c0f1a649`: `gpui: Report the user's preferred languages`. Opened as a pull
request into the fork's `main`; once #63 merges, it is its own commit only. Not opened upstream: the owner decides
and posts there.

## Summary

`languages()` (`Environment::languages`) read `[en-US]` on the GPUI backend whatever the user's list.

### Changes

- **`system.rs`**: the system's settings carry the user's preferred languages, most preferred first, read as the
  native backends read them, declared as the module's other system calls are:
  - Windows: `GetUserPreferredUILanguages` (the display languages);
  - macOS: `CFLocaleCopyPreferredLanguages` (System Settings' list);
  - Linux: gettext's list (`posix_languages`): `LANGUAGE` (unless the messages' locale is `C`), then the first of
    `LC_ALL`, `LC_MESSAGES` and `LANG`, each with its language alone after it. GPUI has no GLib.
  - the locale alone when a system lists none.
- They are read with the other settings: on appearance changes and window activation (Windows, macOS), and on the
  desktop services' changes (Linux).
- **Docs**: the Environment row of `docs/dev/gpui.md` and the module doc of `system.rs`.

## Public API

None: `languages()` (#63) now reports the GPUI backend's list.

## Tests

- `system::tests::posix_languages_list_languages_then_messages_with_their_fallbacks` (new).
- In `crates/gpui`: `cargo clippy --all-targets --features nsh-platform-gpui/dev -- -D warnings` and
  `cargo test -p nsh-platform-gpui --features shot` (201 passed) on macOS: clean.
- The Windows function, in a crate of its own, checked by `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings`:
  clean.
- macOS 26, `nsh test --target gpui` with a probe app showing `languages()`: it shows this Mac's list (`en-HR,en`),
  and `open`'s `languages: ["ja-JP", "fr"]` holds over it.
