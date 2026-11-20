//! `design_mode()`: a Rust component's view asks whether a design tool
//! shows the app, as the app's script asks `design.active()` of
//! `nsh/design`. True only in an app a design tool started
//! (`NSH_DESIGN_SOCKET`, `docs/dev/design-mode.md`), where the platform's
//! design host connects before any view is made; false everywhere else.
//!
//! A process has one design host for its whole life, so the host's case
//! runs in a process of its own.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use nsh_core::{
    MountBatch, MountItem,
    element::Registry,
    prelude::*,
    runtime::design::{self, DesignHooks},
};
use nsh_engine::Environment;

use crate::components::start_with_sets;

/// The app's content, and a note only a design tool's user sees.
#[derive(Debug, Clone, PartialEq, Props)]
pub(crate) struct OutlinedProps {
    /// The content.
    pub(crate) children: Children,
}

/// The content ([`OutlinedProps`]) and, in design mode only, a note after
/// it; a text says which of the two the view was made for.
#[component]
pub(crate) fn Outlined(props: OutlinedProps) -> impl IntoElement {
    view()
        .child(props.children)
        .child(when(design_mode, || text("drop content here")))
        .child(text(if design_mode() {
            "view: design"
        } else {
            "view: app"
        }))
}

fn sets() -> Vec<Registry> {
    let mut ui = Registry::ui_set("test/design-mode");
    ui.register::<Outlined>();
    vec![ui]
}

const APP: &str = r#"import { mount, Text, View } from "nsh";
import { design } from "nsh/design";
import { Outlined } from "test/design-mode";

mount(() => (
  <View>
    <Outlined>
      <Text>content</Text>
    </Outlined>
    <Text>{`script: ${design.active() ? "design" : "app"}`}</Text>
  </View>
));
"#;

/// Every text the app shows until one starts with `script:` (or 5 s
/// passed).
fn texts(rx: &Receiver<MountBatch>) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut shown = Vec::new();
    while let Ok(batch) = rx.recv_deadline(deadline) {
        for item in &batch.items {
            if let MountItem::Text { text, .. } = item {
                shown.push(text.text.clone());
            }
        }
        if shown.iter().any(|t| t.starts_with("script:")) {
            break;
        }
    }
    shown
}

/// An app nothing started as a design host: the view is the app's, with
/// no note, and the script reads the same.
#[test]
fn a_view_reads_no_design_mode_in_an_app_no_design_tool_started() {
    assert!(!design_mode(), "read outside any view");
    let (engine, rx) = start_with_sets(APP, Environment::default(), None, None, sets());
    let shown = texts(&rx);
    assert!(shown.contains(&"view: app".to_owned()), "{shown:?}");
    assert!(shown.contains(&"script: app".to_owned()), "{shown:?}");
    assert!(shown.contains(&"content".to_owned()), "{shown:?}");
    assert!(
        !shown.contains(&"drop content here".to_owned()),
        "{shown:?}"
    );
    assert!(!design_mode());
    drop(engine);
}

/// An app a design tool started: runs [`in_a_design_host`] in a process
/// of its own, with the variable naming where the tool listens.
#[test]
fn a_view_reads_design_mode_in_an_app_a_design_tool_started() {
    // The tool: it listens; the host connects whenever it likes.
    let tool = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tool.local_addr().unwrap().port();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "design_mode::in_a_design_host",
            "--ignored",
            "--nocapture",
        ])
        .env(design::SOCKET, format!("tcp:127.0.0.1:{port}"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    // It ran the test, not a filter that matched nothing.
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("1 passed"),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    drop(tool);
}

/// The process a design tool started: the variable is set, and the
/// platform's design host connects before the app starts, as each
/// platform's `design.rs` does. From then on the view is the design
/// tool's, with its note, and the script reads the same.
#[test]
#[ignore = "run by a_view_reads_design_mode_in_an_app_a_design_tool_started in a process of its own"]
fn in_a_design_host() {
    let endpoint = design::endpoint().expect("started with the tool's endpoint");
    // The variable alone is not design mode: a host that takes no part
    // in it (a platform without one) leaves the app as it is.
    assert!(!design_mode());
    assert!(design::connect(
        endpoint,
        DesignHooks {
            input: Box::new(|_| {}),
            closed: Box::new(|| {}),
        },
    ));
    assert!(design_mode(), "read outside any view");
    let (engine, rx) = start_with_sets(APP, Environment::default(), None, None, sets());
    let shown = texts(&rx);
    assert!(shown.contains(&"view: design".to_owned()), "{shown:?}");
    assert!(shown.contains(&"script: design".to_owned()), "{shown:?}");
    assert!(shown.contains(&"drop content here".to_owned()), "{shown:?}");
    let at = |wanted: &str| shown.iter().position(|t| t == wanted).unwrap();
    assert!(at("content") < at("drop content here"), "{shown:?}");
    drop(engine);
}
