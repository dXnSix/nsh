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

---

**Depends on:** no other PR. **Needed by:** the `web.base` PR (its docs extend this section) and, through it, the
history and location PRs of the same series.

## Description

A web app that shows its own paths in the address bar (`/sports/football`) cannot be linked to or reloaded there: the
dev server and `nsh package --target web --run` answer such a path with a 404, as no file has it.

Both now answer a browser loading a path that names no file with the page: a `GET` (or `HEAD`) whose last path segment
has no extension and whose `Accept` header asks for `text/html`, the rule static hosts apply to single-page apps. Files
keep their URLs, and the app's own requests (`fetch` accepts `*/*`) still get their 404.

The Web page's "Deploy it" says the rule and how nginx, Netlify, Cloudflare Pages and GitHub Pages apply it (en and
zh-CN); `docs/dev/tooling.md` names it.

## Public API

None: what the two servers answer.

## How to Test

- `cargo test -p nsh-dev --features server,client`: `server::tests::browsers_loading_the_apps_paths_get_the_page`
  (the rule), `the_web_host_serves_its_page_at_the_apps_paths` and `a_packaged_site_serves_its_page_at_the_apps_paths`
  (over real sockets: the page at `/sports/football`, files and the app's requests still missing).
- By hand: `nsh package examples/counter --target web --run`, then open `http://127.0.0.1:4173/football`.
