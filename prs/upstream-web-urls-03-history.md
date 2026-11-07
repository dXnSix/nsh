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
- 2026-11-06: fork CI twin #149 on `34b2e226`: 15 pass (iOS included), the 5 failures known (#148's four and Android's
  emulator death). PR 7 opened as longbridge/nsh#1155, so the body's first line is filled. **Opened as longbridge/nsh#1158**
  on the user's go (head `1056a626`, mergeable with `main` `7571948e`); the body below is what was posted.
- 2026-11-07: GitHub found it conflicting with `main` `beeb9fc7`; `2512b1702` merges `main` in (pushed, a
  fast-forward; #1155 stayed mergeable and is unchanged). The one conflict was the `nsh.json` table on the Project
  structure page, en and zh-CN: this PR's `web` row (`web.history`) beside #1189's `updater` row (OTA `packages`),
  both kept. The app's Auto-fix had started a merge of the fork's `origin/main` instead, which was aborted. After the
  merge: `nsh-cli`'s `web::` and `nsh_json` tests (30), the node tests (67), fmt and `nsh-cli` clippy pass; #1190's
  lenient `nsh.json` reader takes `web.history` as a known key. Upstream CI on `2512b170`: everything passes (Website
  build and `web` included, now that main fixed #1111's) but Linux's new `gpui-gallery` (#1169): `form.test.tsx`'s
  "Shift with a key types its shifted character" (`expected "A82" to be "A*@"`), which fails the same on `main`
  `beeb9fc7`'s own push run (longbridge/nsh run 42341039969, job 128308614848).
- 2026-11-07: huacnlee's review (5810586075, commented, no inline threads): no blocking defects; `web.history`'s name
  and shape kept (`browserHistory` considered and turned down, the key matches `createPlayer`'s option), the owner to
  decide on the key. Scope: the branch carries #1155's `8fcee61`, so asks to merge #1155 first or drop that commit.
  Dropping it without a force push means a new branch (`git cherry-pick dff9c0464` onto `main`) and a new PR; waiting
  for #1155 and then merging `main` in leaves only the `web.history` change in the diff. The user chose to wait for
  #1155: once it merges, merge `main` into `pr/web-urls/03-history-v2` (a fast-forward push) and check that the diff
  is the `web.history` change alone.

---

**Depends on:** #1155 (`web: Load an app's files and images from where its page started`), for the series' order
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
- Fork CI on every platform (dXnSix/nsh#149: this branch, so PR 7 too, merged with `main` `e1163cd7`, plus the fork's
  CI files and drops of `main`'s #1111 and #901 clippy errors): iOS, Core, Compat, Examples, Windows, GPUI, TUI and
  Rust application's desktop checks pass. What fails is `main`'s own: Linux `check`'s `nsh-platform-linux`
  `view::tests::a_scroll_views_content_is_read_in_screen_order` (#1122) under xvfb, which fails the same on `main`
  `05f77b7a` with only #1111's drop ([run](https://github.com/dXnSix/nsh/actions/runs/42253107970)); the web tests that
  open the gallery's index, which shows nothing on the web on `main`; macOS's `dock.test.tsx` drawer that a press does
  not dismiss (since #907); and Android's emulator, which stops answering after the gallery's switch test runs past
  30 s.
