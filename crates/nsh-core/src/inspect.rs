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
    /// The name of the Rust component whose host it is.
    pub component: Option<Arc<str>>,
    /// `class` and `classList` as written, joined by a space.
    pub class: Option<String>,
    /// `testID`.
    pub test_id: Option<Arc<str>>,
    /// The text of a raw text run.
    pub text: Option<String>,
    /// The frame relative to its parent, from the last layout.
    pub frame: Bounds<Points>,
    /// Its top left corner in the root view it is in, as touches'
    /// `pageX` / `pageY`; `None` when it is not laid out.
    pub page: Option<Point<Points>>,
    /// What else a tree view shows.
    pub flags: DevFlags,
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
    /// The style a dev tool overrides it with, as declarations.
    pub override_style: Option<Vec<StyleDecl>>,
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
    /// The script's: answered by the engine ([`ScriptSources`]).
    Source(NodeId, crate::element::AnswerSlot<Option<DevSource>>),
    Instances(NodeId, crate::element::AnswerSlot<Vec<DevInstance>>),
    Props(u32, crate::element::AnswerSlot<Vec<(String, DevProp)>>),
}

impl Request {
    /// Whether the script answers it, not the core's layout.
    pub(crate) fn is_script(&self) -> bool {
        matches!(
            self,
            Request::Source(..) | Request::Instances(..) | Request::Props(..)
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
