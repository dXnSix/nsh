//! Rust traits TS objects implement (`#[interface]`), headless: the
//! stand-in calls the JS object's methods on the spot inside a synchronous
//! call from JS, posts or awaits them outside one, reads its properties,
//! and runs the trait's default where the object has no member.

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::{Pin, pin},
    task::{Context, Poll, Waker},
};

use nsh_core::{element::Registry, prelude::*};

use crate::{Host, HostOptions};

thread_local! {
    /// The greeter `Hall.keep` keeps.
    static KEPT: RefCell<Option<JsGreeter>> = const { RefCell::new(None) };
    /// What the default `hear` heard.
    static HEARD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The `Hall`s dropped so far on this thread.
    static DROPPED: Cell<usize> = const { Cell::new(0) };
}

/// Greets people.
#[interface]
pub(crate) trait Greeter {
    /// The greeting for `name`.
    fn greet(&self, name: String) -> Result<String, String>;
    /// How loud it is (1 by default).
    fn volume(&self) -> f64 {
        1.0
    }
    /// Says `text`.
    fn say(&self, text: String);
    /// Hears `text` (Rust hears it by default).
    fn hear(&self, text: String) {
        HEARD.with(|h| h.borrow_mut().push(format!("rust {text}")));
    }
    /// Its language.
    #[getter]
    fn language(&self) -> String;
    /// Its region (`default` by default).
    #[getter]
    fn region(&self) -> Option<String> {
        Some("default".to_owned())
    }
    /// Loads `count` items from `from`; resolves with how much loaded.
    async fn load(&self, from: String, count: u32) -> Result<f64, String>;
    /// Warms up (`cold` by default).
    async fn warm(&self) -> Result<String, String> {
        Ok("cold".to_owned())
    }
    /// Its tag.
    fn tag(&self) -> Result<Instance<Tag>, String>;
}

/// A tag.
#[class]
pub(crate) struct Tag {
    name: String,
}

#[class]
impl Tag {
    pub(crate) fn new(name: String) -> Self {
        Tag { name }
    }
}

/// Uses greeters.
#[class]
pub(crate) struct Hall {
    greeters: Vec<JsGreeter>,
}

impl Drop for Hall {
    fn drop(&mut self) {
        DROPPED.with(|d| d.set(d.get() + 1));
    }
}

#[class]
impl Hall {
    pub(crate) fn new() -> Self {
        Hall {
            greeters: Vec::new(),
        }
    }

    /// `greeter`'s greeting for `name`.
    pub(crate) fn welcome(greeter: JsGreeter, name: String) -> Result<String, String> {
        greeter.greet(name)
    }

    /// How loud `greeter` is.
    pub(crate) fn volume_of(greeter: JsGreeter) -> f64 {
        greeter.volume()
    }

    /// Has `greeter` say `text`.
    pub(crate) fn tell(greeter: JsGreeter, text: String) {
        greeter.say(text);
    }

    /// Has `greeter` hear `text`.
    pub(crate) fn pass(greeter: JsGreeter, text: String) {
        greeter.hear(text);
    }

    /// `greeter`'s language.
    pub(crate) fn language_of(greeter: JsGreeter) -> String {
        greeter.language()
    }

    /// `greeter`'s region.
    pub(crate) fn region_of(greeter: JsGreeter) -> Option<String> {
        greeter.region()
    }

    /// The name of `greeter`'s tag.
    pub(crate) fn tag_of(greeter: JsGreeter) -> Result<String, String> {
        greeter.tag().map(|tag| tag.with(|t| t.name.clone()))
    }

    /// `greeter` back.
    pub(crate) fn same(greeter: JsGreeter) -> JsGreeter {
        greeter
    }

    /// Keeps `greeter` for Rust.
    pub(crate) fn keep(greeter: JsGreeter) {
        KEPT.with(|k| *k.borrow_mut() = Some(greeter));
    }

    /// Adds `greeter` to the hall.
    pub(crate) fn add(&mut self, greeter: JsGreeter) {
        self.greeters.push(greeter);
    }

    /// What the default `hear` heard.
    pub(crate) fn heard() -> Vec<String> {
        HEARD.with(|h| h.borrow().clone())
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/interfaces");
    r.interface::<JsGreeter>().class::<Hall>();
    r
}

fn host() -> Host {
    Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        ..HostOptions::default()
    })
    .unwrap()
}

/// What every test's greeter starts from.
const GREETER: &str = r#"
globalThis.said = [];
globalThis.greeter = {
    greet(name) { return `hello ${name}`; },
    say(text) { said.push(text); },
    get language() { return "en"; },
    async load(from, count) { await null; return count + 0.5; },
    tag() { return new Tag("mine"); },
};
"#;

/// Runs `code` (a module importing the classes, after [`GREETER`]); the
/// texts it shows with `show(...)`.
fn run(host: &Host, code: &str) -> Result<Vec<String>, String> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Hall, Tag }} from \"test/interfaces\";\nglobalThis.Tag = Tag;\n{code}"
    ))
    .map_err(|e| e.message)?;
    let mut ops = Vec::new();
    host.take_ops(&mut ops);
    host.apply_posts(&mut ops);
    Ok(ops
        .into_iter()
        .filter_map(|op| match op {
            nsh_core::Op::CreateText { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect())
}

/// `future`, polled once.
fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn members_run_on_the_spot_inside_a_call_from_js() {
    let host = host();
    run(&host, GREETER).unwrap();
    let shown = run(
        &host,
        r#"
        const g = greeter;
        show(Hall.welcome(g, "ann"));
        show(`${Hall.volumeOf(g)} ${Hall.volumeOf({ ...g, volume() { return 7; } })}`);
        Hall.tell(g, "now");
        Hall.pass(g, "a");
        Hall.pass({ ...g, hear(text) { said.push(`js ${text}`); } }, "b");
        show(said.join(","));
        show(Hall.heard().join(","));
        show(Hall.languageOf(g));
        show(`${Hall.regionOf(g)} ${Hall.regionOf({ ...g, region: "eu" })} ${Hall.regionOf({ ...g, region: null })}`);
        show(Hall.tagOf(g));
        show(String(Hall.same(g) === g));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "hello ann",
            "1 7",
            "now,js b",
            "rust a",
            "en",
            "default eu null",
            "mine",
            "true",
        ]
    );
}

#[test]
fn what_js_throws_or_gets_wrong_fails_the_member() {
    let host = host();
    run(&host, GREETER).unwrap();
    let shown = run(
        &host,
        r#"
        const g = greeter;
        const msg = (f) => { try { return String(f()); } catch (e) { return `${e.name}: ${e.message}`; } };
        show(msg(() => Hall.welcome({ ...g, greet() { throw new Error("shy"); } }, "x")));
        show(msg(() => Hall.welcome({ ...g, greet() { return 3; } }, "x")));
        show(msg(() => Hall.volumeOf({ ...g, volume() { throw new Error("loud"); } })));
        show(msg(() => Hall.tagOf({ ...g, tag() { return {}; } })));
        show(msg(() => Hall.welcome({}, "x")));
        show(msg(() => Hall.welcome({ ...g, say: 1 }, "x")));
        show(msg(() => Hall.welcome(3, "x")));
        show(msg(() => Hall.volumeOf({ ...g, volume: 2 })));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "Error: shy",
            "Error: Greeter.greet: expected a string, got 3",
            "Error: Greeter.volume: loud",
            "Error: Greeter.tag: expected a Tag, got an object",
            "TypeError: Hall.welcome: `greeter`: expected a Greeter, got an object whose `greet` is undefined",
            "TypeError: Hall.welcome: `greeter`: expected a Greeter, got an object whose `say` is 1",
            "TypeError: Hall.welcome: `greeter`: expected a Greeter, got 3",
            "Error: Greeter.volume: `volume` is not a function, got 2",
        ]
    );
}

#[test]
fn outside_a_call_members_are_posted_run_or_awaited() {
    let host = host();
    run(&host, GREETER).unwrap();
    run(&host, "Hall.keep(greeter);").unwrap();
    let g = KEPT.with(|k| k.borrow_mut().take()).unwrap();
    // A value is answered on the spot between JS's turns.
    assert_eq!(g.greet("x".to_owned()), Ok("hello x".to_owned()));
    // `()` is posted: in JS's next turn, a missing member's default then.
    g.say("later".to_owned());
    g.hear("unheard".to_owned());
    assert_eq!(run(&host, "show(said.join(','));").unwrap(), [""]);
    host.run_posted_without_core();
    assert_eq!(run(&host, "show(said.join(','));").unwrap(), ["later"]);
    assert!(HEARD.with(|h| h.borrow().contains(&"rust unheard".to_owned())));
    // `async` methods are awaited, their promises settled.
    let mut load = pin!(g.load("db".to_owned(), 2));
    assert!(poll(load.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(load.as_mut()), Poll::Ready(Ok(2.5)));
    let mut warm = pin!(g.warm());
    assert!(poll(warm.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(warm.as_mut()), Poll::Ready(Ok("cold".to_owned())));
    // A rejection, or a method JS returns its answer from.
    run(
        &host,
        r#"Hall.keep({
            ...greeter,
            async load() { throw new Error("offline"); },
            warm() { return "warm"; },
        });"#,
    )
    .unwrap();
    let other = KEPT.with(|k| k.borrow_mut().take()).unwrap();
    let mut load = pin!(other.load("db".to_owned(), 1));
    let mut warm = pin!(other.warm());
    // Each posts its call when first polled.
    assert!(poll(load.as_mut()).is_pending());
    assert!(poll(warm.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(load.as_mut()), Poll::Ready(Err("offline".to_owned())));
    assert_eq!(poll(warm.as_mut()), Poll::Ready(Ok("warm".to_owned())));
    // Once the VM is gone, an awaited member says so.
    drop(host);
    let mut late = pin!(g.load("db".to_owned(), 1));
    assert_eq!(
        poll(late.as_mut()),
        Poll::Ready(Err("the JS function was not called (JS stopped)".to_owned()))
    );
}

#[test]
fn a_cycle_through_a_kept_stand_in_is_collected() {
    let host = host();
    run(&host, GREETER).unwrap();
    let before = DROPPED.with(Cell::get);
    run(
        &host,
        r#"
        (() => {
            const hall = new Hall();
            hall.add({ ...greeter, hall });
        })();
        "#,
    )
    .unwrap();
    assert_eq!(
        DROPPED.with(Cell::get),
        before,
        "the cycle waits for the collector"
    );
    host.collect_garbage();
    assert_eq!(DROPPED.with(Cell::get), before + 1);
}
