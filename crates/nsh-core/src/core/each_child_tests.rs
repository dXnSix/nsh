//! `children_each` placing each JS child as it is, with no wrapper of the
//! view's (`.children_each(children, |child, _| child)`): every child is
//! in the host, in the app's order, as the app adds, moves and removes
//! them. A row the children's region keeps is found by its key, as a
//! wrapped one is.

use super::*;
use crate::{Prop, element::Registry, prelude::*};

/// Its children, as the app passes them.
#[derive(Clone, Copy, PartialEq, Props)]
struct BareProps {
    children: Children,
}

/// Places each of its children as it is.
#[component]
fn Bare(props: BareProps) -> impl IntoElement {
    view().children_each(props.children, |child, _| child)
}

/// The ids of node `id`'s children, in order.
fn children_of(c: &Core<FakeEngine>, id: NodeId) -> Vec<NodeId> {
    let key = c.key(id).unwrap();
    c.nodes[key as usize]
        .children
        .iter()
        .map(|&k| c.nodes[k as usize].id)
        .collect()
}

fn settle(c: &mut Core<FakeEngine>) {
    for _ in 0..8 {
        if c.commit().is_none() {
            break;
        }
    }
}

#[test]
fn bare_children_stay_in_the_host_as_the_app_adds_moves_and_removes_them() {
    let mut c = core();
    let mut registry = Registry::new();
    registry.register::<Bare>();
    c.set_components(registry);
    let insert = |child, before| Op::Insert {
        parent: 2,
        child,
        before,
    };
    apply(
        &mut c,
        vec![
            Op::Create {
                id: 2,
                tag: Tag::View,
            },
            Op::SetProp {
                id: 2,
                prop: Prop::Component(std::any::type_name::<Bare>().into()),
            },
            Op::Create {
                id: 4,
                tag: Tag::View,
            },
            insert(4, None),
            Op::Insert {
                parent: ROOT_ID,
                child: 2,
                before: None,
            },
        ],
    );
    settle(&mut c);
    assert_eq!(children_of(&c, 2), [4]);
    // A second: the first stays.
    apply(
        &mut c,
        vec![
            Op::Create {
                id: 6,
                tag: Tag::View,
            },
            insert(6, None),
        ],
    );
    settle(&mut c);
    assert_eq!(children_of(&c, 2), [4, 6]);
    // A third, first.
    apply(
        &mut c,
        vec![
            Op::Create {
                id: 8,
                tag: Tag::View,
            },
            insert(8, Some(4)),
        ],
    );
    settle(&mut c);
    assert_eq!(children_of(&c, 2), [8, 4, 6]);
    // Moved to the end.
    apply(&mut c, vec![insert(8, None)]);
    settle(&mut c);
    assert_eq!(children_of(&c, 2), [4, 6, 8]);
    // The first goes: the others stay.
    apply(
        &mut c,
        vec![
            Op::Remove {
                parent: 2,
                child: 4,
            },
            Op::Release { id: 4 },
        ],
    );
    settle(&mut c);
    assert_eq!(children_of(&c, 2), [6, 8]);
}
