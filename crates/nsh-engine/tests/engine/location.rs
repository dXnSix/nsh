//! The app's location (`location()` / `navigate`, Rust and TSX): it
//! starts where the embedding app set it, the app's navigations reach the
//! platform as `Command::Navigate` and the embedding app's navigation
//! handler, and the moves the platform or the embedding app make reach
//! what read it.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crossbeam_channel::Receiver;
use nsh_core::{
    ApproxTextEngine, Command, EventKind, EventPayload, MountBatch, MountItem, PayloadValue,
    ROOT_ID, Size,
    element::Root,
    geometry::px,
    location::{HistoryBehavior, Navigation},
    prelude::*,
};
use nsh_engine::{App, Bundle, Engine, EngineConfig, HostChannel};

use crate::ChannelSink;

fn bundle(app: &str) -> Bundle {
    static APPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = crate::temporary_root().join(format!(
        "nsh-location-{}-{}",
        std::process::id(),
        APPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(&entry, app).unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: Vec::new(),
    })
    .unwrap();
    let bytecode = nsh_js::compile_bytecode_with(&bundle.code, false, &[]).unwrap();
    // SAFETY: compiled just now by this runtime.
    unsafe { Bundle::bytecode(bytecode) }
}

fn start(app: App, host: &HostChannel) -> (Engine, Receiver<MountBatch>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app,
        viewport: Size::new(px(400.0), px(600.0)),
        host: host.clone(),
        ..EngineConfig::new(bundle("export {};"))
    };
    (
        Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap(),
        rx,
    )
}

/// The texts shown and the navigations sent to the platform, until a text
/// is `until` (or 5 seconds passed).
fn until(rx: &Receiver<MountBatch>, until: &str) -> (Vec<String>, Vec<Navigation>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut texts, mut navigations) = (Vec::new(), Vec::new());
    while let Ok(batch) = rx.recv_deadline(deadline) {
        for item in batch.items {
            match item {
                MountItem::Text { text, .. } => texts.push(text.text.to_string()),
                MountItem::Command {
                    id,
                    command: Command::Navigate(navigation),
                } => {
                    assert_eq!(id, ROOT_ID);
                    navigations.push(navigation);
                }
                _ => {}
            }
        }
        if texts.iter().any(|t| t == until) {
            return (texts, navigations);
        }
    }
    panic!("no text {until:?}; texts: {texts:?}");
}

/// The embedding app's ears: the navigations it heard.
fn heard(host: &HostChannel) -> Arc<Mutex<Vec<Navigation>>> {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let into = heard.clone();
    host.handle_navigate(move |navigation| into.lock().unwrap().push(navigation));
    heard
}

/// Shows its location, goes to the sports page on a press, and to its
/// table (replacing the entry) on another.
#[component]
fn Located() -> impl IntoElement {
    view()
        .child(text(|| format!("at {}", location())))
        .child(text("Sports").on_press(|_: &()| {
            navigate("sports?tab=live", HistoryBehavior::Push).unwrap();
            // At once, before the platform heard of it.
            assert_eq!(location().query(), Some("tab=live"));
            assert!(navigate("https://example.com/", HistoryBehavior::Push).is_err());
        }))
        .child(text("Table").on_press(|_: &()| {
            navigate("?tab=table", HistoryBehavior::Replace).unwrap();
        }))
}

/// The id of the text `label` in `rx`'s batches so far.
fn press(engine: &Engine, ids: &[(String, nsh_core::NodeId)], label: &str) {
    let id = ids
        .iter()
        .find(|(t, _)| t == label)
        .map(|(_, id)| *id)
        .unwrap_or_else(|| panic!("no {label} in {ids:?}"));
    engine
        .control()
        .dispatch_event(id, EventKind::Press, EventPayload::None);
}

#[test]
fn rust_components_follow_and_move_the_location() {
    let host = HostChannel::new();
    host.set_location("/news").unwrap();
    let heard = heard(&host);
    let (tx, rx) = crossbeam_channel::unbounded();
    let config = EngineConfig {
        app: App::Rust(Root::of::<Located>()),
        viewport: Size::new(px(400.0), px(600.0)),
        host: host.clone(),
        ..EngineConfig::new(bundle("export {};"))
    };
    let engine = Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap();
    // The texts' ids, to press them.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ids = Vec::new();
    while let Ok(batch) = rx.recv_deadline(deadline) {
        for item in batch.items {
            if let MountItem::Text { id, text, .. } = item {
                ids.push((text.text.to_string(), id));
            }
        }
        if ids.iter().any(|(t, _)| t == "at /news") {
            break;
        }
    }
    press(&engine, &ids, "Sports");
    let (_, navigations) = until(&rx, "at /sports?tab=live");
    let pushed = Navigation::new(
        nsh_core::location::Location::parse("/sports?tab=live").unwrap(),
        HistoryBehavior::Push,
    );
    assert_eq!(navigations, std::slice::from_ref(&pushed));
    press(&engine, &ids, "Table");
    until(&rx, "at /sports?tab=table");
    assert_eq!(host.location().as_str(), "/sports?tab=table");
    let replaced = Navigation::new(
        nsh_core::location::Location::parse("/sports?tab=table").unwrap(),
        HistoryBehavior::Replace,
    );
    assert_eq!(*heard.lock().unwrap(), [pushed, replaced]);
    // The embedding app moves the app: not a navigation of the app's.
    host.set_location("/teams/7").unwrap();
    until(&rx, "at /teams/7");
    // And so does the platform (the web's back button).
    engine.control().dispatch_event(
        ROOT_ID,
        EventKind::Location,
        EventPayload::Map(vec![("location", PayloadValue::Str("/news".into()))]),
    );
    until(&rx, "at /news");
    assert_eq!(heard.lock().unwrap().len(), 2);
    assert_eq!(host.location().as_str(), "/news");
}

const APP: &str = r#"import { location, mount, navigate, Text } from "nsh";

let refused = "";
try {
  navigate("https://example.com/");
} catch (e) {
  refused = (e as Error).name;
}
try {
  navigate("/x", { history: "sideways" as "push" });
} catch (e) {
  refused += ` ${(e as Error).name}`;
}
const start = location().url;
// At once, in the same turn.
navigate("../sports/football?tab=live#top");
const after = location();

mount(() => (
  <Text>
    {`${start} ${after.pathname}|${after.search}|${after.hash} now ${location().url} ${refused}`}
  </Text>
));
"#;

#[test]
fn js_apps_follow_and_move_the_location() {
    let host = HostChannel::new();
    host.set_location("/news/today").unwrap();
    let heard = heard(&host);
    let (engine, rx) = start(App::Js(bundle(APP)), &host);
    let shown = "/news/today /sports/football|?tab=live|#top now /sports/football?tab=live#top TypeError TypeError";
    let (_, navigations) = until(&rx, shown);
    let pushed = Navigation::new(
        nsh_core::location::Location::parse("/sports/football?tab=live#top").unwrap(),
        HistoryBehavior::Push,
    );
    assert_eq!(navigations, std::slice::from_ref(&pushed));
    assert_eq!(*heard.lock().unwrap(), [pushed]);
    host.set_location("/teams").unwrap();
    until(
        &rx,
        "/news/today /sports/football|?tab=live|#top now /teams TypeError TypeError",
    );
    drop(engine);
}
