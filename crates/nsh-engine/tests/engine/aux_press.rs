//! `onAuxPress` in a TSX app: a view's handler is in its event mask (the
//! platform finds the nearest listener from it), a disabled view mounts
//! none, and the press reaches the handler with the button and where the
//! mouse was.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, EventKind, EventPayload, MountBatch, MountItem, MouseButton, MouseEvent,
    NodeId, Size, geometry::px, keymap::Modifiers,
};
use nsh_engine::{Bundle, Engine, EngineConfig};

use crate::ChannelSink;

const APP: &str = r#"import { mount, signal, Text, View } from "nsh";

function App() {
  const log = signal<string[]>([]);
  const note = (s: string) => log.update((l) => [...l, s]);
  return (
    <View>
      <View
        testID="tab"
        style={{ height: 40 }}
        onPress={() => note("press")}
        onAuxPress={(e) => note(`${e.pressedButton} ${e.position.x},${e.position.y} ${e.pagePosition.x},${e.pagePosition.y}${e.modifiers.shift ? " shift" : ""}`)}
      />
      <View testID="off" disabled onAuxPress={() => note("disabled")} />
      <Text>{"log " + log().join("|")}</Text>
    </View>
  );
}

mount(App);
"#;

/// The app on an engine; `name` keeps the tests' directories apart.
fn start(name: &str) -> (Engine, Receiver<MountBatch>) {
    let dir = std::env::temp_dir().join(format!("nsh-aux-press-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("App.tsx"), APP).unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry: dir.join("App.tsx"),
        minify: false,
        ui_sets: vec![nsh_base::registry().into()],
        split: false,
    })
    .unwrap();
    let bytecode = nsh_js::compile_bytecode(&bundle.code, false).unwrap();
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        // SAFETY: compiled just now by this runtime.
        app: nsh_engine::App::Js(unsafe { Bundle::bytecode(bytecode) }),
        viewport: Size::new(px(390.0), px(844.0)),
        trace: false,
        content_culling: false,
        network: None,
        websocket: None,
        refuse_connections: None,
        filesystem: None,
        assets: None,
        environment: Default::default(),
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: Vec::new(),
        runtime: None,
        data_dir: None,
        cache: nsh_engine::CacheOptions::off(),
        storage: None,
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        speech: None,
        audio: None,
        windows: None,
    };
    (
        Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap(),
        rx,
    )
}

/// Waits until the log text is `want`, within a few seconds.
fn until_log(rx: &Receiver<MountBatch>, want: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = String::new();
    while let Ok(batch) = rx.recv_deadline(deadline) {
        if let Some(text) = batch.items.iter().find_map(|i| match i {
            MountItem::Text { text, .. } if text.text.starts_with("log ") => Some(&text.text),
            _ => None,
        }) {
            last = text.clone();
        }
        if last == want {
            return;
        }
    }
    panic!("the log never became {want:?}: {last:?}");
}

/// A press of `button` at `x` in the view (10 points in from the root
/// view), with Shift held or not.
fn at(x: f32, shift: bool, button: MouseButton) -> EventPayload {
    EventPayload::Mouse(MouseEvent::new(
        nsh_core::geometry::point(px(x), px(2.0)),
        nsh_core::geometry::point(px(x + 10.0), px(12.0)),
        Modifiers {
            shift,
            ..Default::default()
        },
        Some(button),
    ))
}

#[test]
fn an_aux_press_reaches_its_handler_with_the_button() {
    let (engine, rx) = start("events");
    let first = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first frame");
    let id = |wanted: &str| -> NodeId {
        first
            .items
            .iter()
            .find_map(|i| match i {
                MountItem::ViewProps { id, props } if props.test_id.as_deref() == Some(wanted) => {
                    Some(*id)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{wanted} is mounted"))
    };
    let events = |id: NodeId| {
        first
            .items
            .iter()
            .find_map(|i| match i {
                MountItem::Events { id: i, events } if *i == id => Some(*events),
                _ => None,
            })
            .unwrap_or_default()
    };
    let (tab, off) = (id("tab"), id("off"));
    // The platform finds the nearest listener from the masks; a disabled
    // view's is not mounted, so a press there goes on to the views around.
    assert!(events(tab).contains(EventKind::AuxPress));
    assert!(events(tab).contains(EventKind::Press));
    assert!(!events(off).contains(EventKind::AuxPress));

    let control = engine.control();
    control.dispatch_event(
        tab,
        EventKind::AuxPress,
        at(4.0, false, MouseButton::Middle),
    );
    until_log(&rx, "log middle 4,2 14,12");
    control.dispatch_event(tab, EventKind::AuxPress, at(6.0, true, MouseButton::Back));
    control.dispatch_event(
        tab,
        EventKind::AuxPress,
        at(8.0, false, MouseButton::Forward),
    );
    control.dispatch_event(tab, EventKind::AuxPress, at(9.0, false, MouseButton::Right));
    until_log(
        &rx,
        "log middle 4,2 14,12|back 6,2 16,12 shift|forward 8,2 18,12|right 9,2 19,12",
    );
}
