//! A host whose engine runs on its own event loop (`LocalEngine`, the web's
//! main thread) learns when there is work outside a pump: a task woken by
//! one of the host's callbacks (a WebSocket message through wasm-bindgen)
//! calls the wake set with `LocalEngine::set_wake` exactly once, and the
//! next pump runs it.

use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, MountBatch, MountItem, Size,
    ui::{Root, prelude::*},
};
use nsh_engine::{App, EngineConfig, Environment, LocalEngine};
use tokio::sync::oneshot;

use crate::ChannelSink;

thread_local! {
    /// What the app's task waits for: sent by the test, as a host's
    /// callback would deliver it.
    static MESSAGE: RefCell<Option<oneshot::Receiver<String>>> = const { RefCell::new(None) };
}

#[component]
fn Echo() -> impl IntoElement {
    let shown = signal(String::from("waiting"));
    let message = MESSAGE.with(|m| m.borrow_mut().take());
    spawn(async move {
        if let Some(message) = message
            && let Ok(text) = message.await
        {
            shown.set(text);
        }
    });
    text(move || shown.get())
}

/// The texts in the batches received so far.
fn texts(rx: &Receiver<MountBatch>) -> Vec<String> {
    let mut texts = Vec::new();
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(10)) {
        for item in &batch.items {
            if let MountItem::Text { text, .. } = item {
                texts.push(text.text.clone());
            }
        }
    }
    texts
}

#[test]
fn external_wake_asks_for_one_pump_and_the_pump_runs_the_task() {
    let (send, message) = oneshot::channel();
    MESSAGE.with(|m| *m.borrow_mut() = Some(message));
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(Root::of::<Echo>()),
        viewport: Size::new(nsh_core::Points(390.0), nsh_core::Points(844.0)),
        trace: false,
        network: None,
        websocket: None,
        filesystem: None,
        assets: None,
        environment: Environment::default(),
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: Vec::new(),
        runtime: None,
        data_dir: None,
        storage: None,
        clock: Default::default(),
    };
    let mut engine = LocalEngine::start(config, ApproxTextEngine, ChannelSink(tx)).unwrap();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = wakes.clone();
    engine.set_wake(Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));
    engine.pump();
    assert_eq!(texts(&rx).last().map(String::as_str), Some("waiting"));
    // Nothing is due: no timer, no input; the task waits for the host.
    let before = wakes.load(Ordering::SeqCst);
    // The host's callback, outside any pump.
    send.send("hello".into()).unwrap();
    assert_eq!(
        wakes.load(Ordering::SeqCst) - before,
        1,
        "one wake for one woken task"
    );
    assert!(texts(&rx).is_empty(), "nothing runs before the host pumps");
    engine.pump();
    assert_eq!(texts(&rx).last().map(String::as_str), Some("hello"));
    assert_eq!(
        wakes.load(Ordering::SeqCst) - before,
        1,
        "running the task asks for no further pump"
    );
}

#[test]
fn external_wake_is_optional() {
    // Without a wake the engine behaves as before: the host pumps at the
    // deadline or on input, and the woken task runs then.
    let (send, message) = oneshot::channel();
    MESSAGE.with(|m| *m.borrow_mut() = Some(message));
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(Root::of::<Echo>()),
        viewport: Size::new(nsh_core::Points(390.0), nsh_core::Points(844.0)),
        trace: false,
        network: None,
        websocket: None,
        filesystem: None,
        assets: None,
        environment: Environment::default(),
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: Vec::new(),
        runtime: None,
        data_dir: None,
        storage: None,
        clock: Default::default(),
    };
    let mut engine = LocalEngine::start(config, ApproxTextEngine, ChannelSink(tx)).unwrap();
    engine.pump();
    assert_eq!(texts(&rx).last().map(String::as_str), Some("waiting"));
    send.send("later".into()).unwrap();
    engine.pump();
    assert_eq!(texts(&rx).last().map(String::as_str), Some("later"));
}
