//! Pictures of a window's content drawn in the app's process, for a design
//! tool (`nsh/design`'s `settled` and `picture`, [`crate::design`]).
//!
//! - **Settled**: a window settles by the core's rule
//!   ([`core::Settling`]) on the batches its surface applied
//!   ([`Surface::applied`]) and whether an animation that ends by itself, or
//!   a platform motion (a transition, a scroll's glide), runs
//!   ([`Surface::animating`]), looked at every [`core::SETTLE_POLL`].
//!   Batches never wait for a frame here: each is applied as it arrives.
//! - **Picture**: once what the engine sent before the question is applied
//!   (the overrides among it) and a frame has followed it, the window's
//!   root is drawn offscreen with Direct2D ([`crate::render::picture`]) at
//!   the scale asked, all of it or the part asked, over transparent pixels,
//!   and written as raw RGBA, premultiplied, sRGB, rows from the top. Its
//!   outer box shadows are drawn as in the window; animations that loop
//!   with no end stand at the start of the run they are in
//!   ([`nsh_core::animation::Animator::at_loop_starts`]), and the views
//!   show their animated values again once it is drawn. What is drawn is
//!   never the swap chain's, so a window larger than any texture has its
//!   pictures too. Composed components (a web view's page, a native
//!   component's own visual) are not in it: the system composes them, and
//!   the offscreen drawing has none of them.

use std::time::Instant;

use nsh_core::runtime::design::{self as core, Failure, codes};

use crate::{surface::Surface, view::NshView, window::UiThread};

/// About a display frame, in milliseconds: how long a picture waits after
/// the batches before it are applied.
const FRAME_MS: u64 = 17;

/// Gives design mode this platform's pictures; once, by
/// [`crate::design::start`] after it connected. The questions come on the
/// engine's thread and are answered from the UI thread `ui`.
pub(crate) fn install(ui: UiThread) {
    core::set_picture_hooks(core::PictureHooks {
        settled: Box::new(move |ask| ui.invoke(move || settle(ask, None))),
        picture: Box::new(move |ask| ui.invoke(move || picture(ask, false))),
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
        None => crate::input::after_ms(core::SETTLE_POLL.as_millis() as u64, move || {
            settle(ask, Some(settling))
        }),
    }
}

/// Draws `ask`'s picture once what the engine sent before it is applied
/// and a frame has followed (`waited`).
fn picture(ask: core::PictureAsk, waited: bool) {
    let Some((surface, root, _)) = crate::design::window(&ask.window) else {
        let failure = no_window(&ask.window);
        ask.reply.send(Err(failure));
        return;
    };
    if !waited {
        // What came before the question, now; a frame more for what it set
        // going (the animated values it binds are written on frames).
        surface.drain();
        crate::input::after_ms(FRAME_MS, move || picture(ask, true));
        return;
    }
    let drawn = draw(&surface, &root, &ask).and_then(|(width, height, pixels)| {
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

/// Stands the keyframes repeating forever of `view` and the views under it
/// at their first frame, noting what each drew with in `out`.
fn stand_keyframes(view: &NshView, out: &mut Vec<(NshView, nsh_core::transition::Shown)>) {
    if let Some(shown) = view
        .state_mut()
        .live
        .as_deref_mut()
        .and_then(|live| live.stand_at_loop_start())
    {
        out.push((view.clone(), shown));
    }
    view.with_children(|children| {
        for child in children {
            stand_keyframes(child, out);
        }
    });
}

/// Window `root`'s content (or `ask.rect` of it, which must lie within it)
/// drawn at `ask.scale`: its size in pixels and its RGBA rows.
fn draw(
    surface: &std::rc::Rc<Surface>,
    root: &NshView,
    ask: &core::PictureAsk,
) -> Result<(u32, u32, Vec<u8>), Failure> {
    let (width, height) = {
        let s = root.state();
        (s.frame.size.width.0, s.frame.size.height.0)
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
    // Endless animations at the start of the run they are in, for the
    // picture only: the graph's loops, and the views' keyframes repeating
    // forever (`spin`, `ping`, `pulse`, `bounce`).
    let mut still = surface.anim.borrow().at_loop_starts(surface.clock());
    still.write(&surface.mounter.borrow());
    let mut keyframes = Vec::new();
    stand_keyframes(root, &mut keyframes);
    let drawn = crate::render::picture(surface, root, ask.scale as f32, (x, y), (pw, ph));
    // The animations' values now, as the window shows them.
    for (view, shown) in keyframes {
        if let Some(live) = view.state_mut().live.as_deref_mut() {
            live.put_back(shown);
        }
    }
    surface.anim.borrow_mut().write(&surface.mounter.borrow());
    let pixels = drawn.map_err(|err| {
        Failure::new(
            codes::NO_WINDOW,
            format!("the window's content could not be drawn: {err}"),
        )
    })?;
    Ok((pw, ph, pixels))
}
