# windows: Report the user's preferred languages

> **Upstream (checked 2026-11-01):** in the prepared languages PR (not opened), this platform's commit.

Branch: `feat/windows-languages` on `github.com/dXnSix/nsh`, stacked on `feat/languages` (`core: Tell apps the
user's preferred languages`, #63). One commit, `083835e4`: `windows: Report the user's preferred languages`. Opened as a
pull request into the fork's `main`; once #63 merges, it is its own commit only. Not opened upstream: the owner
decides and posts there.

## Summary

`languages()` (`Environment::languages`) read `[en-US]` on Windows whatever the user's list.

### Changes

- **`system.rs`**: the system's settings carry the user's display languages, most preferred first
  (`GetUserPreferredUILanguages` with `MUI_LANGUAGE_NAME`: Settings > Time & language > Language & region), the
  locale alone when it lists none. They are read again with the other settings on `WM_SETTINGCHANGE`.
- **Docs**: the Environment row of `docs/dev/windows.md` and the module doc of `system.rs`.

## Public API

None: `languages()` (#63) now reports Windows's list.

## Tests

- `cargo fmt --all --check`: clean.
- The new function, in a crate of its own with `windows` 0.62 (`Win32_Globalization`), checked by
  `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings` on macOS: clean.
- Not built or run on Windows by hand: the fork's CI (the Windows workflow) builds and tests it.
