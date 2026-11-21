//! A design tool's pictures of a window, drawn in the app's process
//! (`nsh/design`'s `picture`): with no Screen Recording permission and the
//! window never shown, the picture matches the window's content as the
//! window server composites it once shown: at most 2 % of its pixels differ
//! by more than 8 of 255 in a channel (anti-aliased edges differ a little
//! between the two). The whole content, a part of it (`rect`), another
//! scale, and a part outside the content refused.
//!
//! The window is shown only to take the window server's picture to compare
//! with, below the desktop (nobody sees it). AppKit lives on the main
//! thread, so this test runs without libtest's harness; its arguments pick
//! the cases (`whole`, `rect`, `scale`, `outside`), none runs all. Each case
//! prints `design_pictures: <case>: ok`, and the last line says how many
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

    use nsh_core::{element::Root, prelude::*};
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
    const SIZE: (f64, f64) = (300.0, 200.0);

    /// A card on a page: rounded boxes, a border, colours and text.
    #[component]
    fn Card() -> impl IntoElement {
        view()
            .test_id("content")
            .size_full()
            .bg(rgb(0xeef0f4))
            .p_4()
            .child(
                view()
                    .w(px(240.0))
                    .h(px(130.0))
                    .bg(rgb(0xffffff))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(rgb(0xc8ccd6))
                    .p_3()
                    .gap_2()
                    .child(text("Pictures of windows").text_color(rgb(0x1d2230)))
                    .child(
                        view()
                            .flex_row()
                            .gap_2()
                            .child(
                                view()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(9.0))
                                    .bg(rgb(0x3355ff))
                                    .child(text("New").text_color(rgb(0xffffff))),
                            )
                            .child(
                                view()
                                    .w(px(60.0))
                                    .h(px(20.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x22aa66)),
                            ),
                    )
                    .child(text("Drawn in process").text_color(rgb(0x5a6070))),
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

    /// A borderless, transparent window with `root` mounted in it, never
    /// shown: the app's view, the window and the view (its content).
    pub(crate) fn mounted(
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
    pub(crate) fn as_composited(window: &NSWindow) -> Raw {
        // Below the desktop picture: on screen for the window server,
        // seen by nobody.
        window.setLevel(objc2_core_graphics::kCGDesktopWindowLevel as isize - 1);
        window.setAlphaValue(1.0);
        window.orderFrontRegardless();
        spin(0.4);
        let raw = composited(window).expect("the window server's picture");
        window.orderOut(None);
        raw
    }

    /// `raw`'s pixels in `rect` (pixels: x, y, width, height).
    pub(crate) fn crop(raw: &Raw, (x, y, w, h): (u32, u32, u32, u32)) -> Raw {
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in y..y + h {
            let start = ((row * raw.width + x) * 4) as usize;
            pixels.extend_from_slice(&raw.pixels[start..start + (w * 4) as usize]);
        }
        Raw {
            width: w,
            height: h,
            pixels,
        }
    }

    /// The share of pixels whose largest channel differs by more than 8,
    /// and the largest difference.
    pub(crate) fn difference(a: &Raw, b: &Raw) -> (f64, u8) {
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

    /// Whether the picture shows more than one colour.
    fn drawn(raw: &Raw) -> bool {
        let first = raw.pixels.as_chunks::<4>().0[0];
        raw.pixels.as_chunks::<4>().0.iter().any(|p| *p != first)
    }

    /// Within the accepted difference: at most 2 % of the pixels over 8.
    pub(crate) fn within(case: &str, picture: &Raw, live: &Raw) {
        let (share, largest) = difference(picture, live);
        eprintln!(
            "{case}: {:.3} % of pixels differ by more than 8, the largest by {largest}",
            share * 100.0
        );
        assert!(
            share <= 0.02,
            "{case}: {:.2} % of the pixels differ by more than 8",
            share * 100.0
        );
    }

    fn whole(mtm: MainThreadMarker) {
        let (app, window, view) = mounted(mtm, Root::of::<Card>());
        let scale = window.backingScaleFactor();
        let drawn_in = picture(&view, scale, None).expect("a picture");
        assert!(!window.isVisible(), "the picture showed the window");
        assert_eq!(
            (drawn_in.width, drawn_in.height),
            ((SIZE.0 * scale) as u32, (SIZE.1 * scale) as u32)
        );
        assert!(drawn(&drawn_in), "the picture is empty");
        let live = as_composited(&window);
        within("whole", &drawn_in, &live);
        app.stop();
        window.close();
    }

    fn rect(mtm: MainThreadMarker) {
        let (app, window, view) = mounted(mtm, Root::of::<Card>());
        let scale = window.backingScaleFactor();
        let part = NSRect::new(NSPoint::new(20.0, 30.0), NSSize::new(160.0, 90.0));
        let drawn_in = picture(&view, scale, Some(part)).expect("a picture of a part");
        assert!(!window.isVisible(), "the picture showed the window");
        assert_eq!(
            (drawn_in.width, drawn_in.height),
            ((160.0 * scale) as u32, (90.0 * scale) as u32)
        );
        let live = as_composited(&window);
        let s = |v: f64| (v * scale) as u32;
        within(
            "rect",
            &drawn_in,
            &crop(&live, (s(20.0), s(30.0), s(160.0), s(90.0))),
        );
        app.stop();
        window.close();
    }

    fn scale(mtm: MainThreadMarker) {
        let (app, window, view) = mounted(mtm, Root::of::<Card>());
        for (scale, part) in [
            (1.0, None),
            (0.5, None),
            (
                3.0,
                Some(NSRect::new(
                    NSPoint::new(100.0, 50.0),
                    NSSize::new(64.0, 32.0),
                )),
            ),
        ] {
            let drawn_in = picture(&view, scale, part).expect("a picture");
            let (w, h) = part.map_or(SIZE, |r| (r.size.width, r.size.height));
            assert_eq!(
                (drawn_in.width, drawn_in.height),
                ((w * scale).round() as u32, (h * scale).round() as u32),
                "at {scale}"
            );
            assert!(drawn(&drawn_in), "at {scale}: empty");
        }
        assert!(!window.isVisible());
        app.stop();
        window.close();
    }

    fn outside(mtm: MainThreadMarker) {
        let (app, window, view) = mounted(mtm, Root::of::<Card>());
        for part in [
            NSRect::new(NSPoint::new(250.0, 10.0), NSSize::new(100.0, 10.0)),
            NSRect::new(NSPoint::new(-5.0, 0.0), NSSize::new(10.0, 10.0)),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 10.0)),
        ] {
            let error = picture(&view, 2.0, Some(part)).expect_err("refused");
            assert!(
                error.starts_with("ERR_NSH_DESIGN_RECT_OUTSIDE"),
                "{part:?}: {error}"
            );
        }
        app.stop();
        window.close();
    }

    /// A case: its name, and what it runs.
    type Case = (&'static str, fn(MainThreadMarker));

    pub(super) fn run() {
        let mtm = MainThreadMarker::new().expect("the main thread");
        let _app = NSApplication::sharedApplication(mtm);
        let cases: [Case; 4] = [
            ("whole", whole),
            ("rect", rect),
            ("scale", scale),
            ("outside", outside),
        ];
        let picked: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .collect();
        let mut ran = 0;
        for (name, case) in cases {
            if picked.is_empty() || picked.iter().any(|p| p == name) {
                case(mtm);
                println!("design_pictures: {name}: ok");
                ran += 1;
            }
        }
        println!("design_pictures: {ran} cases ok");
    }
}
