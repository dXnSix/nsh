//! Where nodes were written and who rendered them, for dev tools (hot
//! bundles only).
//!
//! The JSX compiler stamps hot bundles (`nsh-bundler`'s `jsx.rs`): each
//! top-level statement with JSX registers an anchor, `anchor(module,
//! statement, line)`, every time its module runs, and every element and
//! component use carries `(anchor, lineDelta, column)`. A node keeps its
//! stamp; its line is computed when asked, from its anchor's line now, so a
//! hot update that moves a statement leaves the nodes of components it did
//! not render again with their right lines.
//!
//! A component use sets the call site it was written at as pending while
//! its component starts: a native component's host node, made with
//! [`crate::dom::create_host_element`], takes it, and so does the instance
//! of a hot component, so nothing else made in its body (slots, rows'
//! placeholders, other nodes) gets it.
//!
//! Each render of a hot component (the bundler's proxies, `hot.rs`) is an
//! instance: its `module:name` key, its call site and its props, provided
//! to what it renders as a context value and gone with the owner that
//! rendered it. A node made under an instance keeps its id, so
//! [`node_instances`] answers with the chain that rendered a node. Before a
//! hot component has rendered, nodes look nothing up.
//!
//! Nothing here sends an op or touches the core.

use std::cell::{Cell, RefCell};

use nsh_core::NodeId;
use rquickjs::qjs;
use rustc_hash::FxHashMap;

use crate::reactive::arg;

type JsValue = qjs::JSValue;
type JsContext = qjs::JSContext;

const UNDEFINED: JsValue = qjs::JS_UNDEFINED;

/// Where an element was written: its anchor, its line relative to the
/// anchor's and its column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    anchor: u32,
    delta: i32,
    column: u32,
}

/// Where a node or a component use was written, as the source says now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSource {
    /// The module's key in the hot runtime (its path in the app).
    pub module: String,
    /// The top-level statement it was written in: the name it declares,
    /// `default`, or `#<n>` for the `n`th statement with JSX that names
    /// nothing.
    pub anchor: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column, in characters.
    pub column: u32,
}

/// An instance of a hot component that rendered a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceRef {
    /// The instance's id, live while it is rendered.
    pub id: u32,
    /// The component's `module:name` key.
    pub key: String,
    /// Where the component was used, when the use was stamped.
    pub call_site: Option<NodeSource>,
}

/// A recorded prop of an instance, read without running anything.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    /// A getter: a prop that follows what it reads (never called here).
    Getter,
    /// A function (a handler, a render prop).
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

/// The anchors of a context's modules.
#[derive(Default)]
struct Anchors {
    /// (module, statement) -> anchor id.
    ids: FxHashMap<(String, String), u32>,
    /// By id: module, statement, line now.
    entries: Vec<(String, String, u32)>,
}

struct Instance {
    key: String,
    call_site: Option<Stamp>,
    /// The instance that rendered this one's use (0: none).
    parent: u32,
    /// The props object (owned).
    props: JsValue,
}

/// A context's stamps and instances.
pub(crate) struct DevSource {
    anchors: RefCell<Anchors>,
    /// The call site of the component use running now.
    pending: Cell<Option<Stamp>>,
    instances: RefCell<FxHashMap<u32, Instance>>,
    /// The last instance id given (0: none yet, and nodes look nothing up).
    last_instance: Cell<u32>,
    /// The context key instances are provided under (owned).
    key: Cell<JsValue>,
}

impl DevSource {
    pub(crate) fn new() -> DevSource {
        DevSource {
            anchors: RefCell::default(),
            pending: Cell::new(None),
            instances: RefCell::default(),
            last_instance: Cell::new(0),
            key: Cell::new(UNDEFINED),
        }
    }

    /// Frees the values it holds (the context goes).
    pub(crate) fn free_all(&self, cx: *mut JsContext) {
        for (_, instance) in self.instances.take() {
            // SAFETY: values it owned.
            unsafe { qjs::JS_FreeValue(cx, instance.props) };
        }
        unsafe { qjs::JS_FreeValue(cx, self.key.replace(UNDEFINED)) };
    }
}

fn dev<'a>(cx: *mut JsContext) -> &'a DevSource {
    crate::dom::dev_source(cx)
}

fn number(cx: *mut JsContext, v: JsValue) -> Option<f64> {
    let mut n = 0f64;
    // SAFETY: a borrowed value.
    (unsafe { qjs::JS_IsNumber(v) } && unsafe { qjs::JS_ToFloat64(cx, &mut n, v) } == 0)
        .then_some(n)
}

/// The stamp in arguments `first..first + 3` of a call, if they are one.
pub(crate) fn stamp_args(
    cx: *mut JsContext,
    argc: std::ffi::c_int,
    argv: *mut JsValue,
    first: usize,
) -> Option<Stamp> {
    if (argc as usize) < first + 3 {
        return None;
    }
    let anchor = number(cx, arg(argc, argv, first))?;
    let delta = number(cx, arg(argc, argv, first + 1))?;
    let column = number(cx, arg(argc, argv, first + 2))?;
    let anchors = dev(cx).anchors.borrow();
    ((anchor as usize) < anchors.entries.len()).then_some(Stamp {
        anchor: anchor as u32,
        delta: delta as i32,
        column: column.max(0.0) as u32,
    })
}

/// `anchor(module, statement, line)`: the id of a top-level statement's
/// anchor, its line now `line`.
pub(crate) unsafe extern "C" fn anchor(
    cx: *mut JsContext,
    _this: JsValue,
    argc: std::ffi::c_int,
    argv: *mut JsValue,
) -> JsValue {
    let (Some(module), Some(statement), Some(line)) = (
        crate::dom::string_arg(cx, arg(argc, argv, 0)),
        crate::dom::string_arg(cx, arg(argc, argv, 1)),
        number(cx, arg(argc, argv, 2)),
    ) else {
        // SAFETY: a live context.
        return unsafe {
            qjs::JS_ThrowTypeError(cx, c"nsh: anchor(module, statement, line)".as_ptr())
        };
    };
    let line = line.max(1.0) as u32;
    let mut anchors = dev(cx).anchors.borrow_mut();
    let anchors = &mut *anchors;
    let id = match anchors.ids.get(&(module.clone(), statement.clone())) {
        Some(&id) => {
            anchors.entries[id as usize].2 = line;
            id
        }
        None => {
            let id = anchors.entries.len() as u32;
            anchors
                .entries
                .push((module.clone(), statement.clone(), line));
            anchors.ids.insert((module, statement), id);
            id
        }
    };
    qjs::JS_NewFloat64(f64::from(id))
}

/// Starts a component use written at `site`: pending until a host node
/// takes it or the use returns. Returns what to restore (`None`: an
/// unstamped use changes nothing).
pub(crate) fn enter_use(cx: *mut JsContext, site: Option<Stamp>) -> Option<Option<Stamp>> {
    site.map(|site| dev(cx).pending.replace(Some(site)))
}

/// Ends a component use: what was pending before it is again.
pub(crate) fn leave_use(cx: *mut JsContext, before: Option<Option<Stamp>>) {
    if let Some(before) = before {
        dev(cx).pending.set(before);
    }
}

/// The pending call site, taken (a native component's host node).
pub(crate) fn take_pending(cx: *mut JsContext) -> Option<Stamp> {
    dev(cx).pending.take()
}

/// The instance a node made now belongs to (0: none).
pub(crate) fn current_instance(cx: *mut JsContext) -> u32 {
    let d = dev(cx);
    if d.last_instance.get() == 0 {
        return 0;
    }
    let Some(value) = crate::reactive::lookup(cx, d.key.get()) else {
        return 0;
    };
    let id = number(cx, value).unwrap_or(0.0);
    // SAFETY: an owned value.
    unsafe { qjs::JS_FreeValue(cx, value) };
    id as u32
}

/// Records an instance of the hot component `key` rendered with `props`
/// (borrowed), used at the pending call site.
pub(crate) fn begin_instance(cx: *mut JsContext, key: &str, props: JsValue) -> u32 {
    let parent = current_instance(cx);
    let d = dev(cx);
    // SAFETY: a type check of an owned value.
    if unsafe { qjs::JS_IsUndefined(d.key.get()) } {
        // SAFETY: a new object, owned by the state.
        d.key.set(unsafe { qjs::JS_NewObject(cx) });
    }
    let id = d.last_instance.get() + 1;
    d.last_instance.set(id);
    d.instances.borrow_mut().insert(
        id,
        Instance {
            key: key.to_owned(),
            // Taken: the component's body is not at its use's site.
            call_site: d.pending.take(),
            parent,
            // SAFETY: kept until the instance goes.
            props: unsafe { qjs::JS_DupValue(cx, props) },
        },
    );
    id
}

/// Provides instance `id` to what renders under the current owner.
pub(crate) fn provide_instance(cx: *mut JsContext, id: u32) {
    crate::reactive::provide(cx, dev(cx).key.get(), qjs::JS_NewFloat64(f64::from(id)));
}

/// Forgets instance `id` (its owner went).
pub(crate) fn end_instance(cx: *mut JsContext, id: u32) {
    let removed = dev(cx).instances.borrow_mut().remove(&id);
    if let Some(instance) = removed {
        // SAFETY: a value it owned.
        unsafe { qjs::JS_FreeValue(cx, instance.props) };
    }
}

fn resolve(cx: *mut JsContext, stamp: Stamp) -> Option<NodeSource> {
    let anchors = dev(cx).anchors.borrow();
    let (module, anchor, base) = anchors.entries.get(stamp.anchor as usize)?;
    Some(NodeSource {
        module: module.clone(),
        anchor: anchor.clone(),
        line: (i64::from(*base) + i64::from(stamp.delta)).max(1) as u32,
        column: stamp.column,
    })
}

/// Where node `id` was written; `None` for text nodes, nodes made natively
/// and unknown ids.
pub(crate) fn node_source(cx: *mut JsContext, id: NodeId) -> Option<NodeSource> {
    resolve(cx, crate::dom::node_stamp(cx, id)?)
}

/// The instances that rendered node `id`, innermost first.
pub(crate) fn node_instances(cx: *mut JsContext, id: NodeId) -> Vec<InstanceRef> {
    let mut out = Vec::new();
    let mut next = crate::dom::node_instance(cx, id);
    while next != 0 && out.len() < 1024 {
        let found = dev(cx)
            .instances
            .borrow()
            .get(&next)
            .map(|i| (i.key.clone(), i.call_site, i.parent));
        let Some((key, site, parent)) = found else {
            break;
        };
        out.push(InstanceRef {
            id: next,
            key,
            call_site: site.and_then(|s| resolve(cx, s)),
        });
        next = parent;
    }
    out
}

/// The props instance `id` was rendered with, by name, read from their
/// descriptors: a getter is reported, never called.
pub(crate) fn instance_props(cx: *mut JsContext, id: u32) -> Vec<(String, PropValue)> {
    let props = match dev(cx).instances.borrow().get(&id) {
        // SAFETY: kept alive while read.
        Some(i) => unsafe { qjs::JS_DupValue(cx, i.props) },
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    // SAFETY: an enumeration of a live object; every value read is freed.
    unsafe {
        if qjs::JS_IsObject(props) {
            let mut tab: *mut qjs::JSPropertyEnum = std::ptr::null_mut();
            let mut len = 0u32;
            if qjs::JS_GetOwnPropertyNames(
                cx,
                &mut tab,
                &mut len,
                props,
                qjs::JS_GPN_STRING_MASK as i32,
            ) >= 0
            {
                for i in 0..len as usize {
                    let atom = (*tab.add(i)).atom;
                    let name = qjs::JS_AtomToCStringLen(cx, std::ptr::null_mut(), atom);
                    if name.is_null() {
                        continue;
                    }
                    let key = std::ffi::CStr::from_ptr(name)
                        .to_string_lossy()
                        .into_owned();
                    qjs::JS_FreeCString(cx, name);
                    let mut desc = qjs::JSPropertyDescriptor {
                        flags: 0,
                        value: UNDEFINED,
                        getter: UNDEFINED,
                        setter: UNDEFINED,
                    };
                    if qjs::JS_GetOwnProperty(cx, &mut desc, props, atom) <= 0 {
                        continue;
                    }
                    let value = if desc.flags & qjs::JS_PROP_GETSET as std::ffi::c_int != 0 {
                        PropValue::Getter
                    } else {
                        describe(cx, desc.value)
                    };
                    qjs::JS_FreeValue(cx, desc.value);
                    qjs::JS_FreeValue(cx, desc.getter);
                    qjs::JS_FreeValue(cx, desc.setter);
                    out.push((key, value));
                }
                qjs::JS_FreePropertyEnum(cx, tab, len);
            }
        }
        qjs::JS_FreeValue(cx, props);
    }
    out
}

/// A value as a [`PropValue`], read without running anything.
fn describe(cx: *mut JsContext, v: JsValue) -> PropValue {
    // SAFETY: a borrowed value; only type checks and primitive reads.
    unsafe {
        if qjs::JS_IsUndefined(v) {
            PropValue::Undefined
        } else if qjs::JS_IsNull(v) {
            PropValue::Null
        } else if qjs::JS_IsBool(v) {
            PropValue::Bool(qjs::JS_ToBool(cx, v) == 1)
        } else if qjs::JS_IsNumber(v) {
            PropValue::Number(number(cx, v).unwrap_or(f64::NAN))
        } else if qjs::JS_IsString(v) {
            PropValue::Text(crate::dom::string_arg(cx, v).unwrap_or_default())
        } else if qjs::JS_IsFunction(cx, v) {
            PropValue::Function
        } else {
            PropValue::Object
        }
    }
}

#[cfg(test)]
mod test_support {
    use nsh_core::NodeId;
    use rquickjs::qjs;

    use crate::Host;

    /// The id of the node in global `name`, if it holds one.
    pub(super) fn global_node(host: &Host, name: &str) -> Option<NodeId> {
        host.with_raw(|cx| {
            let name = std::ffi::CString::new(name).unwrap();
            // SAFETY: a live context; the values read are freed.
            unsafe {
                let global = qjs::JS_GetGlobalObject(cx);
                let v = qjs::JS_GetPropertyStr(cx, global, name.as_ptr());
                let id = crate::dom::node_id(cx, v);
                qjs::JS_FreeValue(cx, v);
                qjs::JS_FreeValue(cx, global);
                id
            }
        })
    }
}

/// Stamps per node and `node_source`, through a hot update.
#[cfg(test)]
mod stamp_tests {
    use super::{NodeSource, test_support::global_node};
    use crate::Host;

    fn source(module: &str, anchor: &str, line: u32, column: u32) -> Option<NodeSource> {
        Some(NodeSource {
            module: module.to_owned(),
            anchor: anchor.to_owned(),
            line,
            column,
        })
    }

    #[test]
    fn stamped_elements_say_where_they_were_written() {
        let host = Host::new().unwrap();
        host.eval_source(
            r#"
            import { element, text, anchor } from "nsh:rt";
            const a0 = anchor("src/App.tsx", "App", 3);
            const a1 = anchor("src/App.tsx", "Footer", 12);
            globalThis.view = element("view", a0, 2, 7);
            globalThis.footer = element("text", a1, 0, 30);
            globalThis.label = text("hi");
            globalThis.plain = element("view");
            "#,
        )
        .unwrap();
        let id = |name| global_node(&host, name).unwrap();
        assert_eq!(
            host.node_source(id("view")),
            source("src/App.tsx", "App", 5, 7)
        );
        assert_eq!(
            host.node_source(id("footer")),
            source("src/App.tsx", "Footer", 12, 30)
        );
        // Text nodes and nodes made without a stamp say nothing.
        assert_eq!(host.node_source(id("label")), None);
        assert_eq!(host.node_source(id("plain")), None);
        assert_eq!(host.node_source(nsh_core::NodeId::MAX - 1), None);
    }

    #[test]
    fn an_anchor_run_again_moves_its_nodes() {
        let host = Host::new().unwrap();
        host.eval_source(
            r#"
            import { element, anchor } from "nsh:rt";
            globalThis.anchor = anchor;
            const a0 = anchor("m", "List", 4);
            globalThis.row = element("view", a0, 3, 9);
            "#,
        )
        .unwrap();
        let row = global_node(&host, "row").unwrap();
        assert_eq!(host.node_source(row), source("m", "List", 7, 9));
        // The module runs again with its statement two lines lower: same anchor.
        host.eval_script("again.js", r#"anchor("m", "List", 6) === 0"#)
            .unwrap();
        assert_eq!(host.node_source(row), source("m", "List", 9, 9));
    }

    /// A hot update that edits one component: the other component's live
    /// nodes, which it does not render again, report their new lines.
    #[test]
    fn untouched_components_report_their_new_lines_after_a_hot_update() {
        let host = Host::new().unwrap();
        host.eval_source(
            r#"
            import "nsh:hot";
            import { mount } from "nsh";
            import { element, insert, anchor, component } from "nsh:rt";
            // The module as the compiler writes it, `Header` growing by `grow`
            // lines and pushing `Footer` and `App` down.
            const run = (label, grow) => (module, exports, require) => {
                const a0 = anchor("main", "Header", 3);
                const a1 = anchor("main", "Footer", 10 + grow);
                const a2 = anchor("main", "App", 14 + grow);
                const Header = __nsh_hot.component("main", "Header", () => {
                    const v = element("text", a0, 1, 10);
                    insert(v, label);
                    globalThis.header = v;
                    return v;
                }, "header-" + label);
                const Footer = __nsh_hot.component("main", "Footer", () => {
                    const v = element("view", a1, 1, 10);
                    globalThis.footer = v;
                    return v;
                }, "footer");
                const App = __nsh_hot.component("main", "App", () => {
                    const v = element("view", a2, 1, 10);
                    insert(v, [component(Header, {}, a2, 2, 5), component(Footer, {}, a2, 3, 5)]);
                    return v;
                }, "app");
                mount(App);
            };
            globalThis.run = run;
            __nsh_hot.define("main", run("one", 0), false);
            __nsh_hot.start("main", {});
            "#,
        )
        .unwrap();
        let footer = global_node(&host, "footer").unwrap();
        assert_eq!(host.node_source(footer), source("main", "Footer", 11, 10));
        let header = global_node(&host, "header").unwrap();
        assert_eq!(host.node_source(header), source("main", "Header", 4, 10));
        host.eval_source(r#"__nsh_hot.update({ main: run("two", 3) });"#)
            .unwrap();
        // The header rendered again; the footer's node is the same one.
        let new_header = global_node(&host, "header").unwrap();
        assert_ne!(new_header, header);
        assert_eq!(global_node(&host, "footer"), Some(footer));
        assert_eq!(host.node_source(footer), source("main", "Footer", 14, 10));
        assert_eq!(
            host.node_source(new_header),
            source("main", "Header", 4, 10)
        );
    }
}

/// Instances of hot components, their call sites, props and cleanup.
#[cfg(test)]
mod instance_tests {
    use super::{InstanceRef, NodeSource, PropValue, test_support::global_node};
    use crate::Host;

    const MODULE: &str = r#"
        import "nsh:hot";
        import { mount, signal } from "nsh";
        import { element, insert, anchor, component, cond, list } from "nsh:rt";
        globalThis.renders = 0;
        globalThis.shown = signal(true);
        const run = (module, exports, require) => {
            const a0 = anchor("main", "Card", 2);
            const a1 = anchor("main", "App", 9);
            const Card = __nsh_hot.component("main", "Card", (props) => {
                const v = element("view", a0, 1, 10);
                globalThis.card = v;
                // A nested, anonymous component folds into Card.
                insert(v, component((p) => {
                    const inner = element("text");
                    globalThis.inner = inner;
                    return inner;
                }, {}));
                return v;
            }, "card");
            const App = __nsh_hot.component("main", "App", () => {
                const v = element("view", a1, 1, 10);
                globalThis.app = v;
                insert(v, cond(() => shown(), () => component(Card, {
                    title: "Hello",
                    count: 3,
                    on: true,
                    none: null,
                    onPress() {},
                    get children() { globalThis.renders++; return element("text"); },
                }, a1, 2, 7)));
                insert(v, component(list, { each: [], children: () => element("view") }, a1, 4, 7));
                return v;
            }, "app");
            mount(App);
        };
        __nsh_hot.define("main", run, false);
        __nsh_hot.start("main", {});
    "#;

    fn site(line: u32, column: u32) -> Option<NodeSource> {
        Some(NodeSource {
            module: "main".to_owned(),
            anchor: "App".to_owned(),
            line,
            column,
        })
    }

    #[test]
    fn nodes_name_the_instances_that_rendered_them_innermost_first() {
        let host = Host::new().unwrap();
        host.eval_source(MODULE).unwrap();
        let card = global_node(&host, "card").unwrap();
        let chain: Vec<(String, Option<NodeSource>)> = host
            .node_instances(card)
            .into_iter()
            .map(|i: InstanceRef| (i.key, i.call_site))
            .collect();
        assert_eq!(
            chain,
            [
                ("main:Card".to_owned(), site(11, 7)),
                ("main:App".to_owned(), None),
            ]
        );
        // The nested component's node belongs to Card's instance.
        let inner = global_node(&host, "inner").unwrap();
        assert_eq!(host.node_instances(inner)[0].key, "main:Card");
        let app = global_node(&host, "app").unwrap();
        assert_eq!(host.node_instances(app).len(), 1);
    }

    #[test]
    fn a_native_components_host_node_takes_its_call_site() {
        let host = Host::new().unwrap();
        host.eval_source(MODULE).unwrap();
        let mut ops = Vec::new();
        host.take_ops(&mut ops);
        let list = ops
            .iter()
            .find_map(|op| match op {
                nsh_core::Op::Create {
                    id,
                    tag: nsh_core::Tag::List,
                } => Some(*id),
                _ => None,
            })
            .unwrap();
        assert_eq!(host.node_source(list), site(13, 7));
    }

    #[test]
    fn props_are_read_without_calling_getters() {
        let host = Host::new().unwrap();
        host.eval_source(MODULE).unwrap();
        let card = global_node(&host, "card").unwrap();
        let instance = host.node_instances(card)[0].id;
        let renders = |host: &Host| host.eval_script("r.js", "renders === 0").unwrap();
        assert!(renders(&host));
        let props = host.instance_props(instance);
        assert_eq!(
            props,
            [
                ("title".to_owned(), PropValue::Text("Hello".to_owned())),
                ("count".to_owned(), PropValue::Number(3.0)),
                ("on".to_owned(), PropValue::Bool(true)),
                ("none".to_owned(), PropValue::Null),
                ("onPress".to_owned(), PropValue::Function),
                ("children".to_owned(), PropValue::Getter),
            ]
        );
        // Listing them ran no getter.
        assert!(renders(&host));
    }

    /// Only a native component's host node takes its use's site: not the
    /// placeholders of rows rendered later, nor what the body of a hot
    /// component makes, nor a second native component used without a stamp.
    #[test]
    fn only_host_nodes_take_a_use_s_call_site() {
        let host = Host::new().unwrap();
        host.set_lazy_rows(true);
        host.eval_source(
            r#"
            import "nsh:hot";
            import { mount } from "nsh";
            import { element, insert, anchor, component, each, list } from "nsh:rt";
            const run = (module, exports, require) => {
                const a0 = anchor("main", "Rows", 2);
                const a1 = anchor("main", "App", 20);
                const Rows = __nsh_hot.component("main", "Rows", () => {
                    const v = element("view", a0, 1, 10);
                    const items = Array.from({ length: 200 }, (_, i) => i);
                    insert(v, each(() => items, (i) => element("view")));
                    insert(v, component(list, { each: [], children: () => element("view") }, a0, 2, 5));
                    insert(v, component(list, { each: [], children: () => element("view") }, a0, 3, 5));
                    insert(v, component(list, { each: [], children: () => element("view") }));
                    return v;
                }, "rows");
                const App = __nsh_hot.component("main", "App", () => {
                    const v = element("view", a1, 1, 10);
                    insert(v, component(Rows, {}, a1, 2, 7));
                    return v;
                }, "app");
                mount(App);
            };
            __nsh_hot.define("main", run, false);
            __nsh_hot.start("main", {});
            "#,
        )
        .unwrap();
        let mut ops = Vec::new();
        host.take_ops(&mut ops);
        let placeholders: Vec<nsh_core::NodeId> = ops
            .iter()
            .filter_map(|op| match op {
                nsh_core::Op::Placeholder { id } => Some(*id),
                _ => None,
            })
            .collect();
        assert!(!placeholders.is_empty(), "lazy rows made placeholders");
        for id in placeholders {
            assert_eq!(host.node_source(id), None, "placeholder {id}");
        }
        let lists: Vec<nsh_core::NodeId> = ops
            .iter()
            .filter_map(|op| match op {
                nsh_core::Op::Create {
                    id,
                    tag: nsh_core::Tag::List,
                } => Some(*id),
                _ => None,
            })
            .collect();
        let rows = |line| {
            Some(NodeSource {
                module: "main".to_owned(),
                anchor: "Rows".to_owned(),
                line,
                column: 5,
            })
        };
        assert_eq!(lists.len(), 3);
        assert_eq!(host.node_source(lists[0]), rows(4));
        assert_eq!(host.node_source(lists[1]), rows(5));
        // Unstamped: neither the hot component's use nor a sibling's site.
        assert_eq!(host.node_source(lists[2]), None);
    }

    #[test]
    fn an_instance_goes_with_its_owner() {
        let host = Host::new().unwrap();
        host.eval_source(MODULE).unwrap();
        let card = global_node(&host, "card").unwrap();
        let instance = host.node_instances(card)[0].id;
        assert!(!host.instance_props(instance).is_empty());
        host.eval_script("hide.js", "shown.set(false)").unwrap();
        assert!(host.instance_props(instance).is_empty());
    }
}
