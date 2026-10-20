//! SVG images for the native backends (feature `svg`).
//!
//! The platforms' image decoders (ImageIO, `BitmapFactory`, WIC, GDK) do not
//! read SVG, so every native backend hands sniffed SVG bytes to this module
//! and draws the premultiplied RGBA [`Raster`] it gets back through its usual
//! bitmap path (tint, blur and `resizeMode` included). One renderer, resvg,
//! draws them on every platform, so an SVG looks the same everywhere. The web
//! runtime does not enable the feature: the browser draws SVG itself.
//!
//! An SVG is a self-contained picture here: [`parse`] gives usvg no resources
//! directory and an `<image>` resolver that resolves nothing, so `href`s to
//! files, URLs or `data:` never load, and resvg is built without text, system
//! fonts and raster images, so `<text>` is not drawn. Remote SVGs come through
//! each backend's own fetch, with its capability checks.
//!
//! Sizes are in points: a CSS pixel of the SVG (its root `width` / `height`,
//! else its `viewBox`) is one point. Icons and logos carry a `viewBox`; one
//! without a size and a `viewBox` reaches as far as what it draws.

use std::{fmt, io::Read};

use resvg::{tiny_skia, usvg};

use crate::geometry::{Points, Size};

#[cfg(test)]
mod tests;

/// How much of a compressed SVG (`.svgz`) [`sniff`] inflates to find the
/// root element.
const SNIFF_INFLATED: u64 = 4096;

/// The largest an SVG may be once inflated (`.svgz`), in bytes: larger ones
/// are refused rather than filling memory.
pub const MAX_INFLATED: usize = 16 << 20;

/// The largest side [`rasterize`] draws, in pixels.
pub const MAX_PX: u32 = 8192;

/// How many pixels per point backends that draw one raster at every size
/// (Windows, Linux, GPUI) render an SVG at, so it stays sharp when shown
/// larger than its intrinsic size or on high-density screens.
pub const OVERSAMPLE: f32 = 4.0;

/// The largest side of an oversampled raster ([`rasterize_oversampled`]), in
/// pixels.
pub const MAX_OVERSAMPLED_PX: u32 = 2048;

/// A drawn SVG: premultiplied RGBA, row by row without padding.
#[derive(Debug, Clone, PartialEq)]
pub struct Raster {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes of premultiplied RGBA.
    pub pixels: Vec<u8>,
    /// The SVG's intrinsic size in points (what [`size`] returns).
    pub size: Size<Points>,
}

/// Why an SVG could not be read or drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid SVG: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// Whether `bytes` are an SVG image: the root element is `<svg`, after an
/// optional byte order mark, XML declaration, comments and doctype, or the
/// same compressed with gzip (`.svgz`). File names and MIME types are never
/// trusted; this looks at the bytes.
pub fn sniff(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut head = Vec::new();
        // A truncated read still holds the start of the document.
        let _ = flate2::read::GzDecoder::new(bytes)
            .take(SNIFF_INFLATED)
            .read_to_end(&mut head);
        return sniff_text(&head);
    }
    sniff_text(bytes)
}

fn sniff_text(bytes: &[u8]) -> bool {
    let mut rest = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    loop {
        rest = rest.trim_ascii_start();
        let skipped = if rest.starts_with(b"<?") {
            after(rest, b"?>")
        } else if rest.starts_with(b"<!--") {
            after(rest, b"-->")
        } else if rest.starts_with(b"<!") {
            // A doctype, maybe with an internal subset (`[ ... ]`).
            match (find(rest, b"["), find(rest, b">")) {
                (Some(open), Some(close)) if open < close => {
                    after(&rest[open..], b"]").and_then(|r| after(r, b">"))
                }
                _ => after(rest, b">"),
            }
        } else {
            break;
        };
        match skipped {
            Some(r) => rest = r,
            None => return false,
        }
    }
    rest.starts_with(b"<svg")
        && matches!(
            rest.get(4),
            Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')
        )
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn after<'a>(haystack: &'a [u8], needle: &[u8]) -> Option<&'a [u8]> {
    find(haystack, needle).map(|i| &haystack[i + needle.len()..])
}

/// The options every SVG is parsed with: nothing outside the document is
/// read (no resources directory, `<image>` `href`s resolve to nothing).
fn options() -> usvg::Options<'static> {
    usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

/// Parses an SVG (or gzip-compressed SVG) with the safe [`options`].
pub(crate) fn parse(bytes: &[u8]) -> Result<usvg::Tree, Error> {
    parse_with(bytes, &options())
}

fn parse_with(bytes: &[u8], options: &usvg::Options<'_>) -> Result<usvg::Tree, Error> {
    let inflated;
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        inflated = inflate(bytes)?;
        &inflated[..]
    } else {
        bytes
    };
    let text = std::str::from_utf8(bytes).map_err(|_| Error("not UTF-8".into()))?;
    let doc = xml(text)?;
    let tree = usvg::Tree::from_xmltree(&doc, options).map_err(|e| Error(e.to_string()))?;
    // A root with only a width or only a height takes the other from the
    // viewBox's aspect ratio, as in browsers (usvg makes it 100%).
    match missing_side(&doc, text, tree.size()) {
        Some(fixed) => {
            usvg::Tree::from_xmltree(&xml(&fixed)?, options).map_err(|e| Error(e.to_string()))
        }
        None => Ok(tree),
    }
}

fn xml(text: &str) -> Result<roxmltree::Document<'_>, Error> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    roxmltree::Document::parse_with_options(text, options).map_err(|e| Error(e.to_string()))
}

/// `text` with the root's missing `width` or `height` added, when it has
/// the other one (not a percentage) and a `viewBox`; `size` is usvg's size
/// of the root, which resolves the given side's units.
fn missing_side(doc: &roxmltree::Document<'_>, text: &str, size: usvg::Size) -> Option<String> {
    let root = doc.root_element();
    let view_box: Vec<f32> = root
        .attribute("viewBox")?
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let [_, _, vw, vh] = view_box[..] else {
        return None;
    };
    if vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    let given = |name| {
        root.attribute(name)
            .is_some_and(|v: &str| !v.trim_end().ends_with('%'))
    };
    let added = match (root.has_attribute("width"), root.has_attribute("height")) {
        (true, false) if given("width") => format!(" height=\"{}\"", size.width() * vh / vw),
        (false, true) if given("height") => format!(" width=\"{}\"", size.height() * vw / vh),
        _ => return None,
    };
    // After the root's tag name.
    let start = root.range().start + 1;
    let name_end = text[start..]
        .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .map_or(text.len(), |i| start + i);
    let mut fixed = String::with_capacity(text.len() + added.len());
    fixed.push_str(&text[..name_end]);
    fixed.push_str(&added);
    fixed.push_str(&text[name_end..]);
    Some(fixed)
}

/// Inflates a `.svgz` up to [`MAX_INFLATED`] bytes.
fn inflate(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut inflated = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_INFLATED as u64 + 1)
        .read_to_end(&mut inflated)
        .map_err(|_| Error("malformed gzip data".into()))?;
    if inflated.len() > MAX_INFLATED {
        return Err(Error(format!(
            "larger than {MAX_INFLATED} bytes once inflated"
        )));
    }
    Ok(inflated)
}

/// The intrinsic size of an SVG in points: its root `width` and `height`
/// (percentages of the `viewBox`; one of them alone takes the other from
/// the `viewBox`'s aspect ratio), else the `viewBox`'s size. An SVG with
/// neither a size nor a `viewBox` reaches as far as what it draws.
pub fn size(bytes: &[u8]) -> Result<Size<Points>, Error> {
    let size = parse(bytes)?.size();
    Ok(Size::new(Points(size.width()), Points(size.height())))
}

/// Draws an SVG with its longest side `max_px` pixels long (at most
/// [`MAX_PX`]), keeping its aspect ratio.
pub fn rasterize(bytes: &[u8], max_px: u32) -> Result<Raster, Error> {
    draw(&parse(bytes)?, max_px)
}

/// Draws an SVG with its shorter side `px` pixels long (its longer side at
/// most [`MAX_PX`]), keeping its aspect ratio: the raster of backends that
/// decode per view, where `px` is the view's longer side in pixels. Shown
/// in such a view, an image's shorter side is never longer than that, so
/// the raster is never enlarged, in any resize mode (`cover` included).
pub fn rasterize_covering(bytes: &[u8], px: u32) -> Result<Raster, Error> {
    let tree = parse(bytes)?;
    let (w, h) = (tree.size().width(), tree.size().height());
    let longest = (px as f32 * w.max(h) / w.min(h)).ceil();
    draw(&tree, longest.min(MAX_PX as f32) as u32)
}

/// Draws an SVG at [`OVERSAMPLE`] pixels per point, its longest side at most
/// [`MAX_OVERSAMPLED_PX`]: the raster of backends that draw one bitmap at
/// every size the image is shown at.
pub fn rasterize_oversampled(bytes: &[u8]) -> Result<Raster, Error> {
    let tree = parse(bytes)?;
    let longest = tree.size().width().max(tree.size().height());
    let max_px = (longest * OVERSAMPLE).ceil().min(MAX_OVERSAMPLED_PX as f32) as u32;
    draw(&tree, max_px)
}

fn draw(tree: &usvg::Tree, max_px: u32) -> Result<Raster, Error> {
    let max_px = max_px.min(MAX_PX);
    if max_px == 0 {
        return Err(Error("the raster size is zero".into()));
    }
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    let scale = max_px as f32 / w.max(h);
    let width = ((w * scale).round() as u32).clamp(1, max_px);
    let height = ((h * scale).round() as u32).clamp(1, max_px);
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| Error(format!("cannot allocate {width}×{height} pixels")))?;
    let transform = tiny_skia::Transform::from_scale(width as f32 / w, height as f32 / h);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(Raster {
        width,
        height,
        pixels: pixmap.take(),
        size: Size::new(Points(w), Points(h)),
    })
}
