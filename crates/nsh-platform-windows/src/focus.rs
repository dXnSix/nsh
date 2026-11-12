//! Keyboard focus between the views: Tab and Shift+Tab move it between
//! the stops in tree order, the arrow keys to the nearest stop in their
//! direction, Return and Space press the focused view. The
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
//!   view's content as it is on screen; a shown overlay's content after the
//!   view it follows, `nsh_core::input::tab_order`).
//! - **Tab** and Shift+Tab move to the next and previous stop, around from
//!   the last (`focus::tab`, through `nsh_core::input::tab_stop`); a field
//!   reached has its text selected, as in Windows' dialogs, any other stop
//!   takes this module's focus. The stop reached is scrolled into view.
//! - **Arrows** (no modifier), as on GPUI: Left / Right go to the nearest
//!   stop on an overlapping row, Up / Down to the nearest below / above,
//!   one in an overlapping column first (`focus::horizontal`,
//!   `focus::vertical`); with nothing focused Left / Right take the first
//!   stop, Up / Down the first / last in sight, in an open popup anchored
//!   to another view when there is one. While a menu's item has the focus,
//!   Left and Right stay in its popup. The stop reached is scrolled into
//!   view; an arrow that moves no focus goes on to the scroll views
//!   ([`crate::scroll::key`]). What uses the arrows keeps them: the app's
//!   bindings, the focused field (Left / Right, and Up / Down in a
//!   multiline one) and selectable text with a selection come first
//!   ([`crate::input::key`]'s order).
//! - **Return and Space** (neither Control, Alt nor the Windows key held:
//!   `nsh_core::input::presses_focused_control`) are the focused view's
//!   when it takes presses and no field is edited, as on the other
//!   desktops: only the focus' own bindings take them first
//!   (`Keymap::find_at`, [`on_control`]), else they press it (`PressDown`,
//!   `PressUp`, `Press` at its middle: [`press_key`]), before the menu's
//!   shortcuts. A binding of Return elsewhere (a dialog's main action)
//!   does not act instead of the control the focus was moved to. A view
//!   that takes no presses leaves them to the app's bindings
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
    input::{has_press, presses_focused_control, tab_order},
    keymap::Key,
    tailwind::state::FOCUSED,
};

use crate::{geom::Mat4, input::KeyEvent, surface::Surface, view::NshView};

/// `VK_RETURN`.
const VK_RETURN: u16 = 0x0D;
/// `VK_SPACE`.
const VK_SPACE: u16 = 0x20;
/// `VK_LEFT`.
const VK_LEFT: u16 = 0x25;
/// `VK_UP`.
const VK_UP: u16 = 0x26;
/// `VK_RIGHT`.
const VK_RIGHT: u16 = 0x27;
/// `VK_DOWN`.
const VK_DOWN: u16 = 0x28;

/// What a press of Return or Space sends the focused view.
const PRESS: [EventKind; 3] = [EventKind::PressDown, EventKind::PressUp, EventKind::Press];

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
    // Tab's order: a shown overlay's content after the view it follows
    // (`ViewProps::tab_after`, `nsh_core::input::tab_order`), as the
    // headless driver and GPUI have it.
    let order: rustc_hash::FxHashMap<NodeId, usize> = tab_order(
        scope.clone(),
        |view| {
            if left_out(view) {
                return Vec::new();
            }
            view.with_children_in_reading_order(|children| {
                children
                    .iter()
                    .filter(|child| child.kind() != ViewKind::Modal)
                    .cloned()
                    .collect()
            })
        },
        NshView::id,
        |view| view.state().props.tab_after,
    )
    .into_iter()
    .enumerate()
    .map(|(at, view)| (view.id(), at))
    .collect();
    for stop in &mut walk.stops {
        if let Some(&at) = order.get(&stop.node.id) {
            stop.node.order.tree = at;
        }
    }
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

/// `ancestor` is `view` or one of its ancestors.
fn contains(ancestor: &NshView, view: &NshView) -> bool {
    let mut at = Some(view.clone());
    while let Some(v) = at {
        if v == *ancestor {
            return true;
        }
        at = v.parent_view();
    }
    false
}

/// The overlay anchored to another view that `view` is in (a menu's, a
/// select's or a popover's popup), if any.
fn anchored_overlay(view: &NshView) -> Option<NshView> {
    let mut at = Some(view.clone());
    while let Some(v) = at {
        if v.state().props.overlay_anchor.is_some() {
            return Some(v);
        }
        at = v.parent_view();
    }
    None
}

/// Anchored overlay `popup` hangs from a trigger: its anchor is one of
/// `stops`, holds one or is in one.
fn triggered(surface: &Surface, popup: &NshView, stops: &[Stop]) -> bool {
    let anchor = popup
        .state()
        .props
        .overlay_anchor
        .as_ref()
        .and_then(|anchor| anchor.node);
    let Some(anchor) = anchor.and_then(|id| surface.view(id)) else {
        return false;
    };
    stops
        .iter()
        .any(|stop| contains(&anchor, &stop.view) || contains(&stop.view, &anchor))
}

/// `view` is a menu's item.
fn in_menu(view: &NshView) -> bool {
    use nsh_core::AccessibilityRole::{MenuItemCheckbox, MenuItemRadio, Menuitem};
    matches!(
        view.state().props.accessibility.role,
        Menuitem | MenuItemCheckbox | MenuItemRadio
    )
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
        focus_stop(surface, &view);
    }
    true
}

/// Moves the keyboard focus to stop `view`, revealed in its scroll views:
/// a field takes the typing, any other view this module's focus.
fn focus_stop(surface: &Rc<Surface>, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
    } else {
        set(surface, Some(view));
        crate::scroll::reveal(surface, view);
    }
}

/// A key down for the focus: the arrows move it. True when it was taken.
/// (Return and Space come earlier: [`press_key`].)
pub(crate) fn key(surface: &Rc<Surface>, event: &KeyEvent) -> bool {
    let m = event.modifiers;
    if !event.down || m.ctrl || m.alt || m.meta || m.shift {
        return false;
    }
    match event.vk {
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN => arrow(surface, event.vk),
        _ => false,
    }
}

/// An arrow: the focus moves to the stop in that direction, if any.
fn arrow(surface: &Rc<Surface>, vk: u16) -> bool {
    let scope = crate::navigation::focus_scope(surface);
    let mut stops = stops_in(&scope);
    let current = current(surface);
    let horizontal = vk == VK_LEFT || vk == VK_RIGHT;
    // Left and Right on a menu's item its menu does not take (no submenu)
    // would leave the menu open behind the focus: they stay in it.
    if horizontal
        && let Some(popup) = current
            .as_ref()
            .filter(|view| in_menu(view))
            .and_then(anchored_overlay)
            .filter(|popup| triggered(surface, popup, &stops))
    {
        stops.retain(|stop| contains(&popup, &stop.view));
    }
    let at = current
        .as_ref()
        .and_then(|view| stops.iter().position(|stop| stop.view == *view));
    let next = if horizontal {
        focus::horizontal(&stops, at, vk == VK_RIGHT)
    } else {
        focus::vertical(&stops, at, vk == VK_DOWN)
    };
    let Some(next) = next.filter(|&next| Some(next) != at) else {
        return false;
    };
    let view = stops[next].view.clone();
    focus_stop(surface, &view);
    true
}

/// The control Return and Space press: the view with this module's focus
/// when it hears a press ([`PRESS`]) and shows in its window, while no
/// field is edited. With the box it covers in the window's points.
fn control(surface: &Surface) -> Option<(NshView, Edges)> {
    if crate::text_input::focused(surface).is_some() {
        return None;
    }
    let view = focused(surface)?;
    let events = view.state().events;
    if !PRESS.into_iter().any(|kind| events.contains(kind)) {
        return None;
    }
    let own = view.to_root().and_then(|m| edges(&view, &m))?;
    shows(intersect(own, window_edges(&view))).then_some((view, own))
}

/// Whether the keyboard focus is on a control that Return and Space press
/// ([`press_key`]): the app's bindings of those keys then give way to it,
/// but for the focus' own (`Keymap::find_at`).
pub(crate) fn on_control(surface: &Surface) -> bool {
    control(surface).is_some()
}

/// Return or Space (`presses_focused_control`) press the control with the
/// focus (`control`), once the bindings of the focus passed on them
/// ([`crate::keymap::key`]) and before the menu's shortcuts: a binding of
/// Return elsewhere does not act instead of the control the focus was
/// moved to. An auto-repeat presses nothing more. True when it was taken.
pub(crate) fn press_key(surface: &Surface, event: &KeyEvent) -> bool {
    let key = match event.vk {
        VK_RETURN => Key::Enter,
        VK_SPACE => Key::Char(' '),
        _ => return false,
    };
    if !event.down || !presses_focused_control(key, event.modifiers.core()) {
        return false;
    }
    let Some((view, own)) = control(surface) else {
        return false;
    };
    if event.repeat {
        return true;
    }
    let (id, events, frame) = {
        let s = view.state();
        (s.id, s.events, s.frame)
    };
    let (width, height) = (frame.size.width.0, frame.size.height.0);
    let payload = EventPayload::Press(nsh_core::PressEvent {
        position: point(Points(width / 2.0), Points(height / 2.0)),
        page_position: point(Points((own.0 + own.2) / 2.0), Points((own.1 + own.3) / 2.0)),
        modifiers: event.modifiers.core(),
        clicks: 1,
    });
    for kind in PRESS {
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

#[cfg(test)]
mod tests {
    /// Return and Space on the focused control:
    /// `cargo test -p nsh-platform-windows --lib focus::tests::return_and_space`.
    mod return_and_space {
        use std::{rc::Rc, sync::Arc};

        use nsh_core::{
            Bounds, EventKind, Points, ViewKind,
            geometry::{point, size},
            keymap::{KeyBinding, Keymap, Keystroke, When},
        };

        use super::super::{VK_RETURN, VK_SPACE, on_control, press_key};
        use crate::{
            input::{KeyEvent, Modifiers},
            surface::Surface,
            view::NshView,
        };

        /// A window with one button in sight.
        fn window() -> (Rc<Surface>, NshView) {
            let surface = Surface::new(Default::default());
            surface.resized((200, 100), 1.0);
            let button = NshView::new(2, ViewKind::View);
            {
                let mut s = button.state_mut();
                s.frame = Bounds::new(
                    point(Points(10.0), Points(10.0)),
                    size(Points(80.0), Points(30.0)),
                );
                s.events.set(EventKind::Press, true);
            }
            button.insert_before(&surface.root, None);
            (surface, button)
        }

        fn down(vk: u16, ctrl: bool) -> KeyEvent {
            KeyEvent {
                vk,
                down: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl,
                    ..Modifiers::default()
                },
            }
        }

        fn stroke(keys: &str) -> Keystroke {
            Keystroke::parse_with(keys, false).unwrap()
        }

        /// A dialog's Return, Space and Control+Return.
        fn bindings() -> Vec<KeyBinding> {
            vec![
                KeyBinding::new(1, stroke("enter"), When::NotEditing, false),
                KeyBinding::new(2, stroke("space"), When::NotEditing, false),
                KeyBinding::new(3, stroke("ctrl+enter"), When::Always, false),
            ]
        }

        #[test]
        fn press_the_focused_control_before_the_other_bindings() {
            let (surface, button) = window();
            crate::keymap::set(&surface, Arc::new(Keymap::new(bindings())));
            // The focus on no control: the bindings take them, nothing is
            // pressed.
            assert!(!on_control(&surface));
            for vk in [VK_RETURN, VK_SPACE] {
                assert!(crate::keymap::key(&surface, &down(vk, false)));
                assert!(!press_key(&surface, &down(vk, false)));
            }
            // On the button: they are its.
            *surface.focus.view.borrow_mut() = Some(button);
            assert!(on_control(&surface));
            for vk in [VK_RETURN, VK_SPACE] {
                assert!(!crate::keymap::key(&surface, &down(vk, false)));
                assert!(press_key(&surface, &down(vk, false)));
            }
            // A held key presses nothing more, and stays the control's.
            let held = KeyEvent {
                repeat: true,
                ..down(VK_RETURN, false)
            };
            assert!(!crate::keymap::key(&surface, &held));
            assert!(press_key(&surface, &held));
            // With Control held the binding of that keystroke fires.
            assert!(crate::keymap::key(&surface, &down(VK_RETURN, true)));
            assert!(!press_key(&surface, &down(VK_RETURN, true)));
        }

        #[test]
        fn the_focus_own_bindings_come_first() {
            let (surface, button) = window();
            let mut table = bindings();
            // A menu button's Return, bound while it has the focus.
            table.push(
                KeyBinding::new(4, stroke("enter"), When::NotEditing, false).focus_keys(true),
            );
            crate::keymap::set(&surface, Arc::new(Keymap::new(table)));
            *surface.focus.view.borrow_mut() = Some(button);
            assert!(crate::keymap::key(&surface, &down(VK_RETURN, false)));
            // Space has none: it presses.
            assert!(!crate::keymap::key(&surface, &down(VK_SPACE, false)));
            assert!(press_key(&surface, &down(VK_SPACE, false)));
        }

        #[test]
        fn a_view_that_takes_no_presses_leaves_them_to_the_bindings() {
            let (surface, view) = window();
            view.state_mut().events.set(EventKind::Press, false);
            crate::keymap::set(&surface, Arc::new(Keymap::new(bindings())));
            *surface.focus.view.borrow_mut() = Some(view.clone());
            assert!(!on_control(&surface));
            assert!(crate::keymap::key(&surface, &down(VK_RETURN, false)));
            assert!(!press_key(&surface, &down(VK_RETURN, false)));
            // So does a control out of sight: nothing would press it.
            {
                let mut s = view.state_mut();
                s.events.set(EventKind::Press, true);
                s.frame.origin.y = Points(400.0);
            }
            assert!(!on_control(&surface));
            assert!(crate::keymap::key(&surface, &down(VK_SPACE, false)));
            assert!(!press_key(&surface, &down(VK_SPACE, false)));
        }
    }
}
