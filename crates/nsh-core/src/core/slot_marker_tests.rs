//! A slot's target through a marker: a component that only places its
//! children, with styles of its own.
//!
//! A component that marks where a slot's content goes hides itself while
//! it is empty (a bound style, so that it takes no space and no gap),
//! places its children one by one to count them, and may draw a box with
//! a name while it is empty. None of that is something the user presses
//! or focuses: holding one element, it is looked through as a plain
//! wrapper is. Props, listeners, a ref, or anything shown beside its one
//! element keep it the target.

use std::cell::{Cell, RefCell};

use super::*;
use crate::element::{Prop, Signal};

thread_local! {
    /// The most children a marker counted.
    static COUNTED: Cell<usize> = const { Cell::new(0) };
    /// The switch of the last `Hinted` made.
    static HINT: RefCell<Option<Signal<bool>>> = const { RefCell::new(None) };
}

/// Its children, as the app passes them.
#[derive(Clone, Copy, PartialEq, Props)]
struct MarkerProps {
    children: Prop<Children>,
}

/// A marker's view: hidden while empty unless `outlined`, which draws a
/// dashed box and a name while it is empty; its children placed one by
/// one and counted (a dev build warns of more than one).
fn marker_view(props: MarkerProps, outlined: bool) -> impl IntoElement {
    let children = props.children;
    let empty = move || children.get().is_empty();
    view()
        .style_when(move || !outlined && empty(), |s| s.hidden())
        .when(outlined, |v| v.p_2().border_1().border_dashed())
        .child(when(move || outlined && empty(), || text("Trigger")))
        .children_each(children.get(), |child, at| {
            effect(move || {
                let count = at.count();
                COUNTED.set(COUNTED.get().max(count));
            });
            child
        })
}

#[component]
fn Marker(props: MarkerProps) -> impl IntoElement {
    marker_view(props, false)
}

#[component]
fn OutlinedMarker(props: MarkerProps) -> impl IntoElement {
    marker_view(props, true)
}

/// Only styles on its root, static: a padded, rounded box.
#[component]
fn Boxlike(props: WrapProps) -> impl IntoElement {
    view().p_2().rounded_md().child(props.children)
}

/// Its children each in a view of its own: a node beside the slot's.
#[component]
fn EachWrapped(props: MarkerProps) -> impl IntoElement {
    view().children_each(props.children.get(), |child, _| view().child(child))
}

/// A hint beside its children while a switch is on.
#[component]
fn Hinted(props: MarkerProps) -> impl IntoElement {
    let on = signal(false);
    HINT.set(Some(on));
    view()
        .child(when(move || on.get(), || text("hint")))
        .child(props.children.get())
}

/// A role on its root.
#[component]
fn WithRole(props: WrapProps) -> impl IntoElement {
    view().role(AccessibilityRole::Group).child(props.children)
}

/// Focusable itself.
#[component]
fn WithFocus(props: WrapProps) -> impl IntoElement {
    view().focusable(true).child(props.children)
}

/// A ref to its root.
#[component]
fn WithRef(props: WrapProps) -> impl IntoElement {
    let at = element_ref();
    view().element_ref(&at).child(props.children)
}

/// A bound prop on its root (off now).
#[component]
fn WithBoundProp(props: WrapProps) -> impl IntoElement {
    let off = signal(false);
    view().disabled(move || off.get()).child(props.children)
}

/// The slot's component at node 2 with every marker registered.
fn mount_markers() -> Core<FakeEngine> {
    let mut c = core();
    let mut registry = Registry::new();
    registry
        .register::<Pressable>()
        .register::<Marker>()
        .register::<OutlinedMarker>()
        .register::<Boxlike>()
        .register::<EachWrapped>()
        .register::<Hinted>()
        .register::<WithRole>()
        .register::<WithFocus>()
        .register::<WithRef>()
        .register::<WithBoundProp>();
    c.set_components(registry);
    let mut ops = host::<Pressable>(2);
    ops.push(insert(ROOT_ID, 2));
    apply(&mut c, ops);
    c
}

/// Whether text `wanted` is shown now.
fn shows(c: &Core<FakeEngine>, wanted: &str) -> bool {
    c.nodes
        .iter()
        .any(|(_, n)| n.kind == Kind::RawText && n.text.as_str() == wanted)
        || c.runs
            .values()
            .any(|r| r.block != NO_KEY && r.text.as_str() == wanted)
}

/// A marker hidden while empty, placing its children one by one, is
/// looked through while it holds one element, as a plain wrapper is,
/// hidden or outlined while empty; and so is a root with only static
/// styles.
#[test]
fn a_marker_holding_one_element_is_looked_through() {
    fn check<C: Component>() {
        let mut c = mount_markers();
        apply(&mut c, fill_wrapped::<C>());
        settle(&mut c);
        assert_target(&c, 8, &[6, 4]);
        assert_pressed(&mut c, 8, 1);
        assert_pressed(&mut c, 8, 2);
    }
    check::<Marker>();
    check::<OutlinedMarker>();
    check::<Boxlike>();
}

/// Empty, a marker is the target (hidden, or outlined with its name); given one element, that element, which it counted; empty
/// again, the marker; given one again, that one.
#[test]
fn the_target_follows_what_a_marker_holds() {
    fn check<C: Component>(outlined: bool) {
        COUNTED.set(0);
        let mut c = mount_markers();
        let mut ops = holder();
        ops.extend(host::<C>(6));
        ops.extend([insert(4, 6), insert(2, 4)]);
        apply(&mut c, ops);
        settle(&mut c);
        assert_target(&c, 6, &[4]);
        assert_eq!(shows(&c, "Trigger"), outlined);
        let mut ops = plain(8);
        ops.push(insert(6, 8));
        apply(&mut c, ops);
        settle(&mut c);
        assert!(!shows(&c, "Trigger"));
        assert_eq!(COUNTED.get(), 1);
        assert_target(&c, 8, &[6, 4]);
        assert_pressed(&mut c, 8, 1);
        apply(
            &mut c,
            vec![
                Op::Remove {
                    parent: 6,
                    child: 8,
                },
                Op::Release { id: 8 },
            ],
        );
        settle(&mut c);
        assert_eq!(shows(&c, "Trigger"), outlined);
        assert_target(&c, 6, &[4]);
        assert_pressed(&mut c, 6, 2);
        let mut ops = plain(10);
        ops.push(insert(6, 10));
        apply(&mut c, ops);
        settle(&mut c);
        assert!(!shows(&c, "Trigger"));
        assert_target(&c, 10, &[6, 4]);
        assert_pressed(&mut c, 10, 3);
    }
    check::<Marker>(false);
    check::<OutlinedMarker>(true);
}

/// Content a region shows beside the one element makes the component the
/// target while it is shown, and the element again when it goes.
#[test]
fn content_shown_beside_the_element_makes_the_component_the_target() {
    let mut c = mount_markers();
    apply(&mut c, fill_wrapped::<Hinted>());
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    let hint = HINT.with_borrow(|h| h.expect("made"));
    {
        let _entered = c.components.runtime.enter();
        hint.set(true);
    }
    settle(&mut c);
    assert!(shows(&c, "hint"));
    assert_target(&c, 6, &[8, 4]);
    assert_pressed(&mut c, 6, 1);
    {
        let _entered = c.components.runtime.enter();
        hint.set(false);
    }
    settle(&mut c);
    assert!(!shows(&c, "hint"));
    assert_target(&c, 8, &[6, 4]);
    assert_pressed(&mut c, 8, 2);
}

/// A component with more than styles on its root, or a node of its own
/// around each child, stays the target, as before: a role, the focus, a
/// ref, a bound prop.
#[test]
fn a_root_with_more_than_styles_stays_the_target() {
    fn check<C: Component>() {
        let mut c = mount_markers();
        apply(&mut c, fill_wrapped::<C>());
        settle(&mut c);
        assert!(hears_press(&c, 6), "{}", std::any::type_name::<C>());
        assert!(!hears_press(&c, 8), "{}", std::any::type_name::<C>());
        assert!(!c.components.adopted.contains_key(&8));
        assert_pressed(&mut c, 6, 1);
    }
    check::<EachWrapped>();
    check::<WithRole>();
    check::<WithFocus>();
    check::<WithRef>();
    check::<WithBoundProp>();
}
