//! The arrow keys move the keyboard focus to the nearest stop in their
//! direction, as on GPUI and Windows: the decisions are `nsh_core::focus`'s
//! ([`focus::candidates`], [`focus::horizontal`], [`focus::vertical`]),
//! over the stops' frames in the window's points as they show (flipped
//! coordinates, the scroll views' offsets and the views' transforms
//! applied: [`crate::input::to_top`], each frame the box around its
//! corners; AppKit's conversions would leave the layer transforms out).
//!
//! - **Stops**: those Tab goes through ([`crate::input::tab_stops`]) in the
//!   focus scope (the newest modal, else the window's root:
//!   [`crate::navigation::focus_scope`]), in the same order: the views
//!   [`crate::input::is_key_stop`] takes and the inputs' fields, with an
//!   area (a field that edits at any size); a backdrop over the page (a
//!   sheet's, a dialog's) takes those under it out of reach
//!   ([`focus::covering_layer`]). Hosted AppKit views (a web view) are no
//!   stops of the arrows: they keep the keys once they have the focus.
//! - **Moves**: Left / Right go to the nearest stop on an overlapping row,
//!   Up / Down to the nearest below / above, one in an overlapping column
//!   first; with nothing focused (the root has the keyboard) Left / Right
//!   take the first stop, Up / Down the first / last in sight, in an open
//!   popup anchored to another view when there is one. While a menu's item
//!   has the focus, Left and Right stay in its popup. A view reached
//!   becomes first responder (`focus:`, `onFocus`) and is scrolled into
//!   view ([`crate::scroll::reveal`]); a field reached takes the typing,
//!   its caret where `focus()` puts it.
//! - **Order**: the app's bindings come first (the key monitor,
//!   [`crate::keymap`]), then the focused field's own keys (Left / Right
//!   and, in a multiline input, Up / Down; Up / Down of a single line come
//!   here from its `moveUp:` / `moveDown:`, and move its caret to the start
//!   / end when no stop is that way), then the root's selection keys and
//!   going back ([`crate::input::key`]); an arrow that moves no focus
//!   scrolls as before.

use std::rc::Rc;

use nsh_core::{
    AccessibilityRole, Bounds, NodeId, Points, ViewKind,
    focus::{self, Candidate, Layer, Order},
    geometry::{point, size},
    input::has_press,
};
use objc2::rc::Retained;
use objc2_app_kit::{NSEventModifierFlags, NSView};

use crate::{surface::Surface, view::NshView};

/// An arrow key, the way it moves the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

impl Arrow {
    /// The arrow of virtual key code `code` (`kVK_*Arrow`) held with no
    /// modifier (the arrow keys' own Function and NumericPad flags aside):
    /// with one it edits, goes back or scrolls by a page.
    pub(crate) fn of(code: u16, flags: NSEventModifierFlags) -> Option<Arrow> {
        if flags.intersects(
            NSEventModifierFlags::Command
                | NSEventModifierFlags::Control
                | NSEventModifierFlags::Option
                | NSEventModifierFlags::Shift,
        ) {
            return None;
        }
        Some(match code {
            123 => Arrow::Left,
            124 => Arrow::Right,
            125 => Arrow::Down,
            126 => Arrow::Up,
            _ => return None,
        })
    }

    fn horizontal(self) -> bool {
        matches!(self, Arrow::Left | Arrow::Right)
    }
}

/// A stop of the arrows: the core's candidate, its view (an input's, for
/// a field) and the view that becomes first responder.
struct Stop {
    node: Candidate,
    view: NshView,
    key: Retained<NSView>,
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

/// The box `view` covers in its window's root's points, as it shows: the
/// box around its corners, scroll offsets and transforms applied. `None`
/// when a transform's perspective puts a corner at infinity.
fn edges(view: &NshView) -> Option<Edges> {
    let size = view.state().frame.size;
    let (w, h) = (size.width.0, size.height.0);
    let mut e = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for corner in [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)] {
        let (x, y) = crate::input::to_top(view, corner, true)?;
        e = (e.0.min(x), e.1.min(y), e.2.max(x), e.3.max(y));
    }
    Some(e)
}

/// The box of the window `view` shows in: its root's, in its points.
fn window_edges(view: &NshView) -> Edges {
    let mut top = view.clone();
    while let Some(parent) = top.parent_view() {
        top = parent;
    }
    let size = top.state().frame.size;
    (0.0, 0.0, size.width.0, size.height.0)
}

/// Neither `view` nor its subtree takes part, as for Tab: another modal
/// (a scope of its own), hidden, animating out or out of the pointer's
/// reach.
fn left_out(view: &NshView) -> bool {
    view.kind() == ViewKind::Modal || !view.can_target() || view.ns().isHiddenOrHasHiddenAncestor()
}

/// The views of `scope` as the core's candidates, in the order Tab goes
/// through them.
struct Walk {
    scope: NodeId,
    stops: Vec<Stop>,
    order: usize,
}

impl Walk {
    fn view(&mut self, view: &NshView, clip: Edges, layer: NodeId, popup: bool) {
        if left_out(view) {
            return;
        }
        let order = self.order;
        self.order += 1;
        let popup = popup || view.state().props.overlay_anchor.is_some();
        let own = edges(view);
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
            if let (Some(key), Some(own)) = (crate::text_input::key_view(view), own)
                && key.canBecomeKeyView()
            {
                let editable = crate::text_input::editable(view);
                self.stops.push(Stop {
                    node: candidate(editable, own),
                    view: view.clone(),
                    key,
                });
            }
            return;
        }
        if let Some(own) = own
            && crate::input::is_key_stop(view)
        {
            self.stops.push(Stop {
                node: candidate(false, own),
                view: view.clone(),
                key: Retained::from(view.ns()),
            });
        }
        let clip = match own {
            Some(own) if view.state().clips() => intersect(own, clip),
            _ => clip,
        };
        for child in view.children_in_reading_order() {
            self.view(&child, clip, layer, popup);
        }
    }
}

/// The stops of `scope` (a modal, or a window's root) in focus order
/// (`focus::candidates`).
fn stops_in(scope: &NshView) -> Vec<Stop> {
    let mut walk = Walk {
        scope: scope.id(),
        stops: Vec::new(),
        order: 0,
    };
    let window = window_edges(scope);
    let clip = edges(scope).map_or(window, |own| intersect(own, window));
    for child in scope.children_in_reading_order() {
        walk.view(&child, clip, child.id(), false);
    }
    focus::candidates(walk.stops, scope.id(), &layers(scope))
}

/// The children of `scope` as the core's layers, the page first.
fn layers(scope: &NshView) -> Vec<Layer> {
    let window = window_edges(scope);
    scope
        .child_views()
        .iter()
        .filter(|child| child.kind() != ViewKind::Modal)
        .enumerate()
        .map(|(order, child)| {
            let mut layer = Layer::default();
            layer.id = child.id();
            layer.order = order;
            if !left_out(child) {
                layer.frame = edges(child)
                    .map(|e| intersect(e, window))
                    .filter(|&e| shows(e))
                    .map(bounds);
                layer.takes_presses = has_press(child.state().events);
            }
            layer
        })
        .collect()
}

/// `ancestor` is `view` or one of its ancestors.
fn contains(ancestor: &NshView, view: &NshView) -> bool {
    view.ns().isDescendantOf(ancestor.ns())
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

/// `arrow` with `from` having the keyboard (a stop's view, a field's
/// editor or text view; `None`: the root, nothing focused): the focus
/// moves to the stop that way in the focus scope, if any, and that stop
/// comes into view. False when it moved none.
pub(crate) fn arrow(surface: &Rc<Surface>, from: Option<&NSView>, arrow: Arrow) -> bool {
    let Some(scope) = NshView::from_ns(&crate::navigation::focus_scope(surface)) else {
        return false;
    };
    let mut stops = stops_in(&scope);
    // The innermost stop `from` is in (a field's editor is in its field).
    let at = |stops: &[Stop]| {
        from.and_then(|from| stops.iter().rposition(|s| from.isDescendantOf(&s.key)))
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
        Arrow::Left | Arrow::Right => focus::horizontal(&stops, current, arrow == Arrow::Right),
        Arrow::Up | Arrow::Down => focus::vertical(&stops, current, arrow == Arrow::Down),
    };
    let Some(next) = next.filter(|&next| Some(next) != current) else {
        return false;
    };
    let view = &stops[next].view;
    if view.kind() == ViewKind::Input {
        // The field reveals itself as it takes the focus.
        crate::text_input::focus(view);
    } else {
        crate::input::focus_control(surface, view);
        crate::scroll::reveal(surface, view);
    }
    true
}

/// The arrows move the focus only without a modifier:
/// `cargo test -p nsh-platform-macos --lib focus::tests`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_are_the_four_keys_without_modifiers() {
        let none = NSEventModifierFlags::empty();
        assert_eq!(Arrow::of(123, none), Some(Arrow::Left));
        assert_eq!(Arrow::of(124, none), Some(Arrow::Right));
        assert_eq!(Arrow::of(125, none), Some(Arrow::Down));
        assert_eq!(Arrow::of(126, none), Some(Arrow::Up));
        // A real arrow key's event carries these flags of its own.
        let own = NSEventModifierFlags::Function | NSEventModifierFlags::NumericPad;
        assert_eq!(Arrow::of(126, own), Some(Arrow::Up));
        for held in [
            NSEventModifierFlags::Command,
            NSEventModifierFlags::Control,
            NSEventModifierFlags::Option,
            NSEventModifierFlags::Shift,
        ] {
            assert_eq!(
                Arrow::of(123, held | own),
                None,
                "{held:?} edits or scrolls"
            );
        }
        // Return, Tab, Page Down.
        for code in [36, 48, 121] {
            assert_eq!(Arrow::of(code, none), None);
        }
    }

    #[test]
    fn edges_meet_and_intersect() {
        let a = (0.0, 0.0, 10.0, 10.0);
        assert!(shows(intersect(a, (5.0, 5.0, 20.0, 20.0))));
        assert!(
            !shows(intersect(a, (10.0, 0.0, 20.0, 10.0))),
            "edges that meet"
        );
        assert_eq!(
            bounds((1.0, 2.0, 4.0, 8.0)),
            Bounds::new(
                point(Points(1.0), Points(2.0)),
                size(Points(3.0), Points(6.0))
            )
        );
    }
}
