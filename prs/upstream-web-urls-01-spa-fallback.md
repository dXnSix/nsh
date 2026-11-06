# Upstream PR: dev: Serve the page at the app's own paths, as hosts of single-page apps do

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-urls/01-spa-fallback` (`c7aaf02cc`), on `upstream/main` `183b9b7f`: one commit, the fork's
  `feat/spa-fallback` cherry-picked unchanged.
- Size: 6 files, +174 / −11 (code +56 / −11, tests +93, docs +25).
- Title: `dev: Serve the page at the app's own paths, as hosts of single-page apps do`.
- Before opening: the fork's CI passes on `feat/spa-fallback` (dXnSix/nsh), and the branch's own
  `cargo test -p nsh-dev --features server,client` and clippy pass on upstream's head.
- The fork's PR: dXnSix/nsh#108.
- 2026-11-04: rebased onto `upstream/main` `57b1d7ba`: head `97b90b36`. Upstream's precompressed copies (`.br` / `.gz`) reached the same handler: the fallback's `index.html` is served compressed too, and the doc comment names both.
- 2026-11-04 (later): rebased onto `upstream/main` `fe6f54f7` (#911) without conflicts: head `bd0d2fec`.
- 2026-11-05: rebased onto `upstream/main` `90a7efd3` (#951), patch unchanged: head `dfd39954`. Fork PR dXnSix/nsh#128.
- 2026-11-05: **opened as longbridge/nsh#981** at `dfd39954` (fork CI dXnSix/nsh#134: green but Fork CI's macOS job, upstream's #907 dropping typed keys), on the owner's go.
- 2026-11-05: **review fix pushed** for huacnlee's comment (nested routes could not start the app: the pages'
  relative files resolved under `/sports/`), on the owner's go: head `0f01f245`, one commit on `dfd39954`. The dev
  page has `<base href="/">`; nsh.json `web.base` (moved in from `pr/web-urls/02-base`'s `c15ac2aef`, as the
  stack's session decided) gives a packaged page its `<base href>` and `--run` its path; nsh-cli tests load the
  generated pages at nested URLs and fetch every file they name. Fork CI dXnSix/nsh#139: green but `macos` (#907's
  typed keys) and `android` (duplicate Mounter case from #931 / #946). Description updated (Public API: `web.base`,
  `SiteServer::start_at`), title kept; reply: https://github.com/longbridge/nsh/pull/981#issuecomment-6660141720.
  02 still holds the moved hunks and must drop them.
- 2026-11-05 (later): huacnlee's re-review of `0f01f245`: the nested-routes comment addressed, merge after main is
  merged and CI is green; asked whether a new nsh.json key fits nsh-compat. **Merged `upstream/main` `f7afd23f`**
  (a merge, not a rebase: conflicts only in `project-structure.md` en / zh-CN, the `web` row beside main's new
  `updater` row), pushed on the owner's go: head `09db70a4`. Fork CI dXnSix/nsh#139 on twin `debfa602`: green but
  `macos` (dock, form's typed keys, gallery-rs storage) and `web` (#978's Shift+F10, fixed by longbridge/nsh#1017).
  Reply (compat: nsh.json keys are build inputs, nothing for the record to freeze):
  https://github.com/longbridge/nsh/pull/981#issuecomment-6671027924. The desktop app's conflict merge took the
  fork's `origin/main` (twice): aborted both. 02 now builds on `09db70a4`.
- 2026-11-06: huacnlee's review of `09db70a4` ([P2]): `web.base` `/apps//counter/` passed `page_base` into
  `<base href>` while `start_at` collapsed it, so `--run`'s URL answered 404. Fixed in `f7402524` (`page_base`
  refuses empty segments and runs before the runtime build; `start_at` drops them in one pass), pushed on the
  owner's "push now" before fork CI finished: dXnSix/nsh#139 on twin `46f9b11d` had Core, Compat, Rust application,
  Examples, Windows and GPUI green, `macos` / `web` the known failures, `android` an emulator that disconnected
  (rerun pending), Linux `check`, `ios` and Fork CI `web` still running. Public API lines updated; reply:
  https://github.com/longbridge/nsh/pull/981#issuecomment-6672676743.
- 2026-11-06 (later): huacnlee's review of `f7402524` ([P2]): `/apps/%2e/counter/` and `/应用/counter/` passed
  `page_base` while Chromium normalizes / percent-encodes them and `SiteServer` matched the string as written.
  Fixed in `7fd017b1`: `web.base` takes only the form browsers send as written (unreserved characters and `%XX`
  escapes, no dot, escaped-dot or empty segments; other characters percent-encoded), tested against WHATWG
  resolution (`url`) and a real socket under `/%E5%BA%94%E7%94%A8/counter/`, checked by hand in Chromium. Pushed on the
  owner's "push now" with fork CI dXnSix/nsh#139 (twin `fd199d36`) just started. Public API line updated; reply:
  https://github.com/longbridge/nsh/pull/981#issuecomment-6673209367.
- 2026-11-06 (later): huacnlee's source re-review of `7fd017b1`: no new blocker; asked for the cause of the red
  Core `Test` (nsh-base `date_picker::on_a_phone_with_a_time_...` timed out; Core passed on the same tree in the
  fork; a flake, rerun asked: this account has pull access only) and `web` (#978's Shift+F10, fixed by #1017), and
  for exact-head browser checks: dev page, `web.base` `/` and a percent-encoded base, all at nested paths in
  Chromium. Reply: https://github.com/longbridge/nsh/pull/981#issuecomment-6673897314.

---

**Depends on:** no other PR. **Needed by:** the PR that keeps the page's files loading after the app moves the
address with `history.pushState` (`player.baseURL`) and, through it, the history and location PRs of the same series.

## Description

A web app that shows its own paths in the address bar (`/sports/football`) cannot be linked to or reloaded there: the
dev server and `nsh package --target web --run` answer such a path with a 404, as no file has it.

Both now answer a browser loading a path that names no file with the page: a `GET` (or `HEAD`) whose last path segment
has no extension and whose `Accept` header asks for `text/html`, the rule static hosts apply to single-page apps. Files
keep their URLs, and the app's own requests (`fetch` accepts `*/*`) still get their 404.

The page names its files with relative URLs (`./nsh.js`, `./nsh.<digest>.js`), which at a path with several segments
resolve under `/sports/`, where nothing is. Both pages now load them from the app's base:

- The dev server's page has `<base href="/">`: the server serves its files at the root.
- A packaged page is relative to wherever it is deployed, which no path it is answered at tells it, so `nsh.json`
  `web.base` names the path its site is served under (`/`, `/apps/counter/`). The page gets a `<base href>` first in
  its head, and `nsh package --target web --run` serves the site under it (`/` redirects there). The page's script
  resolves the app's URLs against the base as it starts, and the head's requests are keyed the same way. Without
  `web.base` the page's URLs stay relative, so a site still works from any directory (the website's gallery under
  `/play/gallery/`).

The Web page's "Deploy it" says the rule, how nginx, Netlify, Cloudflare Pages and GitHub Pages apply it, and
`web.base`; the `nsh.json` reference lists the key (en and zh-CN). `docs/dev/tooling.md` and `docs/dev/web.md` name
them.

## Public API

- `nsh.json` `web.base` (string: a path from the host's root, a `/` added at its end; no query, fragment, quotes or
  `..`): the packaged page's `<base href>`, and the path `nsh package --target web --run` serves the site under.
- `nsh_dev::SiteServer::start_at(addr: SocketAddr, dir: PathBuf, base: &str) -> io::Result<SiteServer>`: serves a
  site under a path; `SiteServer::start` is `start_at` with `/`.

## How to Test

- `cargo test -p nsh-cli --bin nsh -- web::`: `the_dev_page_loads_its_files_at_the_apps_paths` and
  `a_packaged_page_loads_its_files_at_the_apps_paths` serve the generated pages over real sockets (the dev server's
  page with the runtime's own `nsh.js` and browser host, a wasm-bindgen glue and a page host; a packaged site with
  `web.base` `/` and `/apps/counter/`), load them at `/`, `/football`, `/sports/football` and
  `/sports/football/?tab=live`, and fetch every file each page names (tags, imports, the head's requests, the app's
  URLs) and every module those import, resolved as a browser resolves them, `<base>` included: each must answer 200.
  Without a base, a packaged page at `/sports/football` misses its files. `the_base_is_a_path_from_the_root`:
  normalizing and refusals of `web.base`, the `<base>` first in the head.
- `cargo test -p nsh-dev --features server,client`: `server::tests::browsers_loading_the_apps_paths_get_the_page`
  (the rule), `the_web_host_serves_its_page_at_the_apps_paths`, `a_packaged_site_serves_its_page_at_the_apps_paths`
  and `a_site_deployed_under_a_path_is_served_there` (the page at the app's paths, files and the app's requests still
  missing, `/` redirecting to the base).
- By hand, in Chromium: `nsh run examples/gallery --target web` opened at `/sports/football`; the gallery packaged
  with `"web": { "base": "/" }` and with `"/apps/gallery/"` and served with `--run`, opened at
  `/sports/football` and `/apps/gallery/sports/football`; and packaged without `web.base`, served under
  `/play/gallery/`.
