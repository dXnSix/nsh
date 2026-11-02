# Upstream PR: web: Load the page's files from where it started, and let nsh.json set its base

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-urls/02-base` (`6771891ea`), on `pr/web-urls/01-spa-fallback`: one commit, the fork's
  `feat/web-base` cherry-picked unchanged.
- Size: 14 files, +248 / −37.
- Title: `web: Load the page's files from where it started, and let nsh.json set its base`. Until the SPA fallback PR
  is merged, GitHub shows its commit too.

---

**Depends on:** #<spa-fallback> (`dev: Serve the page at the app's own paths, …`): the paths the page is served at, and
the docs section this one extends. **Needed by:** the `web.history` and location PRs.

## Description

A web app that moves the address to another directory (`history.pushState` to `/sports/football`) can no longer load
the fonts of a chunk, its images, the runtime's data or its relative image URLs: each is a relative URL, resolved
against wherever the address is when it loads. A page opened at such a path (a link into the app, a reload there)
cannot load at all.

The page now resolves every URL it has once as it starts: the packaged page its `app` URLs (the head's requests are
keyed the same way), the player its `assets`, `data`, `assetBase` and the app's relative image URLs, the browser
runtime the entry, the chunks and the app realm's `<base>`, all against `document.baseURI` as the player is made
(`player.baseURL`).

`nsh.json` `web.base` names the path a site is served under (`/`, `/apps/counter/`): the packaged page gets a
`<base href>` first in its head, so it loads at every path of the app, and `nsh package --target web --run` serves the
site under it (`/` redirects there). The dev server's page has `<base href="/">`. Without `web.base` the page's URLs
stay relative to the page, as before, so a site still works from any directory.

## Public API

- `nsh.json`: `web.base` (string, a path from the host's root; a `/` is added at its end): the packaged page's
  `<base href>`, and the path `--run` serves the site under.
- `nsh_dev::SiteServer::start_at(addr: SocketAddr, dir: PathBuf, base: &str) -> io::Result<SiteServer>`: a site served
  under a path (`start` is `start_at` with `/`).
- `createPlayer`: relative URLs of `assets`, `data`, `assetBase` and of the app's images resolve against the page's
  `document.baseURI` as the player is made (`player.baseURL`).

## How to Test

- `cargo test -p nsh-cli --bin nsh -- web::` (`the_base_is_a_path_from_the_root`: normalizing and refusals, the
  `<base>` first in the head), `cargo test -p nsh-dev --features server,client`
  (`a_site_deployed_under_a_path_is_served_there`), `node --test crates/nsh-platform-web/web/nsh.test.mjs`
  ("relative URLs are the page's as the player started").
- By hand: the counter packaged with `"web": { "base": "/" }` opened at `/sports/football`, and with
  `"/apps/counter"` at `/apps/counter/sports/football?tab=live` (`/` redirects to the base).
