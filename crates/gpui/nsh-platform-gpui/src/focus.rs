//! Keyboard focus between the views: the arrow keys move it, Enter and
//! Space press the focused view. The decisions are `nsh_core::focus`'s,
//! over the views' frames in the window's points (as drawn: scroll
//! offsets, shifts and transforms applied, each frame the box around its
//! corners), as the terminal backend's are over its Cells.
//!
//! Text fields keep their own focus ([`crate::text_input`]) and native
//! components theirs ([`crate::native`]); this module holds the focus of
//! every other stop (`ViewProps::focus_stop`: a pressable, a view
//! `focusable` makes one), reported as the fields' is: `FOCUSED` (so
//! `focus:` styles follow), `onFocus` / `onBlur`, and the accessibility
//! focus. One of the three has the keyboard at a time.
//!
//! - **Stops**: the views of the focus scope (the newest modal of the
//!   window the key is in, else the window: [`crate::navigation::focus_scope`])
//!   that are stops, not disabled, shown (not hidden, concealed or out of
//!   the pointer's reach, `pointerEvents: "none"`) and have an area, or
//!   take typing (`focus::candidates`); a backdrop over the page (a
//!   sheet's, a dialog's) takes those under it out of reach
//!   (`focus::covering_layer`). Their order is the order Tab and screen
//!   readers go through the views ([`NshView::with_children_in_reading_order`]).
//! - **Arrows** (no modifier): Left / Right go to the nearest stop on an
//!   overlapping row, Up / Down to the nearest below / above, one in an
//!   overlapping column first (`focus::horizontal`, `focus::vertical`);
//!   with nothing focused Left / Right take the first stop, Up / Down the
//!   first / last in sight, in an open popup anchored to another view when
//!   there is one. While a menu's item has the focus, Left and Right stay
//!   in its popup. The stop reached is scrolled into view
//!   ([`crate::scroll::reveal`], as a focused field is); a field reached
//!   takes the typing, a native component the keyboard. An arrow that
//!   moves no focus goes on to the scroll views ([`crate::scroll::key`]),
//!   as before.
//! - **What keeps its arrows**: the app's bindings and the menus come
//!   first, a field keeps the keys it uses (Left / Right, and Up / Down
//!   in a multiline one: Up / Down leave a single-line field, as in the
//!   terminal), and so does selectable text with a selection and a native
//!   component with the keyboard ([`crate::input::key_now`]'s order).
//! - **Enter and Space** press the focused view when it takes presses
//!   (`PressDown`, `PressUp`, `Press` at its middle, as the terminal
//!   does), once the focus' own bindings passed on them: the app's other
//!   bindings of those keys give way to it ([`on_control`],
//!   `Keymap::find_at`); one that takes none leaves them to the app's
//!   bindings.
//! - **What a TV adds** (`focus::directional`): with nothing focused the
//!   first `preferredFocus` stop in sight takes an arrow; a stop's
//!   `nextFocus*` route (its own, or that of a view around it that is no
//!   stop) goes to the view named or the first stop inside it; a
//!   `FocusGroup`'s view is entered at its first destination in reach (else
//!   its preferred stop) and with `trap` keeps the arrows; while the
//!   innermost scroll view along the arrow's axis around the focus can
//!   scroll on that way, a stop outside it is not reached: the view
//!   scrolls ([`crate::scroll::key_view`]) and the engine is told
//!   (`EngineControl::focus_past`), so a list scrolls to its next row and
//!   the core focuses that row's first stop once it is mounted.
//! - **`autoFocus` and `focus()`**: after a batch that brought a view with
//!   `autoFocus`, or a new focus scope (a modal presented), the first such
//!   stop of the scope not given its chance yet takes the focus
//!   (`focus::auto_focus`); `Command::Focus` on a stop that is no input (a
//!   ref's `focus()`, the core's for a list row) moves the focus to it
//!   after the batch, and the window takes the keyboard; a pointer going
//!   down outside the focused view takes it away ([`pointer_down`]), and a
//!   focused view that is removed loses it at once, without `onBlur`
//!   ([`detached`]).
//! - **Layers**: focus that went into a root view's child (the page, a
//!   sheet's panel, an overlay) from a stop of another comes back to that
//!   stop when the layer goes ([`focus::Layers`]); a stop a backdrop
//!   covers loses the focus and gets it back when that layer goes. Modals
//!   give the focus back themselves ([`crate::navigation`]).

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};

use nsh_core::{
    Bounds, EventKind, EventPayload, NodeId, PointerEvents, Points, ViewKind,
    focus::{self, Candidate, Direction, Group, Layer, NextFocus, Order, Target},
    geometry::{point, size},
    input::has_press,
    tailwind::state::FOCUSED,
};

use crate::{
    geom::Mat4,
    input::KeyEvent,
    keys::{VK_DOWN, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_UP},
    surface::Surface,
    view::NshView,
};

/// A surface's keyboard focus on views that are neither fields nor native
/// components.
#[derive(Default)]
pub(crate) struct Focus {
    /// The stop that has it.
    view: RefCell<Option<NshView>>,
    /// The layers the focus went into, each with the stop it came from.
    layers: RefCell<focus::Layers>,
    /// A view with `autoFocus` came, or the focus scope changed: the stops
    /// that take the focus as they appear are looked at after the batch.
    auto: Cell<bool>,
    /// The `autoFocus` stops given their chance already, by node.
    auto_seen: RefCell<HashSet<NodeId>>,
    /// The focus scope after the last batch.
    scope: Cell<NodeId>,
    /// The view a `Command::Focus` of the batch named (`focus()` on a ref,
    /// the row of a list the arrows reached): it takes the focus after the
    /// batch, when it is a stop of the scope.
    asked: RefCell<Option<NshView>>,
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
/// below the top), or out of the pointer's reach (as Tab skips them).
fn left_out(view: &NshView) -> bool {
    let s = view.state();
    s.style.hidden || s.concealed || s.props.pointer_events == PointerEvents::None
}

/// `view` takes typing: an input that edits.
fn editable(view: &NshView) -> bool {
    view.kind() == ViewKind::Input && crate::text_input::editable(view)
}

/// `view` is a focus stop of its own (whatever is around it).
fn is_stop(surface: &Surface, view: &NshView) -> bool {
    let s = view.state();
    if s.props.accessibility.state.disabled {
        return false;
    }
    match s.kind {
        ViewKind::Native => crate::native::focusable(surface, s.id),
        _ => {
            let takes = has_press(s.events);
            drop(s);
            let editable = editable(view);
            view.state().props.focus_stop(takes || editable)
        }
    }
}

/// The views of `scope` (a modal, or a window's root) as the core's
/// candidates and its children as the core's layers, the page first.
struct Walk<'a> {
    surface: &'a Surface,
    scope: NodeId,
    stops: Vec<Stop>,
    order: usize,
    /// The axis an arrow moves along (`true`: across), when one does: a
    /// stop's scroller is the innermost scroll view along it.
    axis: Option<bool>,
    /// The focus groups met, their destinations as the views named.
    groups: Vec<Group>,
}

const DIRECTIONS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];

/// What a view hands the stops inside it.
#[derive(Clone, Copy, Default)]
struct Around {
    /// The child of the scope's root view it is in.
    layer: Option<NodeId>,
    /// Inside an overlay anchored to another view.
    popup: bool,
    /// The innermost focus group.
    group: Option<NodeId>,
    /// The innermost scroll view along the arrow's axis.
    scroller: Option<NodeId>,
    /// `preferredFocus` of the view or one around it: a row's is its
    /// items'.
    preferred: bool,
    /// `autoFocus` of the view or one around it.
    auto: bool,
    /// The `nextFocus*` routes of the view and those around it, the
    /// nearest's first.
    next: NextFocus,
}

impl Walk<'_> {
    fn view(&mut self, view: &NshView, to_root: Mat4, clip: Edges, around: Around) {
        let order = self.order;
        self.order += 1;
        let mut around = around;
        let group = {
            let s = view.state();
            let props = &s.props;
            around.popup |= props.overlay_anchor.is_some();
            around.preferred |= props.preferred_focus;
            around.auto |= props.auto_focus;
            for direction in DIRECTIONS {
                if let Some(to) = props.next_focus.toward(direction) {
                    around.next.set(direction, Some(to));
                }
            }
            props.focus_group.clone()
        };
        if let Some(config) = group {
            let mut group = Group::default();
            group.id = view.id();
            group.parent = around.group;
            group.trap = config.trap;
            group.destinations = config.destinations;
            self.groups.push(group);
            around.group = Some(view.id());
        }
        let own = edges(view, &to_root);
        if is_stop(self.surface, view)
            && let Some(own) = own
        {
            let mut node = Candidate::default();
            node.id = view.id();
            node.frame = bounds(own);
            node.visible = shows(intersect(own, clip));
            node.stop = true;
            node.editable = editable(view);
            node.scope = self.scope;
            node.layer = around.layer;
            node.popup = around.popup;
            node.order = Order::view(order);
            node.preferred = around.preferred;
            node.auto_focus = around.auto;
            node.next = around.next;
            node.group = around.group;
            node.scroller = around.scroller;
            self.stops.push(Stop {
                node,
                view: view.clone(),
            });
        }
        let clip = match own {
            Some(own) if view.state().clips() => intersect(own, clip),
            _ => clip,
        };
        let along = view.kind() == ViewKind::Scroll && {
            let s = view.state();
            let horizontal = s.scroll.as_ref().is_some_and(|scroll| scroll.horizontal);
            self.axis.is_none_or(|axis| axis == horizontal)
        };
        if along {
            around.scroller = Some(view.id());
        }
        view.with_children_in_reading_order(|children| {
            for child in children {
                // Another modal is a scope of its own.
                if left_out(child) || child.kind() == ViewKind::Modal {
                    continue;
                }
                let to_root = Mat4::mul(&local(view, child), &to_root);
                let mut inside = around;
                inside.layer = around.layer.or(Some(child.id()));
                self.view(child, to_root, clip, inside);
            }
        });
    }
}

/// The stop a route or a group's destination naming view `id` reaches:
/// that view when it is one of `stops`, else the first of them inside it.
fn reached(surface: &Surface, stops: &[Stop], id: NodeId) -> Option<NodeId> {
    if stops.iter().any(|stop| stop.node.id == id) {
        return Some(id);
    }
    let target = surface.view(id)?;
    stops
        .iter()
        .find(|stop| contains(&target, &stop.view))
        .map(|stop| stop.node.id)
}

/// The root of the tree `view` is in (the surface's, or a window's of the
/// app's own).
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

/// The stops of `scope` in focus order (`focus::candidates`), their routes
/// as stops, with its focus groups, their destinations as stops; `axis`
/// the arrow's (`true`: across), whose scroll views are the stops'
/// scrollers.
fn stops_in(surface: &Surface, scope: &NshView, axis: Option<bool>) -> (Vec<Stop>, Vec<Group>) {
    let Some(to_root) = scope.to_root() else {
        return (Vec::new(), Vec::new());
    };
    let mut walk = Walk {
        surface,
        scope: scope.id(),
        stops: Vec::new(),
        order: 0,
        axis,
        groups: Vec::new(),
    };
    let clip = window_edges(scope);
    let clip = edges(scope, &to_root).map_or(clip, |own| intersect(own, clip));
    scope.with_children_in_reading_order(|children| {
        for child in children {
            if left_out(child) || child.kind() == ViewKind::Modal {
                continue;
            }
            let to_root = Mat4::mul(&local(scope, child), &to_root);
            let around = Around {
                layer: Some(child.id()),
                ..Around::default()
            };
            walk.view(child, to_root, clip, around);
        }
    });
    let mut stops = focus::candidates(walk.stops, scope.id(), &layers(scope));
    // Routes and destinations name views: the stops they reach.
    let routes: Vec<NextFocus> = stops
        .iter()
        .map(|stop| {
            let mut next = NextFocus::default();
            for direction in DIRECTIONS {
                let to = stop.node.next.toward(direction);
                next.set(direction, to.and_then(|id| reached(surface, &stops, id)));
            }
            next
        })
        .collect();
    for (stop, next) in stops.iter_mut().zip(routes) {
        stop.node.next = next;
    }
    let mut groups = walk.groups;
    for group in &mut groups {
        group.destinations = std::mem::take(&mut group.destinations)
            .into_iter()
            .filter_map(|id| reached(surface, &stops, id))
            .collect();
    }
    (stops, groups)
}

/// The children of `scope` (a modal, or a window's root) as the core's
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

/// The child of its root view (a window's root, or the modal it is in)
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

/// The view with this module's focus (not a field's, not a component's).
/// One the app removed loses it quietly (its node is gone).
pub(crate) fn focused(surface: &Surface) -> Option<NshView> {
    let view = surface.focus.view.borrow().clone()?;
    if view.attached() {
        return Some(view);
    }
    surface.focus.view.borrow_mut().take();
    None
}

/// The view with the keyboard focus: the focused field, else the view
/// with this module's (a native component's is GPUI's).
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

/// Gives this module's focus to `view` (a stop that is neither a field nor
/// a native component), or takes it away: the field that had the
/// keyboard is blurred first, then the view that had this focus, then
/// `view` is reported focused and takes the accessibility focus.
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

/// Gives the keyboard focus back to `view` (a stop a modal or a layer took
/// it from), as it had it: a field, or this module's focus.
pub(crate) fn restore(surface: &Rc<Surface>, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
    } else {
        set(surface, Some(view));
    }
}

/// Moves the keyboard focus to stop `view`, revealed in its scroll views:
/// a field takes the typing, a native component the keyboard (with the
/// window, [`crate::native::apply_focus`]), any other view this module's
/// focus.
fn focus_stop(surface: &Rc<Surface>, view: &NshView) {
    match view.kind() {
        ViewKind::Input => crate::text_input::focus(view),
        ViewKind::Native => {
            set(surface, None);
            crate::native::tab_to(surface, view.id());
            crate::scroll::reveal(surface, view);
        }
        _ => {
            set(surface, Some(view));
            crate::scroll::reveal(surface, view);
        }
    }
}

/// The pointer went down on `hit`: the view with this module's focus lets
/// go of it unless `hit` is inside it.
pub(crate) fn pointer_down(surface: &Surface, hit: &NshView) {
    if focused(surface).is_some_and(|view| !contains(&view, hit)) {
        blur_view(surface);
    }
}

/// A view was removed: if it holds the focused view, the focus goes, at
/// once (a removal animation or a departing screen keeps it attached a
/// while, when Return and Space must not press it). No `onBlur` goes to a
/// view that no longer exists; its state is cleared all the same, as a
/// kept node may be inserted again.
pub(crate) fn detached(surface: &Surface, view: &NshView) {
    let Some(stop) = focused(surface) else {
        return;
    };
    if contains(view, &stop) {
        surface.focus.view.borrow_mut().take();
        crate::input::set_state(surface, &stop, FOCUSED, false);
    }
}

/// Whether the focused view is a control that Return and Space press (it
/// takes presses): the app's bindings of those keys then give way to it,
/// but for the focus' own (`Keymap::find_at`,
/// [`nsh_core::input::presses_focused_control`]).
pub(crate) fn on_control(surface: &Surface) -> bool {
    focused(surface).is_some_and(|view| has_press(view.state().events))
}

/// A key down for the focus: the arrows move it, Enter and Space press the
/// focused view. True when it was taken.
pub(crate) fn key(surface: &Rc<Surface>, event: &KeyEvent) -> bool {
    let m = event.modifiers;
    if !event.down || m.ctrl || m.alt || m.meta {
        return false;
    }
    match event.vk {
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN if !m.shift => arrow(surface, event),
        VK_RETURN | VK_SPACE => press(surface, event),
        _ => false,
    }
}

/// An arrow: the focus moves to the stop in that direction, if any
/// (`focus::directional`); while the scroll view around the focus can
/// scroll on that way and the stop is outside it, the view scrolls
/// instead, and a list scrolls to its next row
/// (`EngineControl::focus_past`), whose first stop takes the focus once
/// mounted.
fn arrow(surface: &Rc<Surface>, event: &KeyEvent) -> bool {
    let direction = match event.vk {
        VK_LEFT => Direction::Left,
        VK_RIGHT => Direction::Right,
        VK_UP => Direction::Up,
        _ => Direction::Down,
    };
    let horizontal = direction.horizontal();
    let scope = crate::navigation::focus_scope(surface);
    let (mut stops, groups) = stops_in(surface, &scope, Some(horizontal));
    let current = current(surface);
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
    let sign = if direction.forward() { 1.0 } else { -1.0 };
    let target = focus::directional(
        &stops,
        at,
        direction,
        &groups,
        |scroller| {
            surface
                .view(scroller)
                .is_some_and(|view| crate::scroll::key_can_scroll(&view, horizontal, sign))
        },
        |scroller, id| {
            let stop = stops.iter().find(|stop| stop.node.id == id);
            match (surface.view(scroller), stop) {
                (Some(scroller), Some(stop)) => contains(&scroller, &stop.view),
                _ => false,
            }
        },
    );
    match target {
        Target::Stop(next) if Some(next) != at => {
            let view = stops[next].view.clone();
            focus_stop(surface, &view);
            true
        }
        // The view scrolls a key step and the engine hears the arrow: a
        // list with a next row that way scrolls to it by index. Both, as
        // neither does the other's part: a list and a scroll view are
        // both `ViewKind::Scroll` here and the core's answer comes a frame
        // later, so the step is what scrolls a scroll view and a list
        // past its last row (a footer, padding); a list's next row this
        // way is not mounted, so it lies past the rendered screen beyond
        // the viewport, and the core's `ScrollTo` there (absolute,
        // animated) takes over the step's animation the same way.
        Target::Scroll(scroller) => {
            if let Some(from) = &current {
                let from = from.id();
                surface.focus_past(from, direction);
            }
            surface
                .view(scroller)
                .is_some_and(|view| crate::scroll::key_view(surface, &view, event))
        }
        _ => false,
    }
}

/// Enter or Space: presses the view with this module's focus when it takes
/// presses and shows (an auto-repeat presses nothing more).
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
fn still_stop(surface: &Surface, scope: &NshView, view: &NshView) -> (bool, Option<NodeId>) {
    if !view.attached() || !is_stop(surface, view) {
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
            let (stop, cover) = still_stop(surface, &scope, &view);
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
                surface.view(id).is_some_and(|view| {
                    still_stop(surface, &scope, &view).0 && shows_in_window(&view)
                })
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
    // The stop a `focus()` of the batch named.
    let asked = surface.focus.asked.borrow_mut().take();
    if let Some(view) = asked
        && still_stop(surface, &scope, &view).0
    {
        surface.window_focus_requested.set(true);
        focus_stop(surface, &view);
    }
    // A modal that came, or one that went: its `autoFocus` stops appear.
    if surface.focus.scope.replace(scope.id()) != scope.id() {
        surface.focus.auto.set(true);
    }
    if surface.focus.auto.take() {
        auto_focus(surface, &scope);
    }
}

/// The first stop of `scope` with `autoFocus` that has not had its
/// chance takes the focus (`focus::auto_focus`); every one there has had
/// it then.
fn auto_focus(surface: &Rc<Surface>, scope: &NshView) {
    let (stops, _) = stops_in(surface, scope, None);
    let next = {
        let mut seen = surface.focus.auto_seen.borrow_mut();
        seen.retain(|&id| surface.view(id).is_some_and(|view| view.attached()));
        let next = focus::auto_focus(&stops, |id| seen.contains(&id));
        seen.extend(
            stops
                .iter()
                .filter(|stop| stop.node.auto_focus)
                .map(|stop| stop.node.id),
        );
        next
    };
    if let Some(next) = next {
        let view = stops[next].view.clone();
        focus_stop(surface, &view);
    }
}

/// Props of `view` were applied: one with `autoFocus` has the stops that
/// take the focus as they appear looked at after the batch.
pub(crate) fn props_changed(surface: &Surface, view: &NshView) {
    if view.state().props.auto_focus {
        surface.focus.auto.set(true);
    }
}

/// `Command::Focus` on `view`, a view that is no input (`focus()` on its
/// ref, a list row's stop the arrows reached): it takes the focus after
/// the batch, as Tab gives it, when it is a stop of the scope.
pub(crate) fn asked(surface: &Surface, view: &NshView) {
    *surface.focus.asked.borrow_mut() = Some(view.clone());
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
    surface.focus.auto.set(false);
    surface.focus.auto_seen.borrow_mut().clear();
    surface.focus.scope.set(0);
    surface.focus.asked.borrow_mut().take();
}

#[cfg(test)]
mod directional_focus;
#[cfg(test)]
mod tv_focus;
