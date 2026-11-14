//! The core's answers to dev tools ([`crate::inspect`], feature
//! `inspect`): the tree, a node's detail and the nodes under a point, read
//! from the nodes and the last layout. Nothing here runs until a tool
//! asks.

use super::{Core, Key, Kind, NO_KEY, Node, measure::valid};
use crate::{
    NodeId,
    geometry::{Point, Points, point, size_pt},
    inspect::{
        DevBox, DevDetail, DevEdges, DevFlags, DevList, DevNode, DevScroll, DevState, DevTree,
        Request,
    },
    text::TextEngine,
};

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::View => "view",
        Kind::Text => "text",
        Kind::RawText => "raw-text",
        Kind::Image => "image",
        Kind::Scroll => "scroll",
        Kind::List => "list",
        Kind::Cell => "cell",
        Kind::Input => "input",
        Kind::Stack => "stack",
        Kind::Screen => "screen",
        Kind::Modal => "modal",
        Kind::HeaderItem => "header-item",
        Kind::InputAccessory => "input-accessory",
        Kind::Canvas => "canvas",
        Kind::WebView => "webview",
        Kind::Video => "video",
        Kind::Native => "native",
        Kind::Overlay => "overlay",
        Kind::Shader => "shader",
    }
}

fn edges(rect: taffy::Rect<f32>) -> DevEdges {
    DevEdges {
        top: Points(rect.top),
        right: Points(rect.right),
        bottom: Points(rect.bottom),
        left: Points(rect.left),
    }
}

/// A root view of its own: positions in it start at its corner.
fn is_root_view(node: &Node) -> bool {
    matches!(
        node.kind,
        Kind::Modal | Kind::HeaderItem | Kind::InputAccessory
    ) || node.is_overlay()
}

impl<M: TextEngine> Core<M> {
    /// Queues a tool's question for the next commit (the first one makes
    /// the dev state).
    pub(super) fn queue_inspect(&mut self, request: Request) {
        self.dev
            .get_or_insert_with(Box::<DevState>::default)
            .requests
            .push(request);
    }

    /// Answers the tools' questions with this commit's layout.
    pub(super) fn answer_inspections(&mut self) {
        let Some(requests) = self
            .dev
            .as_mut()
            .filter(|dev| !dev.requests.is_empty())
            .map(|dev| std::mem::take(&mut dev.requests))
        else {
            return;
        };
        for request in requests {
            match request {
                Request::Tree(answer) => answer.borrow_mut().deliver(self.dev_tree()),
                Request::Node(id, answer) => answer.borrow_mut().deliver(self.dev_node(id)),
                Request::Pick(page, skip, answer) => {
                    answer.borrow_mut().deliver(self.dev_pick(page, skip))
                }
            }
        }
    }

    /// Whether a tool ever asked this core anything (it then has dev
    /// state).
    pub fn inspected(&self) -> bool {
        self.dev.is_some()
    }

    /// The tree as the last layout left it: every node from the root, in
    /// tree order.
    pub fn dev_tree(&self) -> DevTree {
        let mut nodes = Vec::new();
        let mut stack = vec![self.root];
        while let Some(key) = stack.pop() {
            let node = &self.nodes[key as usize];
            nodes.push(self.dev_node_light(key));
            stack.extend(node.children.iter().rev().copied());
        }
        DevTree {
            root: self.nodes[self.root as usize].id,
            nodes,
        }
    }

    fn dev_node_light(&self, key: Key) -> DevNode {
        let node = &self.nodes[key as usize];
        let parent = (node.parent != NO_KEY).then(|| self.nodes[node.parent as usize].id);
        DevNode {
            id: node.id,
            kind: kind_name(node.kind),
            parent,
            children: node
                .children
                .iter()
                .map(|&c| self.nodes[c as usize].id)
                .collect(),
            component: node.component.as_deref().and_then(|c| c.name.clone()),
            class: node
                .class_names
                .as_deref()
                .map(super::classes::ClassNames::authored)
                .filter(|class| !class.is_empty()),
            test_id: node.props.as_deref().and_then(|p| p.test_id.clone()),
            text: (node.kind == Kind::RawText && !node.text.is_empty()).then(|| node.text.clone()),
            frame: node.frame,
            page: self.measure(node.id).map(|m| m.page),
            flags: DevFlags {
                culled: node.culled,
                spare: node.spare,
                placeholder: node.placeholder,
                hidden: node.view_style.hidden || node.hidden_in_text,
                overlay: node.is_overlay(),
                overridden: false,
            },
        }
    }

    /// Node `id` in full, with the last layout; `None` for an unknown id.
    pub fn dev_node(&self, id: NodeId) -> Option<DevDetail> {
        let key = self.key(id).ok()?;
        let node = &self.nodes[key as usize];
        let layout = match node.layout {
            Some(layout) => {
                let l = self.layout.unrounded_layout(layout);
                DevBox {
                    location: point(Points(l.location.x), Points(l.location.y)),
                    size: size_pt(l.size.width, l.size.height),
                    padding: edges(l.padding),
                    border: edges(l.border),
                    margin: edges(l.margin),
                }
            }
            None => DevBox::default(),
        };
        Some(DevDetail {
            id,
            inline_style: node.inline_style.declarations().to_vec(),
            view_style: node.view_style.clone(),
            text_style: node.text_style,
            layout,
            states: node.states,
            events: node.events,
            accessibility: node.props.as_deref().map(|p| p.accessibility.clone()),
            scroll: node.scroll.as_deref().map(|s| DevScroll {
                offset: point(Points(s.offset.0), Points(s.offset.1)),
                content: s.content,
            }),
            list: node.list.as_deref().map(|l| DevList {
                count: l.count,
                columns: l.columns,
            }),
            cell: node.slot,
            override_style: None,
        })
    }

    /// The nodes under page point `page`, innermost and topmost first:
    /// overlays above the tree, a later sibling (by `zIndex`) above an
    /// earlier one, children before their parent. `pointerEvents`,
    /// listeners and the disabled state do not count; a view that clips
    /// (`overflow: hidden`, scroll views, lists, stacks) leaves out what
    /// lies outside it, and hidden views are left out. The subtree of
    /// `skip` (a tool's own layer) is left out.
    pub fn dev_pick(&self, page: Point<Points>, skip: Option<NodeId>) -> Vec<NodeId> {
        let mut out = Vec::new();
        // Root views of their own above the tree, the last one topmost.
        let mut layers = Vec::new();
        let mut stack = vec![self.root];
        while let Some(key) = stack.pop() {
            let node = &self.nodes[key as usize];
            if key != self.root && is_root_view(node) {
                layers.push(key);
            }
            stack.extend(node.children.iter().rev().copied());
        }
        for &layer in layers.iter().rev() {
            self.pick_in(layer, point(Points(0.0), Points(0.0)), page, skip, &mut out);
        }
        self.pick_in(
            self.root,
            point(Points(0.0), Points(0.0)),
            page,
            skip,
            &mut out,
        );
        out
    }

    /// Adds the nodes under `page` in the subtree of `key`, whose parent's
    /// content starts at page point `origin`.
    fn pick_in(
        &self,
        key: Key,
        origin: Point<Points>,
        page: Point<Points>,
        skip: Option<NodeId>,
        out: &mut Vec<NodeId>,
    ) {
        let node = &self.nodes[key as usize];
        // The root view is the surface: everything is in it.
        let root = key == self.root;
        if Some(node.id) == skip
            || node.inline
            || (!root && (node.layout.is_none() || !valid(&node.frame)))
            || node.view_style.hidden
            || node.hidden_in_text
        {
            return;
        }
        // A root view of its own is placed in its own coordinates.
        let at = if key != self.root && is_root_view(node) {
            node.frame.origin
        } else {
            point(
                origin.x + node.frame.origin.x,
                origin.y + node.frame.origin.y,
            )
        };
        let at = if root { origin } else { at };
        let size = node.frame.size;
        let inside = root
            || (page.x >= at.x
                && page.y >= at.y
                && page.x < at.x + size.width
                && page.y < at.y + size.height);
        let clips = !root
            && (node.view_style.overflow_hidden
                || matches!(node.kind, Kind::Scroll | Kind::List | Kind::Stack));
        if !inside && clips {
            return;
        }
        // Children in a scroll view's content: where the content is now.
        let content = match node.scroll.as_deref() {
            Some(scroll) => point(
                Points(at.x.0 - scroll.offset.0),
                Points(at.y.0 - scroll.offset.1),
            ),
            None => at,
        };
        let mut children: Vec<Key> = node
            .children
            .iter()
            .copied()
            .filter(|&c| !is_root_view(&self.nodes[c as usize]))
            .collect();
        children.sort_by_key(|&c| self.nodes[c as usize].z_index);
        for &child in children.iter().rev() {
            self.pick_in(child, content, page, skip, out);
        }
        if inside {
            out.push(node.id);
        }
    }
}

#[cfg(test)]
mod tree_tests {
    use super::super::tests::{apply, core, style_op};
    use crate::{
        EventKind, Op, Prop, ROOT_ID, StyleValue, Tag,
        element::Answer,
        geometry::{Points, point},
        inspect::{DevTree, Request},
    };

    fn view(id: NodeId, parent: NodeId) -> Vec<Op> {
        vec![
            Op::Create { id, tag: Tag::View },
            Op::Insert {
                parent,
                child: id,
                before: None,
            },
        ]
    }

    use crate::NodeId;

    /// The value of an answer the commit resolved.
    fn ready<T>(answer: Answer<T>) -> T {
        use std::future::Future;
        let mut answer = std::pin::pin!(answer);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match answer.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("not answered"),
        }
    }

    fn sample() -> crate::core::Core<super::super::tests::FakeEngine> {
        let mut c = core();
        let mut ops = view(2, ROOT_ID);
        ops.push(style_op(
            2,
            &[
                ("padding", StyleValue::Number(4.0)),
                ("width", StyleValue::Number(100.0)),
                ("height", StyleValue::Number(80.0)),
            ],
        ));
        ops.push(Op::SetProp {
            id: 2,
            prop: Prop::Class("rounded-md".into()),
        });
        ops.push(Op::SetProp {
            id: 2,
            prop: Prop::TestId(Some("card".into())),
        });
        ops.push(Op::Listen {
            id: 2,
            event: EventKind::Press,
            enabled: true,
        });
        ops.extend([
            Op::Create {
                id: 4,
                tag: Tag::Text,
            },
            Op::Insert {
                parent: 2,
                child: 4,
                before: None,
            },
            Op::CreateText {
                id: 6,
                text: "Hello".into(),
            },
            Op::Insert {
                parent: 4,
                child: 6,
                before: None,
            },
        ]);
        apply(&mut c, ops);
        c.commit();
        c
    }

    #[test]
    fn the_tree_has_every_node_with_its_place_and_names() {
        let c = sample();
        let tree = c.dev_tree();
        assert_eq!(tree.root, ROOT_ID);
        let ids: Vec<NodeId> = tree.nodes.iter().map(|n| n.id).collect();
        assert_eq!(ids, [ROOT_ID, 2, 4, 6]);
        let card = &tree.nodes[1];
        assert_eq!(card.kind, "view");
        assert_eq!(card.parent, Some(ROOT_ID));
        assert_eq!(card.children, [4]);
        assert_eq!(card.class.as_deref(), Some("rounded-md"));
        assert_eq!(card.test_id.as_deref(), Some("card"));
        assert_eq!(card.frame.size.width, Points(100.0));
        assert_eq!(card.page, Some(point(Points(0.0), Points(0.0))));
        assert!(!card.flags.hidden && !card.flags.overlay && !card.flags.overridden);
        let text = &tree.nodes[2];
        assert_eq!(text.kind, "text");
        assert_eq!(text.page, Some(point(Points(4.0), Points(4.0))));
        let run = &tree.nodes[3];
        assert_eq!(run.kind, "raw-text");
        assert_eq!(run.text.as_deref(), Some("Hello"));
    }

    #[test]
    fn a_node_in_full_has_its_style_box_states_and_events() {
        let c = sample();
        let detail = c.dev_node(2).unwrap();
        assert_eq!(detail.inline_style.len(), 3);
        assert_eq!(detail.layout.size.width, Points(100.0));
        assert_eq!(detail.layout.padding.left, Points(4.0));
        assert_eq!(detail.layout.border.top, Points(0.0));
        assert!(detail.events.contains(EventKind::Press));
        assert!(detail.view_style.border_radius.0 > 0.0);
        assert_eq!(detail.override_style, None);
        assert!(c.dev_node(9_999).is_none());
    }

    #[test]
    fn questions_are_answered_in_the_next_commit_with_its_layout() {
        let mut c = sample();
        let (tree, slot) = Answer::<DevTree>::new();
        c.queue_inspect(Request::Tree(slot));
        let (detail, node_slot) = Answer::new();
        c.queue_inspect(Request::Node(2, node_slot));
        // The width changes in the same commit that answers.
        apply(
            &mut c,
            vec![style_op(
                2,
                &[
                    ("padding", StyleValue::Number(4.0)),
                    ("width", StyleValue::Number(150.0)),
                    ("height", StyleValue::Number(80.0)),
                ],
            )],
        );
        c.commit();
        let tree = ready(tree);
        assert_eq!(tree.nodes[1].frame.size.width, Points(150.0));
        let detail: Option<crate::inspect::DevDetail> = ready(detail);
        assert_eq!(detail.unwrap().layout.size.width, Points(150.0));
        assert!(c.dev.as_ref().unwrap().requests.is_empty());
    }

    #[test]
    fn rust_component_hosts_and_placeholders_are_named_and_flagged() {
        let mut c = sample();
        let mut ops = vec![
            Op::Create {
                id: 8,
                tag: Tag::Native,
            },
            Op::SetProp {
                id: 8,
                prop: Prop::Component("com.example.map".into()),
            },
            Op::Insert {
                parent: 2,
                child: 8,
                before: None,
            },
        ];
        ops.extend([
            Op::Create {
                id: 10,
                tag: Tag::View,
            },
            Op::Placeholder { id: 10 },
        ]);
        ops.push(Op::Insert {
            parent: 2,
            child: 10,
            before: None,
        });
        apply(&mut c, ops);
        c.commit();
        let tree = c.dev_tree();
        let find = |id| tree.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(find(8).kind, "native");
        assert_eq!(find(8).component.as_deref(), Some("com.example.map"));
        assert!(find(10).flags.placeholder);
        assert!(!find(2).flags.placeholder && !find(2).flags.culled && !find(2).flags.spare);
    }

    /// INSP-4: a surface that never asks has no dev state and does no dev
    /// work in a commit.
    #[test]
    fn a_surface_that_never_asks_carries_nothing() {
        let mut c = sample();
        apply(&mut c, view(8, 2));
        c.commit();
        assert!(!c.inspected());
        assert!(c.dev.is_none());
    }
}

#[cfg(test)]
mod pick_tests {
    use super::super::tests::{apply, core, style_op};
    use crate::{
        NodeId, Op, Prop, ROOT_ID, StyleValue, Tag,
        geometry::{Points, point},
    };

    fn boxed(id: NodeId, parent: NodeId, x: f32, y: f32, w: f32, h: f32) -> Vec<Op> {
        vec![
            Op::Create { id, tag: Tag::View },
            Op::Insert {
                parent,
                child: id,
                before: None,
            },
            style_op(
                id,
                &[
                    ("position", StyleValue::Str("absolute")),
                    ("left", StyleValue::Number(x)),
                    ("top", StyleValue::Number(y)),
                    ("width", StyleValue::Number(w)),
                    ("height", StyleValue::Number(h)),
                ],
            ),
        ]
    }

    fn at(x: f32, y: f32) -> crate::geometry::Point<Points> {
        point(Points(x), Points(y))
    }

    #[test]
    fn innermost_and_topmost_first_ignoring_pointer_events() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 200.0, 200.0);
        ops.extend(boxed(4, 2, 10.0, 10.0, 100.0, 100.0));
        ops.extend(boxed(6, 4, 10.0, 10.0, 20.0, 20.0));
        // A later sibling overlapping 4, which takes no touches.
        ops.extend(boxed(8, 2, 50.0, 50.0, 100.0, 100.0));
        ops.push(Op::SetProp {
            id: 8,
            prop: Prop::PointerEvents(crate::props::PointerEvents::None),
        });
        apply(&mut c, ops);
        c.commit();
        assert_eq!(c.dev_pick(at(25.0, 25.0), None), [6, 4, 2, ROOT_ID]);
        assert_eq!(c.dev_pick(at(60.0, 60.0), None), [8, 4, 2, ROOT_ID]);
        assert_eq!(c.dev_pick(at(300.0, 300.0), None), [ROOT_ID]);
    }

    #[test]
    fn a_higher_z_index_is_on_top() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend(boxed(4, ROOT_ID, 0.0, 0.0, 100.0, 100.0));
        ops.push(style_op(
            2,
            &[
                ("position", StyleValue::Str("absolute")),
                ("width", StyleValue::Number(100.0)),
                ("height", StyleValue::Number(100.0)),
                ("zIndex", StyleValue::Number(5.0)),
            ],
        ));
        apply(&mut c, ops);
        c.commit();
        assert_eq!(c.dev_pick(at(10.0, 10.0), None), [2, 4, ROOT_ID]);
    }

    #[test]
    fn clipping_hides_what_lies_outside() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 50.0, 50.0);
        ops.extend(boxed(4, 2, 40.0, 40.0, 40.0, 40.0));
        apply(&mut c, ops);
        c.commit();
        // Not clipped: the child sticks out and is found there.
        assert_eq!(c.dev_pick(at(70.0, 70.0), None), [4, ROOT_ID]);
        apply(
            &mut c,
            vec![style_op(
                2,
                &[
                    ("position", StyleValue::Str("absolute")),
                    ("width", StyleValue::Number(50.0)),
                    ("height", StyleValue::Number(50.0)),
                    ("overflow", StyleValue::Str("hidden")),
                ],
            )],
        );
        c.commit();
        assert_eq!(c.dev_pick(at(70.0, 70.0), None), [ROOT_ID]);
        assert_eq!(c.dev_pick(at(45.0, 45.0), None), [4, 2, ROOT_ID]);
    }

    #[test]
    fn overlays_are_above_and_skip_leaves_out_a_subtree() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend([
            Op::Create {
                id: 4,
                tag: Tag::Overlay,
            },
            Op::Insert {
                parent: 2,
                child: 4,
                before: None,
            },
        ]);
        ops.extend(boxed(6, 4, 0.0, 0.0, 30.0, 30.0));
        apply(&mut c, ops);
        c.commit();
        let hits = c.dev_pick(at(5.0, 5.0), None);
        let overlay_first = hits.iter().position(|&id| id == 6);
        let tree_first = hits.iter().position(|&id| id == 2);
        assert!(
            overlay_first.is_some() && overlay_first < tree_first,
            "{hits:?}"
        );
        // The tool's own layer (the overlay) left out.
        assert_eq!(c.dev_pick(at(5.0, 5.0), Some(4)), [2, ROOT_ID]);
    }
}
