# linux: Report the user's preferred languages

Branch: `feat/linux-languages` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the user's
preferred languages`, #63). One commit, `9066c0e6`: `linux: Report the user's preferred languages`. Opened as a pull
request into the fork's `main`; once #63 merges, it is its own commit only. Not opened upstream: the owner decides
and posts there.

## Summary

`languages()` (`Environment::languages`) read `[en-US]` on Linux whatever the user's list.

### Changes

- **`system.rs`**: the environment carries GLib's language names (`g_get_language_names`), most preferred first:
  `LANGUAGE`'s list (gettext's), else `LC_ALL`, `LC_MESSAGES` and `LANG`, each with its fallbacks (`de_DE.UTF-8`
  gives `de-DE`, then `de`; `C` is left out), the locale alone when they name none. Read once, as the locale is: a
  process's environment variables do not change while it runs.
- **Docs**: the Environment row of `docs/dev/linux.md` and the module doc of `system.rs`.

## Public API

None: `languages()` (#63) now reports Linux's list.

## Tests

- `cargo fmt --all --check`: clean.
- The new code, in a crate of its own with `glib` 0.22.10 (the locked version), checked by
  `cargo clippy -- -D warnings` on macOS: clean.
- Not built or run on Linux by hand: the fork's CI (the Linux workflow) builds and tests it.
