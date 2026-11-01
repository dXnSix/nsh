# The fork's features upstream

Checked 2026-11-01 against every pull request of `dXnSix` on longbridge/nsh (51: 31 merged, 14 open, 6 closed) and
against upstream's `main` (`4f2502cd`), not against these records' older notes. The fork's CI changes, its release
workflow and its tests of fork-only code are left out: they never go upstream.

## Not upstream yet

| Feature | Records | State |
|---|---|---|
| Font fallbacks (`fontFamily` lists: the core and Android, iOS, macOS, Windows, GPUI) | [`upstream-font-fallbacks`](upstream-font-fallbacks.md) | prepared 2026-11-01 (`pr/font-fallbacks/01-core`, on `main`); fork runs going |
| Register fonts while the app runs | `upstream-register-font` | rebuilt 2026-11-01 on `main` with #745 and #746 (their latest); fork runs going |
| The device's time zone | `upstream-environment-02-time-zone` | rebuilt 2026-11-01 on #747 without #744; fork runs going |
| The user's preferred languages | `upstream-environment-03-languages` | prepared; stacks on the time zone PR |
| The user's regional preferences | `upstream-environment-04-regional` | prepared; stacks on the languages PR |

## Open upstream

| PR | Title |
|---|---|
| #745 | text: Read WOFF and WOFF2 fonts |
| #746 | engine: Let fetch read the files the app ships |
| #747 | core: Tell apps whether they are in use |
| #752 | testing: Start scrolling a view into view where nothing takes the drag |
| #754 | testing: Keep scrolling into view after a gesture that moves nothing |
| #763 | core: Let pages and dialogs come and go through a shader |
| #764–#770 | {macos, ios, android, web, windows, linux, gpui}: the shader series' platforms |
| #771 | gallery: Show shader effects and transitions on the Shaders page |

## Merged upstream (fork features among them)

#228 wasm-bindgen for an app's crates, #229 image variants for the screen's density, #230 SVG images, #280, #281,
#285, #374, #376 sticky headers and views, #323 Tailwind's browser meanings, #389 diffClamp, #397 the collapsing
header demo, #398 web scroll commands, #404, #405 hidden views and screen readers, #428 the development session's port
(in place of `--strict-port`), #429 the incremental development web runtime, #431 Android density changes, #588–#590
animation evaluation, #675–#681 test and build fixes, #682 shader effects and transitions, #727, #753.

## Closed upstream

| PR | Why |
|---|---|
| #326, #355 | merged through #374's squash |
| #358 | replaced by #374 |
| #360 | replaced by #376 |
| #432 | macOS focus border: upstream already shows it (the maintainer) |
| #744 | environments non-exhaustive: a project decision (Rust API may break before the first recorded release) |

## Not going upstream

The fork's own shader pipeline and its demos (removed when the fork took upstream's `Shader`, #517); Intl's time zone
and hour cycle (the owner's decision); `--strict-port` (upstream's #428 replaces it); the Git source of nsh's crates
(upstream's #213); the iOS clippy allowance (reverted).
