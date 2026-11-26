//! The core's answers to dev tools ([`crate::inspect`], feature
//! `inspect`): the tree, a node's detail and the nodes under a point, read
//! from the nodes and the last layout. Nothing here runs until a tool
//! asks.

use std::sync::Arc;

use super::{Core, Key, Kind, NO_KEY, Node, measure::valid};
use crate::{
    NodeId,
    geometry::{Point, Points, point, size_pt},
    inspect::{
        DevBox, DevComponent, DevDetail, DevEdges, DevEventSpec, DevFlags, DevGiven, DevList,
        DevNode, DevOverride, DevPlace, DevPropSpec, DevScroll, DevState, DevTree, NodeDev,
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

/// An event's handler as TSX names it: `press` is `onPress`.
fn handler_name(event: &str) -> String {
    let mut chars = event.chars();
    match chars.next() {
        Some(first) => format!("on{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => "on".to_owned(),
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
        if let Request::Override(id, layer, answer) = request {
            // Now, so the next commit lays it out.
            let found = self.dev_override(id, layer);
            answer.borrow_mut().deliver(found);
            return;
        }
        let dev = self.dev.get_or_insert_with(Box::<DevState>::default);
        if request.is_script() {
            dev.script.push(request);
        } else {
            dev.requests.push(request);
        }
    }

    /// Answers the questions about the script's nodes (where they were
    /// written, who rendered them) with `sources`, the engine's script;
    /// `None` (no script runs, a Rust app) answers each with nothing. The
    /// engine calls it every round.
    pub fn answer_script_inspections(
        &mut self,
        sources: Option<&dyn crate::inspect::ScriptSources>,
    ) {
        let Some(requests) = self
            .dev
            .as_mut()
            .filter(|dev| !dev.script.is_empty())
            .map(|dev| std::mem::take(&mut dev.script))
        else {
            return;
        };
        for request in requests {
            match request {
                Request::Source(id, answer) => answer
                    .borrow_mut()
                    .deliver(sources.and_then(|s| s.source(id))),
                Request::Instances(id, answer) => answer
                    .borrow_mut()
                    .deliver(sources.map_or_else(Vec::new, |s| s.instances(id))),
                Request::Props(instance, answer) => answer
                    .borrow_mut()
                    .deliver(sources.map_or_else(Vec::new, |s| s.props(instance))),
                Request::HostProps(id, answer) => answer
                    .borrow_mut()
                    .deliver(sources.map_or_else(Vec::new, |s| s.host_props(id))),
                other => self.queue_inspect(other),
            }
        }
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
                Request::Window(id, answer) => {
                    let root = self
                        .window_ids
                        .get(&id)
                        .map(|(node, _)| *node)
                        .filter(|&node| self.key(node).is_ok());
                    answer.borrow_mut().deliver(root)
                }
                script => self.queue_inspect(script),
            }
        }
    }

    /// Puts `layer` over node `id`'s authored classes, style and states, or
    /// takes it away (`None`): the node is then exactly as its own code set
    /// it. Returns whether the node exists.
    pub fn dev_override(&mut self, id: NodeId, layer: Option<DevOverride>) -> bool {
        let Ok(key) = self.key(id) else {
            return false;
        };
        // The classes the node's own code set: kept by its layer, else the
        // incoming list (under its state styles, when it has them).
        let node = &mut self.nodes[key as usize];
        let base = match node.dev.take() {
            Some(dev) => dev.base,
            None => match node.parts.as_deref() {
                Some(parts) => parts.base.clone(),
                None => node.classes.clone(),
            },
        };
        if let Some(layer) = layer {
            self.nodes[key as usize].dev = Some(Box::new(NodeDev {
                layer,
                base: base.clone(),
            }));
        }
        // Resolved again, with the layer or without; styled again even when
        // the classes come out the same (a layer of states or declarations
        // only), which `set_class_list` would leave as they were.
        self.set_class_list(key, base);
        self.update_tracking(key);
        self.restyle(key);
        true
    }

    /// The classes of `key` with its layer: `classes` (what its code set)
    /// becomes the layer's base.
    pub(super) fn overridden_classes(
        &mut self,
        key: Key,
        classes: Option<std::rc::Rc<crate::tailwind::ClassList>>,
    ) -> Option<std::rc::Rc<crate::tailwind::ClassList>> {
        let Some(dev) = self.nodes[key as usize].dev.as_deref_mut() else {
            return classes;
        };
        dev.base = classes.clone();
        let (class, replace) = (dev.layer.class.clone(), dev.layer.replace_class);
        // Parsed with the theme and tokens now (cached), as the node's own.
        let extra = (!class.trim().is_empty()).then(|| self.tailwind.get(&class).0);
        if replace {
            return extra;
        }
        match (classes, extra) {
            (Some(c), Some(e)) => Some(std::rc::Rc::new(c.merged(&e))),
            (None, Some(e)) => Some(e),
            (c, None) => c,
        }
    }

    /// The theme or the tokens changed: overridden nodes resolve their
    /// layers' classes again, those without classes of their own too.
    pub(super) fn restyle_overrides(&mut self) {
        let keys: Vec<(Key, Option<std::rc::Rc<crate::tailwind::ClassList>>)> = self
            .nodes
            .iter()
            .filter_map(|(k, n)| Some((k as Key, n.dev.as_deref()?.base.clone())))
            .collect();
        for (key, base) in keys {
            self.set_class_list(key, base);
        }
    }

    /// Whether a tool ever asked this core anything (it then has dev
    /// state).
    pub fn inspected(&self) -> bool {
        self.dev.is_some()
    }

    /// The tree as the last layout left it: every node from the root, in
    /// tree order. A node the script made in a Rust component's view says
    /// whose content it is ([`DevNode::given`]).
    pub fn dev_tree(&self) -> DevTree {
        let mut nodes = Vec::new();
        let mut places = self.dev_slot_places();
        // With each node, the host whose view it is in.
        let mut stack = vec![(self.root, None::<NodeId>)];
        while let Some((key, within)) = stack.pop() {
            let node = &self.nodes[key as usize];
            let mut dev = self.dev_node_light(key);
            // The core's nodes have odd ids: an even one in a view is the
            // script's, given to the component.
            let scripted = node.id.is_multiple_of(2);
            if let Some(host) = within.filter(|_| scripted) {
                dev.given = Some(DevGiven {
                    host,
                    slot: self.dev_given_as(node.id),
                });
            }
            // A view starts at a host the script made (or one in no view):
            // the components its view nests are part of it.
            let below = if dev.flags.host && (scripted || within.is_none()) {
                Some(node.id)
            } else if scripted {
                None
            } else {
                within
            };
            dev.place = places
                .remove(&node.id)
                .map(|(name, of)| DevPlace { name, of });
            nodes.push(dev);
            stack.extend(node.children.iter().rev().map(|&child| (child, below)));
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
            component: self
                .dev_hosted(node.id)
                .map(|spec| Arc::from(spec.export.as_str()))
                .or_else(|| node.component.as_deref().and_then(|c| c.name.clone())),
            given: None,
            place: None,
            class: node
                .class_names
                .as_deref()
                .map(super::classes::ClassNames::authored)
                .filter(|class| !class.is_empty()),
            test_id: node.props.as_deref().and_then(|p| p.test_id.clone()),
            text: match node.kind {
                Kind::RawText => Some(node.text.as_str()),
                // A text block or span keeps its only run without a node.
                Kind::Text => self.folded_text(key),
                _ => None,
            }
            .filter(|text| !text.is_empty())
            .map(str::to_owned),
            frame: node.frame,
            page: self.measure(node.id).map(|m| m.page),
            flags: DevFlags {
                culled: node.culled,
                spare: node.spare,
                placeholder: node.placeholder,
                hidden: node.view_style.hidden || node.hidden_in_text,
                overlay: node.is_overlay(),
                overridden: node.dev.is_some(),
                host: self.is_host(node.id),
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
            text_style: *node.text_style,
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
            dev_override: node.dev.as_deref().map(|dev| dev.layer.clone()),
            component: self.dev_hosted(id).map(|spec| DevComponent {
                name: spec.export.clone(),
                props: spec
                    .props
                    .iter()
                    .map(|p| DevPropSpec {
                        name: p.name,
                        kind: p.kind,
                        ts: p.ts.clone(),
                        optional: p.optional,
                    })
                    .collect(),
                events: spec
                    .events
                    .iter()
                    .map(|e| DevEventSpec {
                        handler: handler_name(e.name),
                        data: e.data.clone(),
                    })
                    .collect(),
            }),
        })
    }

    /// The nodes under page point `page`, innermost and topmost first:
    /// modals above the surface, the overlays shown in a root view above
    /// it in the order they were shown (the last on top), a later sibling
    /// (by `zIndex`) above an earlier one, children before their parent.
    /// `pointerEvents`, listeners and the disabled state do not count; a
    /// view that clips (`overflow: hidden`, scroll views, lists, stacks)
    /// leaves out what lies outside it, and hidden views and overlays not
    /// shown are left out. The subtree of `skip` (a tool's own layer) is
    /// left out, its overlays included.
    pub fn dev_pick(&self, page: Point<Points>, skip: Option<NodeId>) -> Vec<NodeId> {
        let mut out = Vec::new();
        for layer in self.pick_layers(skip).into_iter().rev() {
            self.pick_in(layer, point(Points(0.0), Points(0.0)), page, skip, &mut out);
        }
        out
    }

    /// The root views to pick in, bottom first: the surface, its header
    /// items and accessories in tree order, the overlays (and fixed views)
    /// shown on it in show order, then each modal in tree order with those
    /// shown on it. Those in `skip`'s subtree or under a
    /// hidden view are left out.
    fn pick_layers(&self, skip: Option<NodeId>) -> Vec<Key> {
        let excluded = |key: Key| {
            let mut k = key;
            while k != NO_KEY {
                let n = &self.nodes[k as usize];
                if Some(n.id) == skip || n.view_style.hidden || n.hidden_in_text {
                    return true;
                }
                k = n.parent;
            }
            false
        };
        let mut surface_views = Vec::new();
        let mut modals = Vec::new();
        let mut stack = vec![self.root];
        while let Some(key) = stack.pop() {
            let node = &self.nodes[key as usize];
            // Overlays and fixed views come in shown, from `self.overlays`.
            match node.kind {
                Kind::Modal => modals.push(key),
                Kind::HeaderItem | Kind::InputAccessory => surface_views.push(key),
                _ => {}
            }
            stack.extend(node.children.iter().rev().copied());
        }
        // Shown overlays, in the order they were shown, by the root view
        // they are shown in.
        let shown_in = |host: NodeId| {
            self.overlays
                .iter()
                .copied()
                .filter(move |&k| self.overlay(k).and_then(|o| o.host) == Some(host))
        };
        let root_id = self.nodes[self.root as usize].id;
        let mut layers = vec![self.root];
        layers.extend(surface_views);
        layers.extend(shown_in(root_id));
        for modal in modals {
            layers.push(modal);
            layers.extend(shown_in(self.nodes[modal as usize].id));
        }
        layers.retain(|&k| k == self.root || !excluded(k));
        layers
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
        // The text block holds its only run without a node (#2366).
        assert_eq!(ids, [ROOT_ID, 2, 4]);
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
        assert_eq!(text.text.as_deref(), Some("Hello"));
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
        assert_eq!(detail.dev_override, None);
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

    /// The script's questions wait for the engine, which answers them with
    /// its VM, or with nothing when no script runs.
    #[test]
    fn script_questions_are_answered_by_the_engine_s_sources() {
        use crate::inspect::{DevInstance, DevProp, DevSource, ScriptSources};
        struct Vm;
        impl ScriptSources for Vm {
            fn source(&self, id: NodeId) -> Option<DevSource> {
                (id == 2).then(|| DevSource {
                    module: "App.tsx".into(),
                    anchor: "App".into(),
                    line: 3,
                    column: 10,
                })
            }
            fn instances(&self, _: NodeId) -> Vec<DevInstance> {
                vec![DevInstance {
                    id: 1,
                    key: "App.tsx:App".into(),
                    call_site: None,
                }]
            }
            fn props(&self, _: u32) -> Vec<(String, DevProp)> {
                vec![("title".into(), DevProp::Getter)]
            }
        }
        let mut c = sample();
        let (source, slot) = Answer::new();
        c.queue_inspect(Request::Source(2, slot));
        let (instances, slot) = Answer::new();
        c.queue_inspect(Request::Instances(2, slot));
        let (props, slot) = Answer::new();
        c.queue_inspect(Request::Props(1, slot));
        // The core's commit leaves them to the engine.
        c.commit();
        assert_eq!(c.dev.as_ref().unwrap().script.len(), 3);
        c.answer_script_inspections(Some(&Vm));
        assert_eq!(ready(source).unwrap().line, 3);
        assert_eq!(ready(instances)[0].key, "App.tsx:App");
        assert_eq!(ready(props), [("title".to_owned(), DevProp::Getter)]);
        // A Rust app: no script, no answer but nothing.
        let (source, slot) = Answer::new();
        c.queue_inspect(Request::Source(2, slot));
        c.answer_script_inspections(None);
        assert_eq!(ready(source), None);
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

/// A text block or span keeps its only run without a node (upstream
/// #2366): the tree reports that run's text on the block, and a tree asked
/// after the run's text changed has the new text.
#[cfg(test)]
mod inline_text_tests {
    use super::super::tests::{apply, core};
    use crate::{
        NodeId, Op, ROOT_ID, Tag,
        element::Answer,
        inspect::{DevTree, Request},
    };

    fn ready<T>(answer: Answer<T>) -> T {
        use std::future::Future;
        let mut answer = std::pin::pin!(answer);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match answer.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("not answered"),
        }
    }

    /// A text block `block` under the root holding the runs `runs`.
    fn label(
        block: NodeId,
        runs: &[(NodeId, &str)],
    ) -> crate::core::Core<super::super::tests::FakeEngine> {
        let mut c = core();
        let mut ops = vec![
            Op::Create {
                id: block,
                tag: Tag::Text,
            },
            Op::Insert {
                parent: ROOT_ID,
                child: block,
                before: None,
            },
        ];
        for &(id, text) in runs {
            ops.push(Op::CreateText {
                id,
                text: text.into(),
            });
            ops.push(Op::Insert {
                parent: block,
                child: id,
                before: None,
            });
        }
        apply(&mut c, ops);
        c.commit();
        c
    }

    fn text_of(tree: &DevTree, id: NodeId) -> Option<&str> {
        tree.nodes.iter().find(|n| n.id == id)?.text.as_deref()
    }

    #[test]
    fn a_text_block_s_only_run_is_the_block_s_text() {
        let c = label(4, &[(6, "Save")]);
        let tree = c.dev_tree();
        let ids: Vec<NodeId> = tree.nodes.iter().map(|n| n.id).collect();
        assert_eq!(ids, [ROOT_ID, 4], "the run has no node of its own");
        assert_eq!(tree.nodes[1].kind, "text");
        assert_eq!(text_of(&tree, 4), Some("Save"));
    }

    #[test]
    fn a_tree_asked_after_the_run_changed_has_its_new_text() {
        let mut c = label(4, &[(6, "Save")]);
        let before = c.dev_tree();
        let (tree, slot) = Answer::<DevTree>::new();
        c.queue_inspect(Request::Tree(slot));
        apply(
            &mut c,
            vec![Op::SetText {
                id: 6,
                text: "Saved".into(),
            }],
        );
        c.commit();
        let after = ready(tree);
        assert_eq!(text_of(&after, 4), Some("Saved"));
        assert_ne!(after, before, "a tool comparing trees sees the change");
    }

    #[test]
    fn runs_with_nodes_keep_their_text_and_the_block_has_none() {
        let c = label(4, &[(6, "Save"), (8, " all")]);
        let tree = c.dev_tree();
        let ids: Vec<NodeId> = tree.nodes.iter().map(|n| n.id).collect();
        assert_eq!(ids, [ROOT_ID, 4, 6, 8]);
        assert_eq!(text_of(&tree, 4), None);
        assert_eq!(tree.nodes[2].kind, "raw-text");
        assert_eq!(text_of(&tree, 6), Some("Save"));
        assert_eq!(text_of(&tree, 8), Some(" all"));
    }

    #[test]
    fn an_empty_run_is_no_text() {
        let c = label(4, &[(6, "")]);
        assert_eq!(text_of(&c.dev_tree(), 4), None);
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

    fn overlay(id: NodeId, parent: NodeId) -> Vec<Op> {
        vec![
            Op::Create {
                id,
                tag: Tag::Overlay,
            },
            Op::Insert {
                parent,
                child: id,
                before: None,
            },
        ]
    }

    #[test]
    fn skip_leaves_out_the_overlays_of_its_subtree_and_hidden_ones_too() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend(overlay(4, 2));
        ops.extend(boxed(6, 4, 0.0, 0.0, 30.0, 30.0));
        apply(&mut c, ops);
        c.commit();
        assert!(c.dev_pick(at(5.0, 5.0), None).contains(&6));
        // The overlay's parent skipped: the overlay goes with it.
        assert_eq!(c.dev_pick(at(5.0, 5.0), Some(2)), [ROOT_ID]);
        // Under a hidden view, an overlay is left out too.
        apply(
            &mut c,
            vec![style_op(
                2,
                &[
                    ("position", StyleValue::Str("absolute")),
                    ("width", StyleValue::Number(100.0)),
                    ("height", StyleValue::Number(100.0)),
                    ("visibility", StyleValue::Str("hidden")),
                ],
            )],
        );
        c.commit();
        assert_eq!(c.dev_pick(at(5.0, 5.0), None), [ROOT_ID]);
    }

    #[test]
    fn overlays_are_picked_in_the_order_they_were_shown() {
        use std::time::Duration;
        let hint = |id: NodeId, open: bool| {
            [
                Op::SetProp {
                    id,
                    prop: Prop::Overlay(crate::OverlayProp::Hint(Some(Duration::from_secs(1)))),
                },
                Op::SetProp {
                    id,
                    prop: Prop::Overlay(crate::OverlayProp::Open(Some(open))),
                },
            ]
        };
        let mut c = core();
        // Two hints on one anchor, so placed alike: 4 first in the tree, 8
        // shown first.
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend(overlay(4, 2));
        ops.extend(boxed(6, 4, 0.0, 0.0, 30.0, 30.0));
        ops.extend(hint(4, false));
        ops.extend(overlay(8, 2));
        ops.extend(boxed(10, 8, 0.0, 0.0, 30.0, 30.0));
        ops.extend(hint(8, true));
        apply(&mut c, ops);
        c.commit();
        let place = |c: &crate::core::Core<_>, id| c.measure(id).unwrap().page;
        let p = place(&c, 10);
        let inside = at(p.x.0 + 5.0, p.y.0 + 5.0);
        // The hint not shown is not picked.
        let hits = c.dev_pick(inside, None);
        assert!(!hits.contains(&6) && hits.contains(&10), "{hits:?}");
        // Shown now, after 8: on top of it, though earlier in the tree.
        apply(&mut c, hint(4, true).into());
        c.commit();
        assert_eq!(place(&c, 6), place(&c, 10));
        let hits = c.dev_pick(inside, None);
        let first = |id| hits.iter().position(|&h| h == id).unwrap_or(usize::MAX);
        assert!(first(6) < first(10) && first(10) < usize::MAX, "{hits:?}");
    }

    #[test]
    fn a_modal_is_above_the_surface_and_its_overlays() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend(overlay(4, 2));
        ops.extend(boxed(6, 4, 0.0, 0.0, 30.0, 30.0));
        ops.extend([
            Op::Create {
                id: 8,
                tag: Tag::Modal,
            },
            Op::Insert {
                parent: 2,
                child: 8,
                before: None,
            },
        ]);
        ops.extend(boxed(10, 8, 0.0, 0.0, 30.0, 30.0));
        apply(&mut c, ops);
        c.commit();
        let hits = c.dev_pick(at(5.0, 5.0), None);
        let first = |id| hits.iter().position(|&h| h == id);
        if let (Some(modal), Some(over)) = (first(10), first(6)) {
            assert!(modal < over, "{hits:?}");
        } else {
            panic!("{hits:?}");
        }
    }

    fn fixed(id: NodeId, parent: NodeId, w: f32, h: f32) -> Vec<Op> {
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
                    ("position", StyleValue::Str("fixed")),
                    ("left", StyleValue::Number(0.0)),
                    ("top", StyleValue::Number(0.0)),
                    ("width", StyleValue::Number(w)),
                    ("height", StyleValue::Number(h)),
                ],
            ),
        ]
    }

    #[test]
    fn a_fixed_view_is_picked_once_above_the_tree() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend(fixed(4, 2, 50.0, 50.0));
        apply(&mut c, ops);
        c.commit();
        assert_eq!(c.dev_pick(at(5.0, 5.0), None), [4, 2, ROOT_ID]);
    }

    #[test]
    fn a_fixed_view_in_a_modal_is_above_the_modal_s_content() {
        let mut c = core();
        let mut ops = boxed(2, ROOT_ID, 0.0, 0.0, 100.0, 100.0);
        ops.extend([
            Op::Create {
                id: 4,
                tag: Tag::Modal,
            },
            Op::Insert {
                parent: 2,
                child: 4,
                before: None,
            },
        ]);
        ops.extend(boxed(6, 4, 0.0, 0.0, 80.0, 80.0));
        ops.extend(fixed(8, 6, 40.0, 40.0));
        apply(&mut c, ops);
        c.commit();
        let hits = c.dev_pick(at(5.0, 5.0), None);
        let first = |id| hits.iter().position(|&h| h == id).unwrap_or(usize::MAX);
        assert_eq!(hits.iter().filter(|&&h| h == 8).count(), 1, "{hits:?}");
        assert!(first(8) < first(6) && first(6) < usize::MAX, "{hits:?}");
    }
}

#[cfg(test)]
mod override_tests {
    use super::super::tests::{apply, core, style_op};
    use crate::{
        NodeId, Op, Prop, ROOT_ID, Style, StylePatch, StyleValue, Tag, geometry::Points,
        inspect::DevOverride, prelude::*, tailwind::state,
    };

    type TestCore = crate::core::Core<super::super::tests::FakeEngine>;

    fn styled(id: NodeId, class: Option<&str>, width: f32) -> Vec<Op> {
        let mut ops = vec![
            Op::Create { id, tag: Tag::View },
            Op::Insert {
                parent: ROOT_ID,
                child: id,
                before: None,
            },
            style_op(
                id,
                &[
                    ("width", StyleValue::Number(width)),
                    ("height", StyleValue::Number(50.0)),
                ],
            ),
        ];
        if let Some(class) = class {
            ops.push(Op::SetProp {
                id,
                prop: Prop::Class(class.into()),
            });
        }
        ops
    }

    fn patch(props: &[(&str, StyleValue<'_>)]) -> StylePatch {
        let mut patch = StylePatch::default();
        let mut style = Style::default();
        for &(k, v) in props {
            patch.set(&mut style, k, v).unwrap();
        }
        patch
    }

    /// What the node shows: its view style and box.
    fn shown(c: &TestCore, id: NodeId) -> (crate::ViewStyle, crate::inspect::DevBox) {
        let d = c.dev_node(id).unwrap();
        (d.view_style, d.layout)
    }

    #[test]
    fn a_layer_survives_the_node_s_own_updates_and_clearing_restores_it() {
        let mut c = core();
        apply(&mut c, styled(2, Some("p-2"), 100.0));
        c.commit();
        assert!(c.dev_override(
            2,
            Some(DevOverride {
                class: "p-8".into(),
                style: patch(&[("opacity", StyleValue::Number(0.5))]),
                ..Default::default()
            })
        ));
        c.commit();
        let (view, layout) = shown(&c, 2);
        assert_eq!(layout.padding.left, Points(32.0));
        assert_eq!(view.opacity, 0.5);
        assert!(
            c.dev_tree()
                .nodes
                .iter()
                .any(|n| n.id == 2 && n.flags.overridden)
        );
        assert_eq!(c.dev_node(2).unwrap().dev_override.unwrap().class, "p-8");
        // The node's own class and style change under the layer.
        apply(
            &mut c,
            vec![Op::SetProp {
                id: 2,
                prop: Prop::Class("p-4".into()),
            }],
        );
        apply(
            &mut c,
            vec![style_op(
                2,
                &[
                    ("width", StyleValue::Number(120.0)),
                    ("height", StyleValue::Number(50.0)),
                ],
            )],
        );
        c.commit();
        let (view, layout) = shown(&c, 2);
        assert_eq!(layout.padding.left, Points(32.0));
        assert_eq!(layout.size.width, Points(120.0));
        assert_eq!(view.opacity, 0.5);
        // Cleared: exactly what a node authored so shows.
        assert!(c.dev_override(2, None));
        apply(&mut c, styled(4, Some("p-4"), 120.0));
        c.commit();
        let (view, layout) = shown(&c, 2);
        let (authored_view, authored_layout) = shown(&c, 4);
        assert_eq!(view, authored_view);
        assert_eq!(layout.padding, authored_layout.padding);
        assert_eq!(layout.size, authored_layout.size);
        assert!(!c.dev_tree().nodes.iter().any(|n| n.flags.overridden));
        assert_eq!(c.dev_node(2).unwrap().dev_override, None);
    }

    /// A node without classes restyles on a fast path: the layer stays.
    #[test]
    fn the_fast_path_of_a_node_without_classes_keeps_the_layer() {
        let mut c = core();
        apply(&mut c, styled(2, None, 100.0));
        c.commit();
        c.dev_override(
            2,
            Some(DevOverride {
                style: patch(&[("opacity", StyleValue::Number(0.3))]),
                ..Default::default()
            }),
        );
        apply(
            &mut c,
            vec![style_op(
                2,
                &[
                    ("width", StyleValue::Number(80.0)),
                    ("height", StyleValue::Number(50.0)),
                ],
            )],
        );
        c.commit();
        let (view, layout) = shown(&c, 2);
        assert_eq!(view.opacity, 0.3);
        assert_eq!(layout.size.width, Points(80.0));
        c.dev_override(2, None);
        c.commit();
        assert_eq!(shown(&c, 2).0.opacity, 1.0);
    }

    #[test]
    fn forced_states_show_their_styles_and_replace_class_replaces() {
        let mut c = core();
        apply(
            &mut c,
            styled(2, Some("p-2 opacity-100 hover:opacity-50"), 100.0),
        );
        c.commit();
        assert_eq!(shown(&c, 2).0.opacity, 1.0);
        c.dev_override(
            2,
            Some(DevOverride {
                states: state::HOVERED,
                ..Default::default()
            }),
        );
        c.commit();
        assert_eq!(shown(&c, 2).0.opacity, 0.5);
        c.dev_override(
            2,
            Some(DevOverride {
                class: "p-6".into(),
                replace_class: true,
                ..Default::default()
            }),
        );
        c.commit();
        let (view, layout) = shown(&c, 2);
        assert_eq!(layout.padding.left, Points(24.0));
        // The node's classes are gone, its hover with them.
        assert_eq!(view.opacity, 1.0);
        c.dev_override(2, None);
        c.commit();
        let (view, layout) = shown(&c, 2);
        assert_eq!(layout.padding.left, Points(8.0));
        assert_eq!(view.opacity, 1.0);
    }

    #[component]
    fn Card() -> impl IntoElement {
        view().p_2().child(view().p_1().child(text("Hi")))
    }

    /// A Rust component's host node and its internal nodes take layers too.
    #[test]
    fn a_rust_component_s_nodes_take_layers() {
        let mut c = core();
        c.mount_root(&crate::element::Root::new::<Card>(|| ()));
        c.commit();
        let padded: Vec<(NodeId, Points)> = c
            .dev_tree()
            .nodes
            .iter()
            .filter_map(|n| Some((n.id, c.dev_node(n.id)?.layout.padding.left)))
            .filter(|(_, p)| p.0 > 0.0)
            .collect();
        assert_eq!(padded.len(), 2, "{padded:?}");
        for &(id, authored) in &padded {
            c.dev_override(
                id,
                Some(DevOverride {
                    class: "p-10".into(),
                    ..Default::default()
                }),
            );
            c.commit();
            assert_eq!(c.dev_node(id).unwrap().layout.padding.left, Points(40.0));
            c.dev_override(id, None);
            c.commit();
            assert_eq!(c.dev_node(id).unwrap().layout.padding.left, authored);
        }
    }

    /// The layer's classes follow the tokens as the node's own do, on a
    /// node with classes of its own and on one without.
    #[test]
    fn a_layer_s_classes_follow_token_changes() {
        let mut c = core();
        apply(&mut c, styled(2, Some("p-2"), 100.0));
        apply(&mut c, styled(4, None, 100.0));
        c.commit();
        for id in [2, 4] {
            c.dev_override(
                id,
                Some(DevOverride {
                    class: "bg-primary".into(),
                    ..Default::default()
                }),
            );
        }
        c.commit();
        let before = shown(&c, 2).0.background_color;
        assert_eq!(shown(&c, 4).0.background_color, before);
        let mut tokens = crate::element::tokens::sample();
        tokens.set_color("primary", crate::Color(0x0000_ffff));
        c.set_test_tokens(tokens);
        c.commit();
        for id in [2, 4] {
            let now = shown(&c, id).0.background_color;
            assert_ne!(now, before, "node {id}");
            assert_eq!(now, crate::Color(0x0000_ffff), "node {id}");
        }
    }

    /// The layer is the node's: it goes with it, and the core keeps none.
    #[test]
    fn a_layer_dies_with_its_node() {
        let mut c = core();
        apply(&mut c, styled(2, Some("p-2"), 100.0));
        c.commit();
        // Through the action path: answered at once, laid out next commit.
        let (answer, slot) = crate::element::Answer::new();
        c.queue_inspect(super::Request::Override(
            2,
            Some(DevOverride {
                class: "p-8".into(),
                ..Default::default()
            }),
            slot,
        ));
        drop(answer);
        c.commit();
        assert_eq!(c.dev_node(2).unwrap().layout.padding.left, Points(32.0));
        apply(
            &mut c,
            vec![
                Op::Remove {
                    parent: ROOT_ID,
                    child: 2,
                },
                Op::Release { id: 2 },
            ],
        );
        c.commit();
        assert!(!c.dev_override(2, None));
        assert!(c.dev.is_none());
    }
}
