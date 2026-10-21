# image: Load the @1x, @2x or @3x file that fits the screen

Branch: `feat/multires-images` on `github.com/dXnSix/nsh`, rebased once on `upstream/main` `87b1cb4` (#186); since then it
follows upstream by merging `upstream/main` in, never by rebasing (the latest merge is `upstream/main` `a8ff77c`). Opened
as a pull request into the fork's `main`, Gamify's integration line, and merged there with a merge commit. Three commits:

1. `a6d4026` `core: Pick image asset variants for the screen's pixel ratio`
2. `e508d5e` `bundler: Ship the @1.5x–@4x siblings of imported images` (with every docs page, English and zh-CN)
3. `069bea5` `core: Give asset! images the variants of their crate's files` (Rust parity, separable)

The first two commits also record their additions to the API baselines in `api/types/nsh/` (`image.d.ts`:
`getSize()`/`prefetch()` docs; `types.d.ts`: the optional `scales` field), which `compat::surface_is_recorded` checks (upstream has
since moved the record from `api/current/` to `api/`; the merge followed it).

Not opened upstream; the owner decides and posts there (upstream squash merges). The issue draft that should go first:
[`docs/nsh-fork/issues/multires-images.md`](../issues/multires-images.md).

## Summary

An imported image used to ship exactly the imported file, so apps imported their `@3x` file and every screen
downloaded, decoded and held it. Now an image imported **without** a density suffix ships its `@Nx` siblings and loads
the one that fits the screen, as React Native does:

```text
assets/logo.png      64×64 px
assets/logo@2x.png   128×128
assets/logo@3x.png   192×192
```

```tsx
import logo from "./assets/logo.png";
// { uri: "asset:/assets/logo.png", width: 64, height: 64, scales: [1, 2, 3] }
<Image src={logo} />; // logo.png on 1×, logo@2x.png on 2×, logo@3x.png on 3× (and 2.625×); always 64×64 pt
```

- **Bundler** (`plugin.rs`): for such an import, it stats the fixed names `<stem>@<s>x.<ext>` for 1.5, 2, 3 and 4 (no
  directory listing), adds the ones it finds to the bundle's assets (so the dev server, `nsh build`, every
  `nsh package` target, `.nshpkg` and the web asset map carry them), and emits `scales` only when there is one. The
  width and height still come from the imported file. A variant whose pixels are not that size times its scale gets a
  warning on `nsh::build`.
- **Core**: `ImageSource` carries a `Copy` bitmask of scales. Where the commit emits `MountItem::Image`, the URI is
  rewritten to the variant for the surface's `pixel_ratio`: the smallest scale at least the ratio, otherwise the
  largest. Only `asset:/` URIs are rewritten, and it applies to `defaultSource` too. When the pixel ratio changes
  (`apply_environment`: a window moved to another screen, browser zoom), only the images whose pick changes are marked
  `dirty::IMAGE` and sent again, with no relayout. `Image.getSize` / `Image.prefetch` pick the same way; `getSize`
  divides by the variant's scale, so it keeps reporting the imported file's size.
- **Backends: unchanged.** Each loads the `asset:/` URI it is given and keys its caches by it. The only diffs are two
  test literals (Windows and GPUI) that gain the field.
- **Rust** (separate commit): `asset!("logo.png")` lists its siblings' scales in `ImageAsset::scales` and
  `include_bytes!`s them (so a changed variant rebuilds the crate). `assets!()` already ships the directory. Rust
  components pick the same way.

Importing an `@Nx` file (`./logo@3x.png`) probes nothing and ships that file alone, as before. An app without
variants bundles byte for byte as before.

## Public API

### TypeScript

- `ImageAsset.scales?: number[]`: the pixel densities the image ships in, 1 being the imported file. It is absent when
  the imported file is the only one.
- The `.nsh/types` image module comments describe the variants.

### Rust

- `nsh_core::IMAGE_SCALES: [f32; 5]` (`[1.0, 1.5, 2.0, 3.0, 4.0]`): the densities a variant can have.
- `nsh_core::ImageScales`: a `Copy` set of `IMAGE_SCALES`, with `NONE`, `from_scales`, `with`, `is_empty`,
  `contains`, `iter` and `pick(pixel_ratio)`.
- `nsh_core::ImageSource::scales: ImageScales` (new field).
- `nsh_core::ImageSource::uri_for(pixel_ratio) -> Arc<str>`: the URI the platform loads at that ratio.
- `nsh_core::ui::ImageAsset::scales: &'static [f32]` (new field), filled by `asset!`.

## Breaking Changes

```diff
 nsh_core::ImageSource {
     uri,
     size,
+    scales: ImageScales::NONE,
 }
 nsh_core::ui::ImageAsset {
     uri,
     width,
     height,
+    scales: &[],
 }
```

Changed behaviour: an app that already has `@1.5x`–`@4x` siblings next to an image it imports without a suffix now
ships them, and loads them on denser screens. Before, only the imported file shipped.

## Performance

- **Hot paths.** Decoding allocates nothing for scales: a `u8` bitmask read from the array by index. Emitting an
  image item allocates the variant URI once per emitted change (a `String` turned into `Arc<str>`); without scales, or
  at 1×, the source's `Arc` is reused. A pixel-ratio change walks the nodes once and marks only the images whose pick
  changes.
- **`nsh trace` of `examples/gallery`** (no variants; release CLIs installed from `upstream/main` `4fd574f` and from
  this branch, before its rebase onto `87b1cb4`; 15 interleaved runs, `--wait 1500`, first commit):

  | | min | lower quartile | median | upper quartile | max | ops | mount items | nodes |
  |---|---|---|---|---|---|---|---|---|
  | main | 508 µs | 528 µs | 567 µs | 861 µs | 1941 µs | 973 | 937 | 247 |
  | this branch | 484 µs | 529 µs | 642 µs | 1173 µs | 2743 µs | 973 | 937 | 247 |

  The counts are identical in every run. The fastest runs take the same time; the spread above them comes from load on
  the machine (other builds ran at the same time), not from the change, which adds an `Arc` clone per image item when
  there are no scales.
- **Web bytes for a list of 216 country flags** (GameX's flags, 34×24 pt):

  | | 1× | 2× | 3× |
  |---|---|---|---|
  | Today, `@3x` imports | 296,564 B | 296,564 B | 296,564 B |
  | Variants | 119,465 B (−59.7%) | 208,451 B (−29.7%) | 296,564 B |

  The cost is two asset-map entries per image in `index.html`: about 29 KB raw and about 5 KB gzipped for 216 flags.
- **Decoding.** iOS and macOS already downsample while decoding (`nsh-platform-ios` / `nsh-platform-macos`
  `image.rs`, `kCGImageSourceThumbnailMaxPixelSize`), so there the saving is bytes. Linux (`nsh-platform-linux`
  `image.rs` `decode`: `gdk::Texture::from_bytes`, falling back to `image::load_from_memory`), Windows (`nsh-platform-windows` `image.rs` `decode_wic` / `decode`)
  and GPUI (`nsh-platform-gpui` `image.rs` `decode_still`, the `image` crate) decode the full file, so a 1× or 2× desktop now decodes and holds
  1/9 or 4/9 of the pixels of an `@3x` file. Android halves the decode size by powers of two while it stays at least the
  view's size (`Images.java` `inSampleSize`, the view's size from `Mounter.java`), so an `@3x` file decoded at full
  size on 2× and 2.625× phones. With variants a 2× phone decodes the `@2x` file (4/9 of the pixels), a 2.625× phone
  still loads `@3x` (React Native's rule), and a 1.5× phone without an `@1.5x` file loads `@2x`, which it decodes at
  full size: slightly more pixels than the halved `@3x` it decoded before (68 px against 51 px wide for a 34 pt flag),
  for fewer bytes. Shipping `@1.5x` avoids that.

## Docs

- `website/docs/images-and-assets.md`: a "Pixel densities" section replaces "the file you import is the one that
  ships"; `getSize` / `prefetch` with imported images.
- `website/docs/project-structure.md`, `website/elements/image.md`, `website/modules/nsh-image.md`,
  `website/modules/nsh.md`, `website/modules/nsh-canvas.md` (`drawImage` draws the imported file).
- The same pages in `website/zh-CN/`.
- `docs/dev/images.md` (core pick, bundler probe, `asset!` parity) and `docs/dev/components.md` (`asset!`).

## Validation

- New test groups:
  - `cargo test -p nsh-core image_scales_`: 7 tests. Picks at 1, 1.5, 2, 2.625, 3 and 3.5; variant names
    (`@1.5x`, extensionless, dotted names) with the 1× file being the source's own `Arc`; `http(s)` and file URIs
    untouched; invalid scales dropped; decoding from JS shapes; mount items at each ratio (source and placeholder) at
    an unchanged layout size; unchanged output without scales.
  - `cargo test -p nsh-engine --test engine variant_repick`: 2 tests. Ratio 3 mounts `@3x`, and 3 → 2 sends exactly
    the scaled images again, in one batch, with no frames. 2 → 2.5 picks `@3x`/`@4x`, 2.5 → 3 sends nothing, an image
    without scales is never sent, and item counts stay bounded over six ratio changes.
  - `cargo test -p nsh-engine --test engine variant_info`: `Image.getSize` and `Image.prefetch` ask for the picked
    variant at ratios 1, 2 and 3, and `getSize` reports the imported file's 40×30.
  - `cargo test -p nsh-bundler variant_`: 6 tests. Siblings shipped and listed, every scale with gaps, `@Nx` imports
    unchanged, the exact bundle without variants, the size warning (1.5× rounding), and the asset module comments.
  - `cargo test -p nsh-js --lib variant_types_`: the `ImageAsset` declaration.
  - `cargo test -p nsh-macros asset_variants_` and `cargo test -p nsh-core asset_variants_`: `asset!` with and
    without siblings, and Rust components picking them.
- `cargo +nightly fmt --all --check` (main workspace and `crates/gpui`). `cargo clippy -D warnings --all-targets` for
  nsh-core, nsh-bundler, nsh-macros, nsh-engine and nsh-js, and for the GPUI workspace.
- **Browser** (Playwright, headless Chromium, fixture app with `img.png` / `@2x` / `@3x` in different colours; CLI
  installed from this branch; the web runtime built from it):
  - at `deviceScaleFactor` 1, 2 and 3 the `<img>` loads `img.png`, `img@2x.png` or `img@3x.png`, and the network log
    shows only that file;
  - the image decodes at 40×30, 80×60 or 120×90 pixels and is laid out at 40×30 points each time;
  - a remote image keeps its URL;
  - changing the device scale factor on the live page from 1 to 2 loads `img@2x.png` and nothing else, at the same
    size.
- **Native, same fixture:**
  - macOS Retina window (ratio 2): only the `@2x` colour, 80×60 device pixels for 40×30 pt;
  - iOS simulator (iPhone 17 Pro Max, ratio 3): only the `@3x` colour, 120×90 device pixels.
- **Linux** (Ubuntu 24.04 aarch64 VM, GTK 4.14.5, a fresh clone of this branch):
  - the new test groups above: 21 tests pass;
  - the Linux CI lane (`linux.yml`): clippy `-D warnings` for nsh-platform-linux, nsh-linux-host, nsh-linux-app,
    nsh-cli, nsh-dev and xtask, and for nsh-platform-linux without WebKitGTK; `cargo test -p nsh-platform-linux -p
    nsh-cli -p nsh-dev` and `-p nsh-platform-linux --features webview` (251 tests);
  - the fixture on the GTK development host (`nsh run --target linux`, X11 under Xvfb, `GDK_SCALE` 1, 2 and 3):
    only `img.png`, `img@2x.png` or `img@3x.png` is drawn, at 40×30, 80×60 or 120×90 device pixels (40×30 pt).
- **GPUI** (the GPUI development host on macOS, `nsh run --target gpui`, Retina, ratio 2): only `img@2x.png` is drawn,
  at 80×60 device pixels (40×30 pt).
- **Android** (emulator, Android 15 arm64, Pixel 4 profile; `nsh run --target android` with the Android runtime built
  from this branch by `cargo xtask android --dev`), app launched at each density:

  | density (ratio) | drawn | device pixels (40×30 pt) |
  |---|---|---|
  | 160 (1×) | `img.png` | 40×30 |
  | 240 (1.5×) | `img@2x.png` (no `@1.5x` file) | 60×45 |
  | 320 (2×) | `img@2x.png` | 80×60 |
  | 420 (2.625×) | `img@3x.png` | 105×79 |
  | 440 (2.75×, the device's own) | `img@3x.png` | 110×83 |

  Changing the density with `adb shell wm density` while the app runs moved nothing, not even the layout: the Android
  backend does not report a density change to a running app today. That is outside this change and fixed on its own branch
  (`feat/android-density`, [its draft](feat-android-density.md)); the core re-picks as soon as a backend reports the
  new ratio (`variant_repick`, and the browser's live change).
- **Fork CI** on `feat/multires-images` at `069bea5` (dispatched with `gh workflow run`), all green:
  - Core: https://github.com/dXnSix/nsh/actions/runs/39383886040
  - Linux: https://github.com/dXnSix/nsh/actions/runs/39383888307
  - Windows: https://github.com/dXnSix/nsh/actions/runs/39383891015
  - GPUI: https://github.com/dXnSix/nsh/actions/runs/39383893853
  - Examples: https://github.com/dXnSix/nsh/actions/runs/39383896355
- **Not run by hand:** a Windows window (Windows is covered by its CI workflow above). There was no external 1× display, so moving a window between screens is covered by `variant_repick`
  and the browser's live ratio change.

- **After the merge of `upstream/main` `a8ff77c`:** the conflicts were the export lists of `nsh-core`
  (upstream dropped `UNDERLAY_OPACITY` and imports its status-bar types in `host_values.rs`), the engine tests'
  module list and the declarations' tests; all kept. The re-pick
  tests set `EngineConfig`'s new `websocket` and `filesystem` (`038b542`). The new test groups above, `cargo clippy
  -D warnings --all-targets` for nsh-core, nsh-bundler, nsh-macros, nsh-engine and nsh-js, both workspaces' `fmt
  --check`, and `cargo test -p nsh-engine --test engine compat::` (19 tests: the record in `api/` matches) pass. The
  browser, native and Android runs above were not repeated after the merge.
- **After the merge of the fork's `main` (`c5261dc`, with fork PRs #1–#6):** the one conflict was the end of
  `nsh-core`'s `core/tests.rs`, where both this branch and `feat/android-density` appended tests; both are kept.
  Android's `Core::remount` re-sends images through the same variant pick. `cargo test -p nsh-core` (390), the
  engine's `variant_` tests and clippy for nsh-core and nsh-engine pass. Merged into the fork's `main` as PR #7.

## Follow-ups

- `nsh run` does not watch image files, so a variant added while it runs shows after a restart or a source edit.
- `VARIANT_SCALES` in `nsh-macros` repeats `IMAGE_SCALES`, because a proc macro cannot depend on nsh-core. Change them
  together (noted in `docs/dev/images.md`).
