# Upstream PR: web: Load an app's files and images from where its page started

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-urls/02-base-v2` (`122912ded`): the change `8fcee613f`, cut from `upstream/main` `39784ec2b`
  (#1131), then a merge of `upstream/main` `e1163cd78` (`122912ded`, conflicts only in the Web page's "Publish an
  app" paragraph, en and zh-CN, beside #997's new `--asset-url` paragraph: both kept). #997 composes with it: with
  `--asset-url` the page names its files absolutely, which the player leaves as they are, and the app's own relative
  URLs keep resolving against the page, now as it started.
- Size: 6 files, +60 / −22 (code +27 / −11, tests +16, docs +17 / −11).
- Title: `web: Load an app's files and images from where its page started` (was `web: Load the page's files from where
  it started, and let nsh.json set its base`: the base and the page's half went upstream with #981).
- Fork CI twin: dXnSix/nsh#148 (`tmp/web-urls-02-base-v2-fork-ci`: `ad07b51cb`, the change + one fork-only commit
  with Fork CI's files, dXnSix/nsh#143's GStreamer packages, and drops of main's #1111 / #901 clippy errors; then
  `45b87d518`, the branch's merge of main merged in).
- History: the first version, `pr/web-urls/02-base` (`c15ac2aef` on `fe6f54f78` + #981's original commit
  `bd0d2fec7`, fork PR dXnSix/nsh#109), also carried `nsh.json` `web.base`, the packaged page's `<base href>` and
  URL resolution, the dev page's `<base href="/">` and `SiteServer::start_at`. #981's review fix moved those in
  (2026-11-05), and #981 merged as `897fd9c2d` (2026-11-06), so v2 is cut from `upstream/main` with only what #981
  did not take: the player (`nsh.js`), the browser runtime (`browser/runtime.js`), the node test and their docs
  lines. `browser/dom.js`'s `metaUrl` (an app module's `import.meta.url`, new since) reads the realm's
  `document.baseURI`, which the realm's `<base>` (now `player.baseURL`) fixes, so it needed no change. The branch
  is new so nothing is force pushed; `pr/web-urls/02-base` stays as it was.
- 2026-11-06: fork CI twin #148 on `45b87d51`: 16 pass (iOS, Android included), the 4 failures `main`'s own (Linux
  `check`'s #1122 test under xvfb, which fails the same on `main` `05f77b7a` with only #1111's drop, dXnSix/nsh run
  42253107970; the web gallery's empty index twice; macOS's dock drawer press, #907). **Opened as longbridge/nsh#1155**
  on the user's go (head `122912de`, mergeable with `main` `6edf36a5`); the body below is what was posted.
- 2026-11-07: huacnlee's review (5810472887, commented, LGTM, no inline threads) found `docs/dev/web.md`'s "the
  browser's own `fetch` resolves the app's against the page" ambiguous. `1257aa9c0` (`docs: Say the app's fetch
  resolves against its realm's base`) says the app realm's `fetch` resolves against its `<base>` (`player.baseURL`)
  and a `fetch` in the page's own realm (the host module's) against the page's address; the Web pages already said
  "the page's URL as the app started" and stay. Twin #148 on `24938b83`: iOS, Android and the rest pass, its four
  failures the same as on `45b87d51` (the web gallery's, #1122, #907). Pushed as a fast-forward on the user's go
  (relayed by the coordinator), and replied on the PR (issuecomment-6706402387) naming the commit and the red
  checks as `main`'s: `build`'s "every element page names a demo" and "the gallery's components are base pages, by
  name", `web`'s two `device.test.tsx` tests; #1158, with this change merged with `main` `beeb9fc7`, passes both.

---

**Depends on:** no open PR (#981, merged, gave the pages their base). **Needed by:** the location PR of this series,
whose navigations move the address of pages without `web.base`.

## Description

A page with no `<base href>` (a site packaged without `nsh.json` `web.base`, a page that makes its own player) whose
address moves to another directory while the app runs (`history.pushState` to `/sports/football`, by the page's
router or the app) loses the files the player loads after that: the player resolved its URLs each time it used one,
against the address at that moment. The runtime's data (`data`: the syntax definitions an `Editor` fetches when it
first highlights), the files under `assetBase` and the app's relative image URLs (`<Image src="photos/a.jpg" />`)
then load from `/sports/`, and the browser runtime builds the import map and the app realm's `<base>` of an app run
later there too.

The player now takes the page's `document.baseURI` once as it is made (`player.baseURL`) and resolves those against
it: `assets`, `data` and `assetBase` as it starts, the app's relative image URLs in `asset()`, and in
`browser/runtime.js` the entry, the chunks and the realm's `<base>`. The engine already got the page's base once for
relative requests (`nsh_set_base_url`) and now gets the same value. Pages with a `<base href>` (`web.base`, the dev
server's page) behave as before, and the packaged page already resolves its own `app` URLs as it starts (#981).
With `--asset-url` (#997) the page names its files absolutely, which the player leaves as they are; the app's own
relative URLs keep resolving against the page, as #997 has them, now as the page started.

`createPlayer`'s comment, the Web page's "Publish an app", "Embed it in a page" and "Networking" (en and zh-CN), and
`docs/dev/web.md` say so.

## Public API

- `createPlayer` (`nsh.js`): relative URLs in `assets`, `data` and `assetBase`, and the app's relative image URLs,
  resolve against the page's `document.baseURI` as the player is made, which `player.baseURL` (string) keeps,
  rather than against the address when each one loads.

## How to Test

- `node --test crates/nsh-platform-web/web/nsh.test.mjs`: "relative URLs are the page's as the player started"
  (relative, root-relative, absolute, `blob:` and `asset:/` URLs, with and without the page's `assets` map).
- By hand, in Chromium, with a test app whose button shows `<Image src="pic.png" />` and pushes a page, packaged
  with `nsh package --target web`:
  - Without `web.base`, served at `/`: after `history.pushState({}, "", "/deep/path/here")` the image loads from
    `/pic.png`. The same site with `main`'s `nsh.js` and `browser/runtime.js` asks for `/deep/path/pic.png` (404).
  - `"web": { "base": "/" }` served with `--run`, opened at `/sports/football` and moved to `/a/b/c/d`; `"base":
    "/apps/urls/"` opened at `/apps/urls/sports/football?tab=live` and moved to `/apps/urls/x/y/z`: every file the
    page loads (the runtime, the app, the browser runtime, the image) answers 200 from the base.
- `nsh test examples/counter --target web` passes; of the gallery's tests that open a fixture rather than the index,
  `network`'s "reads the files the app ships" (the app's assets), `list-search`, `mention`, `input-shrink`,
  `keyboard-taps`, `gallery-ui`, `data-table-columns` and `selection-change` pass in headless Chrome.
  `error-boundary`'s "a failing binding" expects QuickJS's error message and fails the same with `main`'s files.
- Fork CI on every platform (dXnSix/nsh#148: this branch merged with `main` `e1163cd7`, plus the fork's CI files and
  drops of two of `main`'s own clippy errors, #1111's and #901's): iOS, Android, Core, Compat, Examples, Windows, GPUI,
  TUI and Rust application's desktop checks pass. What fails is `main`'s own: Linux `check`'s `nsh-platform-linux`
  `view::tests::a_scroll_views_content_is_read_in_screen_order` (#1122) under xvfb, which fails the same on `main`
  `05f77b7a` alone (dXnSix/nsh run 42253107970); the web tests that open the gallery's index, which shows nothing on
  the web on `main`; and macOS's `dock.test.tsx` drawer that a press does not dismiss (since #907).
- `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets` (only `main`'s #1111, #901 and #776
  warnings) and `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown --features compiler`.
