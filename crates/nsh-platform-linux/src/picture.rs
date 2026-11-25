//! Pictures of a window's content drawn in the app's process on Linux, for a
//! design tool (`nsh/design`'s `settled` and `picture`, [`crate::design`]).
//!
//! - **Settled**: a window settles by the core's rule ([`core::Settling`]) on
//!   the batches its surface applied ([`Surface::applied`]) and whether a
//!   finite animation or transition runs ([`Surface::animating`]), looked at
//!   every [`core::SETTLE_POLL`]. Endless loops and endless keyframe
//!   animations do not block settling.
//! - **Picture**: once what the engine sent before the question is applied and
//!   a frame has followed, the window's root view is snapshotted into a
//!   `gtk::Snapshot` at the requested scale and region (NOT via
//!   `GtkWidgetPaintable`, which drops render nodes on queued draws), rendered
//!   offscreen with GSK's renderer (tiled when larger than [`TILE_MAX`]), and
//!   downloaded as premultiplied sRGB RGBA (`R8g8b8a8Premultiplied`) with no
//!   row padding. Outer box shadows are drawn directly by `decor` in GSK, and
//!   endless animations stand at their first frame while the picture is taken.

use std::time::{Duration, Instant};

use gtk4::{self as gtk, gdk, glib, graphene, gsk, prelude::*};
use nsh_core::runtime::design::{self as core, Failure, codes};

use crate::{surface::Surface, view::NshView};

/// About a display frame, in milliseconds: how long a picture waits after
/// pending batches are drained.
const FRAME_MS: u64 = 17;

/// Largest pixel tile side rendered in one GSK texture pass: keeps large
/// pictures safely within software/GL texture limits.
const TILE_MAX: u32 = 4096;

/// Gives design mode this platform's pictures; once, by
/// [`crate::design::start`] after it connected.
pub(crate) fn install() {
    core::set_picture_hooks(core::PictureHooks {
        settled: Box::new(|ask| {
            glib::MainContext::default().invoke_with_priority(glib::Priority::DEFAULT, move || {
                settle(ask, None);
            });
        }),
        picture: Box::new(|ask| {
            glib::MainContext::default().invoke_with_priority(glib::Priority::DEFAULT, move || {
                picture(ask, false);
            });
        }),
    });
}

fn no_window(id: &str) -> Failure {
    Failure::new(codes::NO_WINDOW, format!("no window {id:?} is open"))
}

/// Looks at window `ask.window` until it settles by `settling` (made at the
/// first look).
fn settle(ask: core::SettleAsk, settling: Option<core::Settling>) {
    let Some((surface, _, _)) = crate::design::window(&ask.window) else {
        let failure = no_window(&ask.window);
        ask.reply.send(Err(failure));
        return;
    };
    let now = Instant::now();
    let mut settling = settling.unwrap_or_else(|| core::Settling::new(now, surface.applied()));
    match settling.step(now, surface.applied(), surface.animating()) {
        Some(settled) => ask.reply.send(Ok(settled)),
        None => {
            glib::timeout_add_local_once(core::SETTLE_POLL, move || {
                settle(ask, Some(settling));
            });
        }
    }
}

/// Draws `ask`'s picture once what the engine sent before it is applied and a
/// frame has followed (`waited`).
fn picture(ask: core::PictureAsk, waited: bool) {
    let Some((surface, root, window)) = crate::design::window(&ask.window) else {
        let failure = no_window(&ask.window);
        ask.reply.send(Err(failure));
        return;
    };
    if !waited {
        surface.drain();
        glib::timeout_add_local_once(Duration::from_millis(FRAME_MS), move || {
            picture(ask, true);
        });
        return;
    }
    let drawn = draw(&surface, &root, &window, &ask).and_then(|(width, height, pixels)| {
        std::fs::write(&ask.path, &pixels)
            .map_err(|e| {
                Failure::new(
                    codes::PATH,
                    format!("cannot write the picture to {}: {e}", ask.path.display()),
                )
            })
            .map(|()| core::Drawn { width, height })
    });
    ask.reply.send(drawn);
}

/// Stands endless keyframe animations of `view` and its subtree at their first
/// frame, noting each view's previous state in `out` and invalidating its
/// cached GSK render node.
fn stand_keyframes(view: &NshView, out: &mut Vec<(NshView, nsh_core::transition::Shown)>) {
    let stood = view
        .state_mut()
        .live
        .as_deref_mut()
        .and_then(|live| live.stand_at_loop_start());
    if let Some(shown) = stood {
        view.changed();
        out.push((view.clone(), shown));
    }
    for child in view.child_views() {
        stand_keyframes(&child, out);
    }
}

/// Renders `node` over `bounds` (`w` × `h` pixels) using `renderer` and
/// downloads premultiplied RGBA bytes without row padding.
fn render_rgba(
    renderer: &gsk::Renderer,
    node: &gsk::RenderNode,
    x: f32,
    y: f32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    let bounds = graphene::Rect::new(x, y, w as f32, h as f32);
    let texture = renderer.render_texture(node, Some(&bounds));
    let mut downloader = gdk::TextureDownloader::new(&texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied);
    let (bytes, stride) = downloader.download_bytes();
    let row_bytes = (w as usize) * 4;
    if stride == row_bytes && bytes.len() >= row_bytes * (h as usize) {
        return bytes[..row_bytes * (h as usize)].to_vec();
    }
    let mut out = vec![0u8; row_bytes * (h as usize)];
    for row in 0..(h as usize) {
        let src_start = row * stride;
        let dst_start = row * row_bytes;
        out[dst_start..dst_start + row_bytes]
            .copy_from_slice(&bytes[src_start..src_start + row_bytes]);
    }
    out
}

/// Window `root`'s content (or `ask.rect` of it, which must lie within it)
/// drawn at `ask.scale`: its size in pixels and its premultiplied RGBA rows.
fn draw(
    surface: &std::rc::Rc<Surface>,
    root: &NshView,
    window: &gtk::Window,
    ask: &core::PictureAsk,
) -> Result<(u32, u32, Vec<u8>), Failure> {
    let (width, height) = {
        let s = root.state();
        let (fw, fh) = (s.frame.size.width.0, s.frame.size.height.0);
        if fw > 0.0 && fh > 0.0 {
            (fw, fh)
        } else {
            (window.width() as f32, window.height() as f32)
        }
    };
    let (x, y, w, h) = ask.rect.map_or((0.0, 0.0, width, height), |r| {
        (r.origin.x.0, r.origin.y.0, r.size.width.0, r.size.height.0)
    });
    // A hair over, from points that went through `f32`.
    const SLACK: f32 = 0.01;
    if x < -SLACK
        || y < -SLACK
        || x + w > width + SLACK
        || y + h > height + SLACK
        || w <= 0.0
        || h <= 0.0
    {
        return Err(Failure::new(
            codes::RECT_OUTSIDE,
            format!(
                "the rect {:?} is not within the window's content ({width} x {height} points)",
                (x, y, w, h)
            ),
        ));
    }
    let pw = (f64::from(w) * ask.scale).round().max(1.0) as u32;
    let ph = (f64::from(h) * ask.scale).round().max(1.0) as u32;

    // Stand endless animations at the start of their current run for the
    // picture only, then restore them right after snapshotting.
    let mut still = surface.anim.borrow().at_loop_starts(surface.now());
    still.write(&surface.mounter.borrow());
    let mut keyframes = Vec::new();
    stand_keyframes(root, &mut keyframes);

    // Ensure every widget in the window has a current allocation before
    // snapshotting (a queued resize leaves widgets unallocated until the next
    // frame).
    let alloc_w = window.width().max(width.ceil() as i32).max(1);
    let alloc_h = window.height().max(height.ceil() as i32).max(1);
    window.allocate(alloc_w, alloc_h, -1, None);

    let snapshot = gtk::Snapshot::new();
    let scale = ask.scale as f32;
    snapshot.scale(scale, scale);
    snapshot.translate(&graphene::Point::new(-x, -y));
    if let Some(parent) = root.parent() {
        parent.snapshot_child(root, &snapshot);
    } else {
        root.draw(&snapshot);
    }

    // Restore live animations before rasterizing/downloading.
    for (view, shown) in keyframes {
        if let Some(live) = view.state_mut().live.as_deref_mut() {
            live.put_back(shown);
        }
        view.changed();
    }
    surface.anim.borrow_mut().write(&surface.mounter.borrow());

    let node = snapshot.to_node().unwrap_or_else(|| {
        gsk::ColorNode::new(
            &gdk::RGBA::TRANSPARENT,
            &graphene::Rect::new(0.0, 0.0, pw as f32, ph as f32),
        )
        .upcast()
    });

    // Prefer the window's realized renderer; fall back to an offscreen Cairo
    // renderer if the window has none yet.
    let fallback_renderer;
    let renderer: &gsk::Renderer = if let Some(r) = window.renderer() {
        fallback_renderer = Some(r);
        fallback_renderer.as_ref().unwrap()
    } else {
        let cairo: gsk::Renderer = gsk::CairoRenderer::new().upcast();
        cairo
            .realize_for_display(&WidgetExt::display(window))
            .map_err(|err| {
                Failure::new(
                    codes::NO_WINDOW,
                    format!("the window has no renderer: {err}"),
                )
            })?;
        fallback_renderer = Some(cairo);
        fallback_renderer.as_ref().unwrap()
    };

    let pixels = if pw <= TILE_MAX && ph <= TILE_MAX {
        render_rgba(renderer, &node, 0.0, 0.0, pw, ph)
    } else {
        let full_stride = (pw as usize) * 4;
        let mut pixels = vec![0u8; full_stride * (ph as usize)];
        let mut ty = 0u32;
        while ty < ph {
            let th = (ph - ty).min(TILE_MAX);
            let mut tx = 0u32;
            while tx < pw {
                let tw = (pw - tx).min(TILE_MAX);
                let tile = render_rgba(renderer, &node, tx as f32, ty as f32, tw, th);
                let tile_stride = (tw as usize) * 4;
                for row in 0..(th as usize) {
                    let dst_y = (ty as usize) + row;
                    let dst_x = (tx as usize) * 4;
                    let dst = dst_y * full_stride + dst_x;
                    let src = row * tile_stride;
                    pixels[dst..dst + tile_stride].copy_from_slice(&tile[src..src + tile_stride]);
                }
                tx += tw;
            }
            ty += th;
        }
        pixels
    };

    Ok((pw, ph, pixels))
}
