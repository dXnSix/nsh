//! A view that hosts a Rust component is that component to dev tools: in
//! the core's tree (`nsh_core::element::dev_tree`) and in a design tool's
//! layers (`nsh/design`'s `layers` and `detail`). What the component's view
//! draws is not a layer, the components it nests neither; what the app gave
//! it is, under the host: its JSX children, the holder of each named slot
//! it fills and the list of its rows. A host's detail says what its
//! component takes and what the app passes.
//!
//! One hot app (stamped, as a dev session's) shows a board per case; a Rust
//! module stands in for a dev tool reading the tree.

use std::sync::OnceLock;

use nsh_core::{
    ApproxTextEngine, JsonValue, Size,
    element::{Registry, dev_instances, dev_source, dev_tree},
    prelude::*,
};
use nsh_engine::{Bundle, Engine, EngineConfig};
use serde_json::json;

use crate::{ChannelSink, rust_classes::shown};

// --- The tests' own components: one nests another ----------------------------

/// A box around content a view passes it.
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct CaptionProps {
    /// The content.
    pub(crate) children: Children,
}

/// A box ([`CaptionProps`]): [`Frame`]'s view nests one.
#[component]
pub(crate) fn Caption(props: CaptionProps) -> impl IntoElement {
    view().p_2().child(props.children)
}

/// A title, a badge the app passes and the app's content in a caption.
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct FrameProps {
    /// The title.
    pub(crate) title: Prop<SharedString>,
    /// Beside the title.
    pub(crate) badge: Prop<Option<Slot>>,
    /// The content, passed on to a nested [`Caption`].
    pub(crate) children: Children,
}

/// A frame ([`FrameProps`]): its view draws a title and a text of its own,
/// and nests a [`Caption`] that places the app's children.
#[component]
pub(crate) fn Frame(props: FrameProps) -> impl IntoElement {
    view()
        .child(text(props.title))
        .child(props.badge.place(|badge| badge))
        .child(
            Caption::element(CaptionProps {
                children: Children::default(),
            })
            .child(text("drawn"))
            .child(props.children),
        )
}

/// The dev tool: reads the core's tree.
#[module]
pub(crate) struct TreeProbe;

#[module]
impl TreeProbe {
    /// Every node of the tree as JSON: its id, kind, component, whether it
    /// is a host, whose content it is, its test id and text, and whether
    /// the script says where it was written and who rendered it.
    pub(crate) async fn tree() -> String {
        let tree = dev_tree().await;
        let mut nodes = Vec::new();
        for node in &tree.nodes {
            let source = dev_source(node.id).await;
            let instances = dev_instances(node.id).await;
            nodes.push(json!({
                "id": node.id,
                "kind": node.kind,
                "parent": node.parent,
                "component": node.component.as_deref(),
                "host": node.flags.host,
                "given": node.given.as_ref().map(|g| json!({ "host": g.host, "slot": g.slot.as_deref() })),
                "testId": node.test_id.as_deref(),
                "text": node.text,
                "source": source.map(|s| format!("{}:{}", s.anchor, s.line)),
                "instances": instances.len(),
            }));
        }
        JsonValue::Array(nodes).to_string()
    }
}

fn sets() -> Vec<Registry> {
    let mut ui = Registry::ui_set("test/layers");
    ui.register::<Caption>().register::<Frame>();
    let mut probe = Registry::ui_set("dev/tree-probe");
    probe.module::<TreeProbe>();
    vec![nsh_base::registry(), crate::test_ui::registry(), ui, probe]
}

const APP: &str = r#"import { mount, nativeModule, signal, Text, View } from "nsh";
import { design } from "nsh/design";
import { Button, Card, ListView } from "test/ui";
import { Frame } from "test/layers";

const { TreeProbe } = nativeModule<{ TreeProbe: { tree(): Promise<string> } }>("dev/tree-probe");

const shown = signal("waiting");
const off = signal(false);

function App() {
  return <View>
    <View testID="board:empty">
      <Button title="Save" variant="secondary" disabled={off()} onPress={() => {}} />
    </View>
    <View testID="board:children">
      <Card>
        <Text>Body</Text>
        <View testID="second" />
      </Card>
    </View>
    <View testID="board:slot">
      <Card header={<Text>Head</Text>} />
    </View>
    <View testID="board:both">
      <Card header={<Text>Head</Text>} footer={<Button title="OK" />}>
        <Text>Body</Text>
      </Card>
    </View>
    <View testID="board:nested">
      <Frame title="Title" badge={<Text>New</Text>}>
        <Text>Inside</Text>
      </Frame>
    </View>
    <View testID="board:rows" class="h-[200px]">
      <ListView each={["A", "B"]}>{(row) => <Text>{row()}</Text>}</ListView>
    </View>
    <Text>{shown()}</Text>
  </View>;
}

mount(App);
setTimeout(async () => {
  const layers = await design.layers("board:");
  const details = {};
  for (const [root, { nodes }] of Object.entries(layers.roots)) {
    details[root] = await design.detail(nodes.find(n => n.kind === "host").id);
  }
  const text = nodes => nodes.find(n => n.kind === "text");
  const plain = await design.detail(text(layers.roots.children.nodes).id);
  const tree = JSON.parse(await TreeProbe.tree());
  shown.set("read " + JSON.stringify({ layers, details, plain, tree }));
}, 0);
"#;

/// What the app read: `{ layers, details, plain, tree }`. Read once: the
/// tests look at its parts.
fn read() -> &'static JsonValue {
    static READ: OnceLock<JsonValue> = OnceLock::new();
    READ.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("nsh-design-layers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("App.tsx"), APP).unwrap();
        let sets = sets();
        let described: Vec<nsh_core::element::description::SetDescription> =
            Registry::served_sets(&sets)
                .into_iter()
                .map(Into::into)
                .collect();
        let bundle = nsh_bundler::bundle_hot(&dir.join("App.tsx"), &described).unwrap();
        let (code, map) = bundle.script();
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut config = EngineConfig::new(nsh_engine::App::Js(
            Bundle::source(code).with_source_map(map.unwrap()),
        ));
        config.viewport = Size::new(px(390.0), px(844.0));
        config.ui_sets = sets;
        let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
        let text = shown(&rx, "read ");
        drop(engine);
        let _ = std::fs::remove_dir_all(&dir);
        let json = text
            .strip_prefix("read ")
            .unwrap_or_else(|| panic!("the app read nothing: {text}"));
        serde_json::from_str(json).unwrap()
    })
}

/// The layers of board `root`.
fn board(root: &str) -> &'static [JsonValue] {
    read()["layers"]["roots"][root]["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("no board {root}: {}", read()["layers"]))
}

/// The 1-based line and column of `written` in the app, as a stamp has
/// them.
fn place(written: &str) -> (u64, u64) {
    let (index, line) = APP
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(written))
        .unwrap_or_else(|| panic!("the app has no {written}"));
    (index as u64 + 1, line.find(written).unwrap() as u64 + 1)
}

/// The layers of `nodes` as lines a test compares: `kind name [slot] "text"`
/// under its parent's line, indented. Component instances (the app's
/// `App`) are left out; their children take their place.
fn outline(nodes: &[JsonValue]) -> Vec<String> {
    let layer = |id: &JsonValue| nodes.iter().find(|n| &n["id"] == id);
    // The nearest layer above that is a node.
    let above = |node: &JsonValue| {
        let mut parent = layer(&node["parent"]);
        while let Some(p) = parent.filter(|p| p["kind"] == "component") {
            parent = layer(&p["parent"]);
        }
        parent
    };
    nodes
        .iter()
        .filter(|n| n["kind"] != "component")
        .map(|node| {
            let mut depth = 0;
            let mut at = above(node);
            while let Some(p) = at {
                depth += 1;
                at = above(p);
            }
            let mut line = format!(
                "{}{} {}",
                "  ".repeat(depth),
                node["kind"].as_str().unwrap(),
                node["name"].as_str().unwrap()
            );
            if let Some(slot) = node["slot"].as_str() {
                line.push_str(&format!(" [{slot}]"));
            }
            if let Some(text) = node["text"].as_str() {
                line.push_str(&format!(" {text:?}"));
            }
            line
        })
        .collect()
}

#[test]
fn a_host_with_no_content_is_its_component_alone() {
    let nodes = board("empty");
    // The label the button's view draws is not a layer.
    assert_eq!(outline(nodes), ["host Button"]);
    let host = nodes.iter().find(|n| n["kind"] == "host").unwrap();
    // Where the app wrote it, as any element.
    let (line, column) = place("<Button title=\"Save\"");
    assert_eq!(
        host["source"],
        json!({ "module": "App.tsx", "line": line, "column": column })
    );
    assert_eq!(host["slot"], JsonValue::Null);
    assert!(host["width"].as_f64().unwrap() > 0.0, "{host}");
}

#[test]
fn a_host_lists_the_jsx_children_the_app_gave_it() {
    // The padded box the card's view places them in is not a layer.
    assert_eq!(
        outline(board("children")),
        ["host Card", "  text Text \"Body\"", "  view View",]
    );
    let nodes = board("children");
    let second = nodes.iter().find(|n| n["testId"] == "second").unwrap();
    assert_eq!(second["slot"], JsonValue::Null);
    assert_eq!(second["source"]["line"], place("<View testID=\"second\"").0);
}

#[test]
fn a_host_lists_the_holder_of_a_named_slot_the_app_fills() {
    assert_eq!(
        outline(board("slot")),
        [
            "host Card",
            "  view View [header]",
            "    text Text \"Head\"",
        ]
    );
    // The holder is the glue's: written nowhere. The app's text is.
    let nodes = board("slot");
    let holder = nodes.iter().find(|n| n["slot"] == "header").unwrap();
    assert_eq!(holder["source"], JsonValue::Null);
    let head = nodes.iter().find(|n| n["text"] == "Head").unwrap();
    assert_eq!(
        head["source"]["line"],
        place("<Card header={<Text>Head</Text>} />").0
    );
    assert_eq!(head["slot"], JsonValue::Null);
}

#[test]
fn a_host_lists_slots_and_children_where_its_view_placed_them() {
    // The header above the content, the footer below; the footer's button
    // is a host of its own, the app's.
    assert_eq!(
        outline(board("both")),
        [
            "host Card",
            "  view View [header]",
            "    text Text \"Head\"",
            "  text Text \"Body\"",
            "  view View [footer]",
            "    host Button",
        ]
    );
}

#[test]
fn a_component_nested_in_a_view_is_not_a_layer() {
    // The frame's title, its nested caption and the caption's own text are
    // the view's; the app's badge and child are the frame's, though the
    // caption places the child.
    assert_eq!(
        outline(board("nested")),
        [
            "host Frame",
            "  view View [badge]",
            "    text Text \"New\"",
            "  text Text \"Inside\"",
        ]
    );
}

#[test]
fn a_host_s_detail_says_what_its_component_takes_and_gets() {
    let detail = &read()["details"]["empty"];
    let component = &detail["component"];
    assert_eq!(component["name"], "Button");
    // A node's detail as any other's.
    assert!(detail["width"].as_f64().unwrap() > 0.0, "{detail}");
    assert_eq!(detail["source"]["line"], place("<Button title=\"Save\"").0);
    let prop = |name: &str| {
        component["props"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("no prop {name}: {component}"))
    };
    // TypeScript only where the build carries the text of declarations
    // (another package of the build may turn that on).
    let typed = |ts: &JsonValue, wanted: &str| ts.is_null() || ts == wanted;
    // Values as written: a literal is read, an expression is a getter.
    let title = prop("title");
    assert_eq!(title["kind"], "value");
    assert_eq!(title["optional"], false);
    assert!(typed(&title["type"], "string"), "{title}");
    assert_eq!(title["value"], json!({ "kind": "string", "value": "Save" }));
    assert_eq!(
        prop("variant")["value"],
        json!({ "kind": "string", "value": "secondary" })
    );
    assert_eq!(prop("variant")["optional"], true);
    assert_eq!(prop("disabled")["value"], json!({ "kind": "getter" }));
    // What the app leaves out has no value.
    assert_eq!(prop("accessibilityLabel")["value"], JsonValue::Null);
    let events = component["events"].as_array().unwrap();
    assert_eq!(events.len(), 1, "{component}");
    assert_eq!(events[0]["name"], "onPress");
    assert_eq!(events[0]["value"], json!({ "kind": "function" }));
    assert!(typed(&events[0]["type"], "void"), "{component}");

    // Slots and children are props of their kinds.
    let card = &read()["details"]["both"]["component"];
    let kinds: Vec<(&str, &str)> = card["props"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap(), p["kind"].as_str().unwrap()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("header", "slot"),
            ("footer", "slot"),
            ("children", "children")
        ]
    );
    assert!(card["props"][0]["value"].is_object(), "{card}");
    // A slot the app leaves empty.
    let card = &read()["details"]["children"]["component"];
    assert_eq!(card["props"][0]["value"], JsonValue::Null);

    // A node that hosts nothing has no component.
    assert_eq!(read()["plain"]["component"], JsonValue::Null);
    assert!(read()["plain"]["style"].is_array());
}

#[test]
fn the_tree_names_hosts_and_what_the_script_gave_them() {
    let tree = read()["tree"].as_array().unwrap();
    let by_id = |id: &JsonValue| tree.iter().find(|n| &n["id"] == id).unwrap();
    let hosts = |name: &str| -> Vec<&JsonValue> {
        tree.iter().filter(|n| n["component"] == name).collect()
    };
    // Every host is named after its component's export and flagged; its
    // kind stays its node's.
    for (name, count) in [("Button", 2), ("Card", 3), ("Frame", 1), ("Caption", 1)] {
        let found = hosts(name);
        assert_eq!(found.len(), count, "{name}");
        assert!(
            found
                .iter()
                .all(|n| n["host"] == true && n["kind"] == "view")
        );
    }
    // The app's hosts are the script's nodes; the caption a view nests is
    // the core's, and given to nothing.
    let caption = hosts("Caption")[0];
    assert_eq!(caption["id"].as_u64().unwrap() % 2, 1);
    assert_eq!(caption["given"], JsonValue::Null);
    let frame = hosts("Frame")[0];
    assert_eq!(frame["id"].as_u64().unwrap() % 2, 0);

    // What the script gave the frame is the frame's, wherever it is: the
    // badge's holder by its slot, the child (in the nested caption) by none.
    let given: Vec<(Option<&str>, Option<&str>)> = tree
        .iter()
        .filter(|n| n["given"]["host"] == frame["id"])
        .map(|n| (n["given"]["slot"].as_str(), n["text"].as_str()))
        .collect();
    assert_eq!(given, [(Some("badge"), None), (None, Some("Inside"))]);
    let inside = tree.iter().find(|n| n["text"] == "Inside").unwrap();
    assert_eq!(by_id(&inside["parent"])["id"], caption["id"]);

    // The content of a holder is the app's own, given to no one.
    let new = tree.iter().find(|n| n["text"] == "New").unwrap();
    assert_eq!(new["given"], JsonValue::Null);
    // The view's own nodes are given to no one either.
    let drawn = tree.iter().find(|n| n["text"] == "drawn").unwrap();
    assert_eq!(drawn["given"], JsonValue::Null);
    assert_eq!(drawn["host"], false);
}

#[test]
fn the_core_s_nodes_were_written_nowhere() {
    // A node a component's view made (an odd id) is not the script's: it
    // has no place in the source and no instance rendered it, whatever
    // the script's node before it has.
    let tree = read()["tree"].as_array().unwrap();
    let cores: Vec<&JsonValue> = tree
        .iter()
        .filter(|n| n["id"].as_u64().unwrap() % 2 == 1)
        .collect();
    assert!(cores.len() > 5, "{cores:?}");
    for node in cores {
        assert_eq!(node["source"], JsonValue::Null, "{node}");
        assert_eq!(node["instances"], 0, "{node}");
    }
    // The script's nodes keep theirs.
    let frame = tree.iter().find(|n| n["component"] == "Frame").unwrap();
    assert_eq!(frame["source"], format!("App:{}", place("<Frame").0));
    assert_eq!(frame["instances"], 1);
}

#[test]
fn a_component_s_list_lists_the_app_s_rows() {
    // The rows prop's holder is the list; the wrapper the component's row
    // function draws around each row is not a layer.
    assert_eq!(
        outline(board("rows")),
        [
            "host ListView",
            "  list List [items]",
            "    cell Cell",
            "      text Text \"A\"",
            "    cell Cell",
            "      text Text \"B\"",
        ]
    );
    // Rows are passed as a list's: the prop's value is the data.
    let component = &read()["details"]["rows"]["component"];
    assert_eq!(component["props"][0]["name"], "items");
    assert_eq!(component["props"][0]["kind"], "items");
    assert!(component["props"][0]["value"].is_object(), "{component}");
    // A cell hosts the component's row, unnamed.
    let tree = read()["tree"].as_array().unwrap();
    let cells: Vec<&JsonValue> = tree.iter().filter(|n| n["kind"] == "cell").collect();
    assert_eq!(cells.len(), 2);
    assert!(
        cells
            .iter()
            .all(|n| n["host"] == true && n["component"].is_null())
    );
}
