//! The app's end: the platform says the app is about to terminate, and the
//! engine runs its terminate listeners once (Rust's `on_terminate`, JS's
//! `Lifecycle.on("terminate")`), then what they started, then writes the
//! app's storage out, before it answers. On a thread of its own (`Engine`,
//! the native platforms') and driven by its host (`LocalEngine`, the web's
//! and tests').

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, MountBatch, MountItem, Size, element::Root, prelude::*, storage::Storage,
};
use nsh_engine::{App, Bundle, Engine, EngineConfig, Environment, FileStorage, LocalEngine};

use crate::ChannelSink;

/// A directory of its own under the test's temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nsh-terminate-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An engine of `app` keeping its storage in `data`.
fn config(app: App, data: &Path) -> EngineConfig {
    EngineConfig {
        app,
        viewport: Size::new(px(390.0), px(844.0)),
        trace: false,
        content_culling: false,
        network: None,
        websocket: None,
        refuse_connections: None,
        filesystem: None,
        assets: None,
        environment: Environment::default(),
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: Vec::new(),
        runtime: None,
        data_dir: None,
        cache: nsh_engine::CacheOptions::off(),
        storage: Some(FileStorage::open(data) as _),
        clock: Default::default(),
        context: Default::default(),
        pinned: Default::default(),
        clipboard: None,
        host: Default::default(),
        picker: None,
        speech: None,
        audio: None,
        windows: None,
    }
}

/// What the storage file in `data` holds now.
fn kept(data: &Path) -> BTreeMap<String, String> {
    match std::fs::read(data.join("storage.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap(),
        Err(_) => BTreeMap::new(),
    }
}

/// The texts of `batches`.
fn texts(batches: impl IntoIterator<Item = MountBatch>) -> Vec<String> {
    batches
        .into_iter()
        .flat_map(|batch| batch.items)
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

/// The texts shown until one is `until` (or 5 seconds passed).
fn until(rx: &Receiver<MountBatch>, until: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut shown = Vec::new();
    while let Ok(batch) = rx.recv_deadline(deadline) {
        shown.extend(texts([batch]));
        if shown.iter().any(|t| t == until) {
            break;
        }
    }
    shown
}

/// Its listener goes with it.
#[component]
fn Leaving() -> impl IntoElement {
    on_terminate(|| {
        let _ = Storage::set_item("leaving", "ran");
    });
    text("leaving")
}

/// Saves as the app terminates, with two listeners and a task one of them
/// starts; a child that listened is gone by then.
#[component]
fn Editor() -> impl IntoElement {
    let child = signal(true);
    on_terminate(|| {
        let saves = Storage::get_item("saves")
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0);
        Storage::set_item("saves", (saves + 1).to_string()).unwrap();
        Storage::set_item("order", "first").unwrap();
    });
    on_terminate(|| {
        let order = Storage::get_item("order").unwrap_or_default();
        Storage::set_item("order", format!("{order}, second")).unwrap();
        spawn(async {
            Storage::set_item("task", "ran").unwrap();
        });
    });
    spawn(async move {
        timer(Duration::from_millis(20)).await;
        child.set(false);
    });
    view().child(when(child, leaving)).child(text(move || {
        if child.get() {
            "with child"
        } else {
            "child gone"
        }
    }))
}

/// What [`Editor`]'s listeners wrote, once.
fn saved_once(data: &Path) {
    let kept = kept(data);
    assert_eq!(kept.get("saves").map(String::as_str), Some("1"), "{kept:?}");
    assert_eq!(kept.get("order").map(String::as_str), Some("first, second"));
    assert_eq!(kept.get("task").map(String::as_str), Some("ran"));
    assert_eq!(kept.get("leaving"), None);
}

#[test]
fn rust_listeners_run_once_and_the_storage_is_written_before_the_engine_answers() {
    let data = scratch("rust");
    let (tx, rx) = crossbeam_channel::unbounded();
    let engine = Engine::start(
        config(App::Rust(Root::of::<Editor>()), &data),
        || ApproxTextEngine,
        ChannelSink(tx),
    )
    .unwrap();
    let shown = until(&rx, "child gone");
    assert!(shown.iter().any(|t| t == "leaving"), "{shown:?}");
    let control = engine.control();
    assert!(control.will_terminate_and_wait(Duration::from_secs(5)));
    // In the file while the engine still runs: its storage is open.
    saved_once(&data);
    // Told again, it runs nothing.
    assert!(control.will_terminate_and_wait(Duration::from_secs(5)));
    saved_once(&data);
    drop(engine);
    // A stopped engine answers at once.
    let start = Instant::now();
    assert!(control.will_terminate_and_wait(Duration::from_secs(5)));
    assert!(start.elapsed() < Duration::from_secs(1));
    std::fs::remove_dir_all(&data).unwrap();
}

#[test]
fn a_local_engine_terminates_when_its_host_says_so() {
    let data = scratch("local");
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut engine = LocalEngine::start(
        config(App::Rust(Root::of::<Editor>()), &data),
        ApproxTextEngine,
        ChannelSink(tx),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut shown = Vec::new();
    while !shown.iter().any(|t| t == "child gone") && Instant::now() < deadline {
        engine.pump();
        shown.extend(texts(rx.try_iter()));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(shown.iter().any(|t| t == "leaving"), "{shown:?}");
    engine.will_terminate();
    saved_once(&data);
    engine.will_terminate();
    saved_once(&data);
    drop(engine);
    std::fs::remove_dir_all(&data).unwrap();
}

const SAVES: &str = r#"import { mount, Lifecycle, Storage, Text } from "nsh";

let times = 0;
Lifecycle.on("terminate", () => {
  times += 1;
  Storage.setItem("times", String(times));
});
const stop = Lifecycle.on("terminate", () => Storage.setItem("stopped", "ran"));
stop();
Lifecycle.on("terminate", async () => {
  await Storage.setItem("draft", "kept");
  Storage.setItem("after", "await");
});

mount(() => <Text>listening</Text>);
"#;

#[test]
fn js_listeners_run_once_with_what_their_promises_settle_at_once() {
    let data = scratch("js");
    let dir = scratch("js-app");
    let entry = dir.join("App.tsx");
    std::fs::write(&entry, SAVES).unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
    })
    .unwrap();
    let bytecode = nsh_js::compile_bytecode_with(&bundle.code, false, &[]).unwrap();
    // SAFETY: compiled just now by this runtime.
    let app = App::Js(unsafe { Bundle::bytecode(bytecode) });
    let (tx, rx) = crossbeam_channel::unbounded();
    let engine = Engine::start(config(app, &data), || ApproxTextEngine, ChannelSink(tx)).unwrap();
    let shown = until(&rx, "listening");
    assert!(shown.iter().any(|t| t == "listening"), "{shown:?}");
    for _ in 0..2 {
        assert!(
            engine
                .control()
                .will_terminate_and_wait(Duration::from_secs(5))
        );
        let kept = kept(&data);
        assert_eq!(
            kept,
            BTreeMap::from([
                ("after".to_owned(), "await".to_owned()),
                ("draft".to_owned(), "kept".to_owned()),
                ("times".to_owned(), "1".to_owned()),
            ])
        );
    }
    drop(engine);
    std::fs::remove_dir_all(&data).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
