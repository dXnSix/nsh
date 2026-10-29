# Upstream branches: the environment and font series

Branches prepared for `longbridge/nsh` (not opened there: the owner decides and posts). Each sits on `upstream/main`
`8d790445` (rebased 2026-10-29) and is one focused PR. The fork merged the same work through its own pull requests;
their drafts (`feat-*.md`) hold the summaries an upstream body starts from. Every upstream body names what it depends
on (`**Depends on:** …`), with the PR numbers once those are open, as below.

## The environment series

Four families stack in this order, each touching the lines the one before it added (the environment's fields, the
test runner's `open` options, the prelude): app state → time zone → languages → regional preferences. Each family
has a core branch (01) and one per platform: 02 iOS, 03 Android, 04 macOS, 05 Windows, 06 Linux, 07 GPUI. A platform
branch holds its family's core commit and that platform's earlier families, so it shows only its own commit once
what it depends on is merged.

| Branch | Depends on |
|---|---|
| `pr/app-state/01-core` | no other PR |
| `pr/app-state/0N-<platform>` | `pr/app-state/01-core` (`03-android` also `pr/android/04-selection-as-chunks`, to pass clippy) |
| `pr/time-zone/01-core` | `pr/app-state/01-core` |
| `pr/time-zone/0N-<platform>` | `pr/time-zone/01-core` and `pr/app-state/0N-<platform>` |
| `pr/languages/01-core` | `pr/time-zone/01-core` |
| `pr/languages/0N-<platform>` | `pr/languages/01-core` and `pr/time-zone/0N-<platform>` |
| `pr/regional/01-core` (with `gpui: Box the dev client's events`) | `pr/languages/01-core` |
| `pr/regional/0N-<platform>` | `pr/regional/01-core` and `pr/languages/0N-<platform>` |
| `pr/intl-time-zone/01-js` | `pr/time-zone/01-core` |
| `pr/intl-hour-cycle/01-js` | `pr/regional/01-core` and `pr/intl-time-zone/01-js` (it carries a copy of that commit) |

- The core branches report fixed values on the native platforms (in use, UTC, en-US) until each platform's branch
  reads the system's; the web and headless runs report theirs in the core branch.
- `pr/regional/01-core` boxes the environment in the engine's messages: with the regional preferences it is the
  largest message by far, which clippy's `large_enum_variant` flags on wasm32.
- Every `03-android` branch and the shader series' `05-android` pass `cargo xtask clippy-android` only with
  upstream's own `selectable_view.rs` lint fixed (Rust 1.99's `chunks_exact_to_as_chunks` fails it on `main` since
  #703): `pr/android/04-selection-as-chunks` ([`upstream-android-selection-as-chunks.md`](upstream-android-selection-as-chunks.md))
  is that fix, so each `03-android` body names it as a build dependency, and it goes first.

## Fonts and bundled files

| Branch | Depends on |
|---|---|
| `pr/woff/01-core` (`text: Read WOFF and WOFF2 fonts`) | no other PR |
| `pr/read-assets/01-core` (`engine: Let fetch read the files the app ships`) | no other PR |
| `pr/register-font/01-core` (`text: Let apps register fonts while they run`) | `pr/woff/01-core` and `pr/read-assets/01-core` (it carries both commits) |

## Checked on 8d790445 (2026-10-29, macOS host, Rust 1.99)

- Every branch: nightly rustfmt on the changed files; `node --test` of the web runtime's `nsh.test.mjs`, `scroll`,
  `mouse` and `local-time` (44 pass).
- Core and JS branches, woff, read-assets, register-font: `cargo clippy --workspace --all-targets -D warnings`;
  `cargo test -p nsh-core -p nsh-js -p nsh-testing -p nsh-engine -p nsh-macros -p nsh-bundler`; wasm32 clippy
  (`compiler`) where the web runtime changes; `nsh check examples/counter -- --skipLibCheck false` on
  register-font and intl-hour-cycle; Catalyst clippy (`embed,dev`) on the core and JS branches (they touch iOS).
- Platform branches: iOS, Catalyst clippy; Android, `cargo xtask clippy-android` (with upstream's lint fixed in the
  working tree); macOS, clippy (`dev`, all targets) and `cargo test -p nsh-platform-macos` (upstream's
  `no_formatted_content` pasteboard test failed once on languages and passed on two reruns). Windows, Linux and
  GPUI: the fork's workflows on the pushed branches.
