# Upstream PR: core: Keep the sticky row before the pinned one rendered

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#281](https://github.com/longbridge/nsh/pull/281) (2026-10-21) with its issue
([`../issues/sticky-row-before-pinned.md`](../issues/sticky-row-before-pinned.md), [#278](https://github.com/longbridge/nsh/issues/278)), from
`pr/sticky-row-before-pinned` rebased onto `upstream/main` `91e5878e` (`ef529978`).

- Branch: `pr/sticky-row-before-pinned`, one commit, `35b511b2`, on `upstream/main` `d5839d7a` (rebased from
  `681738a3`; nothing conflicted); pushed to the fork
  (`dXnSix/nsh`). Open the PR from `dXnSix:pr/sticky-row-before-pinned` into `longbridge/nsh:main`.
- PR title (squash): `core: Keep the sticky row before the pinned one rendered`
- It is the fork's `324ac1a1` (fork PR #13), split out of the web sticky PR ([`upstream-web-sticky.md`](upstream-web-sticky.md)):
  it is independent of it and of `stickyHeaderOffset`, and touches all platforms.
- Images for the Screenshot table, in `Six.Gamify/artifacts-temp/nsh-upstream-sticky/evidence/`:
  `core-gap-before.png` / `core-gap-after.png` (the lab app's horizontal list, frame 9 of a touch scroll at 800 px/s;
  both with the web sticky PR's `nsh.js`, so the web's own lag does not hide the gap).

The body to paste starts below the line.

---

Closes #[issue number]

## Description

A `List` binds the items in its render window (the viewport plus one viewport of overscan each way) and the pinned
sticky item. In a section longer than the overscan, the previous section's header is neither, so when the list scrolls
back up past the pinned header's start, that header is bound only once it becomes the pinned one, a round of binding
later, while the platform already draws the new offset: the frame shows the previous section's rows without their
header. In a horizontal list of sections of 8 cells (680 points, with a 420-point viewport), each touch scroll back
over the sections in Chromium shows that frame once: a 6-point gap at the list's start where the previous header
belongs.

`ListState::update_window` now also keeps the sticky item before the pinned one bound. The platforms place it from its
range of travel, pushed out above the pinned header and out of sight, until the list scrolls back into its section;
then it is there in the first frame. Lists without sticky items bind nothing more; lists with them keep one more cell.

Test: `the_sticky_item_before_the_pinned_one_stays_rendered` (in `core::list`): with item 60 pinned, item 30 (far
above the window) stays bound and item 0 does not; scrolled back into 30's rows, 30 and 0 are bound and 60 is not.

## Screenshot

A horizontal list (sticky headers 40 points wide between 8 cells of 80), Chromium at 420 × 800, frames of the same touch
scroll back to the left at 800 px/s, both with the web sticky PR (#[web sticky PR]) so that the web's own lag does not
hide it:

| Platform | Before | After |
| -------- | ------ | ----- |
| Web | [core-gap-before.png: header C 6 points in, header B missing at the start] | [core-gap-after.png: B's last 6 points, then C] |

In the painted frames of three touch scrolls left and three right (CDP screencast), the gap showed in 1 frame of
354 at 800 px/s and 1 of 303–304 at 1200 px/s in each of three runs before, and in none of the six runs after (on
`681738a3`; on `d5839d7a` again 1 of 355 and 1 of 303 before, 0 of 354 and 0 of 303 after). On
iOS and Android the binding crosses from the main thread to the engine thread and back, so the gap can last longer
there; the iOS simulator was not measured frame by frame.

## Performance

`nsh test --trace`, debug, headless, three interleaved runs each (the timings are noisy):

| | Before | After |
|---|---|---|
| A list of 26 sections of 30 rows (44-point rows, 32-point headers) scrolled 9000 points down and 4000 up | 304 commits, 1424 ops / 1436 mount items, 246 nodes, median commit 815–825 µs | 304 commits, 1423 ops / 1434 mount items, 250 nodes (the cell kept bound), median commit 839–853 µs |
| `examples/counter/App.tsx` | 56 ops / 49 items | the same |
| `examples/gallery/Bench.tsx` scrolled 3000 points | 40 commits, 2270 ops / 1923 items | the same |

The list:

```tsx
import { mount, Text, View } from "nsh";
import { List } from "nsh/list";

type Row = { header: boolean; label: string };
const rows: Row[] = [];
const headers: number[] = [];
for (const letter of "ABCDEFGHIJKLMNOPQRSTUVWXYZ") {
  headers.push(rows.length);
  rows.push({ header: true, label: letter });
  for (let i = 1; i <= 30; i++) rows.push({ header: false, label: `${letter} ${i}` });
}

function App() {
  return (
    <List testID="list" each={rows} stickyHeaderIndices={headers} estimatedItemSize={44} style={{ flex: 1 }}>
      {(row) => (
        <If
          when={row().header}
          else={
            <View style={{ height: 44, justifyContent: "center", paddingLeft: 16, backgroundColor: "#ffffff" }}>
              <Text>{row().label}</Text>
            </View>
          }
        >
          <View style={{ height: 32, justifyContent: "center", paddingLeft: 16, backgroundColor: "#d0006f" }}>
            <Text style={{ color: "#ffffff", fontWeight: "700" }}>{row().label}</Text>
          </View>
        </If>
      )}
    </List>
  );
}

mount(App);
```

```sh
echo 'await app.scroll("id:list", 9000); await app.scroll("id:list", -4000)' | ./target/debug/nsh test App.tsx --trace
```

## How to Test

- `cargo test -p nsh-core --lib core::list`
- The trace above.
- On a device or simulator, a list with sections longer than two screens: scroll into a section and back up past its
  start. The previous header is pinned in the first frame.
- On the web, with the web sticky PR's `nsh.js`: the frame capture above (a Playwright harness: CDP screencast during
  `Input.synthesizeScrollGesture` touch scrolls, the first header-colored run from the list's start in each frame). (On the iOS 26.5 simulator the gallery's Lists › Sections
  demo pins and pushes its headers as before with this change.)

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test --workspace`, `cargo +nightly fmt --all` and clippy (including the Catalyst and Android runs for platform code). On `main` (`d5839d7a`) itself, `cargo +nightly fmt --all --check` reports three files this PR does not touch (`nsh-core/src/lib.rs`, `nsh-js/src/modules.rs`, `nsh-platform-linux/src/input.rs`) and clippy one warning (`nsh-windows-host/src/lib.rs`, items after a test module); the files this PR changes are clean.
- [x] Changes to what apps use are additive and recorded in `api/` (`cargo xtask api`); the fixture apps in `api/apps` are untouched. Nothing apps use changes; `compat::` passes with `api/` untouched.
- [ ] Ran the related `examples/gallery` demos on iOS and Android (if the change is visible or platform-specific). The gallery's Lists › Sections ran on the iOS 26.5 simulator with this change (in a host that also had the web sticky change, which iOS does not run, and `stickyHeaderOffset`, which does nothing without its prop); Android was not run.
