# Images: load the @1x / @2x / @3x file that fits the screen

Draft of an upstream issue for `longbridge/nsh`, to be posted by the owner before the PR
([`docs/nsh-fork/prs/feat-multires-images.md`](../prs/feat-multires-images.md)). Not posted.

## Problem

An imported image ships exactly the file that is imported. `import flag from "./flag@3x.png"` ships `flag@3x.png`,
and the `@3x` suffix only turns pixels into points. The docs say so: "There is no automatic choice between
`logo@2x.png` and `logo@3x.png`: the file you import is the one that ships." To look sharp everywhere, an app imports
its densest file, so every screen downloads, decodes and keeps it:

- **Web:** a 1× desktop or 2× laptop downloads the 3× bytes. For a list of 216 country flags (34×24 pt each) that is
  296,564 bytes on every screen. 119,465 bytes (1×) or 208,451 bytes (2×) would do.
- **Linux, Windows and GPUI** decode the full file (Linux `gdk::Texture::from_bytes`, falling back to
  `image::load_from_memory`; Windows WIC; GPUI `image`) and only then scale it to the view, so a 1× or 2× desktop
  decodes and holds 9× or 2.25× the pixels it shows.
- **Android** halves the decode size by powers of two while it stays at least the view's size (`Images.java`,
  `inSampleSize`), so a 3× file decodes at full size on 2× and 2.625× phones (only a 1.5× phone gets it halved).
- iOS and macOS downsample while decoding (`kCGImageSourceThumbnailMaxPixelSize`), so there only the bytes on disk
  are wasted.

React Native picks among `@1x`/`@2x`/`@3x` siblings automatically, and apps ported from it (or from asset pipelines
that export 1x/2x/3x) expect it.

## Proposal

Pick the variant in the core, ship the siblings from the bundler, and change no platform backend.

1. **Bundler.** For an image imported *without* a density suffix (`import logo from "./logo.png"`), look for the fixed
   sibling names `logo@1.5x.png`, `logo@2x.png`, `logo@3x.png` and `logo@4x.png`: stat calls only, no directory
   listing. Ship the ones found with the app (they join the bundle's assets, so the dev server, `nsh build`, every
   `nsh package` target, `.nshpkg` and the web asset map carry them). The import gains `scales` (`[1, 2, 3]`) only
   when there is a variant. The width and height still come from the imported file. A variant whose pixels are not
   that size times its scale gets a build warning.
2. **Core.** `ImageSource` carries the scales as a `Copy` bitmask. When the core emits `MountItem::Image`, it rewrites
   `asset:/logo.png` to the variant for the surface's pixel ratio (React Native's rule: the smallest scale at least the
   ratio, otherwise the largest), for the source and the `defaultSource` placeholder. The size in points is unchanged,
   so layout never depends on the variant. When the pixel ratio changes (a window moves to another screen, browser
   zoom), only the images whose pick changes are sent again, with no relayout. `Image.getSize` and `Image.prefetch`
   pick the same way, so a prefetch warms the bytes the views load.
3. **Backends: no change.** Every backend already loads one `asset:/` URI and keys its caches by it. On the web,
   `nsh.js` maps the chosen `asset:/logo@2x.png` through the package's hashed asset map, so only that file downloads.
4. **Rust parity.** `asset!("logo.png")` lists the scales of the crate's siblings in `ImageAsset::scales`
   (`assets!()` already ships them), and Rust components pick the same way.

## API

- TS: `ImageAsset.scales?: number[]` (1 is the imported file; absent without variants).
- Rust: `nsh_core::IMAGE_SCALES` (`[1.0, 1.5, 2.0, 3.0, 4.0]`), `nsh_core::ImageScales` (`Copy` set with `pick`,
  `from_scales`, `with`, `iter`, `contains`, `is_empty`), `ImageSource::scales`, `ImageSource::uri_for(pixel_ratio)`,
  `nsh_core::ui::ImageAsset::scales: &'static [f32]`.

## Compatibility

- Importing an `@Nx` file (`./logo@3x.png`) probes nothing and ships that file alone, exactly as today.
- An app without variant siblings bundles byte for byte as before and emits the same mount items.
- Remote (`http(s)`) and file URIs ignore scales. `ImageSource` without scales behaves as today.
- An app with only 1x/2x/3x files gets `@2x` on a 1.5× Android phone, decoded at full size, where the halved `@3x`
  was slightly smaller in memory (68 px against 51 px wide for a 34 pt image); fewer bytes download. An `@1.5x` file
  avoids that.
- Canvas `drawImage` keeps drawing the imported file. That is documented, and canvas apps import the `@3x` file.
- Breaking for Rust struct literals only: `ImageSource { .. }` and `ImageAsset { .. }` gain a field.
- Behaviour changes for an app that already has `@2x`/`@3x` siblings next to a file it imports without a suffix: today
  only that file ships; with this change its siblings ship and load on denser screens.

## Measurements (prototype on the fork)

| 216-flag list (GameX's flags, 34×24 pt), bytes downloaded on the web | 1× | 2× | 3× |
|---|---|---|---|
| Today: `@3x` imports | 296,564 | 296,564 | 296,564 |
| Variants | 119,465 (−59.7%) | 208,451 (−29.7%) | 296,564 |

The page's asset map grows by two entries per image with variants: for 216 flags, about 29 KB raw and about 5 KB
gzipped, in `index.html`.

`nsh trace` of `examples/gallery` (which has no variants), main against the prototype over 15 interleaved runs: the
first commit is identical in every run (973 ops, 937 mount items, 247 nodes), and its fastest runs take the same time
(minimum 508 µs against 484 µs, lower quartile 528 µs against 529 µs). The medians (567 µs and 642 µs) and the slow
runs are dominated by load on the machine.

## Alternatives considered

- **`srcset` / `image-set()` on the web.** It would add a web-only mount encoding, change the tint mask
  (`mask: url(src)`), and leave the other backends without a pick: two selection paths instead of one.
- **Android density buckets (`drawable-xhdpi`) and iOS asset catalogs.** NSH ships assets in `.apk`s and signed
  `.nshpkg` packages under `asset:/` URIs, so neither format can carry them, and each platform would pick differently.
- **React Native's source arrays (`source={[{ uri, scale }]}`).** That is an API for compatibility only; the
  contributing guide rejects changes whose only purpose is React Native compatibility. Siblings found at bundle time
  need no API at all.
- **Pick in JS.** Limits and paths are decided in Rust, and JS does as little as possible. JS hands over only the scale
  numbers.

## Out of scope

- Remote images (choosing a URL per density is the server's business).
- SVG images (separate proposal).
- Rebuilding during `nsh run` when an image file is added: images are not watched today, so a new variant shows after
  a restart or a source edit.
