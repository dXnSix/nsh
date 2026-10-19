use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

use super::*;

const ICON: &[u8] = include_bytes!("testdata/icon.svg");
const LOGO: &[u8] = include_bytes!("testdata/logo.svg");
const HALF_ALPHA: &[u8] = include_bytes!("testdata/half-alpha.svg");
const TEXT: &[u8] = include_bytes!("testdata/text.svg");

/// A red square that fills its 10 × 10 viewport.
const RED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#f00"/></svg>"##;

fn pixel(raster: &Raster, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * raster.width + x) * 4) as usize;
    raster.pixels[i..i + 4].try_into().unwrap()
}

fn transparent(raster: &Raster) -> bool {
    raster.pixels.iter().all(|&b| b == 0)
}

fn svg(attrs: &str) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {attrs}><rect width="1" height="1"/></svg>"#)
        .into_bytes()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// A directory of its own for one test's files.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nsh-svg-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn svg_raster_sniffs_bytes_not_names() {
    assert!(sniff(ICON));
    assert!(
        sniff(LOGO),
        "XML declaration, comment and doctype before <svg"
    );
    assert!(sniff(b"\xEF\xBB\xBF  \n<svg/>"));
    assert!(sniff(b"<!DOCTYPE svg [ <!ENTITY a \"b>\"> ]><svg>"));
    assert!(sniff(&gzip(ICON)), ".svgz");
    assert!(!sniff(b"\x89PNG\r\n\x1a\n"));
    assert!(!sniff(b"\xFF\xD8\xFF\xE0"));
    assert!(!sniff(b"<html><svg></svg></html>"));
    assert!(!sniff(b"<svgx>"));
    assert!(!sniff(b"<?xml version=\"1.0\""));
    assert!(!sniff(b""));
    assert!(!sniff(&gzip(b"not an svg")));
}

#[test]
fn svg_raster_size_from_width_height_and_view_box() {
    assert_eq!(size(ICON).unwrap(), (24.0, 24.0), "viewBox only");
    assert_eq!(
        size(LOGO).unwrap(),
        (48.0, 24.0),
        "width/height win over the viewBox"
    );
    assert_eq!(
        size(&svg(r#"width="30" height="20""#)).unwrap(),
        (30.0, 20.0)
    );
    assert_eq!(
        size(&svg(r#"width="30px" height="1in""#)).unwrap(),
        (30.0, 96.0),
        "CSS units"
    );
    // One side alone takes the other from the viewBox's aspect ratio.
    assert_eq!(
        size(&svg(r#"width="16" viewBox="0 0 32 64""#)).unwrap(),
        (16.0, 32.0)
    );
    assert_eq!(
        size(&svg(r#"height="10" viewBox="0 0 32 64""#)).unwrap(),
        (5.0, 10.0)
    );
    assert_eq!(
        size(&svg(r#"width="0.25in" viewBox="0,0,1,2""#)).unwrap(),
        (24.0, 48.0)
    );
    let doc = br##"<svg xmlns="http://www.w3.org/2000/svg" width="16" viewBox="0 0 32 64"><rect width="32" height="64" fill="#f00"/></svg>"##;
    let one_side = rasterize(doc, 32).unwrap();
    assert_eq!((one_side.width, one_side.height), (16, 32));
    assert_eq!(
        pixel(&one_side, 0, 0),
        [255, 0, 0, 255],
        "the drawing fills the box"
    );
    assert_eq!(pixel(&one_side, 15, 31), [255, 0, 0, 255]);
}

#[test]
fn svg_raster_size_percentages() {
    assert_eq!(
        size(&svg(r#"width="50%" height="100%" viewBox="0 0 24 12""#)).unwrap(),
        (12.0, 12.0)
    );
    assert_eq!(
        size(&svg(r#"width="100%" height="25%" viewBox="0 0 8 8""#)).unwrap(),
        (8.0, 2.0)
    );
    // Without a viewBox, percentages and a missing size fall back to the
    // extent of what the SVG draws, from its origin.
    let drawing = |attrs: &str| {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {attrs}><rect x="2" y="3" width="20" height="10"/></svg>"#)
            .into_bytes()
    };
    assert_eq!(
        size(&drawing(r#"width="100%" height="100%""#)).unwrap(),
        (22.0, 13.0)
    );
    assert_eq!(size(&drawing("")).unwrap(), (22.0, 13.0));
}

#[test]
fn svg_raster_dimensions_fit_the_longest_side() {
    let logo = rasterize(LOGO, 96).unwrap();
    assert_eq!((logo.width, logo.height, logo.size), (96, 48, (48.0, 24.0)));
    assert_eq!(logo.pixels.len(), 96 * 48 * 4);
    let small = rasterize(LOGO, 10).unwrap();
    assert_eq!((small.width, small.height), (10, 5));
    let icon = rasterize(ICON, 54).unwrap();
    assert_eq!((icon.width, icon.height), (54, 54));
    let tall = rasterize(&svg(r#"viewBox="0 0 1 100""#), 50).unwrap();
    assert_eq!(
        (tall.width, tall.height),
        (1, 50),
        "never narrower than a pixel"
    );
    let capped = rasterize(ICON, u32::MAX).unwrap();
    assert_eq!((capped.width, capped.height), (MAX_PX, MAX_PX));
    // Colours land where the SVG puts them.
    assert_eq!(pixel(&logo, 10, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(&logo, 80, 40), [0, 0, 255, 255]);
}

#[test]
fn svg_raster_covering() {
    // A 2:1 logo for a view 96 pixels on its longer side: 96 tall, so a
    // square view's `cover` (96 × 96 of it) shows it unscaled.
    let logo = rasterize_covering(LOGO, 96).unwrap();
    assert_eq!((logo.width, logo.height), (192, 96));
    let tall = rasterize_covering(&svg(r#"viewBox="0 0 10 40""#), 30).unwrap();
    assert_eq!((tall.width, tall.height), (30, 120));
    assert_eq!(
        rasterize_covering(ICON, 54).unwrap().width,
        54,
        "square: as rasterize"
    );
    let capped = rasterize_covering(&svg(r#"viewBox="0 0 100 1""#), 1000).unwrap();
    assert_eq!(capped.width, MAX_PX);
    assert!(rasterize_covering(ICON, 0).is_err());
}

#[test]
fn svg_raster_oversampled() {
    let icon = rasterize_oversampled(ICON).unwrap();
    assert_eq!((icon.width, icon.height), (96, 96));
    let big = rasterize_oversampled(&svg(r#"width="1000" height="500""#)).unwrap();
    assert_eq!(
        (big.width, big.height),
        (MAX_OVERSAMPLED_PX, MAX_OVERSAMPLED_PX / 2)
    );
}

#[test]
fn svg_raster_premultiplied_alpha() {
    let raster = rasterize(HALF_ALPHA, 10).unwrap();
    let [r, g, b, a] = pixel(&raster, 5, 5);
    assert!((127..=128).contains(&a), "alpha {a}");
    assert_eq!((r, g, b), (a, 0, 0), "red is premultiplied by alpha");
}

#[test]
fn svg_raster_svgz() {
    let raster = rasterize(&gzip(LOGO), 96).unwrap();
    assert_eq!(raster, rasterize(LOGO, 96).unwrap());
    assert_eq!(size(&gzip(LOGO)).unwrap(), (48.0, 24.0));
}

#[test]
fn svg_raster_invalid_data_is_an_error() {
    for bytes in [
        &b""[..],
        b"not an svg",
        b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR",
        b"\xff\xfe<\0s\0v\0g\0",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"10\"/>",
        b"<html/>",
        &[0x1f, 0x8b, 0x08, 0x00, 0x01],
    ] {
        assert!(size(bytes).is_err(), "{bytes:?}");
        assert!(rasterize(bytes, 24).is_err(), "{bytes:?}");
    }
    assert!(rasterize(ICON, 0).is_err(), "a zero raster size");
    assert!(!format!("{}", rasterize(b"x", 1).unwrap_err()).is_empty());
}

#[test]
fn svg_raster_icon_fast_enough() {
    rasterize(ICON, 54).unwrap();
    let best = (0..20)
        .map(|_| {
            let start = Instant::now();
            rasterize(ICON, 54).unwrap();
            start.elapsed()
        })
        .min()
        .unwrap();
    log::info!("24×24 icon at 54 px: {best:?}");
    // Debug builds are an order of magnitude slower; the budget is for release.
    if !cfg!(debug_assertions) {
        assert!(best < Duration::from_millis(5), "{best:?}");
    }
}

/// The SVG files the safety tests reference, holding a red square that would
/// show if the reference were followed.
fn sentinel(test: &str) -> PathBuf {
    let path = scratch(test).join("sentinel.svg");
    std::fs::write(&path, RED).unwrap();
    path
}

fn image_href(href: &str) -> Vec<u8> {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10"><image width="10" height="10" href="{href}"/><image width="10" height="10" xlink:href="{href}"/></svg>"#
    )
    .into_bytes()
}

#[test]
fn svg_safety_file_hrefs_are_not_read() {
    let path = sentinel("file");
    let absolute = image_href(path.to_str().unwrap());
    // usvg's own options would read the file: the test would see a read.
    let tree = usvg::Tree::from_data(&absolute, &usvg::Options::default()).unwrap();
    let mut pixmap = tiny_skia::Pixmap::new(10, 10).unwrap();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    assert_eq!(
        pixmap.pixel(5, 5).unwrap().alpha(),
        255,
        "the sentinel is readable"
    );

    assert!(
        transparent(&rasterize(&absolute, 10).unwrap()),
        "absolute path"
    );
    let file_url = image_href(&format!("file://{}", path.display()));
    assert!(transparent(&rasterize(&file_url, 10).unwrap()), "file URL");
    // A relative path resolves against nothing: there is no resources
    // directory, and the resolver ignores one even where it is set.
    let dir = path.parent().unwrap();
    let relative = image_href("sentinel.svg");
    let in_dir = |options: usvg::Options<'static>| {
        let options = usvg::Options {
            resources_dir: Some(dir.into()),
            ..options
        };
        parse_with(&relative, &options)
            .unwrap()
            .root()
            .has_children()
    };
    assert!(
        in_dir(usvg::Options::default()),
        "the sentinel is reachable relatively"
    );
    assert!(!in_dir(options()), "the resolver reads nothing");
    assert!(
        transparent(&rasterize(&relative, 10).unwrap()),
        "relative path"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn svg_safety_urls_are_not_fetched() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    for href in [
        format!("http://127.0.0.1:{port}/a.svg"),
        format!("https://127.0.0.1:{port}/a.svg"),
    ] {
        let raster = rasterize(&image_href(&href), 10).unwrap();
        assert!(transparent(&raster), "{href}");
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        listener.accept().map(|_| ()).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "nothing connected"
    );
}

#[test]
fn svg_safety_data_urls_resolve_nothing() {
    use std::fmt::Write as _;
    let mut encoded = String::new();
    // Percent-encoded, so the test needs no base64 encoder.
    for b in RED.bytes() {
        write!(encoded, "%{b:02X}").unwrap();
    }
    for href in [
        format!("data:image/svg+xml,{encoded}"),
        format!("data:text/plain,{encoded}"),
    ] {
        let raster = rasterize(&image_href(&href), 10).unwrap();
        assert!(transparent(&raster), "{href}");
    }
}

#[test]
fn svg_safety_external_use_resolves_nothing() {
    let path = sentinel("use");
    let doc = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><use href="{}#r"/><use href="sentinel.svg#r"/></svg>"##,
        path.display()
    );
    assert!(transparent(&rasterize(doc.as_bytes(), 10).unwrap()));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn svg_safety_svgz_inflation_is_bounded() {
    // Zeros compress about a thousandfold: a small file, a huge document.
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder
        .write_all(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><!--")
        .unwrap();
    let zeros = vec![b' '; 1 << 20];
    for _ in 0..(MAX_INFLATED >> 20) + 1 {
        encoder.write_all(&zeros).unwrap();
    }
    encoder.write_all(b"--></svg>").unwrap();
    let bomb = encoder.finish().unwrap();
    assert!(bomb.len() < MAX_INFLATED / 100, "{} bytes", bomb.len());
    let error = size(&bomb).unwrap_err();
    assert!(error.to_string().contains("inflated"), "{error}");
    assert!(rasterize(&bomb, 24).is_err());
}

#[test]
fn svg_safety_text_is_not_drawn() {
    let raster = rasterize(TEXT, 40).unwrap();
    assert!(transparent(&raster));
    assert_eq!(raster.size, (40.0, 20.0));
}
