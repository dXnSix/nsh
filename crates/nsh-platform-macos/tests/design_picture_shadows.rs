//! A design tool's in-process pictures draw the outer box shadows as the
//! window server composites them: Core Animation's in-process rendering
//! draws none of NSH's shadow casters (`visual.rs`), masked or not, so the
//! picture draws them itself (`picture.rs`). Each case is a never-shown
//! window of views with outer shadows on an opaque page, pictured in
//! process and compared with the window server's picture of it shown below
//! the desktop (nobody sees it): at most 2 % of the pixels may differ by
//! more than 8 of 255. A rounded box on an opaque background (plain
//! casters) and on a translucent one (masked casters), a box with
//! differing corners, and a box with several shadows.
//!
//! AppKit lives on the main thread, so this test runs without libtest's
//! harness; its arguments pick the cases (`opaque`, `translucent`,
//! `corners`, `several`), none runs all. Each case prints
//! `design_picture_shadows: <case>: ok`, and the last line says how many
//! ran.

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
mod macos {
    use std::time::{Duration, Instant};

    use nsh_core::{BoxShadow, Color, element::Root, prelude::*};
    use nsh_platform_macos::{
        AppOptions, AppView,
        dev::{Raw, composited, picture},
        objc2::{MainThreadMarker, MainThreadOnly, rc::Retained},
        objc2_app_kit::{
            NSAccessibility, NSApplication, NSBackingStoreType, NSColor, NSView, NSWindow,
            NSWindowStyleMask,
        },
        objc2_foundation::{NSDate, NSPoint, NSRect, NSRunLoop, NSSize},
    };

    /// The window's content size, in points.
    const SIZE: (f64, f64) = (320.0, 220.0);

    /// An outer shadow: offsets, blur, spread and colour (`0xRRGGBBAA`).
    fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: u32) -> BoxShadow {
        BoxShadow {
            offset_x: px(x),
            offset_y: px(y),
            blur: px(blur),
            spread: px(spread),
            color: Color(color),
            inset: false,
        }
    }

    /// A page with a box at its middle.
    fn page(card: impl IntoElement) -> impl IntoElement {
        view()
            .test_id("content")
            .size_full()
            .bg(rgb(0xf4f5f8))
            .items_center()
            .justify_center()
            .child(card)
    }

    /// A rounded box on an opaque background, a coloured shadow below it
    /// (the shadow of a hero card: `0 12px 32px #ff3d7740`).
    #[component]
    fn Opaque() -> impl IntoElement {
        page(
            view()
                .w(px(200.0))
                .h(px(110.0))
                .rounded(px(18.0))
                .bg(rgb(0xffffff))
                .shadow([shadow(0.0, 12.0, 32.0, 0.0, 0xff3d_7740)])
                .child(text("Hero")),
        )
    }

    /// A rounded box with a translucent background: its shadow must not
    /// show through it.
    #[component]
    fn Translucent() -> impl IntoElement {
        page(
            view()
                .w(px(200.0))
                .h(px(110.0))
                .rounded(px(18.0))
                .bg(rgba(0xffff_ff80))
                .shadow([shadow(0.0, 10.0, 24.0, 0.0, 0x1020_6060)])
                .child(text("Glass")),
        )
    }

    /// A box with differing corners.
    #[component]
    fn Corners() -> impl IntoElement {
        page(
            view()
                .w(px(180.0))
                .h(px(120.0))
                .rounded_tl(px(32.0))
                .rounded_tr(px(4.0))
                .rounded_br(px(20.0))
                .rounded_bl(px(0.0))
                .bg(rgb(0xffffff))
                .shadow([shadow(6.0, 8.0, 18.0, 2.0, 0x0000_0050)])
                .child(text("Corners")),
        )
    }

    /// A box with several shadows, one spread.
    #[component]
    fn Several() -> impl IntoElement {
        page(
            view()
                .w(px(190.0))
                .h(px(100.0))
                .rounded(px(12.0))
                .bg(rgb(0xffffff))
                .shadow([
                    shadow(0.0, 1.0, 2.0, 0.0, 0x0000_0030),
                    shadow(0.0, 8.0, 24.0, -4.0, 0x3355_ff50),
                    shadow(-10.0, 0.0, 12.0, 4.0, 0x22aa_6640),
                ])
                .child(text("Layers")),
        )
    }

    fn spin(seconds: f64) {
        let soon = NSDate::dateWithTimeIntervalSinceNow(seconds);
        NSRunLoop::currentRunLoop().runUntilDate(&soon);
    }

    fn until(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "{what} never happened");
            spin(0.02);
        }
    }

    fn has(view: &NSView, id: &str) -> bool {
        view.accessibilityIdentifier()
            .is_some_and(|i| i.to_string() == id)
            || view.subviews().iter().any(|v| has(&v, id))
    }

    fn mounted(
        mtm: MainThreadMarker,
        root: Root,
    ) -> (std::rc::Rc<AppView>, Retained<NSWindow>, Retained<NSView>) {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SIZE.0, SIZE.1));
        // SAFETY: a plain borderless window, released by the test's end.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setHasShadow(false);
        window.setIgnoresMouseEvents(true);
        let app = AppView::new(
            mtm,
            AppOptions {
                size: (SIZE.0 as i32, SIZE.1 as i32),
                ..AppOptions::default()
            },
        );
        let view = app.view();
        window.setContentView(Some(&view));
        app.load(root.into());
        until("the first mount", || has(&view, "content"));
        spin(0.3);
        (app, window, view)
    }

    /// The window server's picture of `window`, shown for it below the
    /// desktop (nobody sees it) and ordered out again.
    fn as_composited(window: &NSWindow) -> Raw {
        window.setLevel(objc2_core_graphics::kCGDesktopWindowLevel as isize - 1);
        window.setAlphaValue(1.0);
        window.orderFrontRegardless();
        spin(0.4);
        let raw = composited(window).expect("the window server's picture");
        window.orderOut(None);
        raw
    }

    /// The share of pixels whose largest channel differs by more than 8,
    /// and the largest difference.
    fn difference(a: &Raw, b: &Raw) -> (f64, u8) {
        assert_eq!(
            (a.width, a.height),
            (b.width, b.height),
            "pictures of different sizes"
        );
        let mut over = 0usize;
        let mut largest = 0u8;
        for (p, q) in a
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.pixels.as_chunks::<4>().0.iter())
        {
            let d = p
                .iter()
                .zip(q)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0);
            largest = largest.max(d);
            if d > 8 {
                over += 1;
            }
        }
        (over as f64 / (a.pixels.len() / 4) as f64, largest)
    }

    /// Writes both pictures to `NSH_PICTURE_DUMP` (a directory), when set,
    /// for looking at a failing case.
    fn dump(case: &str, drawn: &Raw, live: &Raw) {
        let Some(dir) = std::env::var_os("NSH_PICTURE_DUMP") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        for (name, raw) in [("drawn", drawn), ("live", live)] {
            let _ = std::fs::write(
                dir.join(format!("{case}-{name}-{}x{}.rgba", raw.width, raw.height)),
                &raw.pixels,
            );
        }
    }

    fn case(mtm: MainThreadMarker, name: &str, root: Root, (bw, bh): (f64, f64)) {
        let (app, window, view) = mounted(mtm, root);
        let scale = window.backingScaleFactor();
        let drawn = picture(&view, scale, None).expect("a picture");
        assert!(!window.isVisible(), "the picture showed the window");
        let live = as_composited(&window);
        dump(name, &drawn, &live);
        let (share, largest) = difference(&drawn, &live);
        eprintln!(
            "{name}: {:.3} % of pixels differ by more than 8, the largest by {largest}",
            share * 100.0
        );
        assert!(
            share <= 0.02,
            "{name}: {:.2} % of the pixels differ by more than 8",
            share * 100.0
        );
        // The shadow is in the picture: without it, the page around the box
        // (in the window's middle) would be the page's colour alone.
        let page = [0xf4, 0xf5, 0xf8, 0xff];
        let (bw, bh) = (bw * scale, bh * scale);
        let (x0, y0) = (
            (f64::from(drawn.width) - bw) / 2.0,
            (f64::from(drawn.height) - bh) / 2.0,
        );
        let shaded = drawn
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                let (x, y) = (
                    (i % drawn.width as usize) as f64,
                    (i / drawn.width as usize) as f64,
                );
                let in_box =
                    x >= x0 - 2.0 && x <= x0 + bw + 2.0 && y >= y0 - 2.0 && y <= y0 + bh + 2.0;
                !in_box && p.iter().zip(page).any(|(a, b)| a.abs_diff(b) > 8)
            })
            .count();
        assert!(
            shaded > (drawn.width * drawn.height / 100) as usize,
            "{name}: the page around the box shows no shadow ({shaded} pixels)"
        );
        app.stop();
        window.close();
    }

    /// A case: its name, its app and its box's size in points.
    type Case = (&'static str, fn() -> Root, (f64, f64));

    pub(super) fn run() {
        let mtm = MainThreadMarker::new().expect("the main thread");
        let _app = NSApplication::sharedApplication(mtm);
        // Each case's box size, in points.
        let cases: [Case; 4] = [
            ("opaque", Root::of::<Opaque>, (200.0, 110.0)),
            ("translucent", Root::of::<Translucent>, (200.0, 110.0)),
            ("corners", Root::of::<Corners>, (180.0, 120.0)),
            ("several", Root::of::<Several>, (190.0, 100.0)),
        ];
        let picked: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .collect();
        let mut ran = 0;
        for (name, root, size) in cases {
            if picked.is_empty() || picked.iter().any(|p| p == name) {
                case(mtm, name, root(), size);
                println!("design_picture_shadows: {name}: ok");
                ran += 1;
            }
        }
        println!("design_picture_shadows: {ran} cases ok");
    }
}
