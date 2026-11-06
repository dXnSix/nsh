# Upstream PR: core: Tell apps the device's time zone

Opened as longbridge/nsh#976. 2026-11-05: moved forward without a force push (the owner's call): `088b2cc9` (`Intl`'s zone), a merge of `main` `aafa450c` after #975's squash merge (`734ce220`), and review-976's notes (`37aeeb1e`, `ce31a9db`, head `f7759492`); fork CI dXnSix/nsh#130: 12 pass, the 5 failures upstream's. The body below is the one before that; the PR's now opens with a note on the merge.

- Branch: `pr/environment/02-time-zone` (`61564e75e`), on `pr/web-lifecycle/01-web`: eight commits of its own, the
  core's, one per platform, then the web's. It replaces `pr/time-zone/01-core` to `07-gpui`; every file is the same as in those
  branches. The literals that build an environment outside `nsh-core` (iOS's root view, the engine's `ui_environment`
  and its tests, `nsh-testing`'s) set the new field by assignment, as `pr/environment/00-non-exhaustive` builds them;
  the branch was rebuilt on it 2026-10-30 (the previous heads are `old-pre-nonexh/02-time-zone` locally).
- Size of its own commits: 42 files, +717 / −50 (code +582 / −29, tests +57 / −5, docs +78 / −16).
- Title: `core: Tell apps the device's time zone`. Until the web lifecycle PR is merged, GitHub shows both PRs' commits.

- 2026-11-01: #744 was closed, so this branch is rebuilt on #747's head `fab837934` (rebased on `upstream/main` `e4e464f4` without #744's commit), its seven commits replayed
  (head `796cfc1b5`). Where #744 had environments built by assignment, the new field goes into the struct
  literals upstream keeps (the engine's `ui_environment`, iOS's root view, the test runner's `open`); iOS's
  observers keep their own doc comments, and the Windows Environment row keeps #747's subclass wording. The core commit's test builds its
  environments as literals (clippy's `field_reassign_with_default`).
- 2026-11-02 (after #747 merged as `lifecycle`): rebased onto `upstream/main` `bd30b1b4` through `pr/web-lifecycle/01-web` (its own PR, `upstream-web-lifecycle.md`), `appState` renamed `lifecycle` in what the commits touch, plus an eighth commit, `web: Give browser apps the device's time zone` (the browser runtime's `timeZone`, `environment.test.mjs`). The Linux and GPUI commits' `docs/dev` Environment rows had repeated half a sentence; they now add only the time zone's sentence (GPUI's ends as the fork's `main` has it: read again whenever a window becomes the active one). The zh-CN `modules/nsh.md` line keeps the time zone in the reactive list. Previous heads: `old-pre-lifecycle/origin/*` locally.
- 2026-11-02 (after #822 merged): rebased onto `upstream/main` `828b1f13` without conflicts, patches unchanged (`git range-diff`). Previous heads: `old-pre-822/*` locally.
- 2026-11-04: rebased onto the new `pr/web-lifecycle/01-web` (`upstream/main` `57b1d7ba`): head `f1190540`. Only context moved (upstream's new prelude names and doc rows beside ours); `old-pre-899/*` locally.
- 2026-11-04 (later): rebased onto the new web lifecycle (`upstream/main` `fe6f54f7`, #911): head `a861dbee`. Upstream's terminal backend (#869) added `keystroke_label` to the prelude and a screenshot row to the testing docs: both kept beside `time_zone` and `setTimeZone`; the platform commits unchanged. The terminal backend reads no system setting (its `Environment` is the default but for its own fields), so `timeZone()` is UTC there, as its `locale()` is the default.
- 2026-11-05: rebased onto the new web lifecycle (`upstream/main` `ddf4af83`, #907): head `4926ad66`. Upstream's notifications (#931) added `close_notification`, `next_notification_press`, `notification_permission`, `request_notification_permission`, `set_badge` and `show_notification` to the prelude and `notifications` to the player's environment, and #929 changed the desktop pages' Window chrome rows beside the Environment rows: all kept beside the time zone's lines; iOS, Android and web commits unchanged. No new event (Android's `NOTIFICATION = 72` is upstream's).
- 2026-11-05 (later): rebased onto `upstream/main` `90a7efd3` (#951), patches unchanged: head `995a48f9`. Fork PR dXnSix/nsh#124.
- 2026-11-05: **opened as longbridge/nsh#976** at `995a48f9` (fork CI dXnSix/nsh#130: green but macOS and Android, both upstream's own: #907's dropped keys, the emulator dying on the Styles demo).
- 2026-11-05 (review): huacnlee's P2 on #976 (issue comment 6654692710): the Linux and GPUI backends took `TZ` only with a slash or as `UTC`, so `TZ=GMT` fell back on the system's zone. Three commits on top, the rest unchanged: `175db903` `core: Read the zone TZ names as the C library does` (`TimeZone::posix(tz, system)`, the one rule both backends call: zone names with a slash or without, `:` and zoneinfo paths, an empty `TZ` UTC, POSIX rules to the system's zone, the seven database names that read as rules taken as zones; its test `platform::tests::posix_time_zones_follow_the_zone_tz_names` runs on any host), `671b03ee` `linux: ...`, `2481dc20` `gpui: Report the zone TZ names when it has no slash (TZ=GMT)`. Head `2481dc20` (fast-forward from `995a48f9`), pushed with the owner's go; fork CI dXnSix/nsh#138 (twin `tmp/review-976-fork-ci`, d608b635 on top): Linux, Core, GPUI on Ubuntu, Windows and macOS, Compat, web green; Android upstream's duplicate Mounter `case 44`; Windows, iOS, macOS and Rust application still running when pushed. Replied on #976 (issue comment 6655859243); the PR body's Public API lists `TimeZone::posix`. Follow-up: `Intl`'s default zone (`nsh-js` `local_zone_name`, from #335) had the same filter: local branch `fix/intl-posix-time-zone` (`b306a8d5`, on this head), not pushed; opens after #976 merges.
- 2026-11-05 (second review, huacnlee's reviewer agent, review 5786907868): wire the zone into the existing `Intl` path instead of a parallel answer, plus two small notes. `088b2cc9` `js: Give Intl the device's time zone, as timeZone() reports it` (`Intl` names local time as `Environment::time_zone` through `env::time_zone(cx)`, as it reads the locale; a VM no platform reported to falls back on `TZ` / `/etc/localtime` via `TimeZone::posix`; this folds in `fix/intl-posix-time-zone`, which no longer opens on its own; tests `host::tests::intl_names_local_time_as_the_devices_zone`, `web::intl::tests::the_systems_zone_is_the_one_tz_names`, nsh-testing's `Intl zone:` lines, compat `expect Intl time zone: UTC`). #975 had been squash-merged (`750141c3`), so #976 conflicted: no force-push, a merge of `upstream/main` `aafa450c1` instead (`734ce220f`, its tree the same as the rebase `4904d4f3a`). Then `37aeeb1e9` `core: Hold any time zone name the tz database allows` (`TimeZone::CAPACITY` 39 → 63: four 14-character parts), `ce31a9db0` `gpui:` and `f77594927` `windows: Look up ICU's zone mapping once per process` (`OnceLock` in place of `thread_local`). Head `f77594927`, a fast-forward from `2481dc20`, pushed by the restack session; mergeable. Fork CI dXnSix/nsh#130 (twin `dd34b5bcc`): 12 jobs pass, Windows and GPUI windows-latest included; the five failures are main's own (#978's Shift+F10 context-menu test on web, Android and iOS; the macOS test host's dropped keys since #907). Replied on #976 (issue comment 6665178700); the `timeZone()` export left to the maintainer. Below the line: the PR's body as posted.
- 2026-11-05 (third review: issue comment 6665221064 and the reviewer agent's review 5791251748): `Intl` named local time as the environment's zone but formatted it with `Date`'s offsets, so a headless run on a non-UTC machine reported `UTC` while writing local time (`TZ=Asia/Singapore`: `07:30` vs `00:00` at epoch 0); also `#[non_exhaustive]` on `TimeZone`. `52cbc023e` `core: Mark TimeZone non-exhaustive`, `ba7aa9e2e` `js: Name Intl's local time by the zone it is computed in` (`Intl` names the zone local times are computed in: the system's as the C library reads it, `TZ` / `/etc/localtime`, the browser's on the web; on Windows and Android the platform's report, `Environment::time_zone`, unless it names a zone of one offset local time lacks; a POSIX-rule `TZ` is local time without a name; a test's pinned zone moves only `timeZone()`. Tests compare `format()` output: `host::tests::intl_names_the_zone_it_formats_local_time_in`, which fails on `f77594927` under `TZ=Asia/Singapore`; `a_zone_of_one_offset_is_local_time_only_with_its_offset`; nsh-testing's `Intl agrees:` lines; compat `expect Intl local time: its zone`). Local: Intl tests under four `TZ`s, nsh-testing environment, nsh-core time zone, clippy native and wasm, fmt, `cargo xtask compat` all pass. Committed locally on `f77594927` (head `ba7aa9e2e`, a fast-forward), **not pushed to #976**; fork CI on dXnSix/nsh#130 (twin `0aaee550d`, a merge on `dd34b5bcc` carrying `ba7aa9e2e`'s tree and the fork CI files); its Android job died at the emulator's start (adb port 5554 refused), to re-run when the run completes. #976's own red checks are main's: GPUI windows-latest's title-bar height flake (`h 1020 new` vs `h 1048 new`, 28 pt) and web's #978 Shift+F10. Fork twin #138 (the first round's) closed as superseded. Reply draft: the shared notes' `976-effective-zone-reply-draft.md`.
- 2026-11-06: #976 conflicted with `main` in docs/dev/windows.md (main's new row for the app's icon beside the Environment row's time zone sentence); merged `main` 9b5960b6 as `2eb645c2f`, keeping both. Fork CI twin `d4dda3296` on #130 (the fork-only `updater.rs` / `script.rs` patches dropped: main carries them since #1002) failed only upstream's own: #978's Shift+F10 test (ios, android), #907's typed keys (macos), compat (nsh-compat #36 reads `KeyframeAnimation.frame_rate`, which waits on #998), TUI's flaky popup test (fails on main 9b5960b68 too). Pushed `2eb645c2f` to #976 (fast-forward from `f77594927`) and replied: issuecomment-6675212248.
- 2026-11-06 (fourth review, 5794986713): naming a POSIX `TZ` rule's local time by today's offset dropped its DST (`TZ=CET-1CEST,M3.5.0,M10.5.0/3`: `+01:00` in winter, summer instants an hour early on a round trip). `a272a6c97` `js: Keep a POSIX TZ rule as the name of Intl's local time`: a `TZ` the system names no zone for is named by its own text, which `zone_option` takes back as `Zone::Local`; only `TZ=:` stays unnamed. Fork CI twin `bdd6d543f`: only upstream's failures (#978, #907, nsh-compat #36; iOS animations retry passed). Pushed as #976's head `a272a6c97` and replied: issuecomment-6676170070.
- 2026-11-06: #976 conflicted with `main` again in docs/dev/components.md (main's `Application.quit()` / `quit()` in the desktop row beside the environment row's `timeZone()` / `time_zone()`); merged `main` ccc23fac2 as `f473e3d4a`, keeping both. Clippy (workspace, macOS, wasm) clean for #976's files; nsh-js Intl / time zone tests pass, also under the CET rule. Pushed as #976's head `f473e3d4a` (fast-forward from `a272a6c97`).
- 2026-11-06: #976 conflicted with `main` again in crates/nsh-testing/src/driver.rs (#1034's `open_with_url` beside `set_time_zone`, in the `Driver` trait and the headless impl); merged `main` bc79c1443 as `46b4d81e5`, keeping both methods. Clippy (workspace, macOS, Catalyst) clean for #976's files (fmt flags only main's `nsh-cli/src/play.rs`); nsh-testing's 17 integration tests and nsh-js's Intl / time zone tests pass. Pushed as #976's head `46b4d81e5` (fast-forward from `f473e3d4a`).
- 2026-11-06: huacnlee's API review (5797781362) proposed one `localeSettings()` value for the #976 / #977 / #1013 stack; the user approved the restack. Local branch `restack/976-locale-settings`: `61596e86b` merges #1013's head 6f61bff8d (which holds #977) into 72863bef4 (a merge of main c32b38478 after #1028's nsh-compat break), `3e2db384f` merges main dc0000560 (#1044: env.rs parts, the macOS observers, the engine's boxed environment beside `Displays`), `377cd9e8d` `core: Tell apps the user's locale settings as one value` (`LocaleSettings` replaces `RegionalPreferences` and the three `Environment` fields; `localeSettings()` / `locale_settings()`; `nsh/test` `localeSettings` / `setLocaleSettings` with `LocaleSettingsOptions`; plumbing `#[doc(hidden)]`; TSX's derived value is `computed`, not `memo`). Local: workspace tests, fmt, clippy (workspace, macOS, Catalyst, Android, web, GPUI), GPUI tests, browser tests, `cargo xtask compat` pass; Windows and Linux backends edited by hand. Fork CI twin `0b17ca5fa` on #130 carries 8bb22429c's iOS app.rs (fork only, #1034's URLContexts abort). Drafts: 976-restack-body-draft.md, 976-restack-reply-draft.md, 977-1013-closing-comments-draft.md in the shared notes.
- 2026-11-06: fork CI on twin `0b17ca5fa` (#130): only main's failures (#978 Shift+F10 on iOS / Android, #1045's `drop(active)` on macOS); everything else passed, iOS running with 8bb22429c on the twin. Under the user's "Yes, once green": pushed `377cd9e8d` to #976 (fast-forward from `46b4d81e5`), retitled it `core: Tell apps the user's locale settings` with the restack body, replied to huacnlee's review (issuecomment-6683466314). Closing #977 and #1013 (comments in 977-1013-closing-comments-draft.md) was refused by the session's permission check: waits on the user.

---

The branch merges `main` after #975's squash merge (734ce220f), so its history still lists #975's original commits;
its diff against `main` is the time zone's alone.

## Description

Apps cannot learn the device's time zone. `Date` follows it, but an app that shows times in the user's zone with a
time zone library, names the zone beside a clock or saves a date with its zone needs the zone's name, and needs to
know when it changes.

`timeZone()` of `nsh` and Rust's `time_zone()` (the prelude) give it, reactively, as the system names it: an IANA name
(`Europe/Berlin`), `UTC`, or an offset zone where the system has one (Android's `GMT+01:00`). It is
`Environment::time_zone` (`platform::TimeZone`, inline so that the environment stays `Copy`), which each platform
writes as the system reports it. `Intl` names local time as the same zone:
`Intl.DateTimeFormat().resolvedOptions().timeZone` is `timeZone()` on every platform, and `{ timeZone: timeZone() }`
formats in local time. `Date`'s local times stay as they are.

The PR is the core, every platform and the browser runtime, a commit each:

- **Core**: `platform::TimeZone` (`UTC`, `parse`, `posix`, `as_str`), `Environment::time_zone`, `time_zone()`, `timeZone` in
  `nsh`. The web reports the browser's zone, read again when the page shows or gets the focus. `nsh/test`'s
  `open({ timeZone })` sets it (pinned on real platforms; an invalid name fails `open`) and `app.setTimeZone(name)`
  moves a headless app to another zone; headless apps are in UTC.
- **JS** (`Intl`): local time is named as `Environment::time_zone`, read where it is used as the locale is; a VM no
  platform has reported to yet falls back on `TZ` and `/etc/localtime`, read by `TimeZone::posix`.
- **iOS** and **macOS**: `NSTimeZone.systemTimeZone`'s IANA name; on `NSSystemTimeZoneDidChangeNotification`
  Foundation's cached zone is reset and the environment published again.
- **Android**: `java.util.TimeZone`'s default ID, sent with the environment; `ACTION_TIMEZONE_CHANGED` gives the new
  zone, from the broadcast, which names it before the process's default zone may have caught up.
- **Windows**: the setting's zone (`GetDynamicTimeZoneInformation`'s key name) by its IANA name, from the ICU Windows
  ships (`ucal_getTimeZoneIDForWindowsID`, loaded at run time so that apps start without it, then UTC); read again on
  `WM_TIMECHANGE`.
- **Linux**: the process's own zone (`TZ`, when it names one: its local times follow it), else systemd's timedated
  (`org.freedesktop.timedate1`'s `Timezone`, followed live), else the zone `/etc/localtime` links to, else
  `/etc/timezone`'s.
- **GPUI**: as the native backends name it (Windows' setting through ICU, elsewhere `TZ`, `/etc/localtime`,
  `/etc/timezone`), read again whenever a window becomes the active one, as GPUI reports no change of it.
- **Web** (the browser runtime, where web apps' code runs): `src/browser/engine.rs` sends `timeZone` to the app's
  realm and `browser/environment.js` serves it, UTC until the engine reports.

Docs: `timeZone` in `website/modules/nsh.md`, `Intl`'s `timeZone` in `website/modules/globals.md`, "Time zone" in `website/docs/device-and-environment.md`, `open`'s
`timeZone` and `setTimeZone` in `website/docs/testing-and-trace.md` (each with its zh-CN page), `docs/dev/elements.md`,
`docs/dev/architecture.md` and the platforms' pages.

## Public API

- `nsh`: `timeZone: Accessor<string>`, the device's zone as the system names it.
- `Intl.DateTimeFormat` (behavior): `resolvedOptions().timeZone` names local time as `timeZone()`'s zone, and a
  `timeZone` option naming that zone is local time.
- `nsh/test`: `open`'s `timeZone?: string` (an IANA name; `"UTC"` by default); `App.setTimeZone(name: string):
  Promise<void>` (headless runs).
- `nsh-core`: `platform::TimeZone` (`TimeZone::UTC`, `parse(identifier: &str) -> Option<TimeZone>`,
  `posix(tz: Option<&str>, system: impl FnOnce() -> Option<TimeZone>) -> TimeZone`, the zone a POSIX process's
  `TZ` names as the C library reads it, else the system's, `as_str(&self) -> &str`; `Default` is UTC; `Display`); `platform::Environment::time_zone` and
  `element::Environment::time_zone` (new fields with defaults); `element::time_zone() -> TimeZone` (also in the
  prelude), reactive.
- `nsh-engine`: `TimeZone` re-exported; `EnvironmentPins::time_zone: Option<TimeZone>`.
- `nsh-dev`: `TestEnvironment::time_zone: Option<String>` (serialized only when set).

## How to Test

- `cargo test -p nsh-core platform`: `time_zones_parse_platform_identifiers`, `posix_time_zones_follow_the_zone_tz_names`.
- `cargo test -p nsh-js --lib intl`: `intl_names_local_time_as_the_devices_zone`, `the_systems_zone_is_the_one_tz_names`.
- `cargo test -p nsh-engine --test engine rust_platform`: `components_follow_the_time_zone`.
- `cargo test -p nsh-testing --features bundle --test testing`: "the time zone follows the system's" and "an invalid
  time zone fails open"; `Intl`'s zone beside `timeZone()` in those and the defaults.
- `node --test crates/nsh-platform-web/web/*.test.mjs`.
- Compat: `tests/compat/cases/modules` shows `Time zone: UTC` and `Intl time zone: UTC` and claims `covers [nsh] timeZone`.
- On a device: `<Text>{timeZone()}</Text>`, then change the system's time zone while the app runs.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches, the Catalyst and Android runs
  included (Windows, Linux and GPUI on CI).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the fork's `main` carries the same commits, and its
  iOS simulator and Android emulator jobs run the gallery's tests there.
