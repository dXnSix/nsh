//! `nsh/design`: an app shown in a design tool, which draws the app's
//! windows in its own and drives them (`docs/dev/design-mode.md`). The
//! tool starts the app with `NSH_DESIGN_SOCKET` set to where it listens: a
//! Unix socket's path, or `tcp:<host>:<port>` (any platform). The
//! platform's design host calls [`connect`] at start; this module keeps the
//! connection, one JSON object a line both ways:
//!
//! - `{"hello":{}}` once connected; then for each window of the app's own
//!   (`Window.open`, named by its `id`) a `{"surface":{"id":…,"width":…,
//!   "height":…, <handle>}}` line whenever it changes, where the handle is
//!   what the tool draws the window with, the platform's
//!   ([`DesignWindow::handle`]: `"context"`, the Core Animation context, on
//!   macOS; `"hwnd"` on Windows; `"xid"`, the X11 window, on Linux), and
//!   `{"surface":{"id":…,"gone":true}}` when it closed ([`report`]).
//! - The tool's `pointer`, `wheel`, `key` and `resize` messages are the
//!   platform's input, delivered to that window ([`INPUT`]); every other
//!   message is the app's, taken with `design.next()`, and what the app
//!   sends (`design.send()`) goes to the tool.
//! - When the connection closes, the tool has gone: the platform stops the
//!   app.
//!
//! In a dev build (feature `inspect`) the module also reads what the
//! windows show for the tool: `design.layers()`, the live tree under each
//! view whose test id starts with a prefix the tool chose, with the
//! component instances that rendered it; `design.detail()`, one layer in
//! full; and `design.setOverrides()`, classes and states put on every
//! element written at a place in the source, again whenever it is made
//! anew. Without the hooks they answer nothing.
//!
//! One design host per process: a design tool starts the app for itself.

use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    sync::{Mutex, OnceLock},
    task::{Poll, Waker},
    time::Duration,
};

use crate::{
    JsonValue,
    element::{JsError, module},
};

/// The variable naming where the tool listens.
pub const SOCKET: &str = "NSH_DESIGN_SOCKET";

/// The tool's messages that are a window's input, which the platform
/// delivers: `{"pointer":{"id","kind":"down|drag|up|move","x","y"}}` (points
/// from the window's content's top left), `{"wheel":{"id","x","y","dx",
/// "dy"}}` (content moves up for a positive `dy`), `{"key":{"id","down",
/// "key","shift","control","alt","meta","repeat"}}` (a key as the DOM names
/// it) and `{"resize":{"id","width","height"}}`.
pub const INPUT: [&str; 4] = ["pointer", "wheel", "key", "resize"];

/// What the platform does for the connection.
pub struct DesignHooks {
    /// Takes a message of [`INPUT`], from the connection's thread, to
    /// deliver on the platform's main thread in order.
    pub input: Box<dyn Fn(JsonValue) + Send + Sync>,
    /// The tool's connection closed: stop the app (and the dev session
    /// that runs it).
    pub closed: Box<dyn FnOnce() + Send>,
}

/// The connection's writing half.
trait Line: Write + Send {}
impl<T: Write + Send> Line for T {}

static LINE: Mutex<Option<Box<dyn Line>>> = Mutex::new(None);
static ATTACHED: OnceLock<()> = OnceLock::new();

/// The tool's messages the app has not taken yet, and the `next()` calls
/// waiting for one.
struct Inbox {
    messages: VecDeque<JsonValue>,
    waiting: Vec<Waker>,
}

static INBOX: Mutex<Inbox> = Mutex::new(Inbox {
    messages: VecDeque::new(),
    waiting: Vec::new(),
});

/// Where the tool listens ([`SOCKET`]), if this process was started as a
/// design host.
pub fn endpoint() -> Option<String> {
    std::env::var(SOCKET).ok().filter(|e| !e.is_empty())
}

/// Makes this process a design host connected to the tool at `endpoint`
/// ([`endpoint`]): a thread connects (the tool may start listening after
/// the app starts), says hello and hands on the tool's messages. Called
/// once, by the platform, before the app starts; `false` when it was
/// called before.
pub fn connect(endpoint: String, hooks: DesignHooks) -> bool {
    if ATTACHED.set(()).is_err() {
        return false;
    }
    std::thread::Builder::new()
        .name("nsh-design".to_owned())
        .spawn(move || {
            let reader = loop {
                match open(&endpoint) {
                    Ok((reader, writer)) => {
                        *LINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(writer);
                        break reader;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(200)),
                }
            };
            log::info!(target: "nsh::design", "connected to the design tool");
            send_line(&serde_json::json!({ "hello": {} }));
            for line in reader.lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<JsonValue>(&line) {
                    Ok(message) if is_input(&message) => (hooks.input)(message),
                    Ok(message) => deliver(message),
                    Err(_) => {
                        log::warn!(target: "nsh::design", "the design tool sent a line that is not JSON")
                    }
                }
            }
            log::info!(target: "nsh::design", "the design tool closed its connection; stopping");
            *LINE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            (hooks.closed)();
        })
        .is_ok()
}

/// A connection to `endpoint`: its reading and writing halves.
fn open(endpoint: &str) -> std::io::Result<(Box<dyn BufRead + Send>, Box<dyn Line>)> {
    if let Some(address) = endpoint.strip_prefix("tcp:") {
        let stream = std::net::TcpStream::connect(address)?;
        let _ = stream.set_nodelay(true);
        let writer = stream.try_clone()?;
        return Ok((Box::new(BufReader::new(stream)), Box::new(writer)));
    }
    #[cfg(unix)]
    {
        let stream = std::os::unix::net::UnixStream::connect(endpoint)?;
        let writer = stream.try_clone()?;
        Ok((Box::new(BufReader::new(stream)), Box::new(writer)))
    }
    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "NSH_DESIGN_SOCKET: a Unix socket's path; name tcp:<host>:<port> here",
        ))
    }
}

/// Whether this process is a design host ([`connect`]).
pub fn attached() -> bool {
    ATTACHED.get().is_some()
}

/// Whether `message` is a window's input ([`INPUT`]).
pub fn is_input(message: &JsonValue) -> bool {
    INPUT.iter().any(|kind| message.get(kind).is_some())
}

fn send_line(message: &JsonValue) {
    if let Some(line) = LINE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = line.write_all(format!("{message}\n").as_bytes());
        let _ = line.flush();
    }
}

/// Hands the app a message of the tool's, from any thread.
pub fn deliver(message: JsonValue) {
    let waiting = {
        let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
        inbox.messages.push_back(message);
        std::mem::take(&mut inbox.waiting)
    };
    for waker in waiting {
        waker.wake();
    }
}

/// A window of the app's own as the tool sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignWindow {
    /// Its `id` (`Window.open`).
    pub id: String,
    /// What the tool draws it with: `("context", <CAContext id>)` on macOS,
    /// `("hwnd", <HWND>)` on Windows, `("xid", <X11 window>)` on Linux.
    pub handle: (&'static str, u64),
    /// Its content's size.
    pub size: crate::geometry::Size<crate::geometry::Points>,
}

static TOLD: Mutex<Option<HashMap<String, DesignWindow>>> = Mutex::new(None);

/// The app's windows now (shown, with a handle the tool can draw): tells
/// the tool of those new or changed since the last call, and of those gone.
/// The platform calls it on a timer.
pub fn report(windows: &[DesignWindow]) {
    let mut lines = Vec::new();
    {
        let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
        let told = told.get_or_insert_with(HashMap::new);
        for window in windows {
            if told.get(&window.id) != Some(window) {
                told.insert(window.id.clone(), window.clone());
                let mut surface = serde_json::json!({
                    "id": window.id,
                    "width": f64::from(window.size.width.0),
                    "height": f64::from(window.size.height.0),
                });
                surface[window.handle.0] = window.handle.1.into();
                lines.push(serde_json::json!({ "surface": surface }));
            }
        }
        let gone: Vec<String> = told
            .keys()
            .filter(|id| !windows.iter().any(|w| &w.id == *id))
            .cloned()
            .collect();
        for id in gone {
            told.remove(&id);
            lines.push(serde_json::json!({ "surface": { "id": id, "gone": true } }));
        }
    }
    for line in &lines {
        send_line(line);
    }
}

/// The app's side of a design tool's connection.
#[module]
pub(crate) struct Design;

#[module(name = "design")]
impl Design {
    /// Whether a design tool shows this app: the app was started as its
    /// design host (`NSH_DESIGN_SOCKET`). Without one, `next()` never
    /// resolves and `send()` does nothing.
    pub(crate) fn active() -> bool {
        attached()
    }

    /// The tool's next message, an object: the first one not taken yet, or
    /// the next to come. Messages arrive in the order the tool sent them.
    pub(crate) async fn next() -> JsonValue {
        std::future::poll_fn(|cx| {
            let mut inbox = INBOX.lock().unwrap_or_else(|e| e.into_inner());
            match inbox.messages.pop_front() {
                Some(message) => Poll::Ready(message),
                None => {
                    if !inbox.waiting.iter().any(|w| w.will_wake(cx.waker())) {
                        inbox.waiting.push(cx.waker().clone());
                    }
                    Poll::Pending
                }
            }
        })
        .await
    }

    /// Sends `message`, an object, to the tool.
    pub(crate) fn send(message: JsonValue) -> Result<(), JsError> {
        if !message.is_object() {
            return Err(JsError::Type("nsh/design: send takes an object".to_owned()));
        }
        send_line(&message);
        Ok(())
    }

    /// What changed under each view whose test id starts with `prefix`
    /// (a root), since the last call: `{ roots: { <the rest of its test
    /// id>: { nodes: [...] } }, gone: [<roots no longer there>] }`, or `null`
    /// when nothing changed or the build has no dev hooks. All roots again
    /// after `setOverrides`. A node is `{ id, parent, kind, name, class,
    /// testId, text, x, y, width, height, source, hidden, overridden }`; a
    /// component instance (`kind: "component"`, `{ id, parent, name, key,
    /// source }`) sits between the nodes it rendered and their parent, its
    /// id above every node's ([`layers::INSTANCE_IDS`]). Instances whose
    /// key contains `hide` (the tool's own components) are left out. What a
    /// Rust component draws inside its host is not listed.
    pub(crate) async fn layers(prefix: String, hide: Option<String>) -> JsonValue {
        layers::layers(&prefix, hide.as_deref()).await
    }

    /// Layer `id` in full: a node's style, box, states, events, role,
    /// label and source, or a component instance's props (`{ props: [{
    /// name, value: { kind, value } }] }`); `null` when it is gone or the
    /// build has no dev hooks.
    pub(crate) async fn detail(id: f64) -> JsonValue {
        layers::detail(id).await
    }

    /// The overrides: `{ "<module>:<line>:<column>": { class, replace,
    /// states } }`. Every element written at one of those places gets its
    /// classes (over its own, or in their place with `replace`) and states
    /// (a `DevOverride`'s bits), and gets them again when it is made anew;
    /// the others none. Applied by the next `layers()`.
    pub(crate) fn set_overrides(overrides: JsonValue) {
        layers::set_overrides(&overrides);
    }
}

pub(crate) mod layers {
    #[cfg(not(feature = "inspect"))]
    use crate::JsonValue;

    /// Where component instances' layer ids start: above every node id
    /// (`u32`).
    #[cfg(feature = "inspect")]
    pub(crate) const INSTANCE_IDS: u64 = 1 << 32;

    #[cfg(not(feature = "inspect"))]
    pub(crate) async fn layers(_prefix: &str, _hide: Option<&str>) -> JsonValue {
        JsonValue::Null
    }

    #[cfg(not(feature = "inspect"))]
    pub(crate) async fn detail(_id: f64) -> JsonValue {
        JsonValue::Null
    }

    #[cfg(not(feature = "inspect"))]
    pub(crate) fn set_overrides(_overrides: &JsonValue) {}

    #[cfg(feature = "inspect")]
    pub(crate) use shown::{detail, layers, set_overrides};

    #[cfg(feature = "inspect")]
    mod shown {
        use std::{
            cell::RefCell,
            collections::{HashMap, HashSet},
            hash::{Hash, Hasher},
        };

        use serde_json::{Map, Value, json};

        use super::INSTANCE_IDS;
        use crate::{
            NodeId,
            element::{
                dev_instance_props, dev_instances, dev_node, dev_override, dev_source, dev_tree,
            },
            inspect::{DevDetail, DevInstance, DevNode, DevOverride, DevProp, DevSource},
        };

        /// An override as the tool asks for it.
        #[derive(Clone, Debug, PartialEq)]
        struct Layer {
            class: String,
            replace: bool,
            states: u16,
        }

        #[derive(Default)]
        struct State {
            /// Each root's fingerprint, as last reported.
            seen: HashMap<String, u64>,
            /// The overrides asked for, by where the element was written.
            wanted: HashMap<String, Layer>,
            /// The override on each node now.
            applied: HashMap<NodeId, Layer>,
            /// The overrides changed: every root is looked at again.
            dirty: bool,
        }

        thread_local! {
            static STATE: RefCell<State> = RefCell::default();
        }

        /// What the core says of a node besides the tree.
        struct Facts {
            source: Option<DevSource>,
            instances: Vec<DevInstance>,
        }

        pub(crate) fn set_overrides(overrides: &Value) {
            let wanted = overrides
                .as_object()
                .into_iter()
                .flatten()
                .map(|(key, value)| {
                    let layer = Layer {
                        class: value["class"].as_str().unwrap_or("").to_owned(),
                        replace: value["replace"].as_bool().unwrap_or(false),
                        states: value["states"].as_u64().unwrap_or(0) as u16,
                    };
                    (key.clone(), layer)
                })
                .collect();
            STATE.with_borrow_mut(|s| {
                s.wanted = wanted;
                s.dirty = true;
            });
        }

        pub(crate) async fn layers(prefix: &str, hide: Option<&str>) -> Value {
            let tree = dev_tree().await;
            let by_id: HashMap<NodeId, &DevNode> = tree.nodes.iter().map(|n| (n.id, n)).collect();
            let dirty = STATE.with_borrow_mut(|s| std::mem::take(&mut s.dirty));
            let mut roots = Map::new();
            let mut present = HashSet::new();
            for node in &tree.nodes {
                let Some(root) = node.test_id.as_deref().and_then(|t| t.strip_prefix(prefix))
                else {
                    continue;
                };
                present.insert(root.to_owned());
                let nodes = below(node, &by_id);
                let print = fingerprint(&nodes);
                let unchanged = STATE.with_borrow(|s| s.seen.get(root) == Some(&print));
                if unchanged && !dirty {
                    continue;
                }
                STATE.with_borrow_mut(|s| s.seen.insert(root.to_owned(), print));
                let facts = facts(&nodes).await;
                apply(&nodes, &facts).await;
                roots.insert(
                    root.to_owned(),
                    json!({ "nodes": list(&nodes, &facts, hide) }),
                );
            }
            let gone: Vec<String> = STATE.with_borrow_mut(|s| {
                let gone: Vec<String> = s
                    .seen
                    .keys()
                    .filter(|k| !present.contains(*k))
                    .cloned()
                    .collect();
                for key in &gone {
                    s.seen.remove(key);
                }
                gone
            });
            if roots.is_empty() && gone.is_empty() {
                return Value::Null;
            }
            json!({ "roots": roots, "gone": gone })
        }

        pub(crate) async fn detail(id: f64) -> Value {
            if !(id.is_finite() && id >= 0.0) {
                return Value::Null;
            }
            let id = id as u64;
            if id >= INSTANCE_IDS {
                let Ok(instance) = u32::try_from(id - INSTANCE_IDS) else {
                    return Value::Null;
                };
                let props = dev_instance_props(instance).await;
                let props: Vec<Value> = props
                    .iter()
                    .map(|(name, value)| json!({ "name": name, "value": prop(value) }))
                    .collect();
                return json!({ "props": props });
            }
            let Ok(node) = NodeId::try_from(id) else {
                return Value::Null;
            };
            // Both asked before either is awaited: one round trip.
            let (detail, source) = (dev_node(node), dev_source(node));
            let (detail, source) = (detail.await, source.await);
            let Some(detail) = detail else {
                return Value::Null;
            };
            let edges = |e: &crate::inspect::DevEdges| json!({ "top": finite(e.top), "right": finite(e.right), "bottom": finite(e.bottom), "left": finite(e.left) });
            json!({
                "style": style(&detail),
                "width": finite(detail.layout.size.width),
                "height": finite(detail.layout.size.height),
                "padding": edges(&detail.layout.padding),
                "border": edges(&detail.layout.border),
                "margin": edges(&detail.layout.margin),
                "states": detail.states,
                "events": events(&detail),
                "role": role(&detail),
                "label": detail.accessibility.as_ref().and_then(|a| a.label.as_deref()),
                "source": source.as_ref().map(stamp),
            })
        }

        /// The nodes under `root`, in tree order, without `root` and
        /// without what a Rust component draws inside its host.
        fn below<'a>(root: &'a DevNode, by_id: &HashMap<NodeId, &'a DevNode>) -> Vec<&'a DevNode> {
            let mut out = Vec::new();
            let mut stack: Vec<NodeId> = root.children.iter().rev().copied().collect();
            while let Some(id) = stack.pop() {
                let Some(node) = by_id.get(&id) else {
                    continue;
                };
                out.push(*node);
                if node.component.is_none() {
                    stack.extend(node.children.iter().rev().copied());
                }
            }
            out
        }

        /// What the nodes show: their shape, classes, text and frames.
        fn fingerprint(nodes: &[&DevNode]) -> u64 {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for node in nodes {
                node.id.hash(&mut hasher);
                node.kind.hash(&mut hasher);
                node.children.hash(&mut hasher);
                node.class.hash(&mut hasher);
                node.test_id.hash(&mut hasher);
                node.text.hash(&mut hasher);
                let page = node.page.unwrap_or(node.frame.origin);
                for value in [
                    page.x,
                    page.y,
                    node.frame.size.width,
                    node.frame.size.height,
                ] {
                    finite(value).to_bits().hash(&mut hasher);
                }
                (node.flags.hidden || node.flags.culled).hash(&mut hasher);
            }
            hasher.finish()
        }

        /// Every node's source and instances, all asked before any is
        /// awaited: one round trip.
        async fn facts(nodes: &[&DevNode]) -> HashMap<NodeId, Facts> {
            let asked: Vec<_> = nodes
                .iter()
                .map(|node| (node.id, dev_source(node.id), dev_instances(node.id)))
                .collect();
            let mut out = HashMap::with_capacity(asked.len());
            for (id, source, instances) in asked {
                out.insert(
                    id,
                    Facts {
                        source: source.await,
                        instances: instances.await,
                    },
                );
            }
            out
        }

        /// Puts on each node the override its source asks for, and takes
        /// it off the others.
        async fn apply(nodes: &[&DevNode], facts: &HashMap<NodeId, Facts>) {
            let changes: Vec<(NodeId, Option<Layer>)> = STATE.with_borrow_mut(|s| {
                let mut changes = Vec::new();
                for node in nodes {
                    let wanted = facts
                        .get(&node.id)
                        .and_then(|f| f.source.as_ref())
                        .and_then(|source| s.wanted.get(&key(source)))
                        .cloned();
                    // A node made again has none, whatever was put on the
                    // node that had its id.
                    let on_node = if node.flags.overridden {
                        s.applied.get(&node.id).cloned()
                    } else {
                        None
                    };
                    if wanted != on_node {
                        match &wanted {
                            Some(layer) => s.applied.insert(node.id, layer.clone()),
                            None => s.applied.remove(&node.id),
                        };
                        changes.push((node.id, wanted));
                    }
                }
                changes
            });
            let asked: Vec<_> = changes
                .into_iter()
                .map(|(id, layer)| {
                    dev_override(
                        id,
                        layer.map(|l| DevOverride {
                            class: l.class,
                            replace_class: l.replace,
                            states: l.states,
                            ..DevOverride::default()
                        }),
                    )
                })
                .collect();
            for answer in asked {
                answer.await;
            }
        }

        /// The layers, parents before children: each node, and each
        /// component instance before the first node it rendered.
        fn list(
            nodes: &[&DevNode],
            facts: &HashMap<NodeId, Facts>,
            hide: Option<&str>,
        ) -> Vec<Value> {
            let hidden = |key: &str| hide.is_some_and(|h| !h.is_empty() && key.contains(h));
            let inside: HashSet<NodeId> = nodes.iter().map(|n| n.id).collect();
            let chain = |id: NodeId| -> Vec<&DevInstance> {
                facts.get(&id).map_or_else(Vec::new, |f| {
                    f.instances
                        .iter()
                        .rev()
                        .filter(|i| !hidden(&i.key))
                        .collect()
                })
            };
            let wanted: HashSet<String> = STATE.with_borrow(|s| s.wanted.keys().cloned().collect());
            let mut out = Vec::new();
            let mut made = HashSet::new();
            for node in nodes {
                let parent = node.parent.filter(|p| inside.contains(p));
                let mine = chain(node.id);
                let theirs = parent.map(chain).unwrap_or_default();
                let shared = mine
                    .iter()
                    .zip(theirs.iter())
                    .take_while(|(a, b)| a.id == b.id)
                    .count();
                let mut layer_parent = parent.map(u64::from);
                for instance in &mine[shared..] {
                    let pseudo = INSTANCE_IDS + u64::from(instance.id);
                    if made.insert(instance.id) {
                        let source = instance.call_site.as_ref().filter(|s| !hidden(&s.module));
                        out.push(json!({
                            "id": pseudo,
                            "parent": layer_parent,
                            "kind": "component",
                            "name": instance.key.rsplit(':').next().unwrap_or(&instance.key),
                            "key": instance.key,
                            "source": source.map(stamp),
                        }));
                    }
                    layer_parent = Some(pseudo);
                }
                let source = facts.get(&node.id).and_then(|f| f.source.as_ref());
                let page = node.page.unwrap_or(node.frame.origin);
                out.push(json!({
                    "id": u64::from(node.id),
                    "parent": layer_parent,
                    "kind": if node.component.is_some() { "native" } else { node.kind },
                    "name": node.component.as_deref().map_or_else(|| title(node.kind), str::to_owned),
                    "class": node.class,
                    "testId": node.test_id.as_deref(),
                    "text": node.text,
                    "x": finite(page.x),
                    "y": finite(page.y),
                    "width": finite(node.frame.size.width),
                    "height": finite(node.frame.size.height),
                    "source": source.map(stamp),
                    "hidden": node.flags.hidden || node.flags.culled,
                    "overridden": source.is_some_and(|s| wanted.contains(&key(s))),
                }));
            }
            out
        }

        fn key(source: &DevSource) -> String {
            format!("{}:{}:{}", source.module, source.line, source.column)
        }

        fn stamp(source: &DevSource) -> Value {
            json!({ "module": source.module, "line": source.line, "column": source.column })
        }

        /// `view` as `View`, `text-input` as `Text input`.
        fn title(kind: &str) -> String {
            let mut chars = kind.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars)
                    .collect::<String>()
                    .replace('-', " ")
            })
        }

        fn lower_first(name: &str) -> String {
            let mut chars = name.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_lowercase().chain(chars).collect()
            })
        }

        /// A length as JSON carries it: a layout that has not run (a NaN)
        /// is 0.
        fn finite(points: crate::geometry::Points) -> f32 {
            let value = f32::from(points);
            if value.is_finite() { value } else { 0.0 }
        }

        fn style(detail: &DevDetail) -> Vec<Value> {
            detail
                .inline_style
                .iter()
                .map(|declaration| {
                    let text = format!("{declaration:?}");
                    let (name, value) = text.split_once('(').unwrap_or((&text, ""));
                    json!({ "name": lower_first(name), "value": value.trim_end_matches(')') })
                })
                .collect()
        }

        fn events(detail: &DevDetail) -> Vec<String> {
            crate::EventKind::ALL
                .iter()
                .filter(|event| detail.events.contains(**event))
                .map(|event| lower_first(&format!("{event:?}")))
                .collect()
        }

        fn role(detail: &DevDetail) -> Option<String> {
            let role = detail.accessibility.as_ref()?.role;
            let name = format!("{role:?}");
            (name != "None").then(|| lower_first(&name))
        }

        fn prop(value: &DevProp) -> Value {
            match value {
                DevProp::Getter => json!({ "kind": "getter" }),
                DevProp::Function => json!({ "kind": "function" }),
                DevProp::Undefined => json!({ "kind": "undefined" }),
                DevProp::Null => json!({ "kind": "null" }),
                DevProp::Bool(b) => json!({ "kind": "boolean", "value": b }),
                DevProp::Number(n) => json!({ "kind": "number", "value": n }),
                DevProp::Text(t) => json!({ "kind": "string", "value": t }),
                DevProp::Object => json!({ "kind": "object" }),
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        sync::mpsc,
        time::Duration,
    };

    use serde_json::json;

    use super::*;

    /// One test: a process has one design host.
    #[test]
    fn a_design_host_talks_to_its_tool_one_line_at_a_time() {
        let path = std::env::temp_dir().join(format!("nsh-design-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let (input_tx, input) = mpsc::channel();
        let (closed_tx, closed) = mpsc::channel();
        let input_tx = Mutex::new(input_tx);
        assert!(connect(
            path.to_string_lossy().into_owned(),
            DesignHooks {
                input: Box::new(move |m| input_tx.lock().unwrap().send(m).unwrap()),
                closed: Box::new(move || closed_tx.send(()).unwrap()),
            },
        ));
        assert!(attached());
        assert!(!connect(
            String::new(),
            DesignHooks {
                input: Box::new(|_| {}),
                closed: Box::new(|| {}),
            },
        ));
        let (mut tool, _) = listener.accept().unwrap();
        tool.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut lines = BufReader::new(tool.try_clone().unwrap()).lines();
        let mut next_line =
            || -> JsonValue { serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap() };
        assert_eq!(next_line(), json!({ "hello": {} }));

        // Input is the platform's, the rest the app's, in order.
        writeln!(
            tool,
            "{}",
            json!({ "pointer": { "id": "a", "kind": "down", "x": 1, "y": 2 } })
        )
        .unwrap();
        writeln!(tool, "{}", json!({ "open": { "id": "a" } })).unwrap();
        writeln!(tool, "not json").unwrap();
        writeln!(tool, "{}", json!({ "close": { "id": "a" } })).unwrap();
        let got = input.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got["pointer"]["kind"], "down");
        let take = || {
            for _ in 0..500 {
                if let Some(m) = INBOX.lock().unwrap().messages.pop_front() {
                    return m;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("no message");
        };
        assert_eq!(take(), json!({ "open": { "id": "a" } }));
        assert_eq!(take(), json!({ "close": { "id": "a" } }));

        // Surfaces: new or changed ones, then the ones gone.
        let a = DesignWindow {
            id: "a".into(),
            handle: ("context", 7),
            size: crate::geometry::size(
                crate::geometry::Points(300.0),
                crate::geometry::Points(200.0),
            ),
        };
        report(std::slice::from_ref(&a));
        assert_eq!(
            next_line(),
            json!({ "surface": { "id": "a", "context": 7, "width": 300.0, "height": 200.0 } })
        );
        report(std::slice::from_ref(&a));
        let b = DesignWindow {
            size: crate::geometry::size(
                crate::geometry::Points(320.0),
                crate::geometry::Points(200.0),
            ),
            ..a
        };
        report(&[b]);
        assert_eq!(next_line()["surface"]["width"], 320.0);
        report(&[]);
        assert_eq!(
            next_line(),
            json!({ "surface": { "id": "a", "gone": true } })
        );

        // What the app sends goes to the tool.
        Design::send(json!({ "called": { "id": "a" } })).unwrap();
        assert_eq!(next_line(), json!({ "called": { "id": "a" } }));
        assert!(Design::send(json!("text")).is_err());

        // The tool goes: the platform stops the app.
        drop(lines);
        drop(tool);
        closed.recv_timeout(Duration::from_secs(5)).unwrap();
        let _ = std::fs::remove_file(path);
    }
}
