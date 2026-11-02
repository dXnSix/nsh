# cli: Let nsh.json say whether a web app's pages are browser history entries

> **Fork:** dXnSix/nsh#110 (opened 2026-11-02, Auto-fix on). **Upstream:** prepared, not opened.

Branch: `feat/web-history` on `github.com/dXnSix/nsh`, on `feat/web-base`. One commit of its own:
`cli: Let nsh.json say whether a web app's pages are browser history entries`. Merged into the fork's `main` with a
merge commit. The upstream branch is `pr/web-urls/03-history` (draft: [upstream-web-urls-03-history](upstream-web-urls-03-history.md)).

**Depends on:** `feat/web-base` (the same `WebConfig`, page and `dev.html` lines). **Needed by:** `feat/location`
(`web.history: false` also keeps the app's location out of the address bar).

## Summary

The player's `history` option (pushed pages and open dialogs as browser history entries, by default only where the app
is the whole page) could only be set by a page that makes its own player: the packaged page and the dev server's never
passed it, so an app could not keep the browser's back button out of its pages, or use it inside an iframe.

`nsh.json` `web.history` now reaches `createPlayer({ history })` on both pages: the packaged page reads it from its
`app` object, the dev page from `HISTORY`, which `with_page_host` fills in with the app's host module. Without it the
player's default stays.

## Public API

- `nsh.json`: `web.history` (boolean): whether pushed pages and open dialogs are browser history entries on the
  packaged page and the dev server's (default: when the app is the whole page).

## Tests

- `web::tests::the_dev_page_runs_the_apps_page_host` (`HISTORY` filled in, `false` passed through).
- By hand: the counter packaged with `"history": false` leaves `history.state` untouched.
