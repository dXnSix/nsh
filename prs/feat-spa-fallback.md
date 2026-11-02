# dev: Serve the page at the app's own paths, as hosts of single-page apps do

Branch: `feat/spa-fallback` on `github.com/dXnSix/nsh`, on the fork's `main` at `57713c3c` (after #106). One commit:
`dev: Serve the page at the app's own paths, as hosts of single-page apps do`. Merged into the fork's `main` with a merge
commit. The upstream branch is `pr/web-urls/01-spa-fallback` (draft: [upstream-web-urls-01-spa-fallback](upstream-web-urls-01-spa-fallback.md)).

**Depends on:** no other PR. **Needed by:** `feat/web-base` (its docs extend this section), then `feat/web-history` and
`feat/location`.

## Summary

A web app that shows its own paths in the address bar (`/sports/football`) could not be linked to or reloaded there:
the dev server and `nsh package --target web --run` answered such a path with a 404, as no file has it.

Both now answer a browser loading a path that names no file with the page: a `GET` (or `HEAD`) whose last path segment
has no extension and whose `Accept` header asks for `text/html` (`nsh_dev::server::wants_page`), the rule static hosts
apply to single-page apps. Files keep their URLs, and the app's own requests (`fetch` accepts `*/*`) still get their
404.

### Changes

- **`nsh-dev`**: `wants_page` (dev server and `SiteServer`); the dev server's fallback route serves the page for it, the
  site server `index.html`.
- **Docs**: "Links into the app" under "Deploy it" in `website/docs/web.md` (and zh-CN): the rule, and how nginx,
  Netlify, Cloudflare Pages and GitHub Pages apply it; `docs/dev/tooling.md`.

## Public API

None (the servers' answers only).

## Tests

- `server::tests::browsers_loading_the_apps_paths_get_the_page` (`nsh-dev`).
- `roundtrip::the_web_host_serves_its_page_at_the_apps_paths`, `roundtrip::a_packaged_site_serves_its_page_at_the_apps_paths`
  (real sockets).
- By hand: a packaged counter opened at `/sports/football` in a browser.
