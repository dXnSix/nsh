# Upstream branches: the environment and font series

Branches prepared for `longbridge/nsh` (not opened there: the owner decides and posts). Rebuilt 2026-10-30 on
`upstream/main` `201d780a` (which has #727, #721 and #679), one PR per feature: each family's core commit and its six
platform commits (iOS, Android, macOS, Windows, Linux, GPUI) are one PR, as `core` changes that the platforms carry
out are one area in upstream's rules. The commits stay separate inside each PR so that each platform can be reviewed
on its own; upstream squash-merges. The fork merged the same work through its own pull requests (`feat-*.md`).

Every body names what it depends on (`**Depends on:** …`) with placeholders (`#APP_STATE`, `#TIME_ZONE`,
`#LANGUAGES`, `#REGIONAL`, `#WOFF`, `#READ_ASSETS`) to replace with the PR numbers as they open.

## The PRs, their sizes and order

| # | Branch | Draft | Depends on | Commits | Files | Lines (own) |
|---|---|---|---|---|---|---|
| 1 | `pr/environment/01-app-state` | [01](upstream-environment-01-app-state.md) | none | 7 | 38 | +725 / −62 |
| 2 | `pr/environment/02-time-zone` | [02](upstream-environment-02-time-zone.md) | 1 | 7 | 42 | +718 / −50 |
| 3 | `pr/environment/03-languages` | [03](upstream-environment-03-languages.md) | 2 | 7 | 40 | +650 / −52 |
| 4 | `pr/environment/04-regional` | [04](upstream-environment-04-regional.md) | 3 | 8 | 42 | +1703 / −85 |
| 5 | `pr/environment/05-intl` | [05](upstream-environment-05-intl.md) | 4 | 2 | 16 | +304 / −38 |
| 6 | `pr/woff/01-core` | [WOFF](upstream-woff.md) | none | 1 | 21 | +175 / −27 |
| 7 | `pr/read-assets/01-core` | [assets](upstream-read-assets.md) | none | 1 | 18 | +562 / −15 |
| 8 | `pr/register-font/01-core` | [fonts](upstream-register-font.md) | 6 and 7 | 1 | 27 | +322 / −25 |

"Lines (own)" counts a PR's own commits, as GitHub shows it once what it depends on is merged; before that GitHub
shows the earlier PRs' commits too. By kind: app state code +493, tests +108, docs +124; time zone code +582, tests
+58, docs +78; languages code +495, tests +59, docs +96; regional code +1416 (`nsh-core`'s `platform.rs` +478: four
types, their parsing and docs, CLDR's region lists; GPUI +259, Linux +113), tests +101, docs +186; Intl code +200
(its two tests among it), docs +20, lock files +84. For scale: #682 (the shader series' first) is +1574, and
upstream's largest recent merges run from 4,000 to 11,000 lines (#674, #518, #598, #603).

Order: 1, 6 and 7 apply to `main` now and can open together; 8 after 6 and 7; the environment series one after
another, 2 to 5. The Intl PR is the time zone database and the hour cycle default together (two commits): both change
what `Intl` writes by default toward the user's settings.

## What changed from the per-platform branches

- The per-platform branches (`pr/app-state/01-core` to `pr/regional/07-gpui`, 28 on the fork) and the two Intl
  branches (`pr/intl-time-zone/01-js`, `pr/intl-hour-cycle/01-js`) are superseded; every file of the stacked
  branches is the same as in them, except three. `docs/dev/elements.md` joins the iOS and Android
  sentences of the app state, which two branches each wrote alone. `Cargo.toml` lists the platforms' `windows` and
  `objc2-foundation` features together. `crates/gpui/Cargo.lock` leaves out the `jiff` entries that
  `pr/languages/07-gpui` added: nothing there uses them, cargo dropped them on every build, and they belong to the
  Intl time zone commit, which adds them itself.
- The Intl time zone commit no longer counts its database in the web runtime: since #603 web apps run on the
  browser's own `Intl`. Its message and its docs say so (the database is about 200 KB of each native host).
- The register-font commit adds its compatibility case (`tests/compat/cases/layout/core-register-font.tsx`:
  `registerFont` of the app's font by its `asset:/` URL, and of data that is no font), as a new export lands with one.
- Android needs no workaround any more: #727 fixed `selectable_view.rs`'s lint on `main`.

## Checked on 201d780a (2026-10-30, macOS host, Rust 1.99)

Heads: `01-app-state` `36731a0f6`, `02-time-zone` `cd9070e2a`, `03-languages` `fd6a695d2`, `04-regional`
`8dc94566d`, `05-intl` `0d34b5133`, `woff` `71a8e2b01`, `read-assets` `9d5b13973`, `register-font` `9fabfce60`.

- Every branch: nightly rustfmt on the changed files; `cargo clippy --workspace --all-targets -D warnings`;
  `cargo test -p nsh-core -p nsh-js -p nsh-testing -p nsh-engine -p nsh-macros -p nsh-bundler --features
  nsh-bundler/bundle`; `node --test` of the web runtime's tests; wasm32 clippy (`compiler`).
- Environment branches 1 to 5: Catalyst clippy (`embed,dev`) and `nsh check examples/counter -- --skipLibCheck
  false`; 1 to 4 also macOS clippy (`dev`, all targets), `cargo test -p nsh-platform-macos` and
  `cargo xtask clippy-android` (no workaround: #727 is on `main`). `register-font`: `nsh check` too.
- Lock files: `cargo metadata --locked` passes for the root and GPUI workspaces on every head.
- Compat (`NSH_COMPAT_APP=<app> cargo xtask compat`, `nsh-compat` `main` `2e83111`, one app at a time with its
  work directory cleared, as the claims of every app built there are checked): `modules` on `05-intl` (the series'
  cases and claims), `web` on `read-assets`, `layout` on `register-font` (its new case). The case also passes headless
  (`nsh test tests/compat/cases/layout/App.tsx` with its steps).
- The fork's Windows, Linux and GPUI workflows on `01` to `04`, `05-intl` (run as `pr/intl-hour-cycle/01-js`, the
  same commit) and `register-font` (at `e14007ec4`, before its compat case: the case is TSX the workflows do not
  build): all 18 runs pass.
