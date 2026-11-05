# Upstream PR: web: Give browser apps their lifecycle

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-lifecycle/01-web`, one commit on `upstream/main` `828b1f13` (after #822; #747 added
  `lifecycle()`). The fork's own fix is dXnSix/nsh#114, still under the name `appState` there.
- `pr/environment/02-time-zone` to `04-regional` sit on this branch: each adds its part of the browser runtime and of
  `environment.test.mjs`, which this PR adds. Open this one first.
- Checked locally: `node --test crates/nsh-platform-web/web/browser/environment.test.mjs` fails without the change
  and passes with it; the player's and the browser runtime's other JS tests; nightly rustfmt; the wasm clippy
  (`--features compiler`).
- Title: `web: Give browser apps their lifecycle`.
- 2026-11-04: rebased onto `upstream/main` `57b1d7ba` (#899) without conflicts, patch unchanged: head `32e65431`. Upstream's browser runtime still has no `lifecycle`.
- 2026-11-04 (later): rebased onto `upstream/main` `fe6f54f7` (#911) without conflicts, patch unchanged: head `ca871a22`.
- 2026-11-05: rebased onto `upstream/main` `ddf4af83` (#907) without conflicts, patch unchanged: head `c94a7254`.
- 2026-11-05 (later): rebased onto `upstream/main` `90a7efd3` (#951), patch unchanged: head `4ce3495e`. Fork PR dXnSix/nsh#123 (base `ci/upstream-main`).
- 2026-11-05: **opened as longbridge/nsh#975** at `4ce3495e` (fork CI dXnSix/nsh#129: all green but Fork CI's macOS job, upstream's #907 dropping typed keys), on the owner's go. The body's last line names #976 and #977.
- 2026-11-05: review fix (huacnlee, issue comment 6654614953: `runtime.js` built `nsh/device` from a fixed list
  without `lifecycle`) as a second commit, `072d45ac` `web: Keep the browser's nsh/device namespace to what env.rs
  declares`: `environment.js` groups its accessors as `env.rs` declares them and `runtime.js` serves those groups as
  `nsh/device` / `nsh/accessibility`; `environment.test.mjs` checks the namespaces against `env.rs` (fails on
  `4ce3495e`). Local: that test, the player's JS tests (70), the browser runtime's Wasm tests (76). Fork CI
  dXnSix/nsh#137 (`tmp/review-975-fork-ci`, replaces #129). Not on #975 yet: waiting for CI and the owner's go.
  The stacked branches (#976 `timeZone`, #977 `languages`, 04 `hourCycle`, `firstDayOfWeek`, `measurementSystem`,
  `temperatureUnit`) add `DEVICE_EXPORTS` too: on restack each puts its accessor in `environment.js`'s `device`
  group (the test fails until it does); `runtime.js` needs nothing. Aside: since #488 the bundler refuses
  `nsh/device`, `nsh/accessibility` (and `nsh/host`, `nsh/native`, `nsh/keymap`, `nsh/animated`, `nsh/gesture`,
  `nsh/ios`, `nsh/android`) and native hosts serve none of them; the web runtime still does, as the five #700
  removed. A possible follow-up.
- 2026-11-05 (later): fork CI dXnSix/nsh#137 green but Fork CI's macOS job (the same failures as #129, upstream's
  #907) and android (upstream main's duplicate `Mounter` `case 44`). On the owner's go, pushed `072d45ac` to #975
  (fast-forward on `4ce3495e`) and replied to the review (issue comment 6660135932).

---

## Description

`lifecycle()` reached every platform with #747, the web's player included: `nsh.js` reports `background` for a hidden
tab and `inactive` without the focus. The browser runtime, where web apps' code runs, did not pass it on:
`src/browser/engine.rs` left it out of the environment it hands the app's realm, and `browser/environment.js` has no
`lifecycle` export. An app that imports `lifecycle` from `nsh` failed to load in the browser, and the web's lifecycle
never reached an app.

The engine now sends `lifecycle` with the rest of the environment, and `browser/environment.js` keeps it as a signal
written when it changes, as nsh-js's `env.rs` does. `browser/environment.test.mjs` checks that the browser runtime
serves every export of `nsh/device` and `nsh/accessibility` that `env.rs` declares, so the next part added to the
environment fails there rather than in apps.

Docs: `docs/dev/web.md` ("The environment") and the web paragraph of `website/docs/device-and-environment.md` (and
its zh-CN page).

## How to Test

- `node --test crates/nsh-platform-web/web/browser/environment.test.mjs` (no wasm needed): fails on `main`, passes
  with this change.
- In a browser: an app that shows `lifecycle()` shows `inactive` when the page loses the focus and `background` when
  its tab is hidden.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed the web's JS tests, `cargo +nightly fmt` and the wasm clippy.
- [x] Changes to what apps use: `lifecycle()` works in the browser runtime as #747 documents it; no API changes.
- [ ] Ran the related `examples/gallery` demos on iOS and Android: a web-only fix.
