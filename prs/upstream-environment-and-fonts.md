# Upstream branches: the environment and font series

Branches prepared for `longbridge/nsh` (the owner decides and posts; 0, 1, 5 and 6 are open). Rebuilt 2026-10-30 on
`upstream/main` `201d780a` (which has #727, #721 and #679), one PR per feature: each family's core commit and its six
platform commits (iOS, Android, macOS, Windows, Linux, GPUI) are one PR, as `core` changes that the platforms carry
out are one area in upstream's rules. The commits stay separate inside each PR so that each platform can be reviewed
on its own; upstream squash-merges. The fork merged the same work through its own pull requests (`feat-*.md`).

Every body names what it depends on (`**Depends on:** …`) with placeholders (`#NON_EXHAUSTIVE`, `#APP_STATE`,
`#TIME_ZONE`, `#LANGUAGES`, `#WOFF`, `#READ_ASSETS`) to replace with the PR numbers as they open; those of
the open PRs are replaced in every draft.

The environment series sits on `pr/environment/00-non-exhaustive` (added 2026-10-30, the owner's decision): both
environment structs `#[non_exhaustive]`, so the fields the series adds break no code that builds an environment. The
series' branches were rebuilt on it from their original commits; their new fields are set by assignment outside
`nsh-core`, as that branch builds environments, and their previous heads are `old-pre-nonexh/01-app-state` to
`04-regional` locally.

## The PRs, their sizes and order

| # | Branch | Draft | Depends on | Commits | Files | Lines (own) |
|---|---|---|---|---|---|---|
| 0 | `pr/environment/00-non-exhaustive` | [00](upstream-environment-00-non-exhaustive.md) | none | 1 | 25 | +300 / −359 |
| 1 | `pr/environment/01-app-state` | [01](upstream-environment-01-app-state.md) | 0 | 7 | 38 | +723 / −62 |
| 2 | `pr/environment/02-time-zone` | [02](upstream-environment-02-time-zone.md) | 1 | 7 | 42 | +717 / −50 |
| 3 | `pr/environment/03-languages` | [03](upstream-environment-03-languages.md) | 2 | 7 | 40 | +650 / −52 |
| 4 | `pr/environment/04-regional` | [04](upstream-environment-04-regional.md) | 3 | 8 | 42 | +1702 / −85 |
| 5 | `pr/woff/01-core` | [WOFF](upstream-woff.md) | none | 1 | 21 | +175 / −27 |
| 6 | `pr/read-assets/01-core` | [assets](upstream-read-assets.md) | none | 3 | 18 | +713 / −15 |
| 7 | `pr/register-font/01-core` | [fonts](upstream-register-font.md) | 5 and 6 | 1 | 27 | +322 / −25 |

"Lines (own)" counts a PR's own commits, as GitHub shows it once what it depends on is merged; before that GitHub
shows the earlier PRs' commits too. By kind: non-exhaustive code +140 / −144, tests +160 / −215; app state code
+493, tests +106, docs +124; time zone code +582, tests +57, docs +78; languages code +496, tests +58, docs +96;
regional code +1416 (`nsh-core`'s `platform.rs` +478: four
types, their parsing and docs, CLDR's region lists; GPUI +259, Linux +113), tests +100, docs +186. For scale: #682
(the shader series' first) is +1574, and upstream's largest recent merges run from 4,000 to 11,000 lines (#674, #518,
#598, #603).

Order: 0, 5 and 6 apply to `main` now and open together; 1 can open with them, stacked on 0 (it shows 0's commit
until 0 is merged); 7 after 5 and 6; then 2 to 4, one after another.

Opened 2026-10-31, after the fork's `main` took upstream's `201d780a` (dXnSix/nsh#93): 0 as
[#744](https://github.com/longbridge/nsh/pull/744), 5 as [#745](https://github.com/longbridge/nsh/pull/745), 6 as
[#746](https://github.com/longbridge/nsh/pull/746) and 1 as [#747](https://github.com/longbridge/nsh/pull/747).
Upstream's `main` was then `7e846de4`, three commits past the branches' base with no file in common; each branch
merges with it cleanly. Still to open: 7 (once #745 and #746 are merged), then 2 to 4.

2026-11-01: #744 was closed (a project decision: the Rust API may break until the first recorded
release), so #747 was rebased without its commit (`fab837934`); 2 to 4 still sit on the old 01 and are rebuilt
without 00 when they open.

## Not sent: the Intl PR

`pr/environment/05-intl` (`0d34b5133`, on `04-regional`: the time zone database for NSH's own `Intl`, then its hour
cycle default) is dropped from the upstream set (2026-10-30, the owner's decision). It changed what NSH's `Intl` writes
by default (dates in `timeZone()`'s zone, the device's locale's times in `hourCycle()`), which apps already rely on,
and RayX does not need it: RayX formats in Rust with dXn.Intl through its i18n service, which TSX reaches through
`I18n.t` (MessageFormat 2's `:number`, `:date`, `:time`, `:datetime`, ...) and which already applies the user's hour
cycle (`-u-hc-` on its locale, from NSH's `hour_cycle()`). Converting instants between zones, when RayX needs it,
belongs there too, in Rust. The branch stays on the fork for reference, and so do the fork's own two Intl commits on
`main` (`feat-intl-time-zone.md`, `feat-intl-hour-cycle.md`) until the owner decides on reverting them.

## What changed from the per-platform branches

- The per-platform branches (`pr/app-state/01-core` to `pr/regional/07-gpui`, 28 on the fork) and the two Intl
  branches (`pr/intl-time-zone/01-js`, `pr/intl-hour-cycle/01-js`) are superseded; every file of the stacked
  branches is the same as in them, except three. `docs/dev/elements.md` joins the iOS and Android
  sentences of the app state, which two branches each wrote alone. `Cargo.toml` lists the platforms' `windows` and
  `objc2-foundation` features together. `crates/gpui/Cargo.lock` leaves out the `jiff` entries that
  `pr/languages/07-gpui` added: nothing there uses them (they belong to the Intl time zone commit), and cargo dropped
  them on every build.
- The register-font commit adds its compatibility case (`tests/compat/cases/layout/core-register-font.tsx`:
  `registerFont` of the app's font by its `asset:/` URL, and of data that is no font), as a new export lands with one.
- Android needs no workaround any more: #727 fixed `selectable_view.rs`'s lint on `main`.

## Checked after the restack on `00-non-exhaustive` (2026-10-30, macOS host, Rust 1.99)

Heads: `00-non-exhaustive` `f7fb6d42b`, `01-app-state` `ceba797ee`, `02-time-zone` `c64064514`, `03-languages`
`e8053e214`, `04-regional` `3be233e04`. Since 2026-10-31, after #747's review fixes: `01-app-state` `a44cc19a4`,
`02-time-zone` `ca9231b93`, `03-languages` `a1de5ed5f`, `04-regional` `deffa4c06`; `register-font` `366d830f8`
(on #746's fix), since 2026-11-01 `992932b1e` (on #746's second fix, `a13982b40`).
The fork's Windows workflow passed on `04-regional` `deffa4c06` (run 41161895616: clippy, tests, the Win32 examples);
the rest of 02–04 changed only in Android's `NshView` (compiled with `cargo xtask android`) and docs.

- `00` to `04`: nightly rustfmt on the changed files, `cargo clippy --workspace --all-targets -D warnings`, the
  core/js/testing/engine/macros/bundler tests, wasm32 clippy (`compiler`), Catalyst clippy (`embed,dev`) and
  `cargo xtask clippy-android`. `00` and `04` also macOS clippy (`dev`) and `cargo test -p nsh-platform-macos`,
  `nsh check examples/counter -- --skipLibCheck false` and the web runtime's `node --test`; `00`, `01` and `04` GPUI's
  clippy (`dev`), `00` and `04` its tests, on this Mac.
- Compat: `modules` on `04` (the series' cases and claims).
- The fork's Windows, Linux and GPUI workflows on `00` to `04`: all 15 runs pass.
- The branches before the restack were checked as below; the restack changed only how environments are built outside
  `nsh-core`.

## Checked on 201d780a (2026-10-30, macOS host, Rust 1.99), before the restack

Heads: `01-app-state` `36731a0f6`, `02-time-zone` `cd9070e2a`, `03-languages` `fd6a695d2`, `04-regional`
`8dc94566d`, `woff` `71a8e2b01`, `read-assets` `9d5b13973`, `register-font` `9fabfce60`. The runs below also covered
the dropped `05-intl` (`0d34b5133`), which sits on `04-regional`.

- Every branch: nightly rustfmt on the changed files; `cargo clippy --workspace --all-targets -D warnings`;
  `cargo test -p nsh-core -p nsh-js -p nsh-testing -p nsh-engine -p nsh-macros -p nsh-bundler --features
  nsh-bundler/bundle`; `node --test` of the web runtime's tests; wasm32 clippy (`compiler`).
- Environment branches 1 to 5: Catalyst clippy (`embed,dev`) and `nsh check examples/counter -- --skipLibCheck
  false`; 1 to 4 also macOS clippy (`dev`, all targets), `cargo test -p nsh-platform-macos` and
  `cargo xtask clippy-android` (no workaround: #727 is on `main`). `register-font`: `nsh check` too.
- Lock files: `cargo metadata --locked` passes for the root and GPUI workspaces on every head.
- Compat (`NSH_COMPAT_APP=<app> cargo xtask compat`, `nsh-compat` `main` `2e83111`, one app at a time with its
  work directory cleared, as the claims of every app built there are checked): `modules` on `05-intl` (the series'
  cases and claims, all from `01` to `04`: `05-intl` changes nothing under `tests/compat`), `web` on `read-assets`, `layout` on `register-font` (its new case). The case also passes headless
  (`nsh test tests/compat/cases/layout/App.tsx` with its steps).
- The fork's Windows, Linux and GPUI workflows on `01` to `04`, `05-intl` (run as `pr/intl-hour-cycle/01-js`, the
  same commit) and `register-font` (at `e14007ec4`, before its compat case: the case is TSX the workflows do not
  build): all 18 runs pass.
