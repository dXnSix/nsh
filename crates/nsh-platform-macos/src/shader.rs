//! `<Shader>`: the shader's `CAMetalLayer` ([`nsh_metal::ShaderLayer`]) in
//! the view's layer, above its decoration's fills and below its borders
//! and children (as a canvas's content layer).
//!
//! It draws when what it draws changes (the fill, an animated uniform, an
//! image that loaded, its size or scale, its program once compiled) and,
//! for a shader that reads `nsh.time`, on every display frame while its
//! view is in a window.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use nsh_core::{
    animation::AnimationValue,
    shader::{ShaderFill, ShaderProgram},
};
use objc2::rc::Retained;
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
}

thread_local! {
    /// The shader views, to draw again when their program is ready.
    static VIEWS: RefCell<Vec<objc2::rc::Weak<ViewObj>>> = const { RefCell::new(Vec::new()) };
}

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
    if view.state().shader.is_none() {
        if fill.is_none() {
            return;
        }
        let Some(layer) = nsh_metal::ShaderLayer::new() else {
            log::warn!(target: "js", "<Shader>: Metal is not available: the view shows its children only");
            return;
        };
        // SAFETY: a `CAMetalLayer` is a `CALayer`.
        let ca: &CALayer =
            unsafe { &*(layer.layer() as *const objc2::runtime::AnyObject as *const CALayer) };
        // Above the decoration behind it (fills, gradients, inset shadows),
        // below the children and the borders in front.
        ca.setZPosition(crate::decor::BEHIND / 2.0);
        if let Some(host) = view.ns().layer() {
            host.addSublayer(ca);
        }
        view.state_mut().shader = Some(ShaderState {
            layer,
            ticking: false,
            surface: Rc::downgrade(surface),
        });
        VIEWS.with(|v| v.borrow_mut().push(view.downgrade()));
        resized(view);
    }
    let load = {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        state.layer.set_fill(fill)
    };
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
            state
                .layer
                .set_frame(size.width.0 as f64, size.height.0 as f64, scale)
        })
    };
    if changed {
        draw(view);
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
pub(crate) fn picture_layer(view: &NshView) -> Option<Retained<CALayer>> {
    let (w, h, rgba) = view.state_mut().shader.as_mut()?.layer.picture()?;
    let image = crate::image::rgba_pixels_image(w, h, &rgba)?;
    let layer = CALayer::new();
    let frame = view.state().frame;
    layer.setAnchorPoint(objc2_core_foundation::CGPoint::new(0.0, 0.0));
    layer.setFrame(objc2_core_foundation::CGRect::new(
        objc2_core_foundation::CGPoint::new(0.0, 0.0),
        objc2_core_foundation::CGSize::new(frame.size.width.0 as f64, frame.size.height.0 as f64),
    ));
    let ptr = &*image as *const CGImage as *mut objc2::runtime::AnyObject;
    // SAFETY: CF objects are Objective-C objects; a CGImage is a valid
    // layer content.
    unsafe { layer.setContents(Retained::retain(ptr).as_deref()) };
    layer.setZPosition(crate::decor::BEHIND / 2.0);
    view.ns().layer()?.addSublayer(&layer);
    Some(layer)
}

/// The shader views under `view` (itself included).
pub(crate) fn shader_views() -> Vec<NshView> {
    views()
}
