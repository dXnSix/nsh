# core: Tell apps where the user is in them, and let them navigate

> **Fork:** dXnSix/nsh#111 (opened 2026-11-02, Auto-fix on). **Upstream:** prepared, not opened.

Branch: `feat/location` on `github.com/dXnSix/nsh`, on `feat/web-history`. One commit of its own:
`core: Tell apps where the user is in them, and let them navigate`. Merged into the fork's `main` with a merge commit.
Design: [design-location](design-location.md) (approved by the owner on 2026-11-02). The upstream branches are
`pr/location/01-core` (core, engine, nsh-js, testing, web), `pr/location/02-ios`, `pr/location/03-android` (drafts:
[upstream-location-01-core](upstream-location-01-core.md) and its siblings).

**Depends on:** `feat/web-history` (and through it `feat/web-base`, `feat/spa-fallback`): the location is the page's
address after `web.base`, deep links reach the page through the SPA fallback, and `web.history: false` keeps the
address bar the page's.

## Summary

Apps had no way to know or set where the user is in them: a web app's address bar stayed at the page it loaded, a link
into it or a reload could not open the page it named, and an embedding app could neither open a page of the app (a deep
link it handled) nor follow the app's moves. Routers built on NSH had to reach for the browser's history themselves,
which the player already owns for its pages and dialogs.

`location()` (TS: an accessor of `{ url, pathname, search, hash }`) is where the user is: a path from the app's base with
its query and fragment, `/sports/football?tab=live#top`, the same value on every platform. `navigate(url, { history })`
resolves a URL against it, moves it at once and tells the platform (`Command::Navigate` on the root); the platform's
moves come back as `EventKind::Location` (the web's back and forward) or through the embedding app's channel. The
location is a signal of its own in `Ambient`, so the environment stays `Copy`.

### Changes

- **`nsh-core`**: `location` module (`Location`: `root`, `parse`, `join`, `as_str`, `path`, `query`, `fragment`;
  `InvalidLocation`; `HistoryBehavior::{Push, Replace}`; `NavigateOptions`; `Navigation`), `element::location()`,
  `element::navigate(url, options)` (prelude), `Command::Navigate`, `EventKind::Location` (appended; internal),
  `Core::location`, `set_location`, `take_navigations`; the core notes every root `Command::Navigate` it applies, from
  Rust or JS.
- **`nsh-engine`**: `HostChannel::set_location`, `location`, `handle_navigate`, `remove_navigate_handler`;
  `Message::Location`; the start location (the channel's), the app's navigations handed to the handler after each
  commit round, platform moves noted, and the location published to JS when it changed outside it
  (`Script::set_location`, `BrowserHost::set_location`, `Services::location`).
- **`nsh-js`**: `location` and `navigate` in `nsh` (declarations: `Location`, `NavigateOptions`).
- **`nsh-platform-web`**: the browser engine's `environment.js` (`location`, `navigate` through call 406); the
  environment's `location` at the start; `navigate` commands; `nsh.js`'s history rewritten around entries
  `history.state.nsh = { session, index, depth, url }`: pages, dialogs and navigations are entries alike, one engine
  turn makes one entry, back closes a dialog first then goes back a page and to the entry's location (an app that
  navigates as it goes back keeps its location), forward moves the app to the entry's location and the entry takes the
  page the app shows for it, a reload adopts the earlier load's entries, a new run starts again at the first entry with
  the location. `createPlayer`'s `location` and `onNavigate`, `player.location`, `player.setLocation(url, { history })`.
  Without the history (`web.history: false`, an iframe) the player touches neither the history nor the address bar.
- **`nsh-platform-ios`**: `nsh_app_set_location`, `nsh_app_location`, `nsh_app_set_navigate_handler`;
  Swift `NshAppView.location`, `onNavigate`.
- **`nsh-platform-android`**: `NshView.setLocation`, `getLocation`, `setNavigateHandler` (`NavigateHandler`);
  `Native.LOCATION = 70` and the JNI event key `location`.
- **`nsh-testing`, `nsh-dev`, `nsh-cli`**: `open({ location })`, `app.location`, `app.navigate(url)`, `app.forward()`
  (`--target web` only, else `ERR_NSH_TEST_UNSUPPORTED`); `TestEnvironment::location`, `TestStep::Forward`, `Navigate`,
  `Location`; the agent's `Native::forward`, `navigate`, `location` (the web's over CDP) and `Scene::set_location`,
  `location` (the embedding app's channel elsewhere).
- **Docs** (en and zh-CN): "Locations" in Navigation, "Location" in `nsh`, the web's history and player options,
  testing, "The app's location" in iOS and Android; `docs/dev` elements, embedding, web, tooling, js-modules.
- **Compat**: `modules/mod-location` (`covers [nsh] location`, `covers [nsh] navigate`).

## Public API

- Rust (`nsh-core`): `nsh_core::location::{Location, InvalidLocation, HistoryBehavior, NavigateOptions, Navigation}`;
  `element::location() -> Location`; `element::navigate(url: &str, options: impl Into<NavigateOptions>) -> Result<(),
  InvalidLocation>`; `Command::Navigate(Navigation)`; `EventKind::Location`; `Core::location()`,
  `Core::set_location(Location)`, `Core::take_navigations(&mut Vec<Navigation>)`.
- Rust (`nsh-engine`): `HostChannel::set_location(&str) -> Result<(), InvalidLocation>`, `HostChannel::location() ->
  Location`, `HostChannel::handle_navigate(impl Fn(Navigation))`, `HostChannel::remove_navigate_handler()`;
  re-exports `HistoryBehavior`, `InvalidLocation`, `Location`, `Navigation`; `browser::Services::location`,
  `browser::Host::set_location`.
- TSX (`nsh`): `location: Accessor<Location>`, `navigate(url: string, options?: NavigateOptions): void`, `interface
  Location { url, pathname, search, hash }`, `interface NavigateOptions { history?: "push" | "replace" }`.
- `nsh/test`: `OpenOptions.location`, `App.location`, `App.navigate(url)`, `App.forward()`.
- `nsh_dev::protocol`: `TestEnvironment::location`, `TestStep::{Forward, Navigate { location }, Location}`.
- Web player: `createPlayer({ location, onNavigate })`, `player.location`, `player.setLocation(url, { history })`;
  `history.state.nsh` is now `{ session, index, depth, url }` (it was the depth, with `nshSession` beside it).
- iOS: `nsh_app_set_location`, `nsh_app_location`, `nsh_app_set_navigate_handler`; `NshAppView.location`,
  `NshAppView.onNavigate`.
- Android: `NshView.setLocation(String)`, `NshView.getLocation()`, `NshView.NavigateHandler`,
  `NshView.setNavigateHandler(NavigateHandler)`.

## Tests

- `location::tests` (`nsh-core`): parsing, joining, normalizing, refusals.
- `location::rust_components_follow_and_move_the_location`, `location::js_apps_follow_and_move_the_location`
  (`nsh-engine`).
- `open_and_navigate_set_where_the_app_is` (`nsh-testing`, `tests/testing/location`).
- `nsh.test.mjs`: the history rules on a fake session history (one entry per turn, dialog then location on back,
  forward to a URL entry, forward to a closed dialog, the app's own pops, an app that navigates as it goes back, a
  reload, `setLocation` and `history: false`).
- `NSH_COMPAT_APP=modules cargo xtask compat`.
- On real platforms: see the validation notes below.
