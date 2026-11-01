# web: Let an app's Rust crates use wasm-bindgen in the browser

> **Upstream (checked 2026-11-01):** opened and merged as longbridge/nsh#228 (2026-10-21).

Branch: `feat/wasm-bindgen` on `github.com/dXnSix/nsh`. It follows upstream by merging `upstream/main` in, never by
rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened as a pull request into the fork's `main`, Gamify's
integration line, and merged there with a merge commit. Not opened upstream; the owner
decides and posts there (upstream squash merges).

## Summary

An app's crate that calls browser APIs through `web-sys` / `js-sys` (a WebSocket, `IndexedDB`, the clipboard) links
imports that only wasm-bindgen's JS glue provides. NSH's web runtime is a raw module (`nsh_*` exports, `nsh` / `env`
imports), so `nsh.js` stubbed those imports with functions that throw. And a browser callback that woke one of the
app's tasks (a WebSocket message) went unnoticed until the next timer or input: nothing pumped the engine from
outside an `nsh_*` call (the engine's task notify only queued a message read at the next pump).

This makes both work, opt-in by the app's dependencies:

- **CLI** (`crates/nsh-cli/src/bindgen.rs`, `rust_app.rs`, `web.rs`): when the app's web runtime links `wasm-bindgen`
  (found in the `web` / `rust-web` member's normal dependencies through `cargo metadata --filter-platform
  wasm32-unknown-unknown`), `nsh` runs the `wasm-bindgen` CLI (`--target web --no-typescript`) on the built module.
  The CLI's version must equal the crate's; otherwise the build stops naming both versions and
  `cargo install wasm-bindgen-cli --version <v> --locked`. The glue is adapted for `nsh.js` (the module's own `nsh`
  import becomes an object `nsh.js` fills; `__wbg_get_imports` / `__wbg_finalize_init` are exported) and written next
  to the runtime. `nsh run --target web` serves it (`/nsh-bindgen.js`, part of `nsh.version`); `nsh package --target
  web` minifies it, names it by its content and preloads it (`<link rel="modulepreload">`). The JS snippets the glue
  imports (`#[wasm_bindgen(inline_js = …)]`, `module = …`) are served at the paths it imports them by and packaged
  under their content's names, with the glue pointed at them. Runtimes without
  wasm-bindgen are built and served as before (same files, no glue).
- **`nsh.js`**: `createPlayer`'s new option `glue` imports the glue, merges its imports with NSH's own (`nsh` and
  `env` win), and runs its setup (`__nsh_finalize`: memory views, wasm-bindgen's externref table) right after
  instantiating, before `nsh_app_init`. Unknown imports are still stubbed.
- **Engine** (`nsh-engine`): `LocalEngine::set_wake` calls a host function whenever a task is woken or a JS call is
  posted outside a pump (each still sends `Message::Wake`).
- **Web backend** (`nsh-platform-web`): sets the wake to a new `request_pump` import; `nsh.js` coalesces requests
  into one `nsh_pump` per turn (`setTimeout` 0, so a wake inside a pump never re-enters it; `call` defers it while
  the engine runs). The deadline scheduling is unchanged.

The glue has one instance per page, which suits an app's page; the website's multi-player pages keep the prebuilt
runtime, which has no wasm-bindgen.

## Public API

### `nsh-engine`

- `LocalEngine::set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>)`: called when the engine has work outside a
  pump (a woken task, a posted JS call), so a host driving it from its own event loop pumps soon.

### `nsh-platform-web` (`web/nsh.js`)

- `createPlayer(container, { glue })`: the URL of the wasm-bindgen glue of a runtime whose crates use wasm-bindgen
  (written by the CLI next to the module).
- New wasm import `nsh.request_pump()`: the engine asks for a pump.

### `nsh` CLI

- Apps whose web runtime links `wasm-bindgen` get the glue built, served and packaged; needs the `wasm-bindgen` CLI
  at the crate's version (`NSH_WASM_BINDGEN` names one off `PATH`).

## Breaking Changes

None. The web page templates (`index.html`, `dev.html`) pass `glue` (undefined without wasm-bindgen).

## Size impact

Measured 2026-10-19 with a fixture app (TSX entry, one crate with a `#[class]`), `nsh package --target web`:

| Site file | Without wasm-bindgen (raw / gzip / brotli) | With web-sys `WebSocket` (raw / gzip / brotli) |
|---|---:|---:|
| `nsh.<digest>.wasm` | 3,525,296 / 1,281,275 / 986,604 B | 3,296,851 / 1,295,467 / 989,619 B |
| `nsh-bindgen.<digest>.js` | — | 5,723 / 2,438 / 2,156 B |
| `nsh.<digest>.js` (minified) | 95,549 / 30,197 / 26,692 B | same |

wasm-bindgen's processed module is smaller raw (it drops its descriptor code and custom sections) and about 3 KB
larger with brotli for the WebSocket bindings; the glue adds about 2 KB with brotli. Apps without wasm-bindgen
download nothing new; `nsh.js` grows by 1.4 KB of source (0.4 KB with brotli) for the glue loader and
`requestPump`.

## Docs

- `docs/dev/web.md`: "Crates that use wasm-bindgen" (detection, version check, glue adaptation, serving and
  packaging, the external wake).
- `website/docs/web.md` / `website/zh-CN/docs/web.md`: "Browser APIs from Rust" with a WebSocket example.
- `website/docs/rust.md` / `website/zh-CN/docs/rust.md`: pointer to it.

## Validation

- `cargo test -p nsh-cli wasm_bindgen`: 6 new tests (detection in the runtime's normal dependencies only; the CLI
  version check and its messages; the glue adaptation and refusal of an unknown glue shape; finding the snippets and
  linking them by their published names; the dev page serving the glue and its snippets, with the runtime version
  covering them; the package page preloading the glue, and naming no glue without wasm-bindgen).
- `cargo test -p nsh-engine --test engine external_wake`: 2 new tests (a task woken outside a pump calls the wake
  exactly once and the next pump runs it, with no further wake; without a wake the engine behaves as before). After
  the merge of `upstream/main` `a8ff77c` they set its new `EngineConfig` fields `websocket` and `filesystem`
  (`a17dc03`).
- `cargo test -p nsh-engine --test engine compat::`: 19 tests pass (no app-facing API change to record).
- `cargo +nightly fmt --all`; `cargo clippy -p nsh-cli -p nsh-engine -p nsh-core --all-targets`;
  `cargo clippy -p nsh-platform-web --target wasm32-unknown-unknown`: clean.
- End to end (Gamify `src/frontend/tests/e2e`, spec `wasm-bindgen smoke`): a fixture app whose crate opens a
  `web_sys::WebSocket` and an `inline_js` snippet, built by the CLI installed from this branch with NSH's crates from this branch through git,
  run with `nsh run --target web` in headless Chromium: after one click, the echo shows within 2 s with no further
  input. The same fixture packaged with `nsh package --target web` echoes in 25 ms.
