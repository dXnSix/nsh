# js: Write Intl's times in the user's hour cycle by default

> **Upstream (checked 2026-11-01):** not sent (the owner's decision, 2026-10-30).

Branch: `feat/intl-hour-cycle` on `github.com/dXnSix/nsh`, cut from the upstream sync (`sync/upstream-8c65c8d0`, #78).
One commit, `0bf34b89`. Opened as a pull request into the fork's `main`; once #78 merges, it is its own commit only.
Not opened upstream: the owner decides and posts there.

## Summary

`Intl` wrote times in the locale's own hour cycle, so an app showed a 12-hour clock to a user who had switched to
24-hour time (`hourCycle()` `"h23"`) unless it passed `hourCycle()` itself. As it writes dates in the user's time zone
by default (#61), it now writes the device's locale's times in the user's hour cycle.

### Changes

- **`nsh-js` (`web/intl/mod.rs`)**: when a date format has no `hourCycle` and no `hour12`, and the locales asked for
  are the device's own (none, or its locale first), its hour cycle is the environment's
  (`Environment::regional.hour_cycle`): `toLocaleTimeString()`, `toLocaleString()` and `Intl.DateTimeFormat()` follow
  it, and `resolvedOptions().hourCycle` names it. Another locale's times keep that locale's cycle, and an option wins.
- **Docs** (en and zh-CN): the regional preferences' section of Device and environment (its example no longer passes
  `hourCycle()`), `hourCycle` on the `nsh` page, `Intl`'s dates in Globals, and `hourCycle`'s TS declaration.

## Public API

None new: `Intl`'s default hour cycle for the device's locale is the user's.

## Tests

- `host::tests::intl_writes_times_in_the_users_hour_cycle` (`nsh-js`): in `en-US` with the user's 12-hour time, then
  24-hour time, a time without a locale and `resolvedOptions().hourCycle` follow it; `ja-JP`'s keeps its own 24-hour
  time; `hourCycle: "h12"` wins.
- `cargo clippy -p nsh-js --all-targets -- -D warnings`: clean.
