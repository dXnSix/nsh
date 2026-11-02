# Design: the app's location (`location()` / `navigate()`)

Status: approved by the owner (2026-11-02); built on `feat/location`. Feature 4 of the web and navigation series; stacks on
`feat/web-history` (3), `feat/web-base` (2) and `feat/spa-fallback` (1). Nothing is written yet.

## What it is

The location is where the user is in the app, as a URL relative to the app's base: `/sports/football?tab=live#top`.
It is one value per surface, the same on every platform: the web shows it in the address bar and keeps it in the
browser's history; an embedding app sets it and hears the app's changes of it. Later, native deep links (URL schemes,
universal links / App Links, desktop protocol handlers) feed the same value; none is built now.

It is not part of `Environment` (which stays `Copy`): the core keeps it as a signal of its own in `Ambient`, as the
design tokens are (`tokens: Option<Signal<Arc<Tokens>>>`).

## API

### Rust (`nsh-core`, new `location.rs` next to `open_url.rs`; readers in `element/ambient.rs`, prelude)

```rust
/// "/sports/football?tab=live#top": a path from the app's base, with its query and fragment, normalized as
/// URLs are (percent-encoding, `.` / `..` resolved). Clone, cheap (Arc<str> + offsets).
#[non_exhaustive] #[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location { .. }
impl Location {
    pub fn root() -> Location;                                     // "/" (also Default)
    pub fn parse(url: &str) -> Result<Location, InvalidLocation>;  // must start with "/"
    pub fn join(&self, url: &str) -> Result<Location, InvalidLocation>; // "football", "?tab=2", "#top", "../x"
    pub fn as_str(&self) -> &str;        // the whole location
    pub fn path(&self) -> &str;          // "/sports/football"
    pub fn query(&self) -> Option<&str>; // "tab=live"
    pub fn fragment(&self) -> Option<&str>; // "top"
}
/// What `navigate` does to the history (the DOM Navigation API's `history`).
#[non_exhaustive] #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistoryBehavior { #[default] Push, Replace }
/// `navigate`'s options; `From<HistoryBehavior>`, so a call reads `navigate("/x", HistoryBehavior::Replace)`.
#[non_exhaustive] #[derive(Clone, Debug, Default)]
pub struct NavigateOptions { pub history: HistoryBehavior }

pub fn location() -> Location; // reactive: reading it in a binding, computed or effect subscribes
pub fn navigate(url: &str, options: impl Into<NavigateOptions>) -> Result<(), InvalidLocation>;
```

`InvalidLocation`: a URL with a scheme or an authority (`https://…`, `//host/…`; other sites are `open_url`'s), one
over `open_url::MAX_URL_LEN`, or control characters. `navigate` resolves `url` against the current location
(`Location::join`), sets the signal at once (code after it reads the new location in the same turn) and sends
`Command::Navigate { location, replace }` on the root. A navigation to the location the app is at does nothing.

`ops.rs`: `Command::Navigate { location: String, replace: bool }` next to `OpenUrl`; `EventKind::Location =
"onLocation"` appended after `FileRequest` (root only, payload `location`: the platform moved the app; in
`host::INTERNAL_EVENTS`; `Native.java` 70; the web's and the JNI's payload key lists get `location`).

### TypeScript (`nsh`; nsh-js `env.rs`, the browser host's `environment.js` / `runtime.js`)

```ts
/** Where the user is in the app: a URL from the app's base. Reading it subscribes. */
export const location: Accessor<Location>;
interface Location { readonly url: string; readonly pathname: string; readonly search: string; readonly hash: string }
// url "/sports/football?tab=live#top", pathname "/sports/football", search "?tab=live", hash "#top" (DOM URL's names)

/** Goes to `url` in the app (relative to the current location); throws a TypeError for a URL with a scheme. */
export function navigate(url: string, options?: { history?: "push" | "replace" }): void;
```

`navigate` is a Rust `#[module(flat)]` function of the `nsh` registry (as `openUrl`), so it works on QuickJS and on
the browser's engine alike, with a generated declaration. `location` is an accessor next to `timeZone` (its own
signal, as `tokens_signal`); on the browser engine `environment.js` gets it from the environment the engine sends.

### `nsh/test`

- `open(entry, { location: "/sports" })`: the location the app starts at (headless; pinned on real platforms through
  `TestEnvironment::location` in the dev protocol, no protocol version bump: an optional field).
- `app.location` (getter): the location the platform has for the app, a string (`"/sports/football"`).
- `await app.navigate(url)`: the platform moves the app there, as a link into the app, the embedding app or the
  address bar would (`onLocation`).
- `await app.forward()`: `TestStep::Forward` next to `TestStep::Back`. On the web, `history.forward()` (CDP); the
  other targets and headless reject with a message that says it needs `--target web`: only the browser has a
  forward. `app.back()` stays what it is everywhere (dialogs, then pages); on the web it is still `history.back()`.

Headless keeps no browser history: `back()` does what it does now, the app's navigations move `app.location`, and an
app keeps its location in step with its pages itself (as on the native platforms).

## Embedding (native)

`HostChannel` (nsh-engine), next to `set_params`:

```rust
pub fn set_location(&self, location: &str) -> Result<(), InvalidLocation>; // before start: where the app starts; after: onLocation
pub fn location(&self) -> Location;                                         // the latest, from either side
pub fn handle_navigate(&self, handler: impl Fn(Navigation) + Send + Sync + 'static); // the app's navigate()
pub fn remove_navigate_handler(&self);
#[non_exhaustive] pub struct Navigation { pub location: Location, pub history: HistoryBehavior }
```

The engine hands each `Command::Navigate` to the handler after the commit (as it hands `open_url` requests); the
platforms' own mounts ignore it. Platform faces, where params are set today:

| Platform | Set | Hear |
|---|---|---|
| iOS C ABI / Swift | `nsh_app_set_location(app, url)` / `NshAppView.location` (get, set) | `nsh_app_set_navigate_handler(app, fn(ctx, url, replace), ctx, release)` / `NshAppView.onNavigate: ((String, Bool) -> Void)?`, on the main queue |
| Android | `NshView.setLocation(String)`, `getLocation()` | `NshView.setNavigateHandler(NavigateHandler)` (`void navigate(String location, boolean replace)`), on the UI thread |
| macOS, Windows, Linux, GPUI | `AppView::host().set_location(..)` | `AppView::host().handle_navigate(..)` |

Deep links later: each platform's URL entry point maps `myapp://…/path?q` or `https://example.com/path` to a location
and calls the same `set_location` (initially, or while running). Nothing in the API above changes for that.

## The web player

### Startup

`nsh.js` computes the location from the page's URL as the player starts: `location.href` minus the base's directory
(`player.baseURL`, so `web.base` decides it; `/apps/counter/sports/football` → `/sports/football`), with a leading `/`;
`<base>index.html` reads as `/`. `createPlayer({ location })` overrides it (a page that embeds a player as a widget).
It goes in the first environment report (`environment().location`), which `nsh_environment` hands the engine as the
start location; changes go as `onLocation` events, never through later environment reports.

### History entries

Every entry NSH makes or adopts has `history.state.nsh = { session, index, depth, url }` (the page's own keys in
`history.state` are kept, as now):

- `session`: this page load's id (as `nshSession` now).
- `index`: the entry's position among this load's entries, 0 being the entry the page loaded with.
- `depth`: the back depth at the entry (as now: presented dialogs + pages over the bottom one of each visible stack).
- `url`: the app's location at the entry; the entry's own URL is the base joined with it.

The player mirrors its entries (`this.entries[index] = { depth, url }`) and the current `index`. Depth entries and URL
entries are the same kind of entry: a page push makes one with `depth + 1`, a `navigate` push one with the new `url`,
both in one engine turn one entry with both. The invariant: the current entry describes the app as it is
(`depth` = `backDepth()`, `url` = the location). After each engine turn (`syncHistorySoon`, which coalesces a batch's
stack changes and `navigate` commands) the player restores it:

| Change in the turn | History |
|---|---|
| depth grew by n | n `pushState`s, one per level (back goes back a page or dialog at a time), all with the turn's location |
| same depth, `navigate` push | one `pushState` |
| same depth, `navigate` replace, or a location set from outside | `replaceState` of the current entry |
| depth shrank (the app popped pages or closed a dialog itself) | `history.go` back to the newest earlier entry whose depth is at most the new one, then the rules above from there (a push in the same turn is an entry after it, the location is written into it) |

Our own `history.go` is awaited: popstates while it travels are its own, and syncs wait for it (today's `ignoredPops`
counter becomes one pending traversal).

### popstate (the user's back, forward, or the back button's menu)

On an entry of this load (`nsh.session` is ours), from entry `from` to entry `to`:

1. When `to.depth < from.depth` (back through dialogs and pages): one `back()` per level, as now — a dialog closes
   first, then pages pop (the user's decision). The app answers before the location moves: the player waits until the
   popped pages' `onDismiss` reached the app (they come after the page's slide; 1.5 s at most). If the app moved its
   location itself meanwhile (a page that navigates to its parent, RayX's pattern), the entry takes the app's
   location; otherwise the app goes to the entry's location (`onLocation`).
2. Otherwise (same depth, or forward): `onLocation` with `to.url` at once when it differs from the location.
3. At the end of the turn, the invariant again:
   - the location moved (by the app or to the entry's): the app has shown what the URL names; the entry takes the
     app's depth and location (`replaceState`). Forward therefore works for URL entries: the app pushes its page again
     for the URL, and that page is this entry's.
   - same URL, the app is deeper (it kept the user: `popOnBack={false}`, a dialog that does not close): the levels get
     their entries again (as now).
   - same URL, the app is shallower (forward to a dialog or page that is gone and that no URL brings back): back to
     where the app is (as now).

(Implementation note, found on a real browser: a decision taken right after `back()` saw the location before the app's
`onDismiss` answer, because that answer follows the page's motion.)

On an earlier load's entry (a reload keeps the browser's entries): the entry the page reloads on tells the old session
and index, so that session's entries are adopted at their offset (their `url` is honoured as in 2, their depth is the
app's after it); entries of older loads, the page's own entries and entries without `nsh` state stay as now: the app
goes back to its start and they are passed over. A hash typed in the address bar (a new entry without state) is
adopted as a URL entry after the current one.

A hot reload or another app on the same player (`resetHistory`) goes back to entry 0 and writes the current location
into it, so the app starts again where the developer was.

### `web.history: false` (`createPlayer({ history: false })`)

NSH leaves the browser's history and address bar to the page: no entries, no `replaceState`, no `popstate`. The
location is the app's own: it starts at the page's URL (or `createPlayer`'s `location`), `navigate` moves it inside the
app, and the page takes part as an embedding app does: `player.location`, `player.setLocation(url)` (→ `onLocation`)
and `createPlayer({ onNavigate({ location, history }) })`. `onNavigate` is called with `history: true` too, after the
player's own history step. So an iframe or a widget in a page never writes the embedding page's URL or history, and a
page that wants the address bar to follow does so with `onNavigate`.

## Docs and tests

- Docs: `website/docs/navigation.md` (a "Locations" section: reading, navigating, the web's address bar and history,
  embedding), `device-and-environment.md`, `web.md` (entries, `web.history: false`), `testing-and-trace.md`,
  `ios.md` / `android.md` / `gpui.md` (embedding), `website/modules/nsh.md`; every page with its zh-CN twin; dev docs
  `elements.md`, `embedding.md`, `web.md`, `tooling.md`.
- Tests: `Location` parsing and joining (core); the reactive reader and `navigate` (core, engine `rust_platform`); the
  host channel's `set_location` / `handle_navigate` (engine); the player's history rules with a fake `history`
  (`nsh.test.mjs`: one entry per turn, dialog then route on back, forward to a URL entry, the app's own pops, reloads);
  `nsh/test` headless (`location`, `navigate`, `open({ location })`); `nsh test --target web` (real back and forward
  through pages, a dialog and URL entries); macOS / iOS / Android with `app.navigate` and `app.location` through the
  embedding path; a compat case in `modules` (`covers [nsh] location`, `covers [nsh] navigate`).

## Upstream

One PR for the core, nsh-js, engine, testing and the web (the feature cannot be used without the player), then one per
platform embedding face (iOS, Android), as the environment series was split. Depends on the three web PRs before it.
Upstream has no router, URL, history or deep-link issue or PR (checked 2026-11-02).

## Follow-ups seen on the way (not part of this)

- `nsh_event`'s payload key list on the web has no `"opened"`, so `openUrl` may resolve `false` there
  (`crates/nsh-platform-web/src/lib.rs` ~1053).
- The browser engine's `environment.js` has no `timeZone`, `languages`, `hourCycle` or `appState` (the fork's
  environment series): TSX apps on the web may lack them (`web/browser/environment.js`, `runtime.js:183`).
