# web: Load the page's files from where it started, and let nsh.json set its base

Branch: `feat/web-base` on `github.com/dXnSix/nsh`, on `feat/spa-fallback`. One commit of its own:
`web: Load the page's files from where it started, and let nsh.json set its base`. Merged into the fork's `main` with a
merge commit. The upstream branch is `pr/web-urls/02-base` (draft: [upstream-web-urls-02-base](upstream-web-urls-02-base.md)).

**Depends on:** `feat/spa-fallback` (its docs section, and the paths it serves the page at). **Needed by:**
`feat/web-history`, `feat/location`.

## Summary

A web app that moved the address to another directory (`history.pushState` to `/sports/football`) could no longer load
the fonts of a chunk, its images, the runtime's data or its relative image URLs: each was a relative URL, resolved
against wherever the address was when it loaded. A page opened at such a path (a link into the app, a reload there)
could not load at all.

The page now resolves every URL it has once as it starts: the packaged page its `app` URLs (the head's requests are
keyed the same way), the player its `assets`, `data`, `assetBase` and the app's relative image URLs, the browser
runtime the entry, the chunks and the app realm's `<base>`, all against `document.baseURI` as the player is made
(`player.baseURL`).

`nsh.json` `web.base` names the path a site is served under (`/`, `/apps/counter/`): the packaged page gets a
`<base href>` first in its head, so it loads at every path of the app, and `nsh package --target web --run` serves the
site under it (`SiteServer::start_at`; `/` redirects there). The dev server's page has `<base href="/">`. Without
`web.base` the page's URLs stay relative to the page, as before.

### Changes

- **`nsh-cli`**: `WebConfig::base`, `web::page_base` (a path from the root; a `/` is added at its end; queries,
  fragments, quotes, `..` and other sites refused), the packaged page's `<base>`, its URLs resolved at startup, `--run`
  under the base; `dev.html`'s `<base href="/">`.
- **`nsh-dev`**: `SiteServer::start_at(addr, dir, base)`.
- **`nsh-platform-web`**: `Player.baseURL`, its `assets` / `data` / `assetBase` resolved once, `asset()` resolving
  relative URLs against it; `browser/runtime.js` uses it for the entry, the chunks and the realm's `<base>`.
- **Docs**: `website/docs/web.md` (relative URLs, `web.base`, links into the app), `website/docs/project-structure.md`
  (`web.base`), their zh-CN pages; `docs/dev/web.md`.

## Public API

- `nsh.json`: `web.base` (string, a path from the host's root): the page's `<base href>` and where `--run` serves the
  site.
- `nsh_dev::SiteServer::start_at(addr: SocketAddr, dir: PathBuf, base: &str) -> io::Result<SiteServer>`: a site served
  under a path.
- `createPlayer`: relative URLs of `assets`, `data`, `assetBase` and the app's image URLs resolve against the page's
  `document.baseURI` as the player is made (`player.baseURL`).

## Tests

- `web::tests::the_base_is_a_path_from_the_root` (`nsh-cli`), `roundtrip::a_site_deployed_under_a_path_is_served_there`
  (`nsh-dev`), `nsh.test.mjs` "relative URLs are the page's as the player started".
- By hand: the counter packaged with `"base": "/"` opened at `/sports/football`, and with `"/apps/counter"` at
  `/apps/counter/sports/football?tab=live`; `/` redirects to the base.
