# The fork's features upstream

Checked 2026-11-01 against every pull request of `dXnSix` on longbridge/nsh (51: 31 merged, 14 open, 6 closed) and
against upstream's `main` (`4f2502cd`), not against these records' older notes. The fork's CI changes, its release
workflow and its tests of fork-only code are left out: they never go upstream.

## Not upstream yet

Checked 2026-11-04 against upstream `main` `57b1d7ba` (#899). Every branch below is rebased onto it, pushed to the
fork and green there (CI paths, Core, Linux, Windows, GPUI, Rust application, and Fork CI's macOS, iOS, Android and
web jobs on a `tmp/*-fork-ci` twin), except Compat, which stops at upstream main's own `checkout_apps_run` failure
(the gallery's step 136, "Bars per month"); each branch's own compat case passes locally. Nothing is opened: the
owner says when.

| Feature | Records | Branch, head | Opens after |
|---|---|---|---|
| Web apps' lifecycle | `upstream-web-lifecycle` | `pr/web-lifecycle/01-web` `32e65431` | — |
| The device's time zone | `upstream-environment-02-time-zone` | `pr/environment/02-time-zone` `f1190540` | web lifecycle |
| The user's preferred languages | `upstream-environment-03-languages` | `pr/environment/03-languages` `c27fc5bd` | time zone |
| The user's regional preferences | `upstream-environment-04-regional` | `pr/environment/04-regional` `4a16f25d` | languages |
| Register fonts while the app runs | `upstream-register-font` | `pr/register-font/01-core` `bd236d4a` | — |
| The page at the app's own paths | `upstream-web-urls-01-spa-fallback` | `pr/web-urls/01-spa-fallback` `97b90b36` | — |
| Web pages' base (`web.base`) | `upstream-web-urls-02-base` | `pr/web-urls/02-base` `f08b9e99` | SPA fallback |
| Pages as history entries (`web.history`) | `upstream-web-urls-03-history` | `pr/web-urls/03-history` `7dc0eda5` | base |
| The app's location and navigation | `upstream-location-01-core` | `pr/location/01-core` `1d4e8c97` | history |
| Location on iOS | `upstream-location-02-ios` | `pr/location/02-ios` `09bc5c9e` | location core |
| Location on Android | `upstream-location-03-android` | `pr/location/03-android` `cadd06f8` | location iOS |
| One media type table (#746 nit 3) | side notes `pr-body-core-media-types.md` | `pr/read-assets-nits/01-core` `dbf55778` | — |
| Bundled files by path alone (#746 nits 1, 2) | side notes `pr-body-engine-asset-urls.md` | `pr/read-assets-nits/02-engine` `718a8261` | media types |
| The browser runtime's fetch module (#746 nit 4) | side notes `pr-body-web-fetch-module.md` | `pr/web-fetch-module/01-web` `19908e9d` | — |
| The web's screen effects at full speed | side notes `pr-body-web-screen-effect-frames.md` | `fix/web-screen-effect-frames` `09d1ea94` | — |

Superseded: `upstream-web-open-url` (upstream #892 fixed it). Pairs that conflict, so whichever merges second is
rebased: the environment stack with register-font (`prelude.rs`), with location (the environment's parts) and
with the #746 follow-ups (`platform.rs`); register-font with location and with the engine follow-up; the engine
follow-up with location (`nsh-dev/src/server.rs`).

## Open upstream

None (2026-11-04): #745, #746, #747, #752, #754, #763–#771, #773 and #823 are merged.

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

The fork's own shader pipeline and its demos (removed when the fork took upstream's `Shader`, #517); `--strict-port` (upstream's #428 replaces it); the Git source of nsh's crates
(upstream's #213); the iOS clippy allowance (reverted).
