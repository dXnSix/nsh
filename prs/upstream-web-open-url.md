# Upstream PR: web: Tell apps whether openUrl opened the URL

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

**Superseded (2026-11-04):** upstream fixed the same bug in #892 (`web: Pass every event payload field to the app`,
madcodelife, merged 2026-11-04): `nsh_event` no longer keeps a list of keys, and its `tests/text/events.rs` checks
`opened`. Not to be opened. The fork's own fix, dXnSix/nsh#113, is superseded by #892 once the fork syncs with it.

- Branch: `pr/web-open-url/01-web`, one commit on `upstream/main` `828b1f13` (after #822). The fork's own fix is dXnSix/nsh#113
  (the same commit on the fork's `main`).
- Checked locally: `node --test crates/nsh-platform-web/web/nsh.test.mjs` fails without the one line in `src/lib.rs`
  and passes with it; the player's and the browser runtime's other JS tests; nightly rustfmt. In a page (on the fork):
  `opened: true` when the browser opened the URL, `false` for a blocked pop-up.
- `nsh test --target web` cannot show it: the test agent plays the embedding app and answers `openUrl` itself.
- Title: `web: Tell apps whether openUrl opened the URL`.

---

## Description

On the web `openUrl` always resolved with `{ opened: false }`, even when the browser opened the page. The player
reports the outcome as `onOpenUrl` with `{ id, opened }`, as iOS, Android, macOS, Windows, Linux and GPUI do, but
`nsh_event` (`src/lib.rs`) interns a fixed list of payload keys and `opened` was not on it: the key was dropped, and
the core answered the request as not opened.

`nsh_event` now interns `opened`. The player's test checks both answers (a page opened, a blocked pop-up) and that
`nsh_event` keeps every key of the payload, so a key added to the player but not interned fails there rather than in
apps.

## How to Test

- `node --test crates/nsh-platform-web/web/nsh.test.mjs`: "openUrl answers whether the browser opened the URL, in
  keys the engine reads" fails on `main` and passes with this change.
- In a browser: an app that shows `(await openUrl("https://example.com")).opened` shows `true`; with pop-ups blocked
  for the page, `false`.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed the web's JS tests and `cargo +nightly fmt`.
- [x] Changes to what apps use: `openUrl`'s documented answer on the web; no API changes.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: a web-only fix.
