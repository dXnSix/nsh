# Upstream PR: core: Tell apps where the user is in them, and let them navigate

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/location/01-core`, on `pr/web-urls/03-history`: the fork's `feat/location` without the iOS and Android
  SDK faces (those are `pr/location/02-ios` and `pr/location/03-android`). Android's `Native.LOCATION` constant, its
  JNI event key and the backends' no-op arms for `Command::Navigate` stay here: they keep the platforms in step with
  the core (Android's command encoder matches every command).
- Rebuilt on upstream's code, which has no time zone, languages or regional preferences: the `TestEnvironment`,
  `OpenOptions`, `nsh-js` parts and docs tables take `location` beside what upstream has, `lifecycle()` (#747)
  included (`Part::Location` after `Part::Lifecycle`, 12 parts).
- Design: [design-location](design-location.md) (the history rules and the embedding API).
- Title: `core: Tell apps where the user is in them, and let them navigate`. Until the three web PRs are merged, GitHub
  shows their commits too.
- Before opening: the fork's CI passes on `feat/location`, and this branch's own tests and clippy pass on upstream's
  head (Fork CI's Windows / Linux / GPUI workflows dispatched on it).
- The fork's PR: dXnSix/nsh#111.

---

**Depends on:** #<history> (`cli: Let nsh.json say whether a web app's pages are browser history entries`), and through
it #<base> and #<spa-fallback>: the location is the page's address after `web.base`, a link into the app reaches the
page through the SPA fallback, and `web.history: false` keeps the address bar the page's. **Needed by:** the iOS and
Android PRs of the location.

## Description

Apps cannot know or set where the user is in them: a web app's address bar stays at the page it loaded, a link into it
or a reload cannot open the page it names, and an embedding app can neither open a page of the app (a deep link it
handled) nor follow the app's moves. A router built on NSH would have to reach for the browser's history itself, which
the player already owns for its pages and dialogs.

`location()` is where the user is: a path from the app's base with its query and fragment,
`/sports/football?tab=live#top`, the same value on every platform. `navigate(url, { history })` resolves a URL against
it, moves it at once (code after the call reads the new one) and tells the platform with `Command::Navigate` on the
root; the platform's moves come back as `EventKind::Location` or through the embedding app's channel. It is a signal of
its own in `Ambient`, so the environment stays `Copy`. A turn that navigates twice moves what reads it to each location
in turn and never back: the core sets the location to the turn's last navigation once, as the turn commits, and
setting the location the app is at runs nothing again.

- **Core and engine**: `nsh_core::location` (`Location`, `HistoryBehavior`, `NavigateOptions`, `Navigation`), the
  readers, the root command and event; `HostChannel::set_location`, `location` and `handle_navigate`: the start
  location, the app's navigations handed to the embedding app after each commit, and the location published to JS when
  it changes outside it. The desktops' embedding apps reach it through `AppView::host()`.
- **The web's player** keeps the location (the page's address after the base) and writes it into the browser's
  history. Every entry carries `history.state.nsh = { session, index, depth, url }`, so pushed pages, open dialogs and
  navigations are entries alike, and one engine turn makes one entry. Back closes an open dialog first, then goes back
  a page; the app answers first (a page that navigates to its parent as it goes keeps its location), else it goes to
  the entry's location. Forward moves the app to the entry's location, and the page the app shows for it becomes that
  entry's. A reload adopts the earlier load's entries. Without the history (`web.history: false`, an iframe) the
  player touches neither the history nor the address bar: the page follows with `onNavigate` and moves the app with
  `player.setLocation`.
- **TSX on the browser's engine** has the same exports (`environment.js`).
- **`nsh/test`**: `open({ location })`, `app.location`, `app.navigate(url)`, and `app.forward()` with `--target web`
  (elsewhere it rejects with `ERR_NSH_TEST_UNSUPPORTED`: only the browser goes forward).

Native back buttons and gestures pop pages and close dialogs as they do and leave the location to the app, on every
platform; the docs show the pattern (a page's `onDismiss` navigates to its parent with `history: "replace"`).

## Public API

- `nsh` (TSX): `location: Accessor<Location>`; `navigate(url: string, options?: NavigateOptions): void`;
  `interface Location { url, pathname, search, hash }`; `interface NavigateOptions { history?: "push" | "replace" }`.
- `nsh_core`: `location::{Location, InvalidLocation, HistoryBehavior, NavigateOptions, Navigation}`;
  `element::location() -> Location`, `element::navigate(url: &str, options: impl Into<NavigateOptions>) -> Result<(),
  InvalidLocation>` (prelude); `Command::Navigate(Navigation)`; `EventKind::Location` (`onLocation`, internal);
  `Core::location`, `Core::set_location`, `Core::take_navigations`.
- `nsh_engine`: `HostChannel::set_location(&str) -> Result<(), InvalidLocation>`, `HostChannel::location() ->
  Location`, `HostChannel::handle_navigate(impl Fn(Navigation) + Send + Sync + 'static)`,
  `HostChannel::remove_navigate_handler()`; re-exports of the location types; `browser::Services::location`,
  `browser::Host::set_location`.
- `nsh/test`: `OpenOptions.location`, `App.location`, `App.navigate(url)`, `App.forward()`.
- `nsh_dev::protocol`: `TestEnvironment::location`, `TestStep::{Forward, Navigate { location }, Location}`; the test
  setup's environment is boxed (`Box<TestEnvironment>`).
- The web player: `createPlayer({ location, onNavigate })`, `player.location`, `player.setLocation(url, { history })`;
  its history entries' state is `{ nsh: { session, index, depth, url } }` (it was `{ nsh: depth, nshSession }`).

## How to Test

- `cargo test -p nsh-core --lib location`, `cargo test -p nsh-engine --test engine location`,
  `cargo test -p nsh-testing --features js,bundle --test testing open_and_navigate`.
- `node --test crates/nsh-platform-web/web/nsh.test.mjs` (the history rules on a fake session history).
- `NSH_COMPAT_APP=modules cargo xtask compat` (`mod-location`).
- A router driven by the location (pages `/`, `/teams`, `/teams/7`, a dialog on each): `nsh test --target web` goes
  through the dialog, the pages and their locations with back and forward, and opens a link into the app; the same
  app's native steps pass headless and with `--target macos` and `--target ios`.
