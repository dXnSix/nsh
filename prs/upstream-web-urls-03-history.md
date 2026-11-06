# Upstream PR: cli: Let nsh.json say whether a web app's pages are browser history entries

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-urls/03-history-v2` (`1056a6263`), on `pr/web-urls/02-base-v2` (PR 7's new branch): the change
  `dff9c0464`, the first version's `3e061422d` cherry-picked onto PR 7's `8fcee613f` (on `upstream/main`
  `39784ec2b`); its four conflicts were the lines #981 changed (the `package` hunk's `let base`, which #981 moved up,
  the `docs/dev/web.md` paragraph and the `nsh.json` table's `web` row in en and zh-CN, beside main's new `updater`
  / `ios` / `android` / `macos` rows). Then `1056a6263` merges PR 7's merge of `upstream/main` `e1163cd78`:
  conflicts in the packaged page's runtime import (#997 fills in the specifier whole: `"{{runtime}}"`, kept, with
  this PR's comment) and the Web page's "Going back" (#996's shown stacks, kept, with this PR's `web.history`
  sentence after it; en and zh-CN).
- Size: 9 files, +62 / −16 (code +55 / −11 with its test, docs +7 / −5).
- Title: `cli: Let nsh.json say whether a web app's pages are browser history entries` (unchanged).
- Fork CI twin: dXnSix/nsh#149 (`tmp/web-urls-03-history-v2-fork-ci`: `a21563797`, the change on PR 7's, + the
  same fork-only commit as #148; then `34b2e2263`, the branch's merge merged in).
- It uses none of PR 7's code: `WebConfig::base` and the pages' lines it sits beside came with #981, and its diff
  applies to `upstream/main` without PR 7 (checked on `39784ec2b` and `e1163cd78`). It stays on PR 7's branch for the series' order
  (the location PR needs both). To open it on its own instead: `git cherry-pick dff9c0464` onto `upstream/main`, a
  new branch name, and a twin of its own.
- History: the first version, `pr/web-urls/03-history` (`3e061422d`, on the first 02), fork PR dXnSix/nsh#110.

---

**Depends on:** #<PR 7> (`web: Load an app's files and images from where its page started`), for the series' order
only: this PR uses none of its code, and its diff applies to `main` without it. **Needed by:** the location PR of this
series (`web.history: false` also keeps the app's location out of the address bar).

## Description

The player's `history` option (pushed pages and open dialogs as browser history entries, by default only where the app
is the whole page) can only be set by a page that makes its own player: the packaged page and the dev server's never
pass it, so an app cannot keep the browser's back button out of its pages, or use it inside an iframe.

`nsh.json` `web.history` now reaches `createPlayer({ history })` on both pages: the packaged page reads it from its
`app` object, the dev page from `HISTORY`, which `with_page_host` fills in with the app's host module. Without it the
player's default stays.

The Web page's "Going back" and the `nsh.json` reference (en and zh-CN) and `docs/dev/web.md` say so.

## Public API

- `nsh.json`: `web.history` (boolean, optional): whether pushed pages and open dialogs are browser history entries
  on the packaged page and the dev server's page; without it, the player's default (entries when the app is the
  whole page, none in an iframe).

## How to Test

- `cargo test -p nsh-cli --bin nsh -- web::the_dev_page_runs_the_apps_page_host` (`HISTORY` filled in: `null`
  without the key, `false` passed through) and `web::tests::the_dev_page_knows_its_runtime` (no `{{…}}` left on
  the dev page but the two `with_page_host` fills in).
- By hand, in Chromium, with a test app that pushes a page from a `NavStack`:
  - Packaged without `web.history`: the push adds a browser history entry (`history.length` 5 → 6,
    `history.state.nsh` 0 → 1).
  - Packaged with `"web": { "history": false }` (the page's `app` has `"history":false`): the push shows the page
    and leaves `history.length` and `history.state` (`null`) as they were.
  - `nsh run <app> --target web` with `"history": false`: the dev page has `const HISTORY = false;` and the push
    leaves the history as it was too.
