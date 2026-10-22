//! `<Shader>`: the shader's `CAMetalLayer` ([`nsh_metal::ShaderLayer`]) in
//! the view's layer, above its decoration's fills and below its borders
//! and children (as a canvas's content layer).
//!
//! `<ShaderEffect>`: its children live in a container (as a view that
//! clips them and casts a shadow, [`NshView::ensure_container`]) whose
//! layer is hidden by an empty mask while the shader draws them: AppKit's
//! hit testing follows the views, not what shows, so they keep their
//! clicks. The shader's layer covers the view and `maxSampleOffset` beyond
//! it, above the container.
//!
//! What a shader samples, its content (an effect's children) or what is
//! behind its view, is a picture: `renderInContext:` into a bitmap,
//! uploaded as a texture ([`nsh_metal::ShaderLayer::set_input`]), taken
//! again when it changes: after the core says so
//! ([`nsh_core::MountItem::ShaderInput`]) and when the UI thread changes
//! a view in it ([`ui_changed`]: animations, scrolling, images). Pictures
//! are taken once per display frame, all at once. `renderInContext:`
//! draws no `CAMetalLayer`, video or web page: shaders, videos and web
//! views inside an effect or behind a backdrop read as transparent.
//!
//! `<ShaderTransition>`: as an effect, but only while a transition runs
//! ([`start_transition`]): its children are pictured as they were when it
//! starts (`sample_from`) and as they are after the batch (`sample_to`,
//! taken again as they change), and a frame callback moves `nsh.progress`
//! from 0 to 1 over the duration; then the children show as they are.
//!
//! It draws when what it draws changes (the fill, an animated uniform, an
//! image that loaded, its size or scale, its program once compiled, a new
//! picture of an input) and, for a shader that reads `nsh.time`, on every
//! display frame while its view is in a window.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use nsh_core::{
    ViewKind,
    animation::AnimationValue,
    shader::{ShaderFill, ShaderInput, ShaderProgram},
};
use objc2_core_foundation::CGFloat;
use objc2_core_graphics::CGImage;
use objc2_quartz_core::CALayer;

use crate::{
    surface::Surface,
    view::{NshView, ViewObj},
};

/// What a shader view draws.
pub(crate) struct ShaderState {
    layer: nsh_metal::ShaderLayer,
    /// A frame callback draws it (`nsh.time`).
    ticking: bool,
    /// The surface whose frames draw it.
    surface: std::rc::Weak<Surface>,
    /// A shader effect or transition (its children are its content or its
    /// `sample_to`).
    effect: bool,
    /// A shader transition; when it runs, its start and duration.
    transition: bool,
    running: Option<(std::time::Instant, std::time::Duration)>,
    /// The inputs it samples ([`ShaderFill::inputs`]).
    inputs: u32,
    /// Inputs whose picture is out of date, bit [`ShaderInput::index`].
    stale: u32,
}

thread_local! {
    /// The shader views, to draw again when their program is ready.
    static VIEWS: RefCell<Vec<objc2::rc::Weak<ViewObj>>> = const { RefCell::new(Vec::new()) };
    /// Shader views that sample an input: [`ui_changed`] looks for them.
    static SAMPLING: Cell<usize> = const { Cell::new(0) };
    /// A frame callback takes the pictures due.
    static SCHEDULED: Cell<bool> = const { Cell::new(false) };
}

const CONTENT: u32 = 1 << 0;
const BACKDROP: u32 = 1 << 1;

/// A program for the views to come: compiled now, off the main thread.
pub(crate) fn program(program: &ShaderProgram) {
    let Some(gpu) = nsh_metal::gpu() else { return };
    gpu.set_on_ready(ready);
    gpu.set_on_presented(presented);
    gpu.add_program(program);
}

/// A drawable was shown: the views that skipped a draw draw.
fn presented() {
    for view in views() {
        let stale = view
            .state()
            .shader
            .as_ref()
            .is_some_and(|s| s.layer.is_stale());
        if stale {
            draw(&view);
        }
    }
}

/// Program `id` compiled: the views drawing it draw.
fn ready(id: u32) {
    for view in views() {
        let draws = view.state().shader.as_ref().and_then(|s| s.layer.program()) == Some(id);
        if draws {
            draw(&view);
        }
    }
}

fn views() -> Vec<NshView> {
    VIEWS.with(|v| {
        let mut v = v.borrow_mut();
        v.retain(|w| w.load().is_some());
        v.iter().filter_map(NshView::upgrade).collect()
    })
}

/// What `view` draws now (`None`: its children only).
pub(crate) fn set(surface: &Rc<Surface>, view: &NshView, fill: Option<Arc<ShaderFill>>) {
    let transition = view.kind() == ViewKind::ShaderTransition;
    let effect = transition || view.kind() == ViewKind::ShaderEffect;
    if view.state().shader.is_none() {
        if fill.is_none() {
            return;
        }
        let Some(layer) = nsh_metal::ShaderLayer::new() else {
            log::warn!(target: "js", "<Shader>: Metal is not available: the view shows its children only");
            return;
        };
        let ca = content_layer(&layer);
        if effect {
            // Over the children it draws, below the borders in front.
            ca.setZPosition(crate::decor::EFFECT);
            view.ensure_container();
        } else {
            // Above the decoration behind it (fills, gradients, inset
            // shadows), below the borders and the children.
            ca.setZPosition(crate::decor::SHADER);
        }
        if let Some(host) = view.ns().layer() {
            host.addSublayer(ca);
        }
        {
            let mut s = view.state_mut();
            // A fill follows the view's corners; an effect reaches beyond.
            if !effect {
                s.decor.set_shader_layer(objc2::Message::retain(ca));
            }
            s.shader = Some(ShaderState {
                layer,
                ticking: false,
                surface: Rc::downgrade(surface),
                effect,
                transition,
                running: None,
                inputs: 0,
                stale: 0,
            });
        }
        if !effect {
            crate::decor::apply(view);
        }
        VIEWS.with(|v| v.borrow_mut().push(view.downgrade()));
        resized(view);
    }
    // A transition's children are its `sample_to`.
    let children = if transition {
        ShaderInput::To
    } else {
        ShaderInput::Content
    };
    let inputs = fill.as_ref().map_or(0, |f| {
        (f.samples(children) as u32 * CONTENT)
            | (f.samples(ShaderInput::Backdrop) as u32 * BACKDROP)
    });
    let load = {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        // An effect's content is its children only.
        let inputs = if state.effect {
            inputs
        } else {
            inputs & BACKDROP
        };
        if (state.inputs != 0) != (inputs != 0) {
            SAMPLING.with(|n| {
                n.set(if inputs != 0 {
                    n.get() + 1
                } else {
                    n.get().saturating_sub(1)
                })
            });
        }
        state.inputs = inputs;
        state.stale = inputs;
        if fill.is_none() {
            state.running = None;
        }
        state.layer.set_fill(fill)
    };
    show(view);
    if inputs != 0 {
        schedule(surface);
    }
    for (slot, uri) in load {
        let weak = view.downgrade();
        let key = uri.clone();
        crate::image::load(surface, &uri, move |result| {
            let image = match result {
                Ok(image) => image,
                Err(e) => {
                    log::warn!(target: "js", "<Shader>: {e}");
                    return;
                }
            };
            let Some(view) = NshView::upgrade(&weak) else {
                return;
            };
            let points = (
                CGImage::width(Some(&image)) as f64,
                CGImage::height(Some(&image)) as f64,
            );
            let set = view
                .state_mut()
                .shader
                .as_mut()
                .is_some_and(|s| s.layer.set_image(slot, &key, &image, points));
            if set {
                draw(&view);
            }
        });
    }
    draw(view);
    tick(view);
}

/// Its size or scale changed (or may have): the layer covers it again.
pub(crate) fn resized(view: &NshView) {
    let scale = view.backing_scale();
    let changed = {
        let mut s = view.state_mut();
        let size = s.frame.size;
        s.shader.as_mut().is_some_and(|state| {
            let changed = state
                .layer
                .set_frame(size.width.0 as f64, size.height.0 as f64, scale);
            if changed {
                state.stale = state.inputs;
            }
            changed
        })
    };
    if changed {
        draw(view);
        if let Some(surface) = view
            .state()
            .shader
            .as_ref()
            .and_then(|s| s.surface.upgrade())
        {
            schedule(&surface);
        }
    }
    // In a window again: the frames of a shader that reads `nsh.time` go on.
    tick(view);
}

/// Animated uniforms' values (`AnimationProp::ShaderUniform`).
pub(crate) fn set_args(view: &NshView, values: &[(u16, AnimationValue)]) {
    let changed = view
        .state_mut()
        .shader
        .as_mut()
        .is_some_and(|s| s.layer.set_args(values));
    if changed {
        draw(view);
    }
}

fn draw(view: &NshView) {
    if let Some(state) = view.state_mut().shader.as_mut() {
        state.layer.draw();
    }
}

/// The `CAMetalLayer` of `layer`.
fn content_layer(layer: &nsh_metal::ShaderLayer) -> &CALayer {
    // SAFETY: a `CAMetalLayer` is a `CALayer`.
    unsafe { &*(layer.layer() as *const objc2::runtime::AnyObject as *const CALayer) }
}

/// Shows the shader's layer while it draws, and an effect's children
/// through it only: a fill always, an effect while it has a shader, a
/// transition while it runs.
fn show(view: &NshView) {
    let (shows, effect) = {
        let s = view.state();
        let Some(state) = s.shader.as_ref() else {
            return;
        };
        let shows =
            state.layer.program().is_some() && (!state.transition || state.running.is_some());
        content_layer(&state.layer).setHidden(!shows);
        (shows, state.effect)
    };
    if effect {
        hide_children(view, shows);
    }
}

/// A shader transition starts ([`nsh_core::MountItem::ShaderTransition`],
/// before the rest of its batch): what shows now is its `sample_from`, its
/// children's next picture its `sample_to`, and a frame callback moves
/// `nsh.progress` from 0 to 1 over `duration`.
pub(crate) fn start_transition(
    surface: &Rc<Surface>,
    view: &NshView,
    duration: std::time::Duration,
) {
    let (area, running) = {
        let s = view.state();
        let Some(state) = s.shader.as_ref() else {
            return;
        };
        if !state.transition || state.layer.program().is_none() {
            return;
        }
        (state.layer.area(), state.running.is_some())
    };
    let scale = view.backing_scale();
    // What shows: the transition as it is drawn, or the children.
    let from = if running {
        view.state_mut()
            .shader
            .as_mut()
            .and_then(|s| s.layer.picture())
    } else {
        view.container_view()
            .and_then(|c| c.layer())
            .map(|container| {
                let (pw, ph) = (
                    (area.size.width.0 as f64 * scale).round().max(1.0) as usize,
                    (area.size.height.0 as f64 * scale).round().max(1.0) as usize,
                );
                let origin = (area.origin.x.0 as f64, area.origin.y.0 as f64);
                let rgba = crate::surface::quiet(|| render(&container, origin, pw, ph, scale));
                (pw, ph, rgba)
            })
    };
    {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        if let Some((w, h, rgba)) = from {
            state.layer.set_input(ShaderInput::From, &rgba, w, h, area);
        }
        state.layer.clear_input(ShaderInput::To);
        state.layer.set_progress(0.0);
        state.running = Some((std::time::Instant::now(), duration));
        state.stale |= state.inputs;
    }
    show(view);
    draw(view);
    schedule(surface);
    if running {
        // Its frame callback runs on.
        return;
    }
    let weak = view.downgrade();
    surface.add_frame_callback(move |_, _| {
        let Some(view) = NshView::upgrade(&weak) else {
            return false;
        };
        let done = {
            let mut s = view.state_mut();
            let Some(state) = s.shader.as_mut() else {
                return false;
            };
            let Some((start, duration)) = state.running else {
                return false;
            };
            let t =
                (start.elapsed().as_secs_f32() / duration.as_secs_f32().max(f32::EPSILON)).min(1.0);
            state.layer.set_progress(t);
            if t >= 1.0 {
                state.running = None;
                state.layer.clear_input(ShaderInput::From);
                state.layer.clear_input(ShaderInput::To);
            }
            t >= 1.0
        };
        if done {
            show(&view);
        } else {
            draw(&view);
        }
        !done
    });
}

/// An effect's children show through its shader only, while it has one:
/// their container's layer is masked by an empty layer (nothing shows),
/// and they keep their clicks.
fn hide_children(view: &NshView, hidden: bool) {
    let Some(layer) = view.container_view().and_then(|c| c.layer()) else {
        return;
    };
    crate::surface::quiet(|| {
        let mask = hidden.then(CALayer::new);
        // SAFETY: a fresh, empty mask layer owned by the container's layer.
        unsafe { layer.setMask(mask.as_deref()) };
    });
}

/// The core says what `view`'s shader samples changed
/// ([`nsh_core::MountItem::ShaderInput`]).
pub(crate) fn input_changed(surface: &Rc<Surface>, view: &NshView) {
    let marked = view.state_mut().shader.as_mut().is_some_and(|s| {
        s.stale = s.inputs;
        s.inputs != 0
    });
    if marked {
        schedule(surface);
    }
}

/// The UI thread changed `view` (an animated value, scrolling, an image
/// that loaded): the effects around it and every backdrop take their
/// pictures again.
pub(crate) fn ui_changed(view: &NshView) {
    if SAMPLING.with(Cell::get) == 0 {
        return;
    }
    let mut surface = None;
    let mut ancestor = Some(view.clone());
    // A view whose state is in use (drawing itself again) is not a shader.
    while let Some(v) = ancestor {
        if let Some(mut st) = v.try_state_mut()
            && let Some(s) = st.shader.as_mut()
            && s.effect
            && (!s.transition || s.running.is_some())
            && s.inputs & CONTENT != 0
        {
            s.stale |= CONTENT;
            surface = s.surface.upgrade();
        }
        ancestor = v.parent_view();
    }
    for v in views() {
        if let Some(mut st) = v.try_state_mut()
            && let Some(s) = st.shader.as_mut()
            && s.inputs & BACKDROP != 0
        {
            s.stale |= BACKDROP;
            surface = surface.or_else(|| s.surface.upgrade());
        }
    }
    if let Some(surface) = surface {
        schedule(&surface);
    }
}

/// Takes the pictures due on the next display frame.
fn schedule(surface: &Rc<Surface>) {
    if SCHEDULED.with(|s| s.replace(true)) {
        return;
    }
    surface.add_frame_callback(|_, _| {
        SCHEDULED.with(|s| s.set(false));
        for view in views() {
            take_pictures(&view);
        }
        false
    });
}

/// Takes the pictures `view`'s shader is due, then draws it.
fn take_pictures(view: &NshView) {
    let (stale, area, effect, children, surface) = {
        let s = view.state();
        let Some(state) = s.shader.as_ref() else {
            return;
        };
        // An idle transition draws nothing: its children show.
        let stale = if state.transition && state.running.is_none() {
            0
        } else {
            state.stale
        };
        let children = if state.transition {
            ShaderInput::To
        } else {
            ShaderInput::Content
        };
        (
            stale,
            state.layer.area(),
            state.effect,
            children,
            state.surface.upgrade(),
        )
    };
    if stale == 0 || view.ns().window().is_none() {
        return;
    }
    let scale = view.backing_scale();
    let (pw, ph) = (
        (area.size.width.0 as f64 * scale).round().max(1.0) as usize,
        (area.size.height.0 as f64 * scale).round().max(1.0) as usize,
    );
    let origin = (area.origin.x.0 as f64, area.origin.y.0 as f64);
    let mut pictures = Vec::new();
    if effect
        && stale & CONTENT != 0
        && let Some(container) = view.container_view().and_then(|c| c.layer())
    {
        // The children, without the mask that hides them.
        let rgba = crate::surface::quiet(|| {
            let mask = container.mask();
            // SAFETY: clearing and restoring the container's own mask.
            unsafe { container.setMask(None) };
            let rgba = render(&container, origin, pw, ph, scale);
            unsafe { container.setMask(mask.as_deref()) };
            rgba
        });
        pictures.push((children, rgba));
    }
    if stale & BACKDROP != 0
        && let Some(surface) = &surface
        && let (Some(root), Some(layer)) = (surface.root.ns().layer(), view.ns().layer())
    {
        // What is behind: the surface without the view (and what it holds).
        // Where the view is in the layer tree `render` draws: while a scroll
        // runs, AppKit has moved the views before it moves their layers, and
        // the views' place would picture what is a scroll step away.
        let at = layer.convertRect_toLayer(layer.bounds(), Some(&root));
        let rgba = crate::surface::quiet(|| {
            let opacity = layer.opacity();
            layer.setOpacity(0.0);
            let rgba = render(
                &root,
                (at.origin.x + origin.0, at.origin.y + origin.1),
                pw,
                ph,
                scale,
            );
            layer.setOpacity(opacity);
            rgba
        });
        pictures.push((ShaderInput::Backdrop, rgba));
    }
    {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        state.stale = 0;
        for (input, rgba) in pictures {
            state.layer.set_input(input, &rgba, pw, ph, area);
        }
    }
    draw(view);
}

/// Draws the layers of `layer`'s tree that are due to draw (text and
/// canvases, which draw through a delegate): `renderInContext:` takes
/// their contents as they are, and one marked since Core Animation's last
/// commit (a text drawn again just now) has its old contents still.
fn display_tree(layer: &CALayer) {
    layer.displayIfNeeded();
    // SAFETY: a getter; the sublayers are retained while they are drawn.
    if let Some(sublayers) = unsafe { layer.sublayers() } {
        for sublayer in sublayers.iter() {
            display_tree(&sublayer);
        }
    }
}

/// `layer` (its views draw y down) rendered into premultiplied RGBA of
/// `pw` × `ph` pixels, top row first: its point `origin` at the top-left.
fn render(layer: &CALayer, origin: (f64, f64), pw: usize, ph: usize, scale: CGFloat) -> Vec<u8> {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGImageByteOrderInfo,
        kCGColorSpaceSRGB,
    };
    display_tree(layer);
    let mut out = vec![0u8; pw * ph * 4];
    // SAFETY: `out` holds `pw * 4 * ph` bytes and outlives the context; the
    // layer is rendered on the main thread, which owns it.
    unsafe {
        let Some(space) = CGColorSpace::with_name(Some(kCGColorSpaceSRGB)) else {
            return out;
        };
        let Some(context) = CGBitmapContextCreate(
            out.as_mut_ptr().cast(),
            pw,
            ph,
            8,
            pw * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0,
        ) else {
            return out;
        };
        CGContext::translate_ctm(Some(&context), 0.0, ph as CGFloat);
        CGContext::scale_ctm(Some(&context), scale, -scale);
        CGContext::translate_ctm(Some(&context), -origin.0, -origin.1);
        layer.renderInContext(&context);
    }
    out
}

/// A shader that reads `nsh.time` draws on every frame while its view is
/// in a window.
fn tick(view: &NshView) {
    let surface = {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        if state.ticking || !state.layer.draws_every_frame() || view.ns().window().is_none() {
            return;
        }
        let Some(surface) = state.surface.upgrade() else {
            return;
        };
        state.ticking = true;
        surface
    };
    let weak = view.downgrade();
    surface.add_frame_callback(move |_, _| {
        let Some(view) = NshView::upgrade(&weak) else {
            return false;
        };
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return false;
        };
        if !state.layer.draws_every_frame() || view.ns().window().is_none() {
            state.ticking = false;
            return false;
        }
        state.layer.draw();
        true
    });
}

/// What it shows, for an in-process picture of the window (screenshots),
/// which does not draw a `CAMetalLayer`: a sublayer with it as an image,
/// removed when the returned guard drops.
#[cfg(feature = "dev")]
pub(crate) fn picture_layer(view: &NshView) -> Option<objc2::rc::Retained<CALayer>> {
    let (w, h, rgba) = view.state_mut().shader.as_mut()?.layer.picture()?;
    let image = crate::image::rgba_pixels_image(w, h, &rgba)?;
    let layer = CALayer::new();
    let (area, effect) = {
        let s = view.state();
        let state = s.shader.as_ref()?;
        (state.layer.area(), state.effect)
    };
    {
        use objc2_core_foundation::{CGPoint, CGRect, CGSize};
        layer.setAnchorPoint(CGPoint::new(0.0, 0.0));
        layer.setFrame(CGRect::new(
            CGPoint::new(area.origin.x.0 as f64, area.origin.y.0 as f64),
            CGSize::new(area.size.width.0 as f64, area.size.height.0 as f64),
        ));
    }
    let ptr = &*image as *const CGImage as *mut objc2::runtime::AnyObject;
    // SAFETY: CF objects are Objective-C objects; a CGImage is a valid
    // layer content.
    unsafe { layer.setContents(objc2::rc::Retained::retain(ptr).as_deref()) };
    layer.setZPosition(if effect {
        crate::decor::EFFECT
    } else {
        crate::decor::SHADER
    });
    // Shaped as the shader's layer is (the view's corners).
    {
        let s = view.state();
        // SAFETY: a `CAMetalLayer` is a `CALayer`.
        let content: &CALayer = unsafe {
            &*(s.shader.as_ref()?.layer.layer() as *const objc2::runtime::AnyObject
                as *const CALayer)
        };
        layer.setCornerRadius(content.cornerRadius());
        layer.setCornerCurve(&content.cornerCurve());
        layer.setMasksToBounds(content.masksToBounds());
        if let Some(mask) = content
            .mask()
            .and_then(|m| m.downcast::<objc2_quartz_core::CAShapeLayer>().ok())
        {
            let m = objc2_quartz_core::CAShapeLayer::new();
            m.setPath(mask.path().as_deref());
            // SAFETY: a fresh mask layer, owned by the picture layer.
            unsafe { layer.setMask(Some(&m)) };
        }
    }
    view.ns().layer()?.addSublayer(&layer);
    Some(layer)
}

/// Hides the children of the effects in `window` that draw through their
/// shader, for an in-process picture (which draws their masked container
/// whole); the caller shows them again.
#[cfg(feature = "dev")]
pub(crate) fn hide_effect_children(
    window: &objc2_app_kit::NSWindow,
) -> Vec<objc2::rc::Retained<objc2_app_kit::NSView>> {
    views()
        .iter()
        .filter(|v| v.ns().window().as_deref() == Some(window))
        .filter(|v| {
            v.state()
                .shader
                .as_ref()
                .is_some_and(|s| s.effect && !content_layer(&s.layer).isHidden())
        })
        .filter_map(|v| v.container_view())
        .filter(|c| !c.isHidden())
        .inspect(|c| c.setHidden(true))
        .collect()
}

/// Every shader view of the process (the screenshot keeps those in its
/// window).
#[cfg(feature = "dev")]
pub(crate) fn shader_views() -> Vec<NshView> {
    views()
}
