//! Class extensions (`#[class_extension(C)]`) through an engine: an
//! extension's `async` methods run as tasks of the core, its instance
//! crossing inside the task, and its methods returning `Later<T>` give
//! the task's promise, as the class's own members do.

use std::time::Duration;

use nsh_core::{element::Registry, prelude::*};

use crate::rust_classes::{shown, start};

/// Items kept in Rust.
#[class]
pub(crate) struct Shelf {
    items: Vec<String>,
}

#[class]
impl Shelf {
    /// An empty shelf.
    pub(crate) fn new() -> Self {
        Shelf { items: Vec::new() }
    }

    /// The items, joined by commas.
    #[getter]
    pub(crate) fn items(&self) -> String {
        self.items.join(",")
    }
}

/// Methods another set adds to the shelf.
pub(crate) struct ShelfLoading;

#[class_extension(Shelf)]
impl ShelfLoading {
    /// Adds `item` after `ms` milliseconds; how many items it has.
    pub(crate) async fn add_later(this: Instance<Shelf>, item: String, ms: u32) -> i64 {
        timer(Duration::from_millis(ms.into())).await;
        this.with_mut(|s| {
            s.items.push(item);
            s.items.len() as i64
        })
    }

    /// Fails, naming how many items it has.
    pub(crate) async fn fail_later(this: Instance<Shelf>) -> Result<(), String> {
        Err(format!("no, at {}", this.with(|s| s.items.len())))
    }

    /// The items joined by `separator`, by a task the call starts.
    pub(crate) fn joined_later(this: Instance<Shelf>, separator: String) -> Later<String> {
        Later::new(async move {
            timer(Duration::from_millis(1)).await;
            Ok::<_, String>(this.with(|s| s.items.join(&separator)))
        })
    }

    /// Refuses before starting a task.
    pub(crate) fn refuse_later(this: Instance<Shelf>) -> Result<Later<String>, String> {
        let _ = this;
        Err("refused before the task".to_owned())
    }
}

fn shelf_set() -> Registry {
    let mut r = Registry::ui_set("test/shelf");
    r.class::<Shelf>();
    r
}

fn loading_set() -> Registry {
    let mut r = Registry::ui_set("test/shelf-loading");
    r.class_extension::<ShelfLoading>();
    r
}

const APP: &str = r#"import { mount, signal, Text } from "nsh";
import { Shelf } from "test/shelf";

const shown = signal("waiting");
const msg = async (p) => { try { return String(await p); } catch (e) { return `${e.constructor.name}: ${e.message}`; } };
setTimeout(async () => {
  const lines = [];
  const s = new Shelf();
  const added = s.addLater("a", 10);
  lines.push(`${added instanceof Promise} ${JSON.stringify(s.items)}`);
  lines.push(String(await added));
  lines.push(String(await s.addLater("b", 1)));
  lines.push(s.items);
  lines.push(await msg(s.failLater()));
  const joined = s.joinedLater("+");
  lines.push(String(joined instanceof Promise));
  lines.push(await joined);
  lines.push(await msg(s.refuseLater()));
  lines.push(await msg(Shelf.prototype.addLater.call({}, "x", 1)));
  shown.set(`done ${lines.join("|")}`);
}, 0);

mount(() => <Text>{shown()}</Text>);
"#;

#[test]
fn async_and_later_methods_of_an_extension_give_promises_of_their_tasks() {
    let (_engine, rx) = start(APP, vec![shelf_set(), loading_set()]);
    assert_eq!(
        shown(&rx, "done"),
        "done true \"\"|1|2|a,b|Error: no, at 2|true|a+b|Error: refused before the task|\
         TypeError: Shelf.addLater: Illegal invocation: not a Shelf"
    );
}

#[test]
fn an_async_method_rejects_with_its_err() {
    const FAILS: &str = r#"import { mount, signal, Text } from "nsh";
import { Shelf } from "test/shelf";

const shown = signal("waiting");
new Shelf().failLater().then(
  () => shown.set("done resolved"),
  (e) => shown.set(`done ${e instanceof Error} ${e.message}`),
);
mount(() => <Text>{shown()}</Text>);
"#;
    let (_engine, rx) = start(FAILS, vec![shelf_set(), loading_set()]);
    assert_eq!(shown(&rx, "done"), "done true no, at 0");
}

#[test]
fn a_later_method_s_task_resolves_with_its_value() {
    const JOINED: &str = r#"import { mount, signal, Text } from "nsh";
import { Shelf } from "test/shelf";

const shown = signal("waiting");
const s = new Shelf();
s.addLater("x", 1)
  .then(() => s.addLater("y", 1))
  .then(() => s.joinedLater(" & "))
  .then((text) => shown.set(`done ${text}`));
mount(() => <Text>{shown()}</Text>);
"#;
    let (_engine, rx) = start(JOINED, vec![shelf_set(), loading_set()]);
    assert_eq!(shown(&rx, "done"), "done x & y");
}
