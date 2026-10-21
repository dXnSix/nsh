//! `<Shader>`: a view whose layer holds the shader's `CAMetalLayer`
//! ([`nsh_metal::ShaderLayer`]) above its decoration's fills and below its
//! borders and children.
//!
//! It draws when what it draws changes (the fill, an animated uniform, an
//! image that loaded, its size or scale, its program once compiled) and,
//! for a shader that reads `nsh.time`, on every frame of the mounter's
//! shader display link while its view is in a window.

use std::{cell::RefCell, sync::Arc};

use nsh_core::{animation::AnimationValue, shader::ShaderFill};
use objc2::rc::Retained;
use objc2_core_graphics::CGImage;
use objc2_quartz_core::CALayer;

use crate::native_view::NshView;

/// A shader view and what it draws.
pub(crate) struct ShaderHost {
    /// The view (its children are the shader view's).
    pub(crate) view: Retained<NshView>,
    layer: RefCell<Option<nsh_metal::ShaderLayer>>,
}

impl ShaderHost {
    pub(crate) fn new(view: Retained<NshView>) -> Self {
        Self {
            view,
            layer: RefCell::new(None),
        }
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
            // Above the decoration behind it, below the borders in front and
            // the children.
            ca.setZPosition(crate::decor::BEHIND / 2.0);
            self.view.layer().addSublayer(ca);
            *self.layer.borrow_mut() = Some(layer);
            self.layout();
        }
        let mut layer = self.layer.borrow_mut();
        let Some(layer) = layer.as_mut() else {
            return Vec::new();
        };
        let load = layer.set_fill(fill);
        layer.draw();
        load
    }

    /// The view's bounds or scale changed (or may have): the layer covers
    /// it again.
    pub(crate) fn layout(&self) {
        let mut layer = self.layer.borrow_mut();
        let Some(layer) = layer.as_mut() else { return };
        let size = self.view.bounds().size;
        if layer.set_frame(size.width, size.height, self.view.contentScaleFactor()) {
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
