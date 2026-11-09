//! `onWheel` and `wheelScroll` in a TSX app: a view's handler is in its
//! event mask and its `wheelScroll` in its props (the platform routes the
//! wheel from them), the event reaches the handler with its delta, and
//! wheels one view hears faster than frames come merged, their deltas
//! added up while the modifier keys stay.

use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, EventKind, EventPayload, MountBatch, MountItem, NodeId, Size, WheelEvent,
    WheelScroll,
    geometry::{point, px},
    keymap::Modifiers,
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
        testID="canvas"
        style={{ height: 100 }}
        wheelScroll="none"
        onWheel={(e) => note(`${e.modifiers.control ? "ctrl " : ""}${e.delta.x},${e.delta.y} ${e.position.x},${e.position.y} ${e.pagePosition.x},${e.pagePosition.y}${e.precise ? " precise" : ""}`)}
      />
      <View testID="plain" style={{ height: 100 }} />
      <Text>{"log " + log().join("|")}</Text>
    </View>
  );
}

mount(App);
"#;

/// The app on an engine; `name` keeps the tests' directories apart.
fn start(name: &str) -> (Engine, Receiver<MountBatch>) {
    let dir = std::env::temp_dir().join(format!("nsh-wheel-{name}-{}", std::process::id()));
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
    until(rx, want, |log| log == want);
}

/// Waits until the log text satisfies `done`; returns it.
fn until(rx: &Receiver<MountBatch>, what: &str, done: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = String::new();
    while let Ok(batch) = rx.recv_deadline(deadline) {
        if let Some(text) = batch.items.iter().find_map(|i| match i {
            MountItem::Text { text, .. } if text.text.starts_with("log ") => Some(&text.text),
            _ => None,
        }) {
            last = text.clone();
        }
        if done(&last) {
            return last;
        }
    }
    panic!("the log never became {what}: {last:?}");
}

/// A wheel by `dy` at `x` in the canvas (its origin at the root's), with
/// Control held or not, precise or in notches.
fn wheel(x: f32, dy: f32, ctrl: bool, precise: bool) -> EventPayload {
    EventPayload::Wheel(WheelEvent::new(
        point(px(x), px(2.0)),
        point(px(x), px(2.0)),
        point(px(0.0), px(dy)),
        Modifiers {
            ctrl,
            ..Default::default()
        },
        precise,
    ))
}

#[test]
fn a_wheel_reaches_its_handler_and_wheels_come_merged() {
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
    let (canvas, plain) = (id("canvas"), id("plain"));
    // The platform routes the wheel from the mounted masks and props.
    let props = |id: NodeId| {
        first
            .items
            .iter()
            .find_map(|i| match i {
                MountItem::ViewProps { id: i, props } if *i == id => Some(props.wheel_scroll),
                _ => None,
            })
            .unwrap()
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
    assert_eq!(props(canvas), WheelScroll::None);
    assert_eq!(props(plain), WheelScroll::Auto);
    assert!(events(canvas).contains(EventKind::Wheel));
    assert!(!events(plain).contains(EventKind::Wheel));

    let control = engine.control();
    control.dispatch_event(canvas, EventKind::Wheel, wheel(4.0, -48.0, false, false));
    until_log(&rx, "log 0,-48 4,2 4,2");
    // Wheels faster than frames are merged (how many depends on the
    // engine thread's timing): their deltas add up, where the last one
    // was; another modifier key starts a wheel of its own.
    for x in 1..=20 {
        control.dispatch_event(canvas, EventKind::Wheel, wheel(x as f32, 1.0, false, true));
    }
    control.dispatch_event(canvas, EventKind::Wheel, wheel(21.0, 5.0, true, true));
    let log = until(&rx, "ending with the ctrl wheel", |l| {
        l.ends_with("ctrl 0,5 21,2 21,2 precise")
    });
    let wheels: Vec<&str> = log.split('|').skip(1).collect();
    let merged = &wheels[..wheels.len() - 1];
    assert!(merged.len() < 20, "wheels are merged: {log}");
    let total: f32 = merged
        .iter()
        .map(|w| {
            let delta = w.split(' ').next().unwrap();
            delta.split(',').nth(1).unwrap().parse::<f32>().unwrap()
        })
        .sum();
    assert_eq!(total, 20.0, "{log}");
    assert!(
        merged.last().unwrap().ends_with(" 20,2 20,2 precise"),
        "{log}"
    );
}
