# The fork's features upstream

Checked 2026-11-01 against every pull request of `dXnSix` on longbridge/nsh (51: 31 merged, 14 open, 6 closed) and
against upstream's `main` (`4f2502cd`), not against these records' older notes. The fork's CI changes, its release
workflow and its tests of fork-only code are left out: they never go upstream.

## Not upstream yet

Checked 2026-11-04 against upstream `main` `fe6f54f7` (#911), with the fork's `main` at `5544b26c` (#121, upstream
`9cb48de5`): line by line, everything the fork's `main` adds over upstream is in a branch below (#114's browser
environment included), but for the fork's own CI. The environment stack, register-font, the web URLs and location
are rebased onto `fe6f54f7` (#869's prelude names and testing docs, #921's import map in `browser/runtime.js`) and
pushed to the fork; the #746 follow-ups stay on `43026fb2` and merge cleanly. Locally at the three tips: clippy over
the workspace has only upstream's own error (`nsh-platform-web/src/encode/input.rs`, #776), and the web runtime's JS
tests pass. Fork CI passed at every head below (2026-11-05: CI paths, Core, Linux, Windows, GPUI, Rust
application, Compat, and Fork CI's macOS, iOS, Android and web jobs on the `tmp/*-fork-ci` twins; 81 runs, three
re-run once for flakes outside the branches' code: an Android emulator that did not boot, two iOS gallery timeouts,
a GPUI Windows field wait). Nothing is opened: the owner says when.

| Feature | Records | Branch, head | Opens after |
|---|---|---|---|
| Web apps' lifecycle | `upstream-web-lifecycle` | `pr/web-lifecycle/01-web` `ca871a22` | — |
| The device's time zone | `upstream-environment-02-time-zone` | `pr/environment/02-time-zone` `a861dbee` | web lifecycle |
| The user's preferred languages | `upstream-environment-03-languages` | `pr/environment/03-languages` `e9a13c88` | time zone |
| The user's regional preferences | `upstream-environment-04-regional` | `pr/environment/04-regional` `567f4d28` | languages |
| Register fonts while the app runs | `upstream-register-font` | `pr/register-font/01-core` `c0b7835a` | — |
| The page at the app's own paths | `upstream-web-urls-01-spa-fallback` | `pr/web-urls/01-spa-fallback` `bd0d2fec` | — |
| Web pages' base (`web.base`) | `upstream-web-urls-02-base` | `pr/web-urls/02-base` `c15ac2ae` | SPA fallback |
| Pages as history entries (`web.history`) | `upstream-web-urls-03-history` | `pr/web-urls/03-history` `3e061422` | base |
| The app's location and navigation | `upstream-location-01-core` | `pr/location/01-core` `e33581d2` | history |
| Location on iOS | `upstream-location-02-ios` | `pr/location/02-ios` `fd70a1c3` | location core |
| Location on Android | `upstream-location-03-android` | `pr/location/03-android` `a84ae66e` | location iOS |
| One media type table (#746 nit 3) | side notes `pr-body-core-media-types.md` | `pr/read-assets-nits/01-core` `dbf55778` | — |
| Bundled files by path alone (#746 nits 1, 2) | side notes `pr-body-engine-asset-urls.md` | `pr/read-assets-nits/02-engine` `718a8261` | media types |
| The browser runtime's fetch module (#746 nit 4) | side notes `pr-body-web-fetch-module.md` | `pr/web-fetch-module/01-web` `19908e9d` | — |

Superseded: `upstream-web-open-url` (upstream #892 fixed it; the fork's own dXnSix/nsh#113 is closed). The
`fork-0.4.0-1` sync fixes (dXnSix/nsh#154–#157, checked 2026-11-07 against `c015f2c5e`) are records only:
`upstream-xtask-android-builtin-headers` (#1189 fixed it), `upstream-web-terminal-part` (#1152),
`upstream-linux-accessible-order` (#1176), and `upstream-web-keyboard-context-menu`, which upstream does not need
(#1011's `button === -1` passes the fork's Linux Chrome test without it; open #1098 handles the keys in `keyDown`).
Pairs that conflict, so whichever merges second is
rebased: the environment stack with register-font (`prelude.rs`), with location (the environment's parts) and
with the #746 follow-ups (`platform.rs`); register-font with location and with the engine follow-up; the engine
follow-up with location (`nsh-dev/src/server.rs`).

## Open upstream

#928 (2026-11-04, opened on the owner's go): the web's screen effects at full speed, `fix/web-screen-effect-frames`
`f802404f` on upstream `9cb48de5` (records: side notes `pr-body-web-screen-effect-frames.md`).

Merged: #745, #746, #747, #752, #754, #763–#771, #773 and #823.

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
