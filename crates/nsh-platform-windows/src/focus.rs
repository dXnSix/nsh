//! Keyboard focus between the views: Tab and Shift+Tab move it between
//! the stops in tree order, Return and Space press the focused view. The
//! decisions are `nsh_core::focus`'s, over the views' frames in the
//! window's points (as drawn: scroll offsets, shifts and transforms
//! applied, each frame the box around its corners), as the GPUI backend's
//! are.
//!
//! Text fields keep their own focus ([`crate::text_input`]); this module
//! holds the focus of every other stop (`ViewProps::focus_stop`: a
//! pressable, a view `focusable` makes one), reported as the fields' is:
//! `FOCUSED` (so `focus:` styles follow), `onFocus` / `onBlur`, and the
//! accessibility focus. One of the two has the keyboard at a time.
//!
//! - **Stops**: the views of the focus scope (the newest modal, else the
//!   surface: [`crate::navigation::focus_scope`]) that are stops, not
//!   disabled, shown (not hidden, concealed or out of the pointer's reach,
//!   `pointerEvents: "none"`) and have an area, and the fields that edit
//!   (`focus::candidates`); a backdrop over the page (a sheet's, a
//!   dialog's) takes those under it out of reach (`focus::covering_layer`).
//!   Their order is the order Tab and screen readers go through the views
//!   ([`NshView::with_children_in_reading_order`]: tree order, a scroll
//!   view's content as it is on screen).
//! - **Tab** and Shift+Tab move to the next and previous stop, around from
//!   the last (`focus::tab`, through `nsh_core::input::tab_stop`); a field
//!   reached has its text selected, as in Windows' dialogs, any other stop
//!   takes this module's focus. The stop reached is scrolled into view.
//! - **Return and Space** press the focused view when it takes presses
//!   (`PressDown`, `PressUp`, `Press` at its middle); one that takes none
//!   leaves them to the app's bindings, which come first
//!   ([`crate::input::key`]'s order).
//! - **Layers**: focus that went into a root view's child (the page, a
//!   sheet's panel, an overlay) from a stop of another comes back to that
//!   stop when the layer goes ([`focus::Layers`]); a stop a backdrop
//!   covers loses the focus and gets it back when that layer goes. Modals
//!   give the focus back themselves ([`crate::navigation`]).

use std::{cell::RefCell, rc::Rc};

use nsh_core::{
    Bounds, EventKind, EventPayload, NodeId, PointerEvents, Points, ViewKind,
    focus::{self, Candidate, Layer, Order},
    geometry::{point, size},
    input::has_press,
    tailwind::state::FOCUSED,
};

use crate::{geom::Mat4, input::KeyEvent, surface::Surface, view::NshView};

/// `VK_RETURN`.
const VK_RETURN: u16 = 0x0D;
/// `VK_SPACE`.
const VK_SPACE: u16 = 0x20;

/// A surface's keyboard focus on views that are not fields.
#[derive(Default)]
pub(crate) struct Focus {
    /// The stop that has it.
    view: RefCell<Option<NshView>>,
    /// The layers the focus went into, each with the stop it came from.
    layers: RefCell<focus::Layers>,
}

/// A stop of the focus scope: the core's candidate, and its view.
#[derive(Clone)]
struct Stop {
    node: Candidate,
    view: NshView,
}

impl AsRef<Candidate> for Stop {
    fn as_ref(&self) -> &Candidate {
        &self.node
    }
}

/// An axis-aligned box in the window's points: left, top, right, bottom.
type Edges = (f32, f32, f32, f32);

fn intersect(a: Edges, b: Edges) -> Edges {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

fn shows(e: Edges) -> bool {
    e.2 > e.0 && e.3 > e.1
}

fn bounds(e: Edges) -> Bounds<Points> {
    Bounds::new(
        point(Points(e.0), Points(e.1)),
        size(Points(e.2 - e.0), Points(e.3 - e.1)),
    )
}

/// Where a child view sits in its parent's coordinates (its frame's
/// origin less the parent's scroll offset, its shift, its transform).
fn local(parent: &NshView, child: &NshView) -> Mat4 {
    let s = child.state();
    let (x, y) = parent.state().child_origin(s.frame, s.shift);
    let local = Mat4::new().translate(x, y);
    match s.transform() {
        Some(t) => Mat4::mul(&t, &local),
        None => local,
    }
}

/// The box `view` covers in the window's points, `to_root` its transform
/// there.
fn edges(view: &NshView, to_root: &Mat4) -> Option<Edges> {
    let size = view.state().frame.size;
    let (x, y, w, h) = to_root.bounds(0.0, 0.0, size.width.0, size.height.0)?;
    Some((x, y, x + w, y + h))
}

/// Neither `view` nor its subtree takes part: hidden, concealed (a screen
/// below the top), or out of the pointer's reach.
fn left_out(view: &NshView) -> bool {
    let s = view.state();
    s.style.hidden || s.concealed || s.props.pointer_events == PointerEvents::None
}

/// `view` takes typing: an input that edits.
fn editable(view: &NshView) -> bool {
    view.kind() == ViewKind::Input && crate::text_input::editable(view)
}

/// `view` is a focus stop of its own (whatever is around it). Native
/// components keep the keyboard to themselves: they are no stops.
fn is_stop(view: &NshView) -> bool {
    let s = view.state();
    if s.props.accessibility.state.disabled || s.kind == ViewKind::Native {
        return false;
    }
    let takes = has_press(s.events);
    drop(s);
    let editable = editable(view);
    view.state().props.focus_stop(takes || editable)
}

/// The views of `scope` (a modal, or the surface's root) as the core's
/// candidates.
struct Walk {
    scope: NodeId,
    stops: Vec<Stop>,
    order: usize,
}

impl Walk {
    fn view(
        &mut self,
        view: &NshView,
        to_root: Mat4,
        clip: Edges,
        layer: Option<NodeId>,
        popup: bool,
    ) {
        let order = self.order;
        self.order += 1;
        let popup = popup || view.state().props.overlay_anchor.is_some();
        let own = edges(view, &to_root);
        if is_stop(view)
            && let Some(own) = own
        {
            let mut node = Candidate::default();
            node.id = view.id();
            node.frame = bounds(own);
            node.visible = shows(intersect(own, clip));
            node.stop = true;
            node.editable = editable(view);
            node.scope = self.scope;
            node.layer = layer;
            node.popup = popup;
            node.order = Order::view(order);
            self.stops.push(Stop {
                node,
                view: view.clone(),
            });
        }
        let clip = match own {
            Some(own) if view.state().clips() => intersect(own, clip),
            _ => clip,
        };
        view.with_children_in_reading_order(|children| {
            for child in children {
                // Another modal is a scope of its own.
                if left_out(child) || child.kind() == ViewKind::Modal {
                    continue;
                }
                let to_root = Mat4::mul(&local(view, child), &to_root);
                self.view(child, to_root, clip, layer.or(Some(child.id())), popup);
            }
        });
    }
}

/// The root of the tree `view` is in.
fn tree_root(view: &NshView) -> NshView {
    let mut top = view.clone();
    while let Some(parent) = top.parent_view() {
        top = parent;
    }
    top
}

/// The box of the window `view` shows in, in its points.
fn window_edges(view: &NshView) -> Edges {
    let size = tree_root(view).state().frame.size;
    (0.0, 0.0, size.width.0, size.height.0)
}

/// The stops of `scope` in focus order (`focus::candidates`).
fn stops_in(scope: &NshView) -> Vec<Stop> {
    let Some(to_root) = scope.to_root() else {
        return Vec::new();
    };
    let mut walk = Walk {
        scope: scope.id(),
        stops: Vec::new(),
        order: 0,
    };
    let clip = window_edges(scope);
    let clip = edges(scope, &to_root).map_or(clip, |own| intersect(own, clip));
    scope.with_children_in_reading_order(|children| {
        for child in children {
            if left_out(child) || child.kind() == ViewKind::Modal {
                continue;
            }
            let to_root = Mat4::mul(&local(scope, child), &to_root);
            walk.view(child, to_root, clip, Some(child.id()), false);
        }
    });
    focus::candidates(walk.stops, scope.id(), &layers(scope))
}

/// The children of `scope` (a modal, or the surface's root) as the core's
/// layers, the page first.
fn layers(scope: &NshView) -> Vec<Layer> {
    let clip = window_edges(scope);
    scope
        .child_views()
        .iter()
        .enumerate()
        .filter(|(_, child)| child.kind() != ViewKind::Modal)
        .map(|(order, child)| {
            let mut layer = Layer::default();
            layer.id = child.id();
            layer.order = order;
            if !left_out(child) {
                layer.frame = child
                    .to_root()
                    .and_then(|m| edges(child, &m))
                    .map(|e| intersect(e, clip))
                    .filter(|&e| shows(e))
                    .map(bounds);
                let s = child.state();
                layer.takes_presses = has_press(s.events);
            }
            layer
        })
        .collect()
}

/// The child of its root view (the surface's root, or the modal it is in)
/// around `view`: the layer it is in. With that root view.
fn layer_of(view: &NshView) -> Option<(NshView, NodeId)> {
    let mut at = view.clone();
    loop {
        let parent = at.parent_view()?;
        if parent.parent_view().is_none() || parent.kind() == ViewKind::Modal {
            return Some((parent, at.id()));
        }
        at = parent;
    }
}

/// The view with this module's focus (not a field's). One the app removed
/// loses it quietly (its node is gone).
pub(crate) fn focused(surface: &Surface) -> Option<NshView> {
    let view = surface.focus.view.borrow().clone()?;
    if view.attached() {
        return Some(view);
    }
    surface.focus.view.borrow_mut().take();
    None
}

/// The view with the keyboard focus: the focused field, else the view
/// with this module's.
pub(crate) fn current(surface: &Surface) -> Option<NshView> {
    crate::text_input::focused(surface).or_else(|| focused(surface))
}

/// Notes that the focus went from `from` to `to`: when they are in two
/// layers of one root view, `to`'s gives it back to `from` when it goes.
fn entered(surface: &Surface, from: Option<&NshView>, to: &NshView) {
    let Some(from) = from else {
        return;
    };
    if let (Some((root, from_layer)), Some((into_root, into))) = (layer_of(from), layer_of(to))
        && root == into_root
        && from_layer != into
    {
        surface.focus.layers.borrow_mut().entered(into, from.id());
    }
}

/// Sends `kind` (`Focus`, `Blur`) to `view` when it listens, as fields do.
fn send(surface: &Surface, view: &NshView, kind: EventKind) {
    let (id, listens) = {
        let s = view.state();
        (s.id, s.events.contains(kind))
    };
    if listens {
        surface.event(id, kind, EventPayload::None);
    }
}

/// Takes this module's focus off its view: `FOCUSED` off, `onBlur`.
fn blur_view(surface: &Surface) -> Option<NshView> {
    let old = surface.focus.view.borrow_mut().take()?;
    if !old.attached() {
        return None;
    }
    crate::input::set_state(surface, &old, FOCUSED, false);
    send(surface, &old, EventKind::Blur);
    surface.invalidate();
    Some(old)
}

/// Gives this module's focus to `view` (a stop that is not a field), or
/// takes it away: the field that had the keyboard is blurred first, then
/// the view that had this focus, then `view` is reported focused and takes
/// the accessibility focus.
pub(crate) fn set(surface: &Rc<Surface>, view: Option<&NshView>) {
    let old = focused(surface);
    if old.as_ref() == view {
        return;
    }
    let field = crate::text_input::focused(surface);
    if view.is_some() && field.is_some() {
        crate::text_input::blur_all(surface);
    }
    let old = blur_view(surface);
    let Some(view) = view else {
        return;
    };
    entered(surface, old.as_ref().or(field.as_ref()), view);
    *surface.focus.view.borrow_mut() = Some(view.clone());
    crate::input::set_state(surface, view, FOCUSED, true);
    send(surface, view, EventKind::Focus);
    crate::access::focus(surface, view);
    surface.invalidate();
}

/// Field `field` takes the keyboard (from `old`, the field that had it, if
/// any): the view with this module's focus loses it first.
pub(crate) fn field_focused(surface: &Surface, field: &NshView, old: Option<&NshView>) {
    let view = blur_view(surface);
    entered(surface, view.as_ref().or(old), field);
}

/// Takes the keyboard focus off `view`: a field, or the view with this
/// module's focus.
pub(crate) fn blur(surface: &Surface, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::blur(view);
    } else if focused(surface).as_ref() == Some(view) {
        blur_view(surface);
    }
}

/// Gives the keyboard focus back to `view` (a stop a modal or a layer took
/// it from), as it had it: a field, or this module's focus.
pub(crate) fn restore(surface: &Rc<Surface>, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
    } else {
        set(surface, Some(view));
    }
}

/// `focus()` on `view`: a field takes the typing; any other view takes the
/// focus as Tab gives it, when it is a stop of the scope the focus is in
/// (on another view it does nothing).
pub(crate) fn focus_view(surface: &Rc<Surface>, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
        return;
    }
    let scope = crate::navigation::focus_scope(surface);
    if still_stop(&scope, view).0 {
        set(surface, Some(view));
        crate::scroll::reveal(surface, view);
    }
}

/// `blur()` on `view`: the field, or the view with this module's focus,
/// lets go of it; on any other view it blurs the focused field, as before
/// views took the focus.
pub(crate) fn blur_command(surface: &Rc<Surface>, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::blur(view);
    } else if focused(surface).as_ref() == Some(view) {
        blur_view(surface);
    } else {
        crate::text_input::blur_all(surface);
    }
}

/// Tab (`back`: Shift+Tab): the focus moves to the next stop of the focus
/// scope in tree order, around from the last, and the stop reached comes
/// into view. False when the scope has no stop.
pub(crate) fn tab(surface: &Rc<Surface>, back: bool) -> bool {
    let scope = crate::navigation::focus_scope(surface);
    let stops = stops_in(&scope);
    let current = current(surface);
    let at = current
        .as_ref()
        .and_then(|view| stops.iter().position(|stop| stop.view == *view));
    let Some(next) = focus::tab(&stops, at, None, None, back) else {
        return false;
    };
    let view = stops[next].view.clone();
    if view.kind() == ViewKind::Input {
        crate::text_input::tab_to(&view);
    } else {
        set(surface, Some(&view));
        crate::scroll::reveal(surface, &view);
    }
    true
}

/// A key down for the focused view: Return and Space press it. True when
/// it was taken.
pub(crate) fn key(surface: &Rc<Surface>, event: &KeyEvent) -> bool {
    let m = event.modifiers;
    if !event.down || m.ctrl || m.alt || m.meta {
        return false;
    }
    match event.vk {
        VK_RETURN | VK_SPACE => press(surface, event),
        _ => false,
    }
}

/// Return or Space: presses the view with this module's focus when it
/// takes presses and shows (an auto-repeat presses nothing more).
fn press(surface: &Rc<Surface>, event: &KeyEvent) -> bool {
    let Some(view) = focused(surface) else {
        return false;
    };
    let (id, events, frame) = {
        let s = view.state();
        (s.id, s.events, s.frame)
    };
    if !has_press(events) {
        return false;
    }
    let Some(own) = view.to_root().and_then(|m| edges(&view, &m)) else {
        return false;
    };
    if !shows(intersect(own, window_edges(&view))) {
        return false;
    }
    if event.repeat {
        return true;
    }
    let (width, height) = (frame.size.width.0, frame.size.height.0);
    let payload = EventPayload::Press(nsh_core::PressEvent {
        position: point(Points(width / 2.0), Points(height / 2.0)),
        page_position: point(Points((own.0 + own.2) / 2.0), Points((own.1 + own.3) / 2.0)),
        modifiers: event.modifiers.core(),
        clicks: 1,
    });
    for kind in [EventKind::PressDown, EventKind::PressUp, EventKind::Press] {
        if events.contains(kind) {
            surface.event(id, kind, payload.clone());
        }
    }
    true
}

/// `view` is still a stop of the scope the focus is in, not covered by a
/// backdrop; with the layer that covers it when one does.
fn still_stop(scope: &NshView, view: &NshView) -> (bool, Option<NodeId>) {
    if !view.attached() || !is_stop(view) {
        return (false, None);
    }
    let size = view.state().frame.size;
    if !editable(view) && (size.width.0 <= 0.0 || size.height.0 <= 0.0) {
        return (false, None);
    }
    let mut at = view.clone();
    loop {
        if at == *scope {
            break;
        }
        if left_out(&at) || (at.kind() == ViewKind::Modal) {
            return (false, None);
        }
        match at.parent_view() {
            Some(parent) => at = parent,
            None => return (false, None),
        }
    }
    let cover = layer_of(view)
        .filter(|(root, _)| root == scope)
        .and_then(|(_, own)| focus::covered_by(&layers(scope), own));
    (cover.is_none(), cover)
}

/// A batch was applied: the view with this module's focus that is no
/// longer a stop of the scope loses it (one a backdrop covers gets it back
/// when that layer goes), and with nothing focused the stop a layer that
/// went took the focus from gets it back.
pub(crate) fn end_batch(surface: &Rc<Surface>) {
    let held = surface.focus.view.borrow().clone();
    let scope = crate::navigation::focus_scope(surface);
    if let Some(view) = held {
        if !view.attached() {
            surface.focus.view.borrow_mut().take();
        } else {
            let (stop, cover) = still_stop(&scope, &view);
            if let Some(cover) = cover {
                surface.focus.layers.borrow_mut().entered(cover, view.id());
            }
            if !stop {
                set(surface, None);
            }
        }
    }
    if current(surface).is_none() {
        let restored = surface.focus.layers.borrow().restore(
            |layer| surface.view(layer).is_none_or(|view| !view.attached()),
            |id| {
                surface
                    .view(id)
                    .is_some_and(|view| still_stop(&scope, &view).0 && shows_in_window(&view))
            },
        );
        if let Some(view) = restored.and_then(|id| surface.view(id)) {
            restore(surface, &view);
        }
    }
    surface
        .focus
        .layers
        .borrow_mut()
        .retain(|layer| surface.view(layer).is_some_and(|view| view.attached()));
}

/// Some of `view` is in sight: neither clipped away nor scrolled out.
fn shows_in_window(view: &NshView) -> bool {
    let visible = view.visible_bounds();
    let Some(own) = view.to_root().and_then(|m| {
        m.bounds(
            visible.origin.x.0,
            visible.origin.y.0,
            visible.size.width.0,
            visible.size.height.0,
        )
    }) else {
        return false;
    };
    visible.size.width.0 > 0.0
        && visible.size.height.0 > 0.0
        && shows(intersect(
            (own.0, own.1, own.0 + own.2, own.1 + own.3),
            window_edges(view),
        ))
}

/// Forgets the focus (the app stopped).
pub(crate) fn reset(surface: &Surface) {
    surface.focus.view.borrow_mut().take();
    *surface.focus.layers.borrow_mut() = focus::Layers::default();
}
