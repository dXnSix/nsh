//! Keyboard focus moves between the views beyond Tab: the arrow keys move
//! it to the nearest stop in their direction, Return and Space press the
//! focused view. The decisions are `nsh_core::focus`'s, over the views'
//! frames in the window's points (as drawn: scroll offsets, shifts and
//! transforms applied, each frame the box around its corners), as the
//! GPUI backend's are.
//!
//! The focus itself is held where Tab, clicks and `ref.focus()` put it: a
//! text field's in [`crate::text_input`], any other stop's in
//! [`crate::keyboard`] (with its `FOCUSED` state,
//! `onFocus` / `onBlur`, accessibility focus and modal restore). This
//! module only reads it and moves it.
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
//! - **Arrows** (no modifier), as on GPUI: Left / Right go to the nearest
//!   stop on an overlapping row, Up / Down to the nearest below / above,
//!   one in an overlapping column first (`focus::horizontal`,
//!   `focus::vertical`); with nothing focused Left / Right take the first
//!   stop, Up / Down the first / last in sight, in an open popup anchored
//!   to another view when there is one. While a menu's item has the focus,
//!   Left and Right stay in its popup. The stop reached is scrolled into
//!   view; an arrow that moves no focus goes on to the scroll views
//!   ([`crate::scroll::key`]). What uses the arrows keeps them: the app's
//!   bindings, the focused view's capture rules, the focused field (Left /
//!   Right, and Up / Down in a multiline one) and selectable text with a
//!   selection come first ([`crate::input::key`]'s order).
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

use std::rc::Rc;

use nsh_core::{
    Bounds, EventKind, EventPayload, NodeId, PointerEvents, Points, ViewKind,
    focus::{self, Candidate, Layer, Order},
    geometry::{point, size},
    input::{has_press, presses_focused_control, tab_order},
    keymap::Key,
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

/// The custom view with the keyboard focus in the window the input is in
/// ([`crate::keyboard`]'s holder), not a field.
fn focused(surface: &Surface) -> Option<NshView> {
    crate::keyboard::focused_in(surface, surface.input_root().id())
}

/// The view with the keyboard focus: the focused field, else the custom
/// view with it.
fn current(surface: &Surface) -> Option<NshView> {
    crate::text_input::focused(surface).or_else(|| focused(surface))
}

/// Moves the keyboard focus to stop `view`, revealed in its scroll views:
/// a field takes the typing, any other view [`crate::keyboard`]'s focus.
fn focus_stop(surface: &Rc<Surface>, view: &NshView) {
    if view.kind() == ViewKind::Input {
        crate::text_input::focus(view);
        crate::scroll::reveal(surface, view);
    } else {
        crate::keyboard::focus_control(surface, view);
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
                scan: 0,
                extended: false,
                sys: false,
                translated: false,
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
            assert!(crate::keyboard::focus_control(&surface, &button));
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
            assert!(crate::keyboard::focus_control(&surface, &button));
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
            assert!(crate::keyboard::focus_control(&surface, &view));
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
