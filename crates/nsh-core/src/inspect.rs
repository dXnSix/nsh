//! What the core answers dev tools about its nodes (feature `inspect`,
//! which every platform's `dev` feature turns on; release runners never
//! do).
//!
//! The questions go through the action path, as [`crate::element::app_url`]
//! does: [`crate::element::dev_tree`], [`crate::element::dev_node`] and
//! [`crate::element::dev_pick`] return an [`crate::element::Answer`] the
//! core resolves in its next commit, after layout, so frames belong to the
//! layout of that commit. The core only reads its own nodes: what a tool
//! makes of them is the tool's. Until something asks, no node carries dev
//! state and a commit does no dev work.
//!
//! Positions ignore transforms and stop at modal roots: a view inside a
//! modal (or a page's header item, an input's accessory) is placed in that
//! root view, as `ref.measure()` places it.
//!
//! Where a node was written and which components rendered it are the
//! script's (hot bundles stamp them, `nsh-js` keeps them):
//! [`crate::element::dev_source`], [`crate::element::dev_instances`] and
//! [`crate::element::dev_instance_props`] are answered in the same round
//! by the engine, which hands the core its script as [`ScriptSources`].
//!
//! A view that hosts a Rust component says so ([`DevNode::component`],
//! [`DevFlags::host`]): what is under it is the component's view, but for
//! what the script gave the component ([`DevNode::given`]). A host's detail
//! has what the component takes ([`DevDetail::component`]), and
//! [`crate::element::dev_host_props`] what the script passes it.

use std::sync::Arc;

use crate::{
    EventMask, NodeId,
    geometry::{Bounds, Point, Points, Size},
    props::Accessibility,
    style::{StyleDecl, TextStyle, ViewStyle},
};

/// The core's tree: every node in it, from the root in tree order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DevTree {
    /// The root view's id.
    pub root: NodeId,
    /// The nodes, parents before their children.
    pub nodes: Vec<DevNode>,
}

/// One node of the tree, light: what a tree view shows.
#[derive(Debug, Clone, PartialEq)]
pub struct DevNode {
    /// The node's id (JS's ids are even, the core's odd).
    pub id: NodeId,
    /// Its kind: `view`, `text`, `raw-text`, `image`, `scroll`, `list`,
    /// `cell`, `input`, `stack`, `screen`, `modal`, `header-item`,
    /// `input-accessory`, `canvas`, `webview`, `video`, `native`, `overlay`
    /// or `shader`.
    pub kind: &'static str,
    /// Its parent (`None` for the root).
    pub parent: Option<NodeId>,
    /// Its children, in order.
    pub children: Vec<NodeId>,
    /// The name of the component whose host it is: a Rust component's JS
    /// export (`Button`; [`DevFlags::host`] is set), else a registered
    /// native component's (kind `native`).
    pub component: Option<Arc<str>>,
    /// On a node the script gave a Rust component: whose content it is.
    pub given: Option<DevGiven>,
    /// On the host of a slot marker component: the slot it is the place of
    /// ([`crate::element::design_slot`]).
    pub place: Option<DevPlace>,
    /// `class` and `classList` as written, joined by a space.
    pub class: Option<String>,
    /// `testID`.
    pub test_id: Option<Arc<str>>,
    /// The text of a raw text run, or of the run a text block or span
    /// holds without a node of its own (its only child).
    pub text: Option<String>,
    /// The frame relative to its parent, from the last layout.
    pub frame: Bounds<Points>,
    /// Its top left corner in the root view it is in, as touches'
    /// `pageX` / `pageY`; `None` when it is not laid out.
    pub page: Option<Point<Points>>,
    /// What else a tree view shows.
    pub flags: DevFlags,
}

/// What a node the script made is to the Rust component it is content
/// of: the node is in that component's view, wherever the view placed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevGiven {
    /// The component's host: the nearest one above the node that the
    /// script made too (or that no component's view is around).
    pub host: NodeId,
    /// The JS name of the prop the node holds the content of: a named
    /// slot's or the rows' holder (the glue's node, not the app's), or what
    /// a render function rendered. `None` for a JSX child.
    pub slot: Option<Arc<str>>,
}

/// What the host of a slot marker component marks
/// ([`crate::element::design_slot`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevPlace {
    /// The slot's name, as the marker's view gives it now: the JS name of
    /// the prop that fills it.
    pub name: Arc<str>,
    /// The host of the component whose view drew the marker, whose slot it
    /// is; `None` when the script made it (a marker the app's code wrote).
    pub of: Option<NodeId>,
}

/// The states of a node a tree view marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DevFlags {
    /// Held back in a scroll view's content, out of its window.
    pub culled: bool,
    /// A list cell without an item.
    pub spare: bool,
    /// The stand-in of an `<Each>` row not rendered yet.
    pub placeholder: bool,
    /// Hidden from sight (`hidden`, or in a hidden span).
    pub hidden: bool,
    /// An overlay, or a view made one by `position: fixed`.
    pub overlay: bool,
    /// A dev tool overrides its style.
    pub overridden: bool,
    /// It hosts a Rust component ([`DevNode::component`]; unnamed for a
    /// row a component's list wraps): its view made what is under it, but
    /// for the nodes [`DevNode::given`] it.
    pub host: bool,
}

/// A node in full: what a properties panel shows.
#[derive(Debug, Clone, PartialEq)]
pub struct DevDetail {
    /// The node's id.
    pub id: NodeId,
    /// The `style` prop's declarations as written, in order.
    pub inline_style: Vec<StyleDecl>,
    /// The view style resolved from its classes and `style`.
    pub view_style: ViewStyle,
    /// The text style as set on the node (unset values inherit).
    pub text_style: TextStyle,
    /// Its box from the last layout, unrounded.
    pub layout: DevBox,
    /// Its interaction states ([`crate::tailwind::state`] bits).
    pub states: u16,
    /// The events it listens to.
    pub events: EventMask,
    /// Its accessibility props, when it has any.
    pub accessibility: Option<Accessibility>,
    /// A scroll view's offset and content size.
    pub scroll: Option<DevScroll>,
    /// A list's items and columns.
    pub list: Option<DevList>,
    /// What a list cell holds.
    pub cell: Option<crate::CellSlot>,
    /// The layer a dev tool put over its authored values.
    pub dev_override: Option<DevOverride>,
    /// The Rust component it hosts, as its spec declares it.
    pub component: Option<DevComponent>,
}

/// A Rust component as its spec declares it: what a properties panel
/// offers of it. The values the script passes are the script's
/// ([`crate::element::dev_host_props`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevComponent {
    /// Its JS export (`Button`).
    pub name: String,
    /// Its props, slots, children, render functions and rows, in the
    /// spec's order.
    pub props: Vec<DevPropSpec>,
    /// Its events.
    pub events: Vec<DevEventSpec>,
}

/// One prop of a [`DevComponent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevPropSpec {
    /// Its JS name.
    pub name: &'static str,
    /// What it is: a value, a render function, a named slot, the JSX
    /// children or virtualized rows.
    pub kind: crate::element::PropKind,
    /// Its TypeScript type; empty in a build without the text of
    /// declarations ([`crate::element::DECLARATIONS`]).
    pub ts: String,
    /// It may be left out.
    pub optional: bool,
}

/// One event of a [`DevComponent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevEventSpec {
    /// Its handler's JS name (`onPress`).
    pub handler: String,
    /// The TypeScript type of its data (`void` for none); empty in a
    /// build without the text of declarations.
    pub data: String,
}

/// A dev tool's layer over a node's authored values
/// ([`crate::element::dev_override`]): classes, inline declarations and
/// forced states only, never other props. It survives the node's own class
/// and style updates; cleared, the node is exactly as authored. It dies
/// with its node: nothing keeps it in the core.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DevOverride {
    /// Classes over the node's own (Tailwind, as `class`), or instead of
    /// them with [`Self::replace_class`].
    pub class: String,
    /// `class` replaces the node's classes rather than adding to them.
    pub replace_class: bool,
    /// Declarations applied after the node's classes and `style`.
    pub style: crate::StylePatch,
    /// Interaction states shown as if the platform reported them
    /// ([`crate::tailwind::state`]: hovered, pressed, focused, ...).
    pub states: u16,
}

/// A node's dev state: made by an override, never before.
pub(crate) struct NodeDev {
    /// The layer.
    pub(crate) layer: DevOverride,
    /// The classes the node's own code last set, under the layer.
    pub(crate) base: Option<std::rc::Rc<crate::tailwind::ClassList>>,
}

/// A node's box from its last layout, before rounding.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DevBox {
    /// Its place in its parent.
    pub location: Point<Points>,
    /// Its border box's size.
    pub size: Size<Points>,
    /// Its padding.
    pub padding: DevEdges,
    /// Its border widths.
    pub border: DevEdges,
    /// Its margin.
    pub margin: DevEdges,
}

/// The four sides of a box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DevEdges {
    /// Top.
    pub top: Points,
    /// Right.
    pub right: Points,
    /// Bottom.
    pub bottom: Points,
    /// Left.
    pub left: Points,
}

/// A scroll view's state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DevScroll {
    /// The content offset the platform reported.
    pub offset: Point<Points>,
    /// The content size last sent to the platform.
    pub content: Option<Size<Points>>,
}

/// A list's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DevList {
    /// Its items.
    pub count: u32,
    /// Items per row (`numColumns`).
    pub columns: u32,
}

/// Where a node or a component use was written, as the source says now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevSource {
    /// The module's key in the hot runtime (its path in the app).
    pub module: String,
    /// The top-level statement it was written in.
    pub anchor: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column, in characters.
    pub column: u32,
}

/// An instance of a hot component that rendered a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevInstance {
    /// The instance's id, live while it is rendered.
    pub id: u32,
    /// The component's `module:name` key.
    pub key: String,
    /// Where the component was used.
    pub call_site: Option<DevSource>,
}

/// A prop an instance was rendered with, read without running anything.
#[derive(Debug, Clone, PartialEq)]
pub enum DevProp {
    /// A getter (never called).
    Getter,
    /// A function.
    Function,
    /// `undefined`.
    Undefined,
    /// `null`.
    Null,
    /// A boolean.
    Bool(bool),
    /// A number, unitless as JS has it.
    Number(f64),
    /// A string.
    Text(String),
    /// An object or an array.
    Object,
}

/// What the script knows of its nodes: the engine implements it over its
/// VM and hands it to [`crate::Core::answer_script_inspections`].
pub trait ScriptSources {
    /// Where node `id` was written.
    fn source(&self, id: NodeId) -> Option<DevSource>;
    /// The instances that rendered node `id`, innermost first.
    fn instances(&self, id: NodeId) -> Vec<DevInstance>;
    /// The props instance `instance` was rendered with.
    fn props(&self, instance: u32) -> Vec<(String, DevProp)>;
    /// The props the script passes the Rust component hosted by node `id`,
    /// read as an instance's are; none where the script keeps none.
    fn host_props(&self, id: NodeId) -> Vec<(String, DevProp)> {
        let _ = id;
        Vec::new()
    }
}

/// A question to the core, answered in its next commit.
pub(crate) enum Request {
    Tree(crate::element::AnswerSlot<DevTree>),
    Node(NodeId, crate::element::AnswerSlot<Option<DevDetail>>),
    Pick(
        Point<Points>,
        Option<NodeId>,
        crate::element::AnswerSlot<Vec<NodeId>>,
    ),
    /// A layer over a node, or none: applied when queued, so the commit
    /// lays it out; answers whether the node exists.
    Override(
        NodeId,
        Option<DevOverride>,
        crate::element::AnswerSlot<bool>,
    ),
    /// The script's: answered by the engine ([`ScriptSources`]).
    Source(NodeId, crate::element::AnswerSlot<Option<DevSource>>),
    Instances(NodeId, crate::element::AnswerSlot<Vec<DevInstance>>),
    Props(u32, crate::element::AnswerSlot<Vec<(String, DevProp)>>),
    HostProps(NodeId, crate::element::AnswerSlot<Vec<(String, DevProp)>>),
}

impl Request {
    /// Whether the script answers it, not the core's layout.
    pub(crate) fn is_script(&self) -> bool {
        matches!(
            self,
            Request::Source(..)
                | Request::Instances(..)
                | Request::Props(..)
                | Request::HostProps(..)
        )
    }
}

/// The core's dev state: made by the first question, never before.
#[derive(Default)]
pub(crate) struct DevState {
    /// Questions waiting for the commit.
    pub(crate) requests: Vec<Request>,
    /// Questions the script answers, waiting for the engine.
    pub(crate) script: Vec<Request>,
}

/// What a dev tool inside the app takes steps on it through in a normal dev
/// session (observe mode): the platform's test agent, handed over by
/// `nsh_testing::agent` when the app registered [`observe_driver`]. A step is
/// the dev protocol's (`{"step":"press","selector":"view:12"}`, nsh-dev's
/// `TestStep`) and its answer the step's outcome (`{"outcome":"done",
/// "value":…}`, `StepOutcome`), both JSON, so the tool needs neither
/// nsh-testing nor nsh-dev.
pub trait DevDriver: Send + Sync {
    /// Takes `step`; blocks until it is done and the app is quiet. Its
    /// native calls block on the host's main thread: never call it on that
    /// thread or the engine's.
    fn step(&self, step: serde_json::Value) -> serde_json::Value;
}

/// What [`observe_driver`] registered.
type DriverObserver = Box<dyn Fn(Arc<dyn DevDriver>) + Send + Sync>;

static DRIVER_OBSERVER: std::sync::OnceLock<DriverObserver> = std::sync::OnceLock::new();

/// Registers `observer` to get the [`DevDriver`] of every dev session a
/// host starts (process-wide; the first call wins). An app's crate calls it
/// before its host starts. Returns whether this call registered it.
pub fn observe_driver(observer: impl Fn(Arc<dyn DevDriver>) + Send + Sync + 'static) -> bool {
    DRIVER_OBSERVER.set(Box::new(observer)).is_ok()
}

/// Whether [`observe_driver`] registered an observer.
pub fn driver_wanted() -> bool {
    DRIVER_OBSERVER.get().is_some()
}

/// Hands `driver` to the registered observer, on the calling thread; does
/// nothing without one (nsh-testing calls it on a thread of its own).
pub fn hand_driver(driver: Arc<dyn DevDriver>) {
    if let Some(observer) = DRIVER_OBSERVER.get() {
        observer(driver);
    }
}

#[cfg(test)]
mod driver_tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct Echo;

    impl DevDriver for Echo {
        fn step(&self, step: serde_json::Value) -> serde_json::Value {
            step
        }
    }

    /// The first registration gets every driver handed over, on the
    /// calling thread; a later one is refused.
    #[test]
    fn a_registered_driver_observer_gets_the_driver() {
        static GOT: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
        hand_driver(Arc::new(Echo));
        assert!(!driver_wanted());
        assert!(observe_driver(|driver| {
            GOT.lock()
                .unwrap()
                .push(driver.step(serde_json::json!({ "step": "texts" })));
        }));
        assert!(!observe_driver(|_| panic!(
            "the second observer never runs"
        )));
        assert!(driver_wanted());
        hand_driver(Arc::new(Echo));
        assert_eq!(
            *GOT.lock().unwrap(),
            [serde_json::json!({ "step": "texts" })]
        );
    }
}
