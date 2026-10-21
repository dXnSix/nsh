# Images

- **Images**: background NSURLSession / file read → ImageIO downsampling decode at the view's pixel size → `NSCache` (purged under memory pressure);
  requests for the same key are coalesced and downloads nobody waits for are cancelled (Android: HttpURLConnection / APK assets →
  `BitmapFactory` power-of-two sampling → `LruCache`, same coalescing). During development `asset:/path` goes through the dev
  server's `/assets/`; `nsh build` copies assets to `out/assets/`, `nsh package` puts them in the app. Cache keys include the decode
  size and the blur radius. `blurRadius` is a stack blur on the decoding thread (`blur.rs`, `Blur.java`: the same algorithm, in
  pixels). `defaultSource` loads after the image request and shows until the image arrives; the image then fades in over
  `fadeDuration` (iOS: `CATransition`; Android: alpha over the placeholder; default 0 on iOS and 300ms on Android, as in React
  Native). Animated images: iOS decodes up to 600 frames into an animated `UIImage` (frames repeated by their delays' common
  divisor) played by a discrete `CAKeyframeAnimation` on `contents` (a tinted animated image shows its first frame); Android
  (API 28+) uses `ImageDecoder` → `AnimatedImageDrawable`, not cached (each view plays its own copy; no blur). `onLoad` reports the
  source's size in pixels on both platforms, whatever size the view decodes at. Downloads are shared per URI (also between
  views decoding at different sizes) and report `onProgress` at most every 100ms; `Image.getSize` / `Image.prefetch` (Rust,
  `nsh_core::runtime`) ask with `Command::ImageInfo`, are answered by the root's `ImageInfo` event (which the core
  resolves, never JS) and keep the downloaded bytes in a data cache (32 MB) until a view
  decodes them. Android `ImageBox` draws its borders above the
  image (as Core Animation's layer border).
- **Pixel-density variants**: an `ImageSource` carries `scales` (`ImageScales`, a `Copy` bitmask over `IMAGE_SCALES`,
  `[1, 1.5, 2, 3, 4]`) when a bundled asset ships `@Nx` siblings. The core picks the variant when it emits
  `MountItem::Image` (`ImageSource::uri_for`, for the source and the `defaultSource` placeholder): React Native's rule, the
  smallest scale at least the surface's `pixel_ratio`, otherwise the largest, and only for `asset:/` URIs. The backends
  load that URI as any other, so their caches and decoding are unchanged; the size in points still comes from the source, so
  layout never depends on the variant. When the pixel ratio changes (`apply_environment`: a window moved to another
  screen, browser zoom) only the images whose pick changes are marked `dirty::IMAGE` and sent again, with no relayout.
  `Image.getSize` / `Image.prefetch` pick the same way, so a prefetch warms the bytes the views load.
  The bundler finds the variants: its image `load` hook (`plugin.rs`) stats the fixed names `<stem>@<s>x.<ext>` for the
  scales of `IMAGE_SCALES` above 1 (no directory listing), only for imports without a density suffix (`logo@3x.png`
  probes nothing and ships alone). The ones found join `Bundle::assets`, so the dev server, `out/assets`, every package,
  `.nshpkg` and the web package's asset map carry them, and the module gains `scales` (1 first) only when there is one:
  an app without variants bundles byte for byte as before. Width and height still come from the imported file; a variant
  whose pixels are off that size times its scale by a pixel or more is warned about on `nsh::build`.
  Rust's `asset!` does the same for a crate image (`nsh-macros` `assets.rs`): `ImageAsset::scales` lists the siblings it
  finds (`assets!()` already ships the whole directory) and `include_bytes!`s them, so a changed variant rebuilds the
  crate. A proc macro cannot read nsh-core, so its `VARIANT_SCALES` repeats `IMAGE_SCALES` above 1: change them together.
- **SVG** (`nsh_core::svg`, cargo feature `svg`, which every native platform crate enables and `nsh-platform-web` never
  does: browsers draw SVG with `<img>`, and the tint mask works on it as on bitmaps). The decoders sniff the bytes
  (`svg::sniff`: `<svg` after a BOM, XML declaration, comments and doctype, or gzip) before their platform decoder and
  draw with resvg: iOS, macOS and Android for the requested pixel size, the view's longer side (the size bitmaps are
  downsampled to), with the SVG's shorter side that long (`svg::rasterize_covering`: shown in the view, an image's
  shorter side never exceeds it, so no resize mode, `cover` included, enlarges the raster; Android through `Native.svg`), Windows, Linux and GPUI once at `svg::OVERSAMPLE` (4) pixels per point,
  at most `MAX_OVERSAMPLED_PX` (2048) a side (`svg::rasterize_oversampled`), since they cache one bitmap per location.
  The premultiplied RGBA raster then takes the bitmap path (blur, color matrix, tint through alpha, cache). `onLoad`
  and `Image.getSize` report the intrinsic size in points (`svg::size`: `width` / `height`, one of them plus the
  `viewBox`'s ratio, or the `viewBox`), measured off the main thread, and a file name's `@2x` does not apply (an SVG import ships no `@Nx` variants either: it is drawn for the screen at any ratio). The
  bundler sizes `.svg` imports with the same `svg::size` and warns (`nsh::build`) about `<text>`, animation and
  `<image>`, which only browsers draw. An SVG reads nothing outside itself: usvg gets no resources directory and an
  `<image>` resolver that resolves nothing (its default one reads files), resvg is built without `text`,
  `system-fonts` and `raster-images` (in the main workspace: GPUI's own tree turns them on, where usvg's empty font
  database and the resolver still draw no text and no embedded images, as the GPUI backend's `svg_` tests check), and
  a `.svgz` inflates to at most `svg::MAX_INFLATED` (16 MiB). Like bitmaps, per-view backends draw an SVG at the
  view's size when it loads: a view that grows later scales that raster until the image loads again.
