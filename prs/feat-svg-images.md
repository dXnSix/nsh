# image: Show SVG images, rasterized at the size they are shown

Branch: `feat/svg-images` on `github.com/dXnSix/nsh` (the eight commits below end at `141b27b`). It follows upstream by
merging `upstream/main` in, never by rebasing; the latest merge is `upstream/main` `a8ff77c`. Opened last of the eight feature
pull requests into the fork's `main`, Gamify's integration line, after the fork's `main` was merged in to settle the overlap
with `feat/multires-images` (the bundler's image import and the image docs); merged there with a merge commit. Not
opened upstream: the owner posts the issue ([`../issues/svg-images.md`](../issues/svg-images.md)) first, then decides on
the PR. Squash title upstream: `image: Show SVG images, rasterized at the size they are shown`.

Commits, one area each, in dependency order (`.svg` becomes importable only in the last one, once every backend draws
it):

| Commit | Subject |
|---|---|
| `9825d26` | `core: Draw SVG images for the native backends with resvg` |
| `99dd5ac` | `macos: Show SVG images, drawn at the size of the view in pixels` |
| `a18ceeb` | `ios: Show SVG images, drawn at the size of the view in pixels` |
| `f7dc385` | `android: Show SVG images, drawn at the size of the view in pixels` |
| `5115a57` | `gpui: Show SVG images, drawn at four pixels a point` |
| `59830da` | `windows: Show SVG images, drawn at four pixels a point` |
| `5d90b66` | `linux: Show SVG images, drawn at four pixels a point` |
| `141b27b` | `bundler: Import SVG images, sized in points by their width, height or viewBox` |

46 files, +2231 / −109 (tests and docs included).

## Summary

`<Image>` shows PNG, JPEG, GIF, WebP and HEIC. This adds SVG on every backend, for multi-colour logos, illustrations,
remote and AI-generated SVG, and designers' exports. `Path2D` on a canvas still covers single-path glyphs.

- **Web:** the browser draws SVG. `nsh.js` already shows images with `<img>` and tints them with a CSS mask, so the web
  runtime is unchanged: `nsh.wasm` and `nsh.js` are byte-identical.
- **Native:** one Rust renderer, resvg 0.46, draws on the backends' existing image worker threads. It lives in
  `nsh-core` behind a cargo feature `svg` (default off), which the native platform crates enable and
  `nsh-platform-web` never does. Each decoder sniffs SVG bytes before its platform decoder and hands the premultiplied
  RGBA raster to its usual bitmap path, so blur, colour filters, tint (through the alpha), `resizeMode`, caching,
  placeholders and the load events work unchanged.
- **Raster size:**
  - iOS, macOS and Android decode each view at its pixel size and cache by it. The SVG is drawn with its shorter side
    at the view's longer side (`svg::rasterize_covering`), so no resize mode enlarges it, `cover` included.
  - Windows, Linux and GPUI cache one bitmap per source and scale when drawing. They draw at the intrinsic size × 4,
    at most 2048 px a side (`svg::rasterize_oversampled`).
- **Sizes:** `onLoad` and `Image.getSize` report an SVG's intrinsic size in points (`width`/`height`, one of them plus
  the `viewBox`'s ratio, or the `viewBox`), measured off the main thread. A file name's `@2x` does not apply to SVG.
  On the web the browser reports what it measures; the docs say how that differs for SVGs sized only by a `viewBox`.
- **Imports:** `import logo from "./logo.svg"` gives an `ImageAsset` sized with the same `svg::size` the backends use.
  An invalid SVG fails the build naming the file. An SVG with `<text>`, SMIL/CSS animation or `<image>`, which only
  browsers draw, builds with an `nsh::build` warning naming the file. `.nsh/types` declares `*.svg`.
- **Docs** (English and zh-CN): `images-and-assets.md` (a new "SVG images" section), `elements/image.md`,
  `project-structure.md`, `modules/nsh-image.md`, `canvas.md` (`drawImage` takes bitmaps), `docs/dev/images.md`,
  `docs/dev/web.md`. The gallery's Images demo gains an "SVG images" card.

## Public API

### `nsh-core` (cargo feature `svg`, default off)

- `nsh_core::svg::sniff(bytes: &[u8]) -> bool`: whether bytes are an SVG (`<svg` after a BOM, XML declaration,
  comments and doctype, or gzip `.svgz`); file names are never trusted.
- `nsh_core::svg::size(bytes: &[u8]) -> Result<(f32, f32), svg::Error>`: the intrinsic size in points.
- `nsh_core::svg::rasterize(bytes: &[u8], max_px: u32) -> Result<svg::Raster, svg::Error>`: premultiplied RGBA with the
  longer side `max_px` (at most `MAX_PX`).
- `nsh_core::svg::rasterize_covering(bytes: &[u8], px: u32) -> Result<svg::Raster, svg::Error>`: the shorter side `px`,
  for backends that decode per view.
- `nsh_core::svg::rasterize_oversampled(bytes: &[u8]) -> Result<svg::Raster, svg::Error>`: intrinsic size × `OVERSAMPLE`,
  at most `MAX_OVERSAMPLED_PX`, for backends that draw one bitmap at every size.
- `pub struct svg::Raster { width: u32, height: u32, pixels: Vec<u8>, size: (f32, f32) }`: a drawn SVG.
- `pub struct svg::Error`: why an SVG could not be read or drawn (`Display`, `std::error::Error`).
- `pub const svg::MAX_PX: u32 = 8192`, `OVERSAMPLE: f32 = 4.0`, `MAX_OVERSAMPLED_PX: u32 = 2048`,
  `MAX_INFLATED: usize = 16 MiB`.

### TSX / assets

- `import x from "./x.svg"`: an `ImageAsset` `{ uri, width, height }` in points (`.nsh/types` declares `*.svg`).
- `<Image src>`, `Image.getSize`, `Image.prefetch` accept SVG sources (assets, URLs, files); `onLoad` and `getSize`
  report an SVG's size in points natively.

### Android (internal JNI)

- `Native.svg(byte[] bytes, int maxPx, int[] size): byte[]`: the Rust rasterizer for `Images.java` (`maxPx` < 0
  measures, 0 natural size, > 0 a view's longer side).

## Breaking Changes

None. Every addition is new; `nsh-core` without the `svg` feature is unchanged.

## Performance

### Binary size (release builds on this Mac, `upstream/main` `4fd574f` → branch)

| Binary | Before | After | Delta |
|---|---:|---:|---:|
| `nsh-macos-app` (macOS release runner) | 8,267,760 | 9,154,560 | +886,800 (+866 KB, +10.7%) |
| macOS development host (`nsh-macos-host`, release profile as `cargo xtask dist` ships it) | 8,266,000 | 9,152,800 | +886,800 (+866 KB) |
| `nsh-ios-app` (`aarch64-apple-ios`) | 7,496,872 | 8,396,704 | +899,832 (+879 KB) |
| `libnsh.so` arm64-v8a (`cargo xtask android --release`) | 8,760,240 | 9,871,800 | +1,111,560 (+1,086 KB) |
| `libnsh.so` x86_64 | 8,999,928 | 10,184,880 | +1,184,952 (+1,157 KB) |
| `nsh-gpui-app` (macOS) | 18,776,304 | 19,143,024 | +366,720 (+358 KB; resvg/usvg 0.46 were already in GPUI's tree) |

Windows and Linux binaries weren't measured (they build only in CI). resvg is built without `text`, `system-fonts` and `raster-images` (in the main workspace; GPUI's tree has them already), so
no font stack or raster codecs come with it. The `nsh` CLI also links it (the bundler sizes imports with `svg::size`).

### Web (`nsh package examples/gallery --target web`, bytes raw / gzip -9 / brotli -q 11)

| File | Before | After |
|---|---|---|
| `nsh.<digest>.wasm` | 3,876,850 / 1,309,547 / 992,771 (`d09dd1b4ee`) | identical (same digest) |
| `nsh.<digest>.js` | 95,152 / 30,059 / 26,604 (`aa91a0b793`) | identical |
| `app.<digest>.nshc` (entry) | 46,570 / 17,415 / 14,529 (`6a69967ab7`) | identical |
| `index.html` | 6,253 / 2,720 / 2,223 | 6,333 / 2,756 / 2,266 (the two new assets in the asset map) |
| `chunks/Images.<digest>.nshc` (the demo) | 13,063 / 5,543 / 4,634 | 15,098 / 6,186 / 5,181 (the new SVG card) |
| `assets/star.svg`, `assets/mark.svg` (new) | | 187 + 485 / 191 + 313 / 148 + 256 |
| All files | 34 files, 4,945,603 / 1,798,530 / 1,417,416 | 36 files, 4,948,390 / 1,799,713 / 1,418,410 |

`cargo tree -p nsh-platform-web --target wasm32-unknown-unknown -e normal` lists no `resvg`, `usvg` or `tiny-skia` for
the default, `compiler` and `bytecode-only` features (a guard in the epic's checks).

### Rasterization (release, Apple M5, best of 50 after a warm-up, at `fe677e6`)

| SVG | Raster | Time |
|---|---|---:|
| 24 × 24 icon at 18 pt | 36 / 54 px (@2x / @3x) | 0.020 / 0.024 ms |
| 24 × 24 icon at 36 pt | 72 / 108 px | 0.029 / 0.047 ms |
| 24 × 24 icon at 54 pt | 108 / 162 px | 0.047 / 0.061 ms |
| 24 × 24 icon at 72 pt | 144 / 216 px | 0.032 / 0.045 ms |
| 120 × 60 logo with a gradient (`examples/gallery/mark.svg`) | 512 px | 0.358 ms |
| Wikimedia's SVG logo (`commons/0/02/SVG_logo.svg`, 4,019 bytes) | 512 px | 1.181 ms |
| `svg::size` of the icon | | 0.002 ms |

Measured with a scratch harness outside the fork (`nsh-core` with `svg`, `cargo run --release`) on a machine also
compiling other projects, so the small timings carry some noise.

The work runs on the image worker threads, never on the main thread (getSize/prefetch measure there too).

### `nsh trace examples/gallery --wait 2000`

Initial render 937 mount items and the first tap 1121, before and after: SVG support adds nothing to the mount
stream. The Images demo itself gains the new card's views.

## Security

- An SVG reads nothing outside itself: usvg gets no resources directory and an `<image>` resolver that resolves
  nothing (usvg's default one reads arbitrary local files). File paths, `file:`, `http(s):`, `data:` images and
  external `<use>` draw nothing and touch neither the file system nor the network. Tests include one that observes a
  sentinel file being read with usvg's defaults and not with NSH's options, and a TCP listener that sees no
  connection.
- No `text`/`system-fonts`/`raster-images` features in the main workspace: no fonts are loaded and no raster codecs
  linked. GPUI's own resvg/usvg enable them; usvg's empty font database and the resolver still draw no text and no
  embedded images there (`svg_draws_no_text_or_embedded_images`).
- A `.svgz` inflates to at most 16 MiB (`MAX_INFLATED`); usvg's own `from_data` inflates without a bound (gzip
  bombs).
- Remote SVGs come through each backend's existing fetch with its capability and redirect checks.
- Bytes are sniffed; file extensions and MIME types are never trusted.

## Validation performed

- **Unit tests** (new groups, all passing):
  - `cargo test --release -p nsh-core --features svg svg::`: 16 tests (`svg_raster_*`: sniffing, sizes incl.
    one-sided and percentages, raster dimensions, premultiplied alpha, `.svgz`, invalid data, covering/oversampled
    rasters, a 24 × 24 icon at 54 px under 5 ms; `svg_safety_*`: file/relative/`file:`/`http(s)`/`data:` hrefs, external
    `<use>`, `<text>`, a gzip bomb).
  - `cargo test -p nsh-platform-macos svg_` (3) and `cargo test -p nsh-platform-ios --target aarch64-apple-ios-macabi
    svg_` (3): decode sizes, colours through CGImage, natural size, info size in points, `@2x` ignored, tint through
    alpha, colour matrix.
  - `cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-gpui svg_` (4): oversampled straight BGRA, cap,
    sizes in points, a tinted mip variant, no `<text>` or `data:` `<image>` drawn.
  - `cargo test -p nsh-bundler svg_import_` (6): import sizes (viewBox, one side, `@2x` ignored), an invalid SVG failing
    with the file named, the `nsh::build` warning naming the file, warning detection, the `*.svg` declaration.
- **Builds and lint:** `cargo +nightly fmt --check` (main and GPUI workspaces); clippy `-D warnings` for `nsh-core
  --features svg`, `nsh-platform-macos`, `nsh-bundler`, the Catalyst target (`nsh-platform-ios`, hosts, embed/dev),
  `cargo xtask clippy-android`, the GPUI workspace (`--features nsh-platform-gpui/dev`) and `nsh-platform-web
  --target wasm32-unknown-unknown --features compiler`; `cargo xtask android --dev`.
- **Browser:** a Playwright spec (Gamify) runs an app with `nsh run --target web` using the CLI installed from this
  branch: imported SVGs' boxes equal their sizes in points, the tinted one shows the tint through the CSS mask (pixel
  samples), the multi-colour one keeps its colours, no console errors.
- **On screen:** the gallery's SVG card on macOS (AppKit, dark and light), the iOS 26.5 simulator, an Android API 34
  emulator, GPUI on macOS and Chromium: the same shapes and colours, the tint at 18/36/72 pt, `contain` and `cover`
  sharp, and the remote SVG (Wikimedia's logo).

- **After the merges of `upstream/main` `a8ff77c` and of the fork's `main` (`d5f285f`, which brings in
  `feat/multires-images`):**
  - the upstream merge kept both sides of `nsh-core`'s manifest (resvg, flate2 and roxmltree behind `svg`, beside
    upstream's `url` and its native-only socket, file and process dependencies) and regenerated both lockfiles with the
    same added packages as before;
  - the merge with multires decides the overlap in the bundler's image import: an SVG keeps its own size and ships no
    `@Nx` variants (it is drawn for the screen at any ratio), bitmaps keep their variants; `svg_import_ships_no_variants`
    (new) holds it, and the `*.svg` declaration keeps its own doc while the other image modules describe variants. The
    image docs (`docs/dev/images.md`, `images-and-assets`, `project-structure`, `elements/image`, `modules/nsh-image`,
    English and zh-CN) describe both;
  - the SVG tests of the macOS, Windows, Linux and GPUI backends give `ImageSource` multires's new `scales` field
    (`242bf90`);
  - passing again: `cargo test -p nsh-bundler` (51), `--release -p nsh-core --features svg svg::`, `-p
    nsh-platform-macos svg_` (3), `-p nsh-platform-ios --target aarch64-apple-ios-macabi svg_` (3), the GPUI workspace's
    `svg_` (4), multires's `image_scales_`, `asset_variants_`, `variant_` (engine) and `variant_types_` groups, clippy
    `-D warnings` for nsh-core `--features svg`, nsh-bundler, nsh-macros, nsh-engine, nsh-js and nsh-platform-macos,
    `cargo xtask clippy-android`, `nsh-platform-web` on wasm32, `compat::` (19) and both workspaces' `fmt --check`;
  - the Catalyst clippy run (`nsh-platform-ios` and its hosts) failed with `clippy::large_enum_variant` on
    `nsh-platform-ios/src/app.rs` `AppSource`, a file this branch does not touch (upstream `main` `a8ff77c` fails the
    same way, since #145); fork PR #9 (`feat/ios-app-source-lint`) allows the lint as the macOS, Linux and Windows
    `AppSource` do, and with it merged (`83ad831`) the run is clean;
  - the Playwright `svg web` spec passed again against the merged branch (AgentX `check --epic`, 16 checks); the
    on-screen checks above were taken again from the fork's `main` `83ad831` (the gallery's SVG card on macOS, the
    iPhone 17 Pro Max simulator, an Android API 34 emulator and Chromium, light theme) and match; Windows and Linux
    build in CI.

## Limits

- **Fork CI** (`workflow_dispatch` on `feat/svg-images` at `fe677e6`, all green; after the final review added one GPUI test and docs, GPUI ran again at `141b27b`: see below), for what this Mac can't build:
  - Core: https://github.com/dXnSix/nsh/actions/runs/39335104465
  - Linux (clippy, and `nsh-platform-linux` tests including its 3 `svg_` tests): https://github.com/dXnSix/nsh/actions/runs/39335109056
  - Windows (clippy, and `nsh-platform-windows` tests including its 3 `svg_` tests): https://github.com/dXnSix/nsh/actions/runs/39335112612
  - GPUI (macos-15, ubuntu-24.04, windows-latest): https://github.com/dXnSix/nsh/actions/runs/39335115627
  - GPUI again at `141b27b` (its new `svg_draws_no_text_or_embedded_images` passing on all three): https://github.com/dXnSix/nsh/actions/runs/39337810441
  - Examples: https://github.com/dXnSix/nsh/actions/runs/39335118489
- The website's own bun tests (content and links) were not run here (no bun); the pages were checked by reading.
- Native backends don't draw `<text>`, animation or embedded raster images (documented; the bundler warns).
- Windows, Linux and GPUI draw one 4× raster per source, so an SVG shown far larger than 4× its size softens there.
- iOS, macOS and Android draw an SVG for the view's size when it loads, as they downsample bitmaps: a view that
  grows afterwards scales that raster until the image loads again (documented).

## Follow-ups

- Canvas `drawImage` with SVG sources.
- Inline `data:` SVG URIs on iOS.
- Rust `asset!("x.svg")` as an `ImageAsset` (a breaking change for crates).
- Per-view rasterization on Windows, Linux and GPUI, and redrawing an SVG when a view on iOS, macOS or Android grows
  after it loaded.
- Report an imported SVG's bundler size from the web runtime's `onLoad`/`getSize`, so SVGs sized only by a `viewBox`
  report the same size on the web as natively.
- `<text>` on native backends (it would need fonts, which the safety rules keep out for now).
