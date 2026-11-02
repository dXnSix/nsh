# Upstream PR: cli: Let nsh.json say whether a web app's pages are browser history entries

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-urls/03-history` (`3eefff386`), on `pr/web-urls/02-base`: one commit, the fork's `feat/web-history`
  cherry-picked unchanged.
- Size: 9 files, +62 / −16.
- Title: `cli: Let nsh.json say whether a web app's pages are browser history entries`.

---

**Depends on:** #<base> (fork: dXnSix/nsh#109) (`web: Load the page's files from where it started, …`): the same `WebConfig`, packaged page
and `dev.html` lines. **Needed by:** the location PR (`web.history: false` also keeps the app's location out of the
address bar).

## Description

The player's `history` option (pushed pages and open dialogs as browser history entries, by default only where the app
is the whole page) can only be set by a page that makes its own player: the packaged page and the dev server's never
pass it, so an app cannot keep the browser's back button out of its pages, or use it inside an iframe.

`nsh.json` `web.history` now reaches `createPlayer({ history })` on both pages: the packaged page reads it from its
`app` object, the dev page from `HISTORY`, which `with_page_host` fills in with the app's host module. Without it the
player's default stays.

## Public API

- `nsh.json`: `web.history` (boolean): whether pushed pages and open dialogs are browser history entries on the
  packaged page and the dev server's (default: when the app is the whole page, not in an iframe).

## How to Test

- `cargo test -p nsh-cli --bin nsh -- web::the_dev_page_runs_the_apps_page_host` (`HISTORY` filled in, `false`
  passed through).
- By hand: the counter packaged with `"web": { "history": false }` leaves `history.state` untouched.
