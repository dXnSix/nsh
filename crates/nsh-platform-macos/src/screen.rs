//! Shaders over the whole surface (`<ScreenEffect>`, `<ScreenTransition>`):
//! their views take no space where they are declared; what they draw is
//! the surface's root view, which holds its pages, dialogs and overlays,
//! and never anything of the app around it.
//!
//! A screen effect makes the root an effect ([`crate::shader::set_screen`]):
//! its children move into a container an empty mask hides, the shader's
//! layer is above everything, and its content is the container pictured
//! with the navigation's overlays drawn over it.
//!
//! A screen transition is a [`NavOverlay`] over the root: the root
//! pictured as it shows when the transition starts (before the rest of its
//! batch) is its `sample_from`, pictured again on the next display frame
//! its `sample_to`, while `nsh.progress` goes from 0 to 1. One that starts
//! while another runs starts from what that one shows.

use std::{collections::HashMap, rc::Rc, sync::Arc};

use nsh_core::{NodeId, shader::ShaderFill};

use crate::{
    shader::{NavOverlay, picture_of},
    surface::Surface,
    view::NshView,
};

/// The surface's screen shaders.
#[derive(Default)]
pub(crate) struct Screen {
    /// What each screen transition draws.
    transitions: HashMap<NodeId, Arc<ShaderFill>>,
    /// The screen transition that runs.
    running: Option<Rc<NavOverlay>>,
    /// The screen effect that draws.
    effect: Option<NodeId>,
}

/// What a screen effect samples changed ([`nsh_core::MountItem::ShaderInput`]).
pub(crate) fn input_changed(surface: &Rc<Surface>, view: &NshView) {
    if surface.screen.borrow().effect == Some(view.id()) {
        crate::shader::input_changed(surface, &surface.root);
    }
}

/// A screen effect's animated uniforms.
pub(crate) fn set_args(
    surface: &Rc<Surface>,
    view: &NshView,
    values: &[(u16, nsh_core::animation::AnimationValue)],
) {
    if surface.screen.borrow().effect == Some(view.id()) {
        crate::shader::set_args(&surface.root, values);
    }
}

/// The surface's size changed: a screen effect covers it again.
pub(crate) fn resized(surface: &Rc<Surface>) {
    if surface.screen.borrow().effect.is_some() {
        surface.root.size_container();
        crate::shader::resized(&surface.root);
    }
}

/// What screen view `view` draws now.
pub(crate) fn set(surface: &Rc<Surface>, view: &NshView, fill: Option<Arc<ShaderFill>>) {
    if view.kind() == nsh_core::ViewKind::ScreenEffect {
        // The core sends one effect a fill at a time.
        let drawn = {
            let mut screen = surface.screen.borrow_mut();
            match &fill {
                Some(_) => screen.effect = Some(view.id()),
                None if screen.effect == Some(view.id()) => screen.effect = None,
                None => return,
            }
            screen.effect
        };
        crate::shader::set_screen(surface, fill.filter(|_| drawn.is_some()));
        return;
    }
    let mut screen = surface.screen.borrow_mut();
    match fill {
        Some(fill) => screen.transitions.insert(view.id(), fill),
        None => screen.transitions.remove(&view.id()),
    };
}

/// Screen transition `view` starts, before the rest of its batch.
pub(crate) fn start_transition(
    surface: &Rc<Surface>,
    view: &NshView,
    duration: std::time::Duration,
) {
    let Some(fill) = surface.screen.borrow().transitions.get(&view.id()).cloned() else {
        return;
    };
    let root = surface.root.clone();
    let running = surface.screen.borrow_mut().running.take();
    // What shows: the transition that runs as it is drawn, or the root.
    let from = running
        .as_ref()
        .and_then(|r| r.picture())
        .or_else(|| picture_of(&root, &[], None));
    drop(running);
    let Some(from) = from else { return };
    let mut shader = nsh_core::NavShader::default();
    shader.fill = fill;
    shader.duration = duration;
    let after = root.clone();
    let to = Box::new(move || picture_of(&after, &[], None));
    let Some(overlay) = NavOverlay::new(&root, &shader, from, to, false) else {
        return;
    };
    let overlay = Rc::new(overlay);
    surface.screen.borrow_mut().running = Some(overlay.clone());
    let weak = Rc::downgrade(&overlay);
    surface.add_frame_callback(move |surface, _| {
        let Some(overlay) = weak.upgrade() else {
            return false;
        };
        overlay.tick();
        if overlay.elapsed() < overlay.duration() {
            return true;
        }
        let mut screen = surface.screen.borrow_mut();
        if screen
            .running
            .as_ref()
            .is_some_and(|r| Rc::ptr_eq(r, &overlay))
        {
            screen.running = None;
        }
        false
    });
}
