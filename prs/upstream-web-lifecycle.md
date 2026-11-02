# Upstream PR: web: Give browser apps their lifecycle

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/web-lifecycle/01-web`, one commit on `upstream/main` `bd30b1b4` (after #747, which added
  `lifecycle()`). The fork's own fix is dXnSix/nsh#114, still under the name `appState` there.
- `pr/environment/02-time-zone` to `04-regional` sit on this branch: each adds its part of the browser runtime and of
  `environment.test.mjs`, which this PR adds. Open this one first.
- Checked locally: `node --test crates/nsh-platform-web/web/browser/environment.test.mjs` fails without the change
  and passes with it; the player's and the browser runtime's other JS tests; nightly rustfmt; the wasm clippy
  (`--features compiler`).
- Title: `web: Give browser apps their lifecycle`.

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
