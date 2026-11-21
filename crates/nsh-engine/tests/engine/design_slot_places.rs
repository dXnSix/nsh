//! A slot marker in a design tool's layers: a component whose view calls
//! `design_slot(name)` marks its host as the place of slot `name` of the
//! component whose view drew it. Design mode lists that marker under the
//! component's host, empty or filled, with `place` its slot's name and what
//! the slot holds under it; a marker a nested component draws for its own
//! slot is not the outer component's; a marker the app writes has `place`
//! too. The core's tree says the same (`DevNode::place`).
//!
//! One hot app (stamped, as a dev session's) shows a board per case, read
//! at mount, after the app fills a slot and renames one, and after it
//! empties it again.

use std::sync::OnceLock;

use nsh_core::{
    ApproxTextEngine, JsonValue, Size,
    element::{Registry, dev_tree},
    prelude::*,
};
use nsh_engine::{Bundle, Engine, EngineConfig};
use serde_json::json;

use crate::{ChannelSink, rust_classes::shown};

// --- The tests' own components -----------------------------------------------

/// The props of [`Mark`].
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct MarkProps {
    /// The slot's name.
    pub(crate) name: Prop<SharedString>,
    /// What the slot holds.
    pub(crate) children: Prop<Children>,
}

/// A slot marker: a box of 20 by 20 with its name while empty, around what
/// it holds otherwise.
#[component]
pub(crate) fn Mark(props: MarkProps) -> impl IntoElement {
    design_slot(props.name);
    let empty = move || props.children.get().is_empty();
    view()
        .style_with(move |s| {
            if empty() {
                s.min_w(px(20.0)).min_h(px(20.0))
            } else {
                s
            }
        })
        .children_each(props.children, |child, _| child)
        .child(when(empty, move || text(props.name)))
}

/// The props of [`Chip`], [`Framed`] and [`Outer`].
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct ChipProps {
    /// Its label.
    pub(crate) text: Prop<SharedString>,
    /// Before the label.
    pub(crate) left: Prop<Option<Slot>>,
    /// Makes the slot `left` (off: the view makes none).
    #[prop(default = true)]
    pub(crate) show_left: Prop<bool>,
}

/// Marks its slot `left` with a [`Mark`] in its view, while `showLeft`.
#[component]
pub(crate) fn Chip(props: ChipProps) -> impl IntoElement {
    view()
        .flex_row()
        .child(when(
            move || props.show_left.get(),
            move || mark().name("left").child(props.left.place(|s| s)),
        ))
        .child(text(props.text))
}

/// The props of [`Boxed`].
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct BoxedProps {
    /// What it holds.
    pub(crate) children: Children,
}

/// A padded row of what it holds: the root of [`Framed`].
#[component]
pub(crate) fn Boxed(props: BoxedProps) -> impl IntoElement {
    view().flex_row().p_1().child(props.children)
}

/// A component rooted in another ([`Boxed`]) that marks its slot `left`
/// among the root's children.
#[component]
pub(crate) fn Framed(props: ChipProps) -> impl IntoElement {
    boxed()
        .child(mark().name("left").child(props.left.place(|s| s)))
        .child(text(props.text))
}

/// The props of [`Wrap`].
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct WrapProps {
    /// What it holds.
    pub(crate) children: Children,
}

/// Draws a marker of a slot of its own, and what it holds.
#[component]
pub(crate) fn Wrap(props: WrapProps) -> impl IntoElement {
    view().child(mark().name("inner")).child(props.children)
}

/// Nests a [`Wrap`] in its view and marks its own slot `left`.
#[component]
pub(crate) fn Outer(props: ChipProps) -> impl IntoElement {
    view()
        .child(wrap().child(text("in wrap")))
        .child(mark().name("left").child(props.left.place(|s| s)))
        .child(text(props.text))
}

/// The dev tool: reads the core's tree.
#[module]
pub(crate) struct PlaceProbe;

#[module]
impl PlaceProbe {
    /// Every node of the tree as JSON: its id, component and place.
    pub(crate) async fn tree() -> String {
        let tree = dev_tree().await;
        let nodes: Vec<JsonValue> = tree
            .nodes
            .iter()
            .map(|node| {
                json!({
                    "id": node.id,
                    "parent": node.parent,
                    "component": node.component.as_deref(),
                    "testId": node.test_id.as_deref(),
                    "place": node.place.as_ref().map(|p| json!({ "name": &*p.name, "of": p.of })),
                })
            })
            .collect();
        JsonValue::Array(nodes).to_string()
    }
}

fn sets() -> Vec<Registry> {
    let mut ui = Registry::ui_set("test/places");
    ui.register::<Mark>()
        .register::<Chip>()
        .register::<Boxed>()
        .register::<Framed>()
        .register::<Wrap>()
        .register::<Outer>();
    let mut probe = Registry::ui_set("dev/place-probe");
    probe.module::<PlaceProbe>();
    vec![nsh_base::registry(), crate::test_ui::registry(), ui, probe]
}

const APP: &str = r#"import { mount, nativeModule, signal, Text, View } from "nsh";
import { design } from "nsh/design";
import { Card } from "test/ui";
import { Chip, Framed, Mark, Outer } from "test/places";

const { PlaceProbe } = nativeModule<{ PlaceProbe: { tree(): Promise<string> } }>("dev/place-probe");

const shown = signal("waiting");
const filled = signal(false);
const label = signal("footer");

function App() {
  return <View>
    <View testID="board:empty"><Chip text="A" /></View>
    <View testID="board:filled"><Chip text="A" left={<Text>L</Text>} /></View>
    <View testID="board:later"><Chip text="A" left={filled() ? <Text>L</Text> : undefined} /></View>
    <View testID="board:off"><Chip text="A" showLeft={false} left={<Text>L</Text>} /></View>
    <View testID="board:rooted"><Framed text="A" left={<Text>L</Text>} /></View>
    <View testID="board:nested"><Outer text="B" /></View>
    <View testID="board:written"><Mark name={label()}><Text>F</Text></Mark></View>
    <View testID="board:plain"><Card header={<Text>Head</Text>} /></View>
    <Text>{shown()}</Text>
  </View>;
}

mount(App);
const tick = () => new Promise(r => setTimeout(r, 30));
setTimeout(async () => {
  const mounted = await design.layers("board:");
  const tree = JSON.parse(await PlaceProbe.tree());
  filled.set(true);
  label.set("renamed");
  await tick();
  const filledLater = await design.layers("board:");
  filled.set(false);
  await tick();
  const emptied = await design.layers("board:");
  shown.set("read " + JSON.stringify({ mounted, tree, filledLater, emptied }));
}, 0);
"#;

/// What the app read: `{ mounted, tree, filledLater, emptied }`. Read once:
/// the tests look at its parts.
fn read() -> &'static JsonValue {
    static READ: OnceLock<JsonValue> = OnceLock::new();
    READ.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("nsh-design-places-{}", std::process::id()));
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

/// The layers of board `root` in the read `when` (`mounted`, `filledLater`,
/// `emptied`); `None` when it did not change then.
fn layers(when: &str, root: &str) -> Option<&'static [JsonValue]> {
    read()[when]["roots"][root]["nodes"]
        .as_array()
        .map(Vec::as_slice)
}

/// The layers of board `root` at mount.
fn board(root: &str) -> &'static [JsonValue] {
    layers("mounted", root).unwrap_or_else(|| panic!("no board {root}: {}", read()["mounted"]))
}

/// The layers of `nodes` as lines a test compares: `kind name [slot]
/// {place} "text"` under its parent's line, indented. Component instances
/// (the app's `App`) are left out; their children take their place.
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
            if let Some(place) = node["place"].as_str() {
                line.push_str(&format!(" {{{place}}}"));
            }
            if let Some(text) = node["text"].as_str() {
                line.push_str(&format!(" {text:?}"));
            }
            line
        })
        .collect()
}

/// The layer of `nodes` that marks slot `place`.
fn marker<'a>(nodes: &'a [JsonValue], place: &str) -> &'a JsonValue {
    nodes
        .iter()
        .find(|n| n["place"] == place)
        .unwrap_or_else(|| panic!("no marker {place}: {nodes:?}"))
}

#[test]
fn an_empty_slot_a_view_marks_is_a_layer_of_its_component_with_a_frame() {
    let nodes = board("empty");
    // The marker's name text is its view's: not a layer.
    assert_eq!(outline(nodes), ["host Chip", "  host Mark {left}"]);
    let mark = marker(nodes, "left");
    // The core's: written nowhere, the app gave it nothing.
    assert_eq!(mark["source"], JsonValue::Null);
    assert_eq!(mark["slot"], JsonValue::Null);
    assert!(mark["width"].as_f64().unwrap() >= 20.0, "{mark}");
    assert!(mark["height"].as_f64().unwrap() >= 20.0, "{mark}");
    let host = nodes.iter().find(|n| n["name"] == "Chip").unwrap();
    assert_eq!(mark["parent"], host["id"]);
}

#[test]
fn a_filled_slot_lists_its_holder_and_the_apps_content_under_its_marker() {
    assert_eq!(
        outline(board("filled")),
        [
            "host Chip",
            "  host Mark {left}",
            "    view View [left]",
            "      text Text \"L\"",
        ]
    );
}

#[test]
fn a_marker_follows_the_slot_as_the_app_fills_and_empties_it() {
    assert_eq!(outline(board("later")), ["host Chip", "  host Mark {left}"]);
    let filled = layers("filledLater", "later").expect("the board changed");
    assert_eq!(
        outline(filled),
        [
            "host Chip",
            "  host Mark {left}",
            "    view View [left]",
            "      text Text \"L\"",
        ]
    );
    let emptied = layers("emptied", "later").expect("the board changed");
    assert_eq!(outline(emptied), ["host Chip", "  host Mark {left}"]);
}

#[test]
fn a_slot_its_view_does_not_make_has_no_layer() {
    // `showLeft` off: the view makes no marker, so there is no slot.
    assert_eq!(outline(board("off")), ["host Chip"]);
}

#[test]
fn a_component_rooted_in_another_lists_the_marker_its_view_draws() {
    assert_eq!(
        outline(board("rooted")),
        [
            "host Framed",
            "  host Mark {left}",
            "    view View [left]",
            "      text Text \"L\"",
        ]
    );
}

#[test]
fn a_marker_a_nested_component_draws_for_its_own_slot_is_not_the_outers() {
    // `Wrap`'s slot `inner` is `Wrap`'s, which is the view's own: not listed.
    assert_eq!(
        outline(board("nested")),
        ["host Outer", "  host Mark {left}"]
    );
    let tree = read()["tree"].as_array().unwrap();
    let outer = tree.iter().find(|n| n["component"] == "Outer").unwrap();
    let wrap = tree.iter().find(|n| n["component"] == "Wrap").unwrap();
    let place = |name: &str| {
        tree.iter()
            .find(|n| n["place"]["name"] == name)
            .unwrap_or_else(|| panic!("no place {name}"))["place"]
            .clone()
    };
    assert_eq!(place("inner"), json!({ "name": "inner", "of": wrap["id"] }));
    assert_eq!(
        tree.iter()
            .filter(|n| n["place"]["of"] == outer["id"])
            .count(),
        1
    );
}

#[test]
fn a_marker_the_app_writes_has_its_place_and_follows_its_name() {
    assert_eq!(
        outline(board("written")),
        ["host Mark {footer}", "  text Text \"F\""]
    );
    let mark = marker(board("written"), "footer");
    // Where the app wrote it.
    assert_eq!(mark["source"]["module"], "App.tsx");
    let tree = read()["tree"].as_array().unwrap();
    let place = tree
        .iter()
        .find(|n| n["place"]["name"] == "footer")
        .unwrap();
    // No component's view made it.
    assert_eq!(place["place"]["of"], JsonValue::Null);
    let renamed = layers("filledLater", "written").expect("the board changed");
    assert_eq!(
        outline(renamed),
        ["host Mark {renamed}", "  text Text \"F\""]
    );
}

#[test]
fn layers_that_mark_no_slot_have_no_place() {
    let nodes = board("plain");
    assert_eq!(
        outline(nodes),
        [
            "host Card",
            "  view View [header]",
            "    text Text \"Head\""
        ]
    );
    assert!(
        nodes
            .iter()
            .filter(|n| n["kind"] != "component")
            .all(|n| n["place"] == JsonValue::Null),
        "{nodes:?}"
    );
}
