//! `<Shader>`: a view whose layer holds the shader's `CAMetalLayer`
//! ([`nsh_metal::ShaderLayer`]) above its decoration's fills and below its
//! borders and children.
//!
//! `<ShaderEffect>`: its children live in the decoration's content view,
//! whose layer an empty mask hides while the shader draws them (UIKit's
//! hit testing skips views at alpha 0 but not masked ones, so they keep
//! their touches); the shader's layer covers the view and
//! `maxSampleOffset` beyond it, above the content view. What a shader
//! samples, its content or what is behind it (in the surface's root view,
//! never the host app's), is a picture taken with `renderInContext:` on
//! the shader display link's next frame after it changed, as on macOS.
//!
//! It draws when what it draws changes (the fill, an animated uniform, an
//! image that loaded, its size or scale, its program once compiled) and,
//! for a shader that reads `nsh.time`, on every frame of the mounter's
//! shader display link while its view is in a window.

use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use nsh_core::{
    animation::AnimationValue,
    shader::{ShaderFill, ShaderInput},
};
use objc2::rc::Retained;
use objc2_core_foundation::CGFloat;
use objc2_core_graphics::CGImage;
use objc2_quartz_core::CALayer;
use objc2_ui_kit::UIView;

use crate::native_view::NshView;

/// A shader view and what it draws.
pub(crate) struct ShaderHost {
    /// The view (its children are the shader view's).
    pub(crate) view: Retained<NshView>,
    layer: RefCell<Option<nsh_metal::ShaderLayer>>,
    /// A shader effect (its children are its content).
    pub(crate) effect: bool,
    /// The inputs it samples, [`CONTENT`] and [`BACKDROP`].
    inputs: Cell<u32>,
    /// Inputs whose picture is out of date.
    stale: Cell<u32>,
}

/// An effect's children.
pub(crate) const CONTENT: u32 = 1 << 0;
/// What is behind the view.
pub(crate) const BACKDROP: u32 = 1 << 1;

impl ShaderHost {
    pub(crate) fn new(view: Retained<NshView>, effect: bool) -> Self {
        Self {
            view,
            layer: RefCell::new(None),
            effect,
            inputs: Cell::new(0),
            stale: Cell::new(0),
        }
    }

    /// The inputs it samples.
    pub(crate) fn inputs(&self) -> u32 {
        self.inputs.get()
    }

    /// Its inputs' pictures are out of date (those of `which` it samples).
    pub(crate) fn mark(&self, which: u32) {
        self.stale
            .set(self.stale.get() | (which & self.inputs.get()));
    }

    /// Whether a picture is due.
    pub(crate) fn due(&self) -> bool {
        self.stale.get() != 0
    }

    /// What it draws now; the images to load (slot, URI).
    pub(crate) fn set(&self, fill: Option<Arc<ShaderFill>>) -> Vec<(usize, Arc<str>)> {
        if self.layer.borrow().is_none() {
            if fill.is_none() {
                return Vec::new();
            }
            let Some(layer) = nsh_metal::ShaderLayer::new() else {
                log::warn!(target: "js", "<Shader>: Metal is not available: the view shows its children only");
                return Vec::new();
            };
            // SAFETY: a `CAMetalLayer` is a `CALayer`.
            let ca: &CALayer =
                unsafe { &*(layer.layer() as *const objc2::runtime::AnyObject as *const CALayer) };
            ca.setZPosition(if self.effect {
                // Over the children it draws, below the borders in front.
                crate::decor::EFFECT
            } else {
                // Above the decoration behind it, below the borders and the
                // children.
                crate::decor::SHADER
            });
            self.view.layer().addSublayer(ca);
            *self.layer.borrow_mut() = Some(layer);
            self.layout();
        }
        let shows = fill.is_some();
        let inputs = fill.as_ref().map_or(0, |f| {
            let content = self.effect && f.samples(ShaderInput::Content);
            (content as u32 * CONTENT) | (f.samples(ShaderInput::Backdrop) as u32 * BACKDROP)
        });
        self.inputs.set(inputs);
        self.stale.set(inputs);
        let mut layer = self.layer.borrow_mut();
        let Some(layer) = layer.as_mut() else {
            return Vec::new();
        };
        // SAFETY: a `CAMetalLayer` is a `CALayer`.
        let ca: &CALayer =
            unsafe { &*(layer.layer() as *const objc2::runtime::AnyObject as *const CALayer) };
        ca.setHidden(!shows);
        let load = layer.set_fill(fill);
        layer.draw();
        load
    }

    /// Whether it draws a shader now (an effect then hides its children).
    pub(crate) fn shows(&self) -> bool {
        self.layer
            .borrow()
            .as_ref()
            .is_some_and(|l| l.program().is_some())
    }

    /// Takes the pictures due, `content` holding an effect's children and
    /// `root` the surface's root view, then draws.
    pub(crate) fn take_pictures(&self, content: Option<&UIView>, root: &UIView) {
        let stale = self.stale.replace(0);
        if stale == 0 || self.view.window().is_none() {
            return;
        }
        let mut layer = self.layer.borrow_mut();
        let Some(layer) = layer.as_mut() else { return };
        let area = layer.area();
        let scale = self.view.contentScaleFactor();
        let (pw, ph) = (
            (area.size.width.0 as f64 * scale).round().max(1.0) as usize,
            (area.size.height.0 as f64 * scale).round().max(1.0) as usize,
        );
        let origin = (area.origin.x.0 as f64, area.origin.y.0 as f64);
        if stale & CONTENT != 0
            && let Some(content) = content
        {
            let cl = content.layer();
            let rgba = crate::decor::quiet(|| {
                let mask = cl.mask();
                // SAFETY: clearing and restoring the content view's own mask.
                unsafe { cl.setMask(None) };
                let rgba = render(&cl, origin, pw, ph, scale);
                // SAFETY: as above.
                unsafe { cl.setMask(mask.as_deref()) };
                rgba
            });
            layer.set_input(ShaderInput::Content, &rgba, pw, ph, area);
        }
        if stale & BACKDROP != 0 {
            let own = self.view.layer();
            let at = self.view.convertRect_toView(self.view.bounds(), Some(root));
            let rgba = crate::decor::quiet(|| {
                let opacity = own.opacity();
                own.setOpacity(0.0);
                let rgba = render(
                    &root.layer(),
                    (at.origin.x + origin.0, at.origin.y + origin.1),
                    pw,
                    ph,
                    scale,
                );
                own.setOpacity(opacity);
                rgba
            });
            layer.set_input(ShaderInput::Backdrop, &rgba, pw, ph, area);
        }
        layer.draw();
    }

    /// Its `CAMetalLayer`, once it has one (the decoration shapes it).
    pub(crate) fn content_layer(&self) -> Option<Retained<CALayer>> {
        let layer = self.layer.borrow();
        // SAFETY: a `CAMetalLayer` is a `CALayer`.
        let ca: &CALayer = unsafe {
            &*(layer.as_ref()?.layer() as *const objc2::runtime::AnyObject as *const CALayer)
        };
        Some(objc2::Message::retain(ca))
    }

    /// The view's bounds or scale changed (or may have): the layer covers
    /// it again.
    pub(crate) fn layout(&self) {
        let mut layer = self.layer.borrow_mut();
        let Some(layer) = layer.as_mut() else { return };
        let size = self.view.bounds().size;
        if layer.set_frame(size.width, size.height, self.view.contentScaleFactor()) {
            self.stale.set(self.inputs.get());
            layer.draw();
        }
    }

    /// Animated uniforms' values.
    pub(crate) fn set_args(&self, values: &[(u16, AnimationValue)]) {
        if let Some(layer) = self.layer.borrow_mut().as_mut()
            && layer.set_args(values)
        {
            layer.draw();
        }
    }

    /// The image of `slot` loaded.
    pub(crate) fn set_image(&self, slot: usize, uri: &str, image: &CGImage, points: (f64, f64)) {
        if let Some(layer) = self.layer.borrow_mut().as_mut()
            && layer.set_image(slot, uri, image, points)
        {
            layer.draw();
        }
    }

    /// Draws it again (a display frame, its program ready, a drawable shown).
    pub(crate) fn draw(&self) {
        if let Some(layer) = self.layer.borrow_mut().as_mut() {
            layer.draw();
        }
    }

    /// Whether it draws on every frame while it is in a window.
    pub(crate) fn draws_every_frame(&self) -> bool {
        self.layer
            .borrow()
            .as_ref()
            .is_some_and(|l| l.draws_every_frame())
            && self.view.window().is_some()
    }

    /// Whether it should draw: its program is `program`, or it skipped a draw.
    pub(crate) fn waits_for(&self, program: Option<u32>) -> bool {
        self.layer
            .borrow()
            .as_ref()
            .is_some_and(|l| l.is_stale() || (program.is_some() && l.program() == program))
    }
}

thread_local! {
    /// The mounters drawing shaders, for the GPU's callbacks.
    static MOUNTERS: RefCell<Vec<std::sync::Weak<crate::mount::SharedMounterInner>>> = const { RefCell::new(Vec::new()) };
}

/// Makes `mounter`'s shader views draw when the GPU says a program is
/// ready or a drawable was shown.
pub(crate) fn watch(gpu: &nsh_metal::Gpu, mounter: crate::mount::SharedMounter) {
    MOUNTERS.with(|m| {
        let mut m = m.borrow_mut();
        m.retain(|w| w.strong_count() > 0);
        if !m
            .iter()
            .any(|w| std::ptr::eq(w.as_ptr(), Arc::as_ptr(&mounter)))
        {
            m.push(Arc::downgrade(&mounter));
        }
    });
    gpu.set_on_ready(|id| each(|m| m.shaders_draw(Some(id))));
    gpu.set_on_presented(|| each(|m| m.shaders_draw(None)));
}

fn each(f: impl Fn(&mut crate::mount::Mounter)) {
    let mounters: Vec<_> =
        MOUNTERS.with(|m| m.borrow().iter().filter_map(|w| w.upgrade()).collect());
    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    for mounter in mounters {
        // A mounter busy applying a batch draws its views itself.
        if let Ok(mut guard) = mounter.get(mtm).try_borrow_mut()
            && let Some(m) = guard.as_mut()
        {
            f(m);
        }
    }
}

/// `layer` rendered into premultiplied RGBA of `pw` × `ph` pixels, top row
/// first: its point `origin` at the top-left.
fn render(layer: &CALayer, origin: (f64, f64), pw: usize, ph: usize, scale: CGFloat) -> Vec<u8> {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGImageByteOrderInfo,
        kCGColorSpaceSRGB,
    };
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

/// Hides (or shows) an effect's children: an empty mask on their content
/// view's layer.
pub(crate) fn hide_children(content: &UIView, hidden: bool) {
    let layer = content.layer();
    crate::decor::quiet(|| {
        let mask = hidden.then(CALayer::new);
        // SAFETY: a fresh, empty mask layer owned by the content view's layer.
        unsafe { layer.setMask(mask.as_deref()) };
    });
}
