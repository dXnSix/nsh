//! Rust on the engine thread through an engine: a synchronous call from JS
//! starts a core task (`start_task`, no `CallCx`), and that task (as an
//! `async` member's body) calls a TS factory and reads the TS object it
//! makes (getters, properties) between JS's turns, the reactive runtime
//! entered there and not in the call (`runtime_entered`).

use std::time::Duration;

use nsh_core::{element::Registry, prelude::*};

use crate::rust_classes::{shown, start};

/// What a TS feed publishes.
#[interface]
pub(crate) trait Feed {
    /// Its title.
    #[getter]
    fn title(&self) -> Result<String, String>;
    /// How often it was read (an accessor).
    #[getter]
    fn count(&self) -> Result<f64, String>;
}

/// Links TS feeds from Rust.
#[class]
pub(crate) struct Linker;

/// What a task reads of `feed`, with whether the runtime was entered.
fn read(feed: &JsFeed) -> String {
    format!(
        "{} {} {} {} {}",
        runtime_entered(),
        feed.object().can_run_now(),
        feed.title().unwrap_or_else(|e| e),
        feed.count().map_or_else(|e| e, |n| n.to_string()),
        feed.object()
            .get::<String>("kind")
            .unwrap_or_else(|e| e.message),
    )
}

#[class]
impl Linker {
    /// Starts a task that, after a timer, makes a feed with `make` and
    /// posts what it read of it to `done`; whether the call was in the
    /// runtime, then whether the task started.
    pub(crate) fn link(make: Callback<(), JsFeed>, done: Callback<String>) -> String {
        let entered = runtime_entered();
        let started = start_task(async move {
            timer(Duration::from_millis(1)).await;
            let read = match make.call(()) {
                Ok(feed) => read(&feed),
                Err(e) => e,
            };
            done.post(format!("{} {read}", make.can_call_now()));
        })
        .is_ok();
        format!("{entered} {started}")
    }

    /// Reads `feed` in its task, after a timer.
    pub(crate) async fn read_later(feed: JsFeed) -> String {
        timer(Duration::from_millis(1)).await;
        read(&feed)
    }
}

fn set() -> Registry {
    let mut r = Registry::ui_set("test/engine-thread");
    r.interface::<JsFeed>().class::<Linker>();
    r
}

const APP: &str = r#"import { mount, signal, Text } from "nsh";
import { Linker } from "test/engine-thread";

const shown = signal("waiting");
let reads = 0;
const feed = { title: "news", get count() { return ++reads; }, kind: "rss" };
const lines = [];
lines.push(Linker.link(() => feed, (text) => {
  lines.push(text);
  Linker.readLater(feed).then((later) => {
    lines.push(later);
    shown.set(`done ${lines.join("|")}`);
  });
}));
mount(() => <Text>{shown()}</Text>);
"#;

#[test]
fn a_task_started_in_a_call_reads_a_ts_object_between_turns() {
    let (_engine, rx) = start(APP, vec![set()]);
    assert_eq!(
        shown(&rx, "done"),
        "done false true|false true true news 1 rss|true true news 2 rss"
    );
}
