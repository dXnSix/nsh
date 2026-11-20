//! A slot's place given to a nested component as its child
//! (`inset().child(props.header.place(|h| h))`): the nested component's
//! view places the slot's node where it places its children, at mount and
//! when the app fills the slot later, as it does when the place fills a
//! slot of the nested component.

use super::*;
use crate::{
    Prop,
    element::{Component, Registry},
    prelude::*,
};

/// Places its children in its root, which draws nothing itself.
#[derive(Clone, Copy, PartialEq, Props)]
struct InsetProps {
    children: Children,
}

#[component]
fn Inset(props: InsetProps) -> impl IntoElement {
    view().child(props.children)
}

/// Places its children in a view below a title of its own (a
/// disclosure's shape: nodes of its own around the app's content).
#[derive(Clone, Copy, PartialEq, Props)]
struct BoxedProps {
    children: Children,
}

#[component]
fn Boxed(props: BoxedProps) -> impl IntoElement {
    view()
        .child(text("title"))
        .child(view().child(props.children))
}

#[derive(Clone, Copy, PartialEq, Props)]
struct HeaderProps {
    header: crate::element::Prop<Option<Slot>>,
}

/// Its header as the child of a nested inset.
#[component]
fn InsetHeader(props: HeaderProps) -> impl IntoElement {
    view().child(inset().child(props.header.place(|h| h)))
}

/// Its header as the child of a nested component that draws around it.
#[component]
fn BoxedHeader(props: HeaderProps) -> impl IntoElement {
    view().child(boxed().child(props.header.place(|h| h)))
}

/// Its header, which its place styles, between two nodes of its own, all
/// children of a nested inset.
#[component]
fn AmongHeader(props: HeaderProps) -> impl IntoElement {
    view().child(
        inset()
            .child(text("before"))
            .child(props.header.place(|h| h.px_2()))
            .child(text("after")),
    )
}

/// A header slot of its own in one box and its children in another.
#[derive(Clone, Copy, PartialEq, Props)]
struct HeadedProps {
    header: crate::element::Prop<Option<Slot>>,
    children: Children,
}

#[component]
fn Headed(props: HeadedProps) -> impl IntoElement {
    view()
        .child(view().child(props.header.place(|h| h)))
        .child(view().child(props.children))
}

/// Its header as the child of a nested component that has a slot of the
/// same name.
#[component]
fn SameNameHeader(props: HeaderProps) -> impl IntoElement {
    view().child(headed().child(props.header.place(|h| h)))
}

/// Its header, whose presses it counts, as the child of a nested inset.
#[component]
fn PressedHeader(props: HeaderProps) -> impl IntoElement {
    let presses = signal(0);
    view()
        .child(
            inset().child(
                props
                    .header
                    .place(move |h| h.on_press(move || presses.update(|n| *n += 1))),
            ),
        )
        .child(text(move || format!("pressed {}", presses.get())))
}

/// Its header as the child of a nested inset that a press on its root
/// takes out and puts back.
#[component]
fn ToggledHeader(props: HeaderProps) -> impl IntoElement {
    let shown = signal(true);
    view()
        .on_press(move || shown.update(|s| *s = !*s))
        .child(when(
            move || shown.get(),
            move || inset().child(props.header.place(|h| h)),
        ))
}

/// Fills an inset header's slot from Rust: no holder of JS's.
#[component]
fn RustFilled() -> impl IntoElement {
    view().child(inset_header().header(text("head")))
}

/// A host of component `C` at node 2, in the root.
fn mount_host<C: Component>() -> Core<FakeEngine> {
    let mut c = core();
    let mut registry = Registry::new();
    registry.register::<C>();
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
                prop: Prop::Component(std::any::type_name::<C>().into()),
            },
        ],
    );
    c
}

/// Host 2 joins the root.
fn insert_host() -> Vec<Op> {
    vec![Op::Insert {
        parent: ROOT_ID,
        child: 2,
        before: None,
    }]
}

/// Fills host 2's header: holder 4 holding the app's text 6 ("head"), as the
/// JS face does.
fn fill_header() -> Vec<Op> {
    vec![
        Op::Create {
            id: 4,
            tag: Tag::View,
        },
        Op::SetProp {
            id: 4,
            prop: Prop::ComponentSlot("header".into()),
        },
        Op::Create {
            id: 6,
            tag: Tag::Text,
        },
        Op::CreateText {
            id: 8,
            text: "head".into(),
        },
        Op::Insert {
            parent: 6,
            child: 8,
            before: None,
        },
        Op::Insert {
            parent: 4,
            child: 6,
            before: None,
        },
        Op::Insert {
            parent: 2,
            child: 4,
            before: None,
        },
    ]
}

/// Empties host 2's header.
fn empty_header() -> Vec<Op> {
    vec![Op::Remove {
        parent: 2,
        child: 4,
    }]
}

/// The native parent of node `id`, if it is in the tree.
fn native_parent(c: &Core<FakeEngine>, id: NodeId) -> Option<NodeId> {
    let key = c.key(id).ok()?;
    let parent = c.nodes[key as usize].parent;
    (parent != NO_KEY).then(|| c.nodes[parent as usize].id)
}

/// The ids from node `id` up to the root.
fn path(c: &Core<FakeEngine>, id: NodeId) -> Vec<NodeId> {
    let mut path = vec![id];
    while let Some(parent) = native_parent(c, *path.last().unwrap()) {
        path.push(parent);
    }
    path
}

/// The texts `batches` mounted, as they are under node `id`, in order.
fn texts_under(c: &Core<FakeEngine>, id: NodeId, batches: &[MountBatch]) -> Vec<String> {
    fn walk(c: &Core<FakeEngine>, key: Key, batches: &[MountBatch], out: &mut Vec<String>) {
        let node = &c.nodes[key as usize];
        let shown = batches.iter().flat_map(|b| &b.items).find_map(|i| match i {
            MountItem::Text { id, text, .. } if *id == node.id => Some(text.text.clone()),
            _ => None,
        });
        out.extend(shown);
        for &child in &node.children {
            walk(c, child, batches, out);
        }
    }
    let mut out = Vec::new();
    walk(c, c.key(id).unwrap(), batches, &mut out);
    out
}

/// The header is shown in host 2: its holder is in the tree, under the
/// host, `depth` nodes below it.
#[track_caller]
fn assert_header_shown(c: &Core<FakeEngine>, depth: usize) {
    let path = path(c, 4);
    assert_eq!(
        path.last(),
        Some(&ROOT_ID),
        "the header's holder is in the tree: {path:?}"
    );
    assert_eq!(
        path.iter().position(|&id| id == 2),
        Some(depth),
        "under the host: {path:?}"
    );
}

/// The header is not shown: its holder is out of the tree.
#[track_caller]
fn assert_header_out(c: &Core<FakeEngine>) {
    assert_eq!(native_parent(c, 4), None, "{:?}", path(c, 4));
}

/// Mounts `C` with its header filled from the start, then empties and
/// fills it again; the holder is `depth` nodes below the host.
fn filled_at_mount<C: Component>(depth: usize) {
    let mut c = mount_host::<C>();
    apply(&mut c, fill_header());
    apply(&mut c, insert_host());
    c.commit().unwrap();
    assert_header_shown(&c, depth);
    // Nothing more comes: it was placed in the mount's commit.
    assert!(c.commit().is_none());
    apply(&mut c, empty_header());
    c.commit().unwrap();
    assert_header_out(&c);
    apply(
        &mut c,
        vec![Op::Insert {
            parent: 2,
            child: 4,
            before: None,
        }],
    );
    c.commit().unwrap();
    assert_header_shown(&c, depth);
}

/// Mounts `C` with no header, then fills it.
fn filled_later<C: Component>(depth: usize) {
    let mut c = mount_host::<C>();
    apply(&mut c, insert_host());
    c.commit().unwrap();
    apply(&mut c, fill_header());
    c.commit().unwrap();
    assert_header_shown(&c, depth);
    apply(&mut c, empty_header());
    c.commit().unwrap();
    assert_header_out(&c);
}

#[test]
fn a_place_as_a_nested_components_child_shows_at_mount() {
    // Holder, the inset's root, the view's root (the host).
    filled_at_mount::<InsetHeader>(2);
}

#[test]
fn a_place_as_a_nested_components_child_shows_when_the_slot_fills_later() {
    filled_later::<InsetHeader>(2);
}

#[test]
fn a_place_as_the_child_of_a_component_that_draws_around_it_shows_at_mount() {
    // Holder, the box's inner view, the box's root, the host.
    filled_at_mount::<BoxedHeader>(3);
}

#[test]
fn a_place_as_the_child_of_a_component_that_draws_around_it_shows_when_filled_later() {
    filled_later::<BoxedHeader>(3);
}

/// The place keeps its place among the nested component's other children,
/// and its styles reach the holder.
#[test]
fn a_place_among_a_nested_components_children_keeps_its_place() {
    for at_mount in [true, false] {
        let mut c = mount_host::<AmongHeader>();
        let mut batches = Vec::new();
        if at_mount {
            apply(&mut c, fill_header());
            apply(&mut c, insert_host());
        } else {
            apply(&mut c, insert_host());
            batches.push(c.commit().unwrap());
            apply(&mut c, fill_header());
        }
        batches.push(c.commit().unwrap());
        assert_header_shown(&c, 2);
        assert_eq!(
            texts_under(&c, 2, &batches),
            ["before", "head", "after"],
            "at mount: {at_mount}"
        );
        let key = c.key(6).unwrap();
        assert_eq!(
            c.nodes[key as usize].frame.origin.x,
            px(8.0),
            "the place's padding"
        );
    }
}

/// The nested component has a slot of that name itself: the place is
/// still its child, placed where it places its children, and its own slot
/// stays empty.
#[test]
fn a_place_is_the_nested_components_child_whatever_its_slots_are_named() {
    for at_mount in [true, false] {
        let mut c = mount_host::<SameNameHeader>();
        if at_mount {
            apply(&mut c, fill_header());
            apply(&mut c, insert_host());
        } else {
            apply(&mut c, insert_host());
            c.commit().unwrap();
            apply(&mut c, fill_header());
        }
        c.commit().unwrap();
        // Holder, the box of the children, the nested root, the host.
        assert_header_shown(&c, 3);
        let path = path(&c, 4);
        let boxes = &c.nodes[c.key(path[2]).unwrap() as usize].children;
        assert_eq!(
            boxes
                .iter()
                .position(|&k| c.nodes[k as usize].id == path[1]),
            Some(1),
            "in the children's box, at mount: {at_mount}"
        );
        assert!(
            c.nodes[boxes[0] as usize].children.is_empty(),
            "the nested component's own header is empty"
        );
    }
}

/// What the place gives the slot still goes to the app's one element in
/// it: a press there reaches the component.
#[test]
fn a_place_as_a_nested_components_child_keeps_its_listeners() {
    let mut c = mount_host::<PressedHeader>();
    apply(&mut c, fill_header());
    apply(&mut c, insert_host());
    c.commit().unwrap();
    assert_header_shown(&c, 2);
    let events = c.nodes[c.key(6).unwrap() as usize].events;
    assert!(events.contains(EventKind::Press));
    c.component_event(6, EventKind::Press, &EventPayload::None);
    let batch = c.commit().unwrap();
    assert!(
        batch
            .items
            .iter()
            .any(|i| matches!(i, MountItem::Text { text, .. } if text.text == "pressed 1")),
        "{:?}",
        batch.items
    );
}

/// The nested component goes while it holds the place and comes back:
/// the holder is JS's, so it leaves the tree unreleased and is placed
/// again, still the view's slot.
#[test]
fn a_place_follows_a_nested_component_that_goes_and_comes_back() {
    let mut c = mount_host::<ToggledHeader>();
    apply(&mut c, fill_header());
    apply(&mut c, insert_host());
    c.commit().unwrap();
    assert_header_shown(&c, 2);
    for _ in 0..2 {
        assert!(c.component_event(2, EventKind::Press, &EventPayload::None));
        c.commit().unwrap();
        assert_header_out(&c);
        assert!(c.key(6).is_ok(), "the app's element is kept");
        assert!(c.component_event(2, EventKind::Press, &EventPayload::None));
        c.commit().unwrap();
        assert_header_shown(&c, 2);
    }
    // Emptied and filled while the nested component is out: placed when
    // it is back.
    assert!(c.component_event(2, EventKind::Press, &EventPayload::None));
    c.commit().unwrap();
    apply(&mut c, empty_header());
    c.commit();
    apply(
        &mut c,
        vec![Op::Insert {
            parent: 2,
            child: 4,
            before: None,
        }],
    );
    c.commit();
    assert_header_out(&c);
    assert!(c.component_event(2, EventKind::Press, &EventPayload::None));
    c.commit().unwrap();
    assert_header_shown(&c, 2);
}

/// A Rust app fills the slot with nodes of its own view (no holder):
/// placed as the nested component's child the same way.
#[test]
fn a_rust_fill_placed_as_a_nested_components_child_shows_at_mount() {
    let mut c = core();
    c.mount_root(&crate::element::Root::new::<RustFilled>(|| ()));
    let batch = c.commit().unwrap();
    let head = batch
        .items
        .iter()
        .find_map(|i| match i {
            MountItem::Text { id, text, .. } if text.text == "head" => Some(*id),
            _ => None,
        })
        .expect("the fill shows");
    let path = path(&c, head);
    assert_eq!(path.last(), Some(&ROOT_ID), "{path:?}");
    // The text, the inset's root, the header's root, the top's root.
    assert_eq!(path.len(), 5, "{path:?}");
}
