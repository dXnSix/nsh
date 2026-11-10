//! The keyboard focus on views: Tab moves it between the stops
//! ([`nsh_core::ViewProps::focus_stop`]), fields and the native components
//! that take it, in tree order ([`tab`], through
//! [`nsh_core::input::tab_stop`]), as the other desktops do: GTK's own focus
//! chain would go by the widgets' places on screen. The arrow keys move it
//! to the nearest stop in their direction, as on GPUI and the other
//! desktops ([`arrow`], through `nsh_core::focus`). `focus()` moves it to a
//! stop. A focused stop reports `FOCUSED`, `onFocus` and `onBlur`, Return
//! and Space press it, and Shift+F10 or the Menu key ask it for its context
//! menu.
//!
//! Views stay unfocusable until Tab or `focus()` picks them: GTK also
//! hands the focus around by itself (to the first focusable widget as a
//! window maps, into a parent's children when a focused widget goes, and
//! through a native component's widgets), and a click or a new window must
//! not land on a button.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gtk4::{self as gtk, gdk, glib, graphene, prelude::*};
use nsh_core::{
    AccessibilityRole, Bounds, EventKind, EventPayload, NodeId, ViewKind,
    focus::{self as core_focus, Candidate, Layer, Order},
    geometry::{Points, point, size},
    input as core_input,
    keymap::Key,
    tailwind::state,
};

use crate::{surface::Surface, view::NshView};

thread_local! {
    /// Whether a Tab key is moving the focus right now.
    static TABBING: Cell<bool> = const { Cell::new(false) };
}

/// Lets a view take part in the Tab GTK is moving the focus for
/// (`GtkWidget::focus`): only stops do, only then.
pub(crate) fn offer(view: &NshView) {
    if TABBING.get() {
        view.set_focusable(is_stop(view));
    }
}

/// After [`offer`]: a stop Tab passed over goes back to unfocusable.
pub(crate) fn offered(view: &NshView) {
    if TABBING.get() && !view.is_focus() {
        view.set_focusable(false);
    }
}

/// Whether Tab stops at `view`: it takes presses or is `focusable`, and
/// the pointer could reach it.
pub(crate) fn is_stop(view: &NshView) -> bool {
    {
        let state = view.state();
        if state.kind == ViewKind::Input
            || !state
                .props
                .focus_stop(state.events.contains(EventKind::Press))
        {
            return false;
        }
    }
    view.can_target() && view.is_visible()
}

/// `focus()` on a view that is not an input: the focus moves to it as Tab
/// moves it there, if it is a stop.
pub(crate) fn focus(view: &NshView) {
    if !is_stop(view) || view.is_focus() {
        return;
    }
    view.set_focusable(true);
    if !view.grab_focus() {
        view.set_focusable(false);
    }
}

/// `blur()` on a view that is not an input, when it has the focus.
pub(crate) fn blur(view: &NshView) -> bool {
    if !view.is_focus() {
        return false;
    }
    if let Some(root) = view.root() {
        root.set_focus(None::<&gtk::Widget>);
    }
    true
}

/// Follows the focus of the surface's window, and takes the keys of a
/// focused stop before the app's shortcuts and the focused widget's own
/// (Space would scroll, Return would fire a global binding).
pub(crate) fn install(surface: &Rc<Surface>) {
    let weak = Rc::downgrade(surface);
    let installed = RefCell::new(glib::WeakRef::<gtk::Window>::new());
    surface.root.connect_realize(move |root| {
        let Some(window) = root.root().and_downcast::<gtk::Window>() else {
            return;
        };
        if installed.borrow().upgrade().as_ref() == Some(&window) {
            return;
        }
        installed.borrow().set(Some(&window));
        if let Some(surface) = weak.upgrade() {
            watch(&surface, &window);
        }
    });
}

/// Follows the focus in `window` (the main one or one of the app's own,
/// each with focus chains of its own) and takes a focused stop's keys.
pub(crate) fn watch(surface: &Rc<Surface>, window: &gtk::Window) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed = Rc::downgrade(surface);
    keys.connect_key_pressed(move |keys, key, _, modifiers| match pressed.upgrade() {
        Some(surface) if key_pressed(&surface, keys, key, modifiers) => glib::Propagation::Stop,
        _ => glib::Propagation::Proceed,
    });
    window.add_controller(keys);
    // Tab after the app's bindings and the focused widget's (a native
    // component's own): the window's `move-focus` binding.
    let tabbed = Rc::downgrade(surface);
    window.connect_move_focus(move |window, direction| {
        let back = match direction {
            gtk::DirectionType::TabForward => false,
            gtk::DirectionType::TabBackward => true,
            _ => return,
        };
        if let Some(surface) = tabbed.upgrade()
            && tab(&surface, window, back)
        {
            window.stop_signal_emission_by_name("move-focus");
        }
    });
    // The stop the focus left reports its blur in the window's first
    // handlers, the one it reached its focus after all of them (`after`):
    // an input's own handler, connected later, reports between them, so
    // `onBlur` always comes before `onFocus`.
    let current = Rc::new(RefCell::new(None::<NshView>));
    let weak = Rc::downgrade(surface);
    let leaving = current.clone();
    window.connect_focus_widget_notify(move |window| {
        if let Some(surface) = weak.upgrade() {
            left(&surface, window, &leaving);
        }
    });
    let weak = Rc::downgrade(surface);
    let watched = window.downgrade();
    window.connect_local("notify::focus-widget", true, move |_| {
        if let (Some(surface), Some(window)) = (weak.upgrade(), watched.upgrade()) {
            reached(&surface, &window, &current);
        }
        None
    });
}

/// A stop of Tab: the widget whose subtree has the focus while the stop
/// does (an input's view, a native component's widget), and the widget
/// that takes it.
struct TabStop {
    owner: gtk::Widget,
    target: gtk::Widget,
}

/// Tab (`back`: Shift+Tab) in `window`: the focus moves to the next stop
/// of the window's root view, in tree order (a scroll view's content as it
/// is on screen, [`NshView::reading_children`]), around from the last;
/// the newest modal keeps it (the views under it cannot take the focus:
/// `can-focus`). False when there is no stop: GTK moves the focus then.
fn tab(surface: &Surface, window: &gtk::Window, back: bool) -> bool {
    let Some(root) = window
        .surface()
        .and_then(|s| crate::window::root_at(surface, &s))
    else {
        return false;
    };
    let stops = tab_stops(&root);
    // The innermost stop the focus is in.
    let current = GtkWindowExt::focus(window).and_then(|at| {
        stops
            .iter()
            .rposition(|stop| at == stop.owner || at.is_ancestor(&stop.owner))
    });
    let Some(next) = core_input::tab_stop(current, stops.len(), back, true) else {
        return false;
    };
    if Some(next) != current {
        let target = &stops[next].target;
        match target.downcast_ref::<NshView>() {
            Some(view) => focus(view),
            None => {
                target.grab_focus();
            }
        }
    }
    true
}

/// The stops of Tab under `root`, in tree order.
fn tab_stops(root: &NshView) -> Vec<TabStop> {
    let mut stops = Vec::new();
    for child in children(root) {
        collect_tab_stops(&child, &mut stops);
    }
    stops
}

/// `view`'s child widgets in the order Tab goes through them.
fn children(view: &NshView) -> Vec<gtk::Widget> {
    view.reading_children().unwrap_or_else(|| {
        let mut out = Vec::new();
        let mut child = view.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            out.push(widget);
        }
        out
    })
}

/// Whether `widget` and its subtree may take the focus: shown, sensitive
/// and not kept out (`can-focus`: the views under the newest modal).
fn reachable(widget: &gtk::Widget) -> bool {
    widget.is_visible() && widget.is_child_visible() && widget.is_sensitive() && widget.can_focus()
}

fn collect_tab_stops(widget: &gtk::Widget, out: &mut Vec<TabStop>) {
    if !reachable(widget) {
        return;
    }
    let Some(view) = widget.downcast_ref::<NshView>() else {
        // A native component's widget: one stop, its first focusable one.
        if let Some(target) = first_focusable(widget, 0) {
            out.push(TabStop {
                owner: widget.clone(),
                target,
            });
        }
        return;
    };
    if !view.can_target() {
        return;
    }
    if view.kind() == ViewKind::Input {
        if let Some(editor) = crate::text_input::key_widget(view)
            && reachable(&editor)
            && editor.is_focusable()
        {
            out.push(TabStop {
                owner: widget.clone(),
                target: editor,
            });
        }
        return;
    }
    if is_stop(view) {
        out.push(TabStop {
            owner: widget.clone(),
            target: widget.clone(),
        });
    }
    for child in children(view) {
        collect_tab_stops(&child, out);
    }
}

/// The first focusable widget of a native component's subtree, depth
/// first from `widget`.
fn first_focusable(widget: &gtk::Widget, depth: usize) -> Option<gtk::Widget> {
    if depth > 16 || !reachable(widget) {
        return None;
    }
    if widget.is_focusable() {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = first_focusable(&c, depth + 1) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// The focused stop of `surface`, in whichever of its windows.
fn focused_stop(surface: &Surface, focus: Option<gtk::Widget>) -> Option<NshView> {
    focus
        .and_downcast::<NshView>()
        .filter(|view| crate::window::holds(surface, view))
        .filter(is_stop)
}

/// The window's focus moved, first round: the stop it left says so.
fn left(surface: &Surface, window: &gtk::Window, current: &RefCell<Option<NshView>>) {
    let next = focused_stop(surface, GtkWindowExt::focus(window));
    if *current.borrow() == next {
        return;
    }
    if let Some(view) = current.take() {
        blurred(surface, &view);
    }
}

/// The window's focus moved, last round (after every other handler, an
/// input's own blur among them): the stop it reached says so.
fn reached(surface: &Surface, window: &gtk::Window, current: &RefCell<Option<NshView>>) {
    let next = focused_stop(surface, GtkWindowExt::focus(window));
    if *current.borrow() == next {
        return;
    }
    if let Some(view) = current.replace(next.clone()) {
        blurred(surface, &view);
    }
    if let Some(view) = next {
        crate::input::set_state(surface, &view, state::FOCUSED, true);
        surface.event(view.id(), EventKind::Focus, EventPayload::None);
    }
}

/// Stop `view` lost the focus.
fn blurred(surface: &Surface, view: &NshView) {
    view.set_focusable(false);
    crate::input::set_state(surface, view, state::FOCUSED, false);
    surface.event(view.id(), EventKind::Blur, EventPayload::None);
}

/// A key on its way to the focus: whether it was taken here.
fn key_pressed(
    surface: &Rc<Surface>,
    keys: &gtk::EventControllerKey,
    key: gdk::Key,
    modifiers: gdk::ModifierType,
) -> bool {
    if matches!(
        key,
        gdk::Key::Tab | gdk::Key::ISO_Left_Tab | gdk::Key::KP_Tab
    ) {
        // GTK moves the focus right after this, in the same dispatch.
        TABBING.set(true);
        glib::idle_add_local_once(|| TABBING.set(false));
        return false;
    }
    // The focus of the window the key is going to.
    let focus = keys.widget().and_then(|w| w.root()).and_then(|r| r.focus());
    let Some(view) = focused_stop(surface, focus) else {
        return false;
    };
    // The app's bindings come first (a menu's trigger opens at its first
    // item with Return): the shortcut controller runs right after this.
    if keys
        .current_event()
        .is_some_and(|event| crate::keymap::claims(surface, &event))
    {
        return false;
    }
    let held = crate::input::modifiers(modifiers);
    let function = (key == gdk::Key::F10).then_some(Key::F(10));
    if core_input::opens_context_menu(key == gdk::Key::Menu, function, held) {
        return show_context_menu(surface, &view);
    }
    let activates = matches!(
        key,
        gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter | gdk::Key::space
    ) && !(held.ctrl || held.alt || held.cmd);
    if activates && view.state().events.contains(EventKind::Press) {
        crate::input::activate(surface, &view);
        return true;
    }
    false
}

/// A context menu asked for on `view` without a pointer: the nearest view
/// from it outwards that listens to `onContextMenu` hears it, with no
/// pointer, at its bottom-start corner. True when one heard it.
fn show_context_menu(surface: &Surface, view: &NshView) -> bool {
    let rtl = surface.mounter.try_borrow().is_ok_and(|m| m.rtl);
    let mut at = Some(view.clone());
    while let Some(v) = at {
        let (id, listens, size) = {
            let s = v.state();
            (
                s.id,
                s.events.contains(EventKind::ContextMenu),
                s.frame.size,
            )
        };
        if listens {
            let position = core_input::context_menu_anchor(size, rtl);
            let (x, y) = v
                .compute_point(
                    &crate::window::root_of(surface, &v),
                    &graphene::Point::new(position.x.0, position.y.0),
                )
                .map_or((position.x.0, position.y.0), |p| (p.x(), p.y()));
            let event = nsh_core::ContextMenuEvent::new(
                position,
                point(Points(x), Points(y)),
                Default::default(),
                None,
            );
            surface.event(id, EventKind::ContextMenu, EventPayload::ContextMenu(event));
            return true;
        }
        at = v.parent_view();
    }
    false
}

// --- The arrows -------------------------------------------------------------------

/// An arrow key, the way it moves the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

impl Arrow {
    /// The arrow of `key` (the keypad's too) held with no modifier: with
    /// Shift it selects, with Ctrl, Alt or Super it is a shortcut (Alt+Left
    /// goes back).
    pub(crate) fn of(key: gdk::Key, modifiers: gdk::ModifierType) -> Option<Arrow> {
        use gdk::{Key, ModifierType as M};
        let held = M::SHIFT_MASK
            | M::CONTROL_MASK
            | M::ALT_MASK
            | M::SUPER_MASK
            | M::META_MASK
            | M::HYPER_MASK;
        if modifiers.intersects(held) {
            return None;
        }
        Some(match key {
            Key::Left | Key::KP_Left => Arrow::Left,
            Key::Right | Key::KP_Right => Arrow::Right,
            Key::Up | Key::KP_Up => Arrow::Up,
            Key::Down | Key::KP_Down => Arrow::Down,
            _ => return None,
        })
    }

    fn horizontal(self) -> bool {
        matches!(self, Arrow::Left | Arrow::Right)
    }
}

/// A stop of the arrows: the core's candidate, the widget whose subtree
/// has the focus while the stop does (an input's view) and its view.
struct ArrowStop {
    node: Candidate,
    owner: gtk::Widget,
    view: NshView,
}

impl AsRef<Candidate> for ArrowStop {
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

/// The box `widget` covers in `root`'s points as it shows (scroll offsets
/// and transforms applied: GTK's `compute_bounds`, the box around a
/// transformed one).
fn edges(widget: &gtk::Widget, root: &NshView) -> Option<Edges> {
    let r = widget.compute_bounds(root)?;
    Some((r.x(), r.y(), r.x() + r.width(), r.y() + r.height()))
}

/// The box of `root`'s window, in its points.
fn window_edges(root: &NshView) -> Edges {
    (0.0, 0.0, root.width() as f32, root.height() as f32)
}

/// The views of a focus scope as the core's candidates, in the order Tab
/// goes through them.
struct Walk<'a> {
    root: &'a NshView,
    scope: NodeId,
    stops: Vec<ArrowStop>,
    order: usize,
}

impl Walk<'_> {
    fn widget(&mut self, widget: &gtk::Widget, clip: Edges, layer: NodeId, popup: bool) {
        // A native component keeps the keys once it has the focus: it is
        // no stop of the arrows. Another modal is a scope of its own.
        let Some(view) = widget.downcast_ref::<NshView>() else {
            return;
        };
        if !reachable(widget) || !view.can_target() || view.kind() == ViewKind::Modal {
            return;
        }
        let order = self.order;
        self.order += 1;
        let popup = popup || view.state().props.overlay_anchor.is_some();
        let own = edges(widget, self.root);
        let candidate = |editable: bool, frame: Edges| {
            let mut node = Candidate::default();
            node.id = view.id();
            node.frame = bounds(frame);
            node.visible = shows(intersect(frame, clip));
            node.stop = true;
            node.editable = editable;
            node.scope = self.scope;
            node.layer = Some(layer);
            node.popup = popup;
            node.order = Order::view(order);
            node
        };
        if view.kind() == ViewKind::Input {
            if let (Some(editor), Some(own)) = (crate::text_input::key_widget(view), own)
                && reachable(&editor)
                && editor.is_focusable()
            {
                let editable = crate::text_input::editable(view);
                self.stops.push(ArrowStop {
                    node: candidate(editable, own),
                    owner: widget.clone(),
                    view: view.clone(),
                });
            }
            return;
        }
        if let Some(own) = own
            && is_stop(view)
        {
            self.stops.push(ArrowStop {
                node: candidate(false, own),
                owner: widget.clone(),
                view: view.clone(),
            });
        }
        let clip = match own {
            Some(own) if view.state().clips() => intersect(own, clip),
            _ => clip,
        };
        for child in children(view) {
            self.widget(&child, clip, layer, popup);
        }
    }
}

/// The stops of `scope` (the newest modal of `root`'s window, or `root`)
/// in focus order (`focus::candidates`).
fn arrow_stops(root: &NshView, scope: &NshView) -> Vec<ArrowStop> {
    let mut walk = Walk {
        root,
        scope: scope.id(),
        stops: Vec::new(),
        order: 0,
    };
    let window = window_edges(root);
    let clip = edges(scope.upcast_ref(), root).map_or(window, |own| intersect(own, window));
    for child in children(scope) {
        let layer = child
            .downcast_ref::<NshView>()
            .map_or(scope.id(), NshView::id);
        walk.widget(&child, clip, layer, false);
    }
    core_focus::candidates(walk.stops, scope.id(), &layers(root, scope))
}

/// The children of `scope` as the core's layers, the page first.
fn layers(root: &NshView, scope: &NshView) -> Vec<Layer> {
    let window = window_edges(root);
    scope
        .child_views()
        .iter()
        .filter(|child| child.kind() != ViewKind::Modal)
        .enumerate()
        .map(|(order, child)| {
            let mut layer = Layer::default();
            layer.id = child.id();
            layer.order = order;
            if reachable(child.upcast_ref()) && child.can_target() {
                layer.frame = edges(child.upcast_ref(), root)
                    .map(|e| intersect(e, window))
                    .filter(|&e| shows(e))
                    .map(bounds);
                layer.takes_presses = core_input::has_press(child.state().events);
            }
            layer
        })
        .collect()
}

/// `ancestor` is `view` or one of its ancestors.
fn contains(ancestor: &NshView, view: &NshView) -> bool {
    view == ancestor || view.is_ancestor(ancestor)
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
fn triggered(surface: &Surface, popup: &NshView, stops: &[ArrowStop]) -> bool {
    let anchor = popup
        .state()
        .props
        .overlay_anchor
        .as_ref()
        .and_then(|anchor| anchor.node);
    let Some(anchor) = anchor.and_then(|id| {
        let mounter = surface.mounter.try_borrow().ok()?;
        mounter.views.get(&id).cloned()
    }) else {
        return false;
    };
    stops
        .iter()
        .any(|stop| contains(&anchor, &stop.view) || contains(&stop.view, &anchor))
}

/// `view` is a menu's item.
fn in_menu(view: &NshView) -> bool {
    matches!(
        view.state().props.accessibility.role,
        AccessibilityRole::Menuitem
            | AccessibilityRole::MenuItemCheckbox
            | AccessibilityRole::MenuItemRadio
    )
}

/// Whether the focused widget `focus` keeps `arrow` to itself: a native
/// component's widget keeps them all, an input's editor Left and Right,
/// and a multiline one's text view Up and Down too.
fn keeps(focus: &gtk::Widget, arrow: Arrow) -> bool {
    if focus.is::<NshView>() {
        return false;
    }
    let field = focus
        .ancestor(NshView::static_type())
        .and_downcast::<NshView>()
        .filter(|view| view.kind() == ViewKind::Input)
        .and_then(|view| crate::text_input::key_widget(&view))
        .is_some_and(|editor| editor == *focus);
    !field || arrow.horizontal() || focus.is::<gtk::TextView>()
}

/// `arrow` in the window `root` is the root view of: the focus moves to
/// the stop that way among the stops of the focus scope (the newest modal,
/// else `root`), if any; the window then scrolls it into view
/// ([`crate::scroll`]). With nothing focused Left / Right take the first
/// stop, Up / Down the first / last in sight. False when it moved none, or
/// the focused widget keeps the arrow ([`keeps`]).
pub(crate) fn arrow(surface: &Surface, root: &NshView, arrow: Arrow) -> bool {
    let focused = root.root().and_then(|r| r.focus());
    if focused.as_ref().is_some_and(|at| keeps(at, arrow)) {
        return false;
    }
    let scope = crate::navigation::focus_scope(surface, root);
    let mut stops = arrow_stops(root, &scope);
    // The innermost stop the focus is in.
    let at = |stops: &[ArrowStop]| {
        focused.as_ref().and_then(|at| {
            stops
                .iter()
                .rposition(|stop| *at == stop.owner || at.is_ancestor(&stop.owner))
        })
    };
    // Left and Right on a menu's item would leave its menu open behind
    // the focus: they stay in it.
    if arrow.horizontal()
        && let Some(popup) = at(&stops)
            .map(|i| stops[i].view.clone())
            .filter(in_menu)
            .and_then(|view| anchored_overlay(&view))
            .filter(|popup| triggered(surface, popup, &stops))
    {
        stops.retain(|stop| contains(&popup, &stop.view));
    }
    let current = at(&stops);
    let next = match arrow {
        Arrow::Left | Arrow::Right => {
            core_focus::horizontal(&stops, current, arrow == Arrow::Right)
        }
        Arrow::Up | Arrow::Down => core_focus::vertical(&stops, current, arrow == Arrow::Down),
    };
    let Some(next) = next.filter(|&next| Some(next) != current) else {
        return false;
    };
    let view = &stops[next].view;
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
    } else {
        focus(view);
    }
    true
}

#[cfg(test)]
mod tests {
    use nsh_core::{EventMask, MountItem, ROOT_ID};

    use super::*;
    use crate::gtk_test::{apply, frame, run, show, surface, view, wait_until};

    /// A stop (2, it takes presses), a plain view (3) and a stop (4).
    fn views(surface: &Rc<Surface>) {
        let mut items = Vec::new();
        for (id, y) in [(2, 0.0), (3, 40.0), (4, 80.0)] {
            items.push(MountItem::Create {
                id,
                kind: ViewKind::View,
            });
            items.push(MountItem::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            });
            items.push(MountItem::Frame {
                id,
                frame: frame(0.0, y, 100.0, 30.0),
            });
            if id != 3 {
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                items.push(MountItem::Events { id, events });
            }
        }
        apply(surface, &items);
    }

    fn tab(window: &gtk::Window) {
        TABBING.set(true);
        window.child_focus(gtk::DirectionType::TabForward);
        TABBING.set(false);
    }

    #[test]
    fn tab_stops_at_stops_and_a_window_opening_focuses_none() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let (first, plain, last) = (view(&surface, 2), view(&surface, 3), view(&surface, 4));
            assert!(wait_until(|| first.is_mapped()));
            assert!(
                GtkWindowExt::focus(&window).is_none(),
                "nothing focused on opening"
            );
            tab(&window);
            assert!(first.is_focus());
            tab(&window);
            assert!(last.is_focus(), "the plain view is no stop");
            assert!(!plain.is_focusable());
            assert!(!first.is_focusable(), "unfocusable again once left");
            window.destroy();
        });
    }

    #[test]
    fn focus_moves_to_stops_only_and_blur_lets_go() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let (first, plain) = (view(&surface, 2), view(&surface, 3));
            assert!(wait_until(|| first.is_mapped()));
            focus(&plain);
            assert!(!plain.is_focus());
            focus(&first);
            assert!(first.is_focus());
            assert!(blur(&first));
            assert!(!first.is_focus());
            window.destroy();
        });
    }
}

/// Tab goes through the stops in tree order, not by their places on
/// screen: `cargo test -p nsh-platform-linux --lib key_focus::tab_order`.
#[cfg(test)]
mod tab_order {
    use nsh_core::{EventMask, MountItem, ROOT_ID};

    use super::*;
    use crate::gtk_test::{apply, frame, run, show, surface, view, wait_until};

    /// Stops 2, 3 and 4 in tree order, 3 above 2 on screen, and a plain
    /// view (5) between them.
    fn views(surface: &Rc<Surface>) {
        let mut items = Vec::new();
        for (id, y) in [(2, 40.0), (5, 80.0), (3, 0.0), (4, 120.0)] {
            items.push(MountItem::Create {
                id,
                kind: ViewKind::View,
            });
            items.push(MountItem::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            });
            items.push(MountItem::Frame {
                id,
                frame: frame(0.0, y, 100.0, 30.0),
            });
            if id != 5 {
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                items.push(MountItem::Events { id, events });
            }
        }
        apply(surface, &items);
    }

    fn tab(window: &gtk::Window, back: bool) {
        window.emit_move_focus(if back {
            gtk::DirectionType::TabBackward
        } else {
            gtk::DirectionType::TabForward
        });
    }

    #[test]
    fn tab_goes_through_the_stops_in_tree_order_and_around() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let stops = [view(&surface, 2), view(&surface, 3), view(&surface, 4)];
            assert!(wait_until(|| stops.iter().all(|v| v.is_mapped())));
            for stop in &stops {
                tab(&window, false);
                assert!(stop.is_focus(), "{} next in tree order", stop.id());
            }
            tab(&window, false);
            assert!(stops[0].is_focus(), "around from the last");
            tab(&window, true);
            assert!(stops[2].is_focus(), "Shift+Tab back around");
            tab(&window, true);
            assert!(stops[1].is_focus());
            assert!(
                !view(&surface, 5).is_focusable(),
                "the plain view is no stop"
            );
            window.destroy();
        });
    }
}

/// The view the focus leaves reports `onBlur` before the one it reaches
/// reports `onFocus`, an input among them (whose own handler on the
/// window's focus is connected after the stops'):
/// `cargo test -p nsh-platform-linux --lib key_focus::blur_first`.
#[cfg(test)]
mod blur_first {
    use std::sync::{Arc, Mutex};

    use nsh_core::element::IntoElement;

    use super::*;
    use crate::gtk_test::{run, show, surface, wait_until};

    type Heard = Arc<Mutex<Vec<(&'static str, EventKind)>>>;

    struct Recorder(Heard);

    /// A field (`name`) and a stop (`first`) after it.
    #[nsh_core::element::component]
    fn FieldAndStop() -> impl IntoElement {
        use nsh_core::prelude::*;

        let heard = engine_context::<Recorder>().unwrap().0.clone();
        let hear = |name: &'static str, kind: EventKind| {
            let heard = heard.clone();
            move |_: &EventPayload| heard.lock().unwrap().push((name, kind))
        };
        view()
            .child(
                nsh_core::element::input()
                    .test_id("name")
                    .w(px(100.0))
                    .h(px(30.0))
                    .on(EventKind::Focus, hear("name", EventKind::Focus))
                    .on(EventKind::Blur, hear("name", EventKind::Blur)),
            )
            .child(
                view()
                    .test_id("first")
                    .w(px(100.0))
                    .h(px(30.0))
                    .on(EventKind::Press, |_: &EventPayload| {})
                    .on(EventKind::Focus, hear("first", EventKind::Focus))
                    .on(EventKind::Blur, hear("first", EventKind::Blur)),
            )
    }

    /// Waits until the app has handled every event sent so far.
    fn flush(surface: &Surface) {
        let (sent, received) = std::sync::mpsc::channel();
        surface.control(|control| {
            control.flush(move |done| {
                let _ = sent.send(done);
            });
        });
        assert!(wait_until(|| received.try_recv() == Ok(true)));
    }

    fn tab(window: &gtk::Window) {
        window.emit_move_focus(gtk::DirectionType::TabForward);
    }

    #[test]
    fn blur_comes_before_focus_between_a_field_and_a_stop() {
        run(|| {
            let surface = surface();
            let heard = Heard::default();
            let mut config = nsh_engine::EngineConfig::new(nsh_engine::Root::of::<FieldAndStop>());
            config.context = config.context.with(Recorder(heard.clone()));
            surface.start(config);
            let window = show(&surface);
            let mapped = |id: &str| {
                surface
                    .mounter
                    .borrow()
                    .views
                    .values()
                    .any(|v| v.state().props.test_id.as_deref() == Some(id) && v.is_mapped())
            };
            assert!(wait_until(|| mapped("name") && mapped("first")));
            let take = || {
                flush(&surface);
                std::mem::take(&mut *heard.lock().unwrap())
            };
            tab(&window);
            assert_eq!(take(), [("name", EventKind::Focus)]);
            tab(&window);
            assert_eq!(
                take(),
                [("name", EventKind::Blur), ("first", EventKind::Focus)],
                "from the field to the stop"
            );
            tab(&window);
            assert_eq!(
                take(),
                [("first", EventKind::Blur), ("name", EventKind::Focus)],
                "around, from the stop to the field"
            );
            window.destroy();
        });
    }
}

/// The arrows move the focus to the nearest stop in their direction, by
/// the stops' places on screen, not the tree's order, and a field keeps
/// Left and Right: `cargo test -p nsh-platform-linux --lib key_focus::arrows`.
#[cfg(test)]
mod arrows {
    use gdk::{Key, ModifierType as M};
    use nsh_core::{EventMask, MountItem, ROOT_ID, element::IntoElement};

    use super::*;
    use crate::gtk_test::{apply, frame, run, show, surface, view, wait_until};

    #[test]
    fn arrows_are_the_four_keys_without_modifiers() {
        assert_eq!(Arrow::of(Key::Left, M::empty()), Some(Arrow::Left));
        assert_eq!(Arrow::of(Key::KP_Right, M::empty()), Some(Arrow::Right));
        assert_eq!(Arrow::of(Key::Up, M::empty()), Some(Arrow::Up));
        assert_eq!(
            Arrow::of(Key::Down, M::LOCK_MASK),
            Some(Arrow::Down),
            "Caps Lock on"
        );
        for held in [M::SHIFT_MASK, M::CONTROL_MASK, M::ALT_MASK, M::SUPER_MASK] {
            assert_eq!(Arrow::of(Key::Left, held), None, "{held:?}");
        }
        assert_eq!(Arrow::of(Key::Tab, M::empty()), None);
    }

    /// Stops 2 (right of 3 on screen, first in the tree), 3 and 4 (below
    /// 3), and a plain view (5) right of 4.
    fn views(surface: &Rc<Surface>) {
        let mut items = Vec::new();
        for (id, x, y) in [
            (2, 120.0, 0.0),
            (3, 0.0, 0.0),
            (4, 0.0, 60.0),
            (5, 120.0, 60.0),
        ] {
            items.push(MountItem::Create {
                id,
                kind: ViewKind::View,
            });
            items.push(MountItem::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            });
            items.push(MountItem::Frame {
                id,
                frame: frame(x, y, 70.0, 30.0),
            });
            if id != 5 {
                let mut events = EventMask::default();
                events.set(EventKind::Press, true);
                items.push(MountItem::Events { id, events });
            }
        }
        apply(surface, &items);
    }

    #[test]
    fn arrows_go_to_the_nearest_stop_that_way() {
        run(|| {
            let surface = surface();
            views(&surface);
            let window = show(&surface);
            let [right, left, below] = [2, 3, 4].map(|id| view(&surface, id));
            // Laid out: the arrows go by the frames GTK allocated.
            assert!(wait_until(|| [&right, &left, &below]
                .iter()
                .all(|v| v.is_mapped() && v.width() > 0)));
            let root = surface.root.clone();
            let go = |to: Arrow| arrow(&surface, &root, to);
            assert!(go(Arrow::Right), "nothing focused: the first stop");
            assert!(right.is_focus(), "the first in tree order");
            assert!(!go(Arrow::Right), "nothing right of it");
            assert!(right.is_focus());
            assert!(go(Arrow::Left));
            assert!(left.is_focus());
            assert!(go(Arrow::Down));
            assert!(below.is_focus());
            assert!(!go(Arrow::Right), "the plain view is no stop");
            assert!(go(Arrow::Up));
            assert!(left.is_focus(), "the one in its column");
            assert!(!go(Arrow::Up), "nothing above");
            assert!(!right.is_focusable(), "unfocusable again once left");
            GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
            assert!(go(Arrow::Up), "nothing focused: the last in sight");
            assert!(below.is_focus());
            window.destroy();
        });
    }

    /// A field (`name`) and a stop (`below`) under it.
    #[nsh_core::element::component]
    fn FieldOverStop() -> impl IntoElement {
        use nsh_core::prelude::*;

        view()
            .child(
                nsh_core::element::input()
                    .test_id("name")
                    .w(px(100.0))
                    .h(px(30.0)),
            )
            .child(
                view()
                    .test_id("below")
                    .w(px(100.0))
                    .h(px(30.0))
                    .on(EventKind::Press, |_: &EventPayload| {}),
            )
    }

    #[test]
    fn a_field_keeps_left_and_right_and_leaves_with_down() {
        run(|| {
            let surface = surface();
            surface.start(nsh_engine::EngineConfig::new(nsh_engine::Root::of::<
                FieldOverStop,
            >()));
            let window = show(&surface);
            let find = |id: &str| {
                surface
                    .mounter
                    .borrow()
                    .views
                    .values()
                    .find(|v| {
                        v.state().props.test_id.as_deref() == Some(id)
                            && v.is_mapped()
                            && v.width() > 0
                    })
                    .cloned()
            };
            assert!(wait_until(
                || find("name").is_some() && find("below").is_some()
            ));
            let (name, below) = (find("name").unwrap(), find("below").unwrap());
            let editor = crate::text_input::key_widget(&name).unwrap();
            let root = surface.root.clone();
            crate::text_input::focus(&name);
            assert!(wait_until(|| editor.has_focus()));
            assert!(!arrow(&surface, &root, Arrow::Left), "the field's");
            assert!(!arrow(&surface, &root, Arrow::Right), "the field's");
            assert!(editor.has_focus());
            assert!(arrow(&surface, &root, Arrow::Down), "out of a single line");
            assert!(below.is_focus());
            assert!(arrow(&surface, &root, Arrow::Up));
            assert!(wait_until(|| editor.has_focus()), "back to the field");
            window.destroy();
        });
    }
}
