# image: Show SVG images on every backend

Draft of an upstream issue for `longbridge/nsh`, to be posted by the owner before the PR
([`feat-svg-images.md`](../prs/feat-svg-images.md)). Not posted.

---

**Title:** `image: Show SVG images, rasterized at the size they are shown`

## The problem

`<Image>` shows PNG, JPEG, GIF, WebP and HEIC. Vector art that has to stay sharp at every size has one path today:
`Canvas` with `new Path2D(d)`, which is right for single-path, single-colour glyphs and is what the docs recommend
for icons. It doesn't cover:

- **Multi-colour logos and illustrations.** Team crests, brand marks, flags and empty-state illustrations have many
  paths, fills, gradients, opacity and transforms. Porting each to canvas calls by hand is slow and lossy.
- **Remote SVG.** A server, a CMS or a sportsbook feed sends logos as SVG URLs. Apps can't turn those into `Path2D`
  calls without shipping an SVG parser in JS.
- **AI-generated UI.** NSH is built for UI generated and delivered at run time. Models produce SVG far more reliably
  than canvas code, and an `<Image src={…svg}>` is the shape they expect.
- **Designer workflow.** Designers export SVG from Figma or Sketch. With raster images only, every icon is exported at
  @1x/@2x/@3x and re-exported for every size change.

## Proposal: hybrid rendering

- **Web:** the browser draws SVG. `nsh.js` shows images with `<img>` and tints them with a CSS mask, and both already
  handle SVG. The only change is that the bundler accepts `.svg`, so **the web runtime does not change and grows by
  0 bytes.**
- **Native (iOS, macOS, Android, Windows, Linux, GPUI):** one Rust renderer, [`resvg`](https://github.com/linebender/resvg)
  (pure Rust, MIT/Apache-2.0), draws SVGs on the backends' existing image worker threads. Each backend sniffs SVG
  bytes in its decode path and turns the premultiplied RGBA raster into the bitmap type it already draws (`CGImage`,
  `Bitmap`, a WIC/Direct2D bitmap, a `GdkTexture`, GPUI's frame), so tint, blur, `resizeMode`, caching, `onLoad`,
  `Image.getSize` and `Image.prefetch` keep working unchanged.
- **Raster size:**
  - iOS, macOS and Android already decode each view at its pixel size and cache by it, so an SVG is drawn for that
    size: its shorter side at the view's longer side, so no resize mode (`cover` included) enlarges it. As for
    bitmaps, a view that grows after the image loaded scales that raster.
  - Windows, Linux and GPUI cache one bitmap per source and scale when drawing, so they draw the SVG at its intrinsic
    size × 4 (at most 2048 px) and scale down from there.
  - `onLoad` and `getSize` report an SVG's intrinsic size in points (a CSS pixel of the SVG is a point).
- **Imports:** `import logo from "./logo.svg"` gives an `ImageAsset` sized by the SVG's `width`/`height` or
  `viewBox`, computed by the same code the backends use. `@2x` suffixes don't apply. An invalid SVG fails the build;
  one using features that native backends don't draw (below) builds with a warning naming the file.

Platform renderers were considered and rejected: iOS and Android have no public SVG decoder, macOS only a private
one, Windows' Direct2D SVG support is a subset, and Linux would need librsvg at run time. One renderer also keeps NSH's
rule that things look the same everywhere.

## Size

Measured on the draft branch against `main` (release builds on an Apple-silicon Mac):

| Build | Added |
|---|---|
| macOS release runner (`nsh-macos-app`) | +866 KB |
| iOS runner (`nsh-ios-app`, device) | +879 KB |
| Android `libnsh.so` | +1,086 KB (arm64-v8a), +1,157 KB (x86_64) |
| GPUI runner | +358 KB (resvg / usvg 0.46 are already in GPUI's tree) |
| Published web app (`nsh.wasm`, `nsh.js`) | 0 bytes: byte-identical |

Linking resvg into `nsh.wasm` instead would add about 730 KB raw / 217 KB brotli (about +23% of the gallery's web
first load), which this design avoids.

A 24 × 24 icon rasterizes at 54 px in about 0.02 ms, a 512 px logo in 0.4–1.2 ms, on the image worker threads.

## Security

An SVG is treated as a self-contained picture:

- `usvg`'s default `<image href>` resolver reads arbitrary local files. Left on, a remote SVG could read app files and
  bypass the sandbox. The rasterizer sets no resources directory and an `<image>` resolver that resolves nothing:
  file paths, `file:`, `http(s):` and `data:` references draw nothing and touch neither the file system nor the
  network. External `<use>` references resolve nothing.
- `resvg` is built without `text`, `system-fonts` and `raster-images` in the main workspace, so no font files are read
  and no raster codecs are linked. GPUI's own tree enables them; there usvg's empty font database and the resolver still
  draw no text and no embedded images, which a GPUI test checks.
- Remote SVGs come through each backend's existing image fetch, with the capability checks and redirect re-checks
  bitmaps already get.
- A `.svgz` inflates to at most 16 MiB (`svg::MAX_INFLATED`); a larger one (a gzip bomb) is refused instead of
  filling memory, which usvg's own `from_data` doesn't guard against.
- Bytes are sniffed (`<svg` after an optional XML declaration, comments and doctype, or gzip for `.svgz`); file
  extensions and MIME types are never trusted.

Tests cover each of these, including one that would observe a read of a sentinel file.

## Native differences (documented)

Browsers draw everything; native backends draw shapes, paths, fills, strokes, gradients, patterns, clipping, masks,
opacity, filters and transforms. They don't draw `<text>`, SMIL/CSS animation or embedded raster images. The bundler
warns about imported files that use them.

Sizing follows browsers for roots with a `width` and/or `height` in lengths: both, or one of them with the other from
the `viewBox`'s aspect ratio. Other roots differ for remote images: browsers give an `<img>` a default size of 300 × 150
(fitted to the `viewBox`'s aspect ratio when there is one, so a 24 × 24 `viewBox` shows at 150 × 150), while native
backends use the `viewBox`'s size, the percentage of the `viewBox` for a percentage root, and without a `viewBox` the
extent of the drawing. Imported SVGs are laid out with the size the bundler computes, on every platform alike.

## Question for the maintainers: where should the rasterizer live?

The draft puts it in `nsh-core` as a module behind a cargo feature `svg` (default off), enabled by the native
platform crates and never by `nsh-platform-web`:

1. **`nsh-core` feature (the draft).** One implementation of the safety settings, sizing and tests; the bundler
   reuses it for import sizes. `nsh-core` stays platform-independent (resvg is pure Rust).
2. **An `nsh-engine` feature.** Closer to where platforms already get shared services, but the bundler would need the
   engine for sizes.
3. **Per-backend modules.** No shared feature, but six copies of the options and sizing.

The PR follows your preference.

## Follow-ups (not in the first PR)

- Canvas `drawImage` with SVG sources.
- Inline `data:` SVG URIs on iOS.
- Rust `asset!("x.svg")` as an `ImageAsset` (a breaking change for crates).
- Per-view rasterization on Windows, Linux and GPUI, and redrawing an SVG when a view on iOS, macOS or Android grows
  after it loaded.
- Report an imported SVG's bundler size from the web runtime's `onLoad`/`getSize`, so SVGs sized only by a `viewBox`
  report the same size on the web as natively.
- `<text>` on native backends (it would need fonts, which the safety rules keep out for now).
