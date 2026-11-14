//! Dev tools ask where a script's nodes were written and which hot components
//! rendered them (`nsh_core::element::dev_source`, `dev_instances`,
//! `dev_instance_props`): the core queues the question, the engine answers
//! it from the VM's stamps in the same round. A Rust module stands in for
//! the dev tool, as RayX's agent does.

use nsh_core::{
    ApproxTextEngine, Size,
    element::{Registry, dev_instance_props, dev_instances, dev_source, dev_tree},
    inspect::DevProp,
    prelude::*,
};
use nsh_engine::{Bundle, Engine, EngineConfig};

use crate::{ChannelSink, rust_classes::shown};

/// The dev tool: describes the node with a test id.
#[module]
pub(crate) struct Probe;

#[module]
impl Probe {
    /// `module:anchor:line:column` of the node with test id `test_id`, then
    /// its instances' keys and call sites, then its innermost instance's
    /// props.
    pub(crate) async fn describe(test_id: String) -> String {
        let tree = dev_tree().await;
        let Some(node) = tree
            .nodes
            .iter()
            .find(|n| n.test_id.as_deref() == Some(test_id.as_str()))
        else {
            return format!("no node {test_id}");
        };
        let place = |s: &nsh_core::inspect::DevSource| {
            format!("{}:{}:{}:{}", s.module, s.anchor, s.line, s.column)
        };
        let source = dev_source(node.id)
            .await
            .map_or_else(|| "none".to_owned(), |s| place(&s));
        let instances = dev_instances(node.id).await;
        let chain: Vec<String> = instances
            .iter()
            .map(|i| {
                format!(
                    "{}@{}",
                    i.key,
                    i.call_site.as_ref().map_or_else(|| "-".to_owned(), place)
                )
            })
            .collect();
        let props = match instances.first() {
            Some(i) => dev_instance_props(i.id).await,
            None => Vec::new(),
        };
        let props: Vec<String> = props
            .iter()
            .map(|(name, value)| match value {
                DevProp::Text(t) => format!("{name}={t}"),
                DevProp::Getter => format!("{name}=getter"),
                other => format!("{name}={other:?}"),
            })
            .collect();
        format!(
            "described {source} [{}] {{{}}}",
            chain.join(" < "),
            props.join(",")
        )
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("dev/probe");
    r.module::<Probe>();
    r
}

const APP: &str = r#"import { mount, signal, Text, View } from "nsh";
import { nativeModule } from "nsh";

const { Probe } = nativeModule<{ Probe: { describe(id: string): Promise<string> } }>("dev/probe");

export function Card(props: { title: string }) {
  return <View testID="card">
    <Text>{props.title}</Text>
  </View>;
}

const shown = signal("waiting");

function App() {
  return <View>
    <Card title="Hello" />
    <Text>{shown()}</Text>
  </View>;
}

mount(App);
setTimeout(async () => shown.set(await Probe.describe("card")), 0);
"#;

#[test]
fn dev_sources_answer_where_a_scripts_nodes_were_written() {
    let dir = std::env::temp_dir().join(format!("nsh-dev-sources-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("App.tsx"), APP).unwrap();
    let bundle = nsh_bundler::bundle_hot(&dir.join("App.tsx"), &[]).unwrap();
    let (code, map) = bundle.script();
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut config = EngineConfig::new(nsh_engine::App::Js(
        Bundle::source(code).with_source_map(map.unwrap()),
    ));
    config.viewport = Size::new(px(390.0), px(844.0));
    config.ui_sets = vec![registry()];
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    let text = shown(&rx, "described");
    // The card's view: written in `Card` at line 7, column 10; rendered by
    // Card's instance, used in `App` at line 16, inside App's instance
    // (mounted, so used nowhere).
    assert_eq!(
        text,
        "described App.tsx:Card:7:10 [App.tsx:Card@App.tsx:App:16:5 < App.tsx:App@-] {title=Hello}"
    );
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}
