//! A slot's target through a component that only places its children.
//!
//! What a slot's place gives besides styles goes to the app's one element
//! in the slot's holder (`slots.rs`). When that element is the host of a
//! component whose view is a plain view around its children and nothing
//! else, and the app gave it exactly one child, the target is that child:
//! the wrapper draws and hears nothing, so the app's element inside it is
//! what the user presses and focuses. A component with anything of its own
//! on its root stays the target, as before.

use super::*;
use crate::{
    AccessibilityRole, Prop,
    element::{Component, Registry},
    prelude::*,
};

/// Only places its children.
#[derive(Clone, Copy, PartialEq, Props)]
struct WrapProps {
    children: Children,
}

#[component]
fn Wrap(props: WrapProps) -> impl IntoElement {
    view().child(props.children)
}

/// Places its children in a root that listens.
#[component]
fn Hearing(props: WrapProps) -> impl IntoElement {
    view()
        .on_mouse_enter(|_: &crate::element::MouseEvent| {})
        .child(props.children)
}

/// Places its children beside a node of its own.
#[component]
fn Labelled(props: WrapProps) -> impl IntoElement {
    view().child(text("label")).child(props.children)
}

/// Places its children in a view inside its root.
#[component]
fn Boxed(props: WrapProps) -> impl IntoElement {
    view().child(view().child(props.children))
}

#[derive(Clone, Copy, PartialEq, Props)]
struct TriggerProps {
    trigger: crate::element::Prop<Option<Slot>>,
}

/// Makes its trigger a button and counts its presses.
#[component]
fn Pressable(props: TriggerProps) -> impl IntoElement {
    let presses = signal(0);
    view()
        .child(props.trigger.place(move |t| {
            t.role(AccessibilityRole::Button)
                .focusable(true)
                .on_press(move || presses.update(|n| *n += 1))
        }))
        .child(text(move || format!("pressed {}", presses.get())))
}

/// The slot's component at node 2 with every wrapper registered.
fn mount() -> Core<FakeEngine> {
    let mut c = core();
    let mut registry = Registry::new();
    registry
        .register::<Pressable>()
        .register::<Wrap>()
        .register::<Hearing>()
        .register::<Labelled>()
        .register::<Boxed>();
    c.set_components(registry);
    apply(
        &mut c,
        vec![
            Op::Create {
                id: 2,
                tag: Tag::View,
            },
            Op::SetProp {
                id: 2,
                prop: Prop::Component(std::any::type_name::<Pressable>().into()),
            },
            Op::Insert {
                parent: ROOT_ID,
                child: 2,
                before: None,
            },
        ],
    );
    c
}

/// View `id`, the host of component `C`.
fn host<C: Component>(id: NodeId) -> Vec<Op> {
    vec![
        Op::Create { id, tag: Tag::View },
        Op::SetProp {
            id,
            prop: Prop::Component(std::any::type_name::<C>().into()),
        },
    ]
}

/// View `id`.
fn plain(id: NodeId) -> Vec<Op> {
    vec![Op::Create { id, tag: Tag::View }]
}

/// `child` at the end of `parent`.
fn insert(parent: NodeId, child: NodeId) -> Op {
    Op::Insert {
        parent,
        child,
        before: None,
    }
}

/// Host 2's trigger: holder 4, as the JS face makes it.
fn holder() -> Vec<Op> {
    vec![
        Op::Create {
            id: 4,
            tag: Tag::View,
        },
        Op::SetProp {
            id: 4,
            prop: Prop::ComponentSlot("trigger".into()),
        },
    ]
}

/// Fills host 2's trigger with wrapper 6 (a `C`) around the app's view 8,
/// the content first, as the JS face does.
fn fill_wrapped<C: Component>() -> Vec<Op> {
    let mut ops = holder();
    ops.extend(host::<C>(6));
    ops.extend(plain(8));
    ops.extend([insert(6, 8), insert(4, 6), insert(2, 4)]);
    ops
}

fn role(c: &Core<FakeEngine>, id: NodeId) -> AccessibilityRole {
    let key = c.key(id).unwrap();
    c.nodes[key as usize]
        .props
        .as_deref()
        .map_or(AccessibilityRole::None, |p| p.accessibility.role)
}

fn focusable(c: &Core<FakeEngine>, id: NodeId) -> Option<bool> {
    let key = c.key(id).unwrap();
    c.nodes[key as usize]
        .props
        .as_deref()
        .and_then(|p| p.focusable)
}

fn hears_press(c: &Core<FakeEngine>, id: NodeId) -> bool {
    c.nodes[c.key(id).unwrap() as usize]
        .events
        .contains(EventKind::Press)
}

/// Commits until nothing comes; the last text starting with `pressed`.
fn settle(c: &mut Core<FakeEngine>) -> Option<String> {
    let mut pressed = None;
    for _ in 0..8 {
        for (id, kind, payload) in js_events(c) {
            c.component_event(id, kind, &payload);
        }
        let Some(batch) = c.commit() else { break };
        for item in &batch.items {
            if let MountItem::Text { text, .. } = item
                && text.text.starts_with("pressed")
            {
                pressed = Some(text.text.clone());
            }
        }
    }
    pressed
}

/// The slot's part is on node `target` and on none of `others`.
#[track_caller]
fn assert_target(c: &Core<FakeEngine>, target: NodeId, others: &[NodeId]) {
    assert_eq!(role(c, target), AccessibilityRole::Button, "{target}");
    assert_eq!(focusable(c, target), Some(true), "{target}");
    assert!(hears_press(c, target), "{target}");
    for &other in others {
        assert_eq!(role(c, other), AccessibilityRole::None, "{other}");
        assert_eq!(focusable(c, other), None, "{other}");
        assert!(!hears_press(c, other), "{other}");
    }
}

/// A press on `id` reaches the slot's component (its count is then
/// `count`).
#[track_caller]
fn assert_pressed(c: &mut Core<FakeEngine>, id: NodeId, count: u32) {
    c.component_event(id, EventKind::Press, &EventPayload::None);
    assert_eq!(settle(c), Some(format!("pressed {count}")));
}

/// The app's one element in a wrapper that only places its children is
/// the slot's target: it has the role and the focus, and its presses are
/// the component's. The wrapper's host and the holder have none of them.
#[test]
fn a_slots_target_is_the_apps_element_in_a_wrapper_that_only_places_it() {
    let mut c = mount();
    apply(&mut c, fill_wrapped::<Wrap>());
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    assert_pressed(&mut c, 8, 1);
    assert_pressed(&mut c, 8, 2);
}

/// The same when the app fills the slot after the component mounted, and
/// when the wrapper gets its child after it joined the holder.
#[test]
fn the_target_follows_a_wrapper_filled_later() {
    let mut c = mount();
    settle(&mut c);
    apply(&mut c, fill_wrapped::<Wrap>());
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    assert_pressed(&mut c, 8, 1);

    let mut c = mount();
    let mut ops = holder();
    ops.extend(host::<Wrap>(6));
    ops.extend([insert(4, 6), insert(2, 4)]);
    apply(&mut c, ops);
    settle(&mut c);
    // An empty wrapper is the holder's one element: the target.
    assert_target(&c, 6, &[4]);
    let mut ops = plain(8);
    ops.push(insert(6, 8));
    apply(&mut c, ops);
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    assert_pressed(&mut c, 8, 1);
}

/// With a second child the wrapper is the target, as any component's
/// host; with one again, that child.
#[test]
fn the_target_is_the_wrapper_while_it_holds_several_children() {
    let mut c = mount();
    apply(&mut c, fill_wrapped::<Wrap>());
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    let mut ops = plain(10);
    ops.push(insert(6, 10));
    apply(&mut c, ops);
    settle(&mut c);
    assert_target(&c, 6, &[8, 10, 4]);
    assert_pressed(&mut c, 6, 1);
    // The first leaves: the one left is the target.
    apply(
        &mut c,
        vec![Op::Remove {
            parent: 6,
            child: 8,
        }],
    );
    settle(&mut c);
    assert_target(&c, 10, &[6, 4]);
    assert_pressed(&mut c, 10, 2);
    // None left: the wrapper.
    apply(
        &mut c,
        vec![
            Op::Remove {
                parent: 6,
                child: 10,
            },
            Op::Release { id: 10 },
        ],
    );
    settle(&mut c);
    assert_target(&c, 6, &[4]);
}

/// Wrappers nest: the target is the element in the innermost.
#[test]
fn the_target_is_found_through_nested_wrappers() {
    let mut c = mount();
    let mut ops = holder();
    ops.extend(host::<Wrap>(6));
    ops.extend(host::<Wrap>(8));
    ops.extend(plain(10));
    ops.extend([insert(8, 10), insert(6, 8), insert(4, 6), insert(2, 4)]);
    apply(&mut c, ops);
    settle(&mut c);
    assert_target(&c, 10, &[8, 6, 4]);
    assert_pressed(&mut c, 10, 1);
}

/// JS's own props and listeners on the element combine with the slot's,
/// and come back when the element leaves the wrapper.
#[test]
fn the_apps_own_props_on_the_element_combine_and_come_back() {
    let mut c = mount();
    let mut ops = fill_wrapped::<Wrap>();
    ops.extend([
        Op::SetProp {
            id: 8,
            prop: Prop::Accessibility(crate::AccessibilityProp::Label(Some("Open".into()))),
        },
        Op::Listen {
            id: 8,
            event: EventKind::Press,
            enabled: true,
        },
    ]);
    apply(&mut c, ops);
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    let label = |c: &Core<FakeEngine>| {
        c.nodes[c.key(8).unwrap() as usize]
            .props
            .as_deref()
            .and_then(|p| p.accessibility.label.clone())
    };
    assert_eq!(label(&c).as_deref(), Some("Open"));
    apply(
        &mut c,
        vec![Op::Remove {
            parent: 6,
            child: 8,
        }],
    );
    settle(&mut c);
    assert_eq!(role(&c, 8), AccessibilityRole::None);
    assert_eq!(focusable(&c, 8), None);
    assert_eq!(label(&c).as_deref(), Some("Open"));
    assert!(hears_press(&c, 8), "JS's own listener stays");
    assert_target(&c, 6, &[4]);
}

/// A component with anything of its own but styles is the target itself,
/// as before: a listener or a node of its own on or in its root, or its
/// children placed deeper than its root. (A root with styles alone is
/// looked through: `markers`.)
#[test]
fn a_component_with_anything_of_its_own_stays_the_target() {
    fn check<C: Component>() {
        let mut c = mount();
        apply(&mut c, fill_wrapped::<C>());
        settle(&mut c);
        assert_target(&c, 6, &[8, 4]);
        assert_pressed(&mut c, 6, 1);
    }
    check::<Hearing>();
    check::<Labelled>();
    check::<Boxed>();
}

/// The app's element directly in the holder, with no wrapper: unchanged.
#[test]
fn an_element_directly_in_the_holder_is_the_target_as_before() {
    let mut c = mount();
    let mut ops = holder();
    ops.extend(plain(8));
    ops.extend([insert(4, 8), insert(2, 4)]);
    apply(&mut c, ops);
    settle(&mut c);
    assert_target(&c, 8, &[4]);
    assert_pressed(&mut c, 8, 1);
}

/// The wrapper leaves the holder, or the app releases the element: the
/// element has JS's own props back, and the holder takes the slot's.
#[test]
fn the_target_returns_to_the_holder_when_the_wrapper_or_the_element_goes() {
    let mut c = mount();
    apply(&mut c, fill_wrapped::<Wrap>());
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    apply(
        &mut c,
        vec![Op::Remove {
            parent: 4,
            child: 6,
        }],
    );
    settle(&mut c);
    // The holder is empty: the slot is.
    assert_eq!(role(&c, 8), AccessibilityRole::None);
    assert!(!hears_press(&c, 8));
    assert_eq!(role(&c, 6), AccessibilityRole::None);
    // Back in: through the wrapper again.
    apply(&mut c, vec![insert(4, 6)]);
    settle(&mut c);
    assert_target(&c, 8, &[6, 4]);
    assert_pressed(&mut c, 8, 1);
    // The element is released as JS releases a view: out of its parent,
    // then gone. The wrapper is the one element left.
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
    assert!(c.key(8).is_err());
    assert_target(&c, 6, &[4]);
    assert_pressed(&mut c, 6, 2);
    // The wrapper and the holder go with their component.
    apply(
        &mut c,
        vec![
            Op::Remove {
                parent: 4,
                child: 6,
            },
            Op::Release { id: 6 },
            Op::Remove {
                parent: 2,
                child: 4,
            },
            Op::Release { id: 4 },
            Op::Remove {
                parent: ROOT_ID,
                child: 2,
            },
            Op::Release { id: 2 },
        ],
    );
    settle(&mut c);
    assert!(c.components.adopted.is_empty());
}

/// Gives its trigger a ref and, once the trigger is pressed, shows a
/// 40 × 20 overlay 4 points below what the trigger draws, centered on it
/// (a popover's).
#[component]
fn Anchored(props: TriggerProps) -> impl IntoElement {
    let anchor = element_ref();
    let open = signal(false);
    view()
        .child(
            props
                .trigger
                .place(move |t| t.element_ref(&anchor).on_press(move || open.set(true))),
        )
        .child(when(
            move || open.get(),
            move || {
                overlay()
                    .anchor_content(&anchor)
                    .placement(crate::element::Placement::Bottom)
                    .align(Align::Center)
                    .offset(px(4.0))
                    .w(px(40.0))
                    .h(px(20.0))
                    .child(text("pop"))
            },
        ))
}

/// An overlay anchored to what the slot's element draws goes by the
/// app's element in the wrapper: its own box (100 × 30, 10 points in),
/// not the wrapper's, which the app pads here, so that it is a box of its
/// own that the component's column stretches to 320 points.
#[test]
fn an_overlay_anchored_to_the_slots_element_goes_by_the_element_in_the_wrapper() {
    let mut c = core();
    let mut registry = Registry::new();
    registry.register::<Anchored>().register::<Wrap>();
    c.set_components(registry);
    let mut ops = vec![
        Op::Create {
            id: 2,
            tag: Tag::View,
        },
        Op::SetProp {
            id: 2,
            prop: Prop::Component(std::any::type_name::<Anchored>().into()),
        },
        insert(ROOT_ID, 2),
    ];
    ops.extend(holder());
    ops.extend(host::<Wrap>(6));
    ops.extend(plain(8));
    ops.push(class_op(6, "px-[10px]"));
    ops.push(class_op(8, "self-start w-[100px] h-[30px]"));
    ops.extend([insert(6, 8), insert(4, 6), insert(2, 4)]);
    apply(&mut c, ops);
    settle(&mut c);
    assert_eq!(c.components.adopted.get(&8), Some(&4));
    let wrapper = c.measure(6).expect("laid out");
    assert_eq!(wrapper.frame.size, size_pt(320.0, 30.0));
    c.component_event(8, EventKind::Press, &EventPayload::None);
    let batch = c.commit().unwrap();
    let pop = batch
        .items
        .iter()
        .find_map(|i| match i {
            MountItem::Text { id, text, .. } if text.text == "pop" => Some(*id),
            _ => None,
        })
        .expect("shown");
    let parent = c.nodes[c.key(pop).unwrap() as usize].parent;
    let pop = c.nodes[parent as usize].id;
    // Centered below the element (10, 0, 100 × 30): not at (140, 34),
    // below the wrapper.
    assert_eq!(frame_of(&batch, pop), Some(rect(40.0, 34.0, 40.0, 20.0)));
}

/// A Rust app passes the wrapper as the slot's fill: the node the view
/// wraps the fill in is the holder, and the element in the wrapper the
/// target, as a TSX app's.
#[component]
fn RustTop() -> impl IntoElement {
    view().child(pressable().trigger(wrap().child(view().child(text("inner")))))
}

#[test]
fn a_rust_apps_element_in_a_wrapper_is_the_target() {
    let mut c = core();
    c.mount_root(&crate::element::Root::new::<RustTop>(|| ()));
    let batch = c.commit().unwrap();
    let inner = batch
        .items
        .iter()
        .find_map(|i| match i {
            MountItem::Text { id, text, .. } if text.text == "inner" => Some(*id),
            _ => None,
        })
        .expect("shown");
    let parent = |c: &Core<FakeEngine>, id: NodeId| {
        let key = c.nodes[c.key(id).unwrap() as usize].parent;
        c.nodes[key as usize].id
    };
    let element = parent(&c, inner);
    let wrapper = parent(&c, element);
    let holder = parent(&c, wrapper);
    settle(&mut c);
    assert_target(&c, element, &[wrapper, holder]);
    assert!(c.component_event(element, EventKind::Press, &EventPayload::None));
    assert_eq!(settle(&mut c).as_deref(), Some("pressed 1"));
}

#[path = "slot_marker_tests.rs"]
mod markers;
