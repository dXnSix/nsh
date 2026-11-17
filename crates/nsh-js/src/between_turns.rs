//! Running JS from Rust between JS's turns, headless: an interface's
//! `#[getter]`, its methods returning a value, a kept object's property
//! (`JsHandle::get`) and a function (`Callback::call`) run on the spot from
//! Rust code that is not in a call from JS (a task, a factory), as inside
//! one; a method returning `()` is still posted there; not in a finalizer,
//! while JS of another VM runs, or once the VM is gone; a call that
//! overruns the budget is interrupted and the VM goes on.

use std::{cell::RefCell, time::Duration};

use nsh_core::{element::Registry, js::Limits, prelude::*};

use crate::{Host, HostOptions};

thread_local! {
    /// What `Holder.keep` keeps: an object and its stand-in.
    static KEPT: RefCell<Option<(JsHandle, JsStore)>> = const { RefCell::new(None) };
    /// What `Holder.keepFn` keeps.
    static KEPT_FN: RefCell<Option<Callback<f64, String>>> = const { RefCell::new(None) };
    /// What the holders' drops answered.
    static DROPS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// State a TS implementation publishes.
#[interface]
pub(crate) trait Store {
    /// Its label.
    #[getter]
    fn label(&self) -> Result<String, String>;
    /// How many reads it counted (an accessor).
    #[getter]
    fn reads(&self) -> Result<f64, String>;
    /// Its limit (none by default).
    #[getter]
    fn limit(&self) -> Option<f64> {
        None
    }
    /// Its name, as a method.
    fn name(&self) -> Result<String, String>;
    /// Notes `text`.
    fn note(&self, text: String);
}

/// Holds stores for Rust.
#[class]
pub(crate) struct Holder {
    label: String,
    watched: Option<JsHandle>,
}

impl Drop for Holder {
    fn drop(&mut self) {
        if let Some(object) = &self.watched {
            let answer = object.can_run_now();
            let read = object.get::<String>("label").is_ok();
            DROPS.with(|d| {
                d.borrow_mut()
                    .push(format!("{} {answer} {read}", self.label))
            });
        }
    }
}

#[class]
impl Holder {
    /// A holder watching nothing.
    pub(crate) fn new() -> Self {
        Holder {
            label: String::new(),
            watched: None,
        }
    }

    /// Keeps `object` (and its stand-in) for Rust.
    pub(crate) fn keep(object: JsHandle) {
        let store = JsStore::new(object.clone());
        KEPT.with(|k| *k.borrow_mut() = Some((object, store)));
    }

    /// Reads `store`'s getters and its `label` property inside this call.
    pub(crate) fn read_here(store: JsStore) -> String {
        let object = store.object().clone();
        format!(
            "{} {} {:?} {}",
            object.can_run_now(),
            store.label().unwrap_or_else(|e| e),
            store.limit(),
            object.get::<String>("label").unwrap_or_else(|e| e.message),
        )
    }

    /// Reads the kept store's label (`none` without one): the other VM's,
    /// asked inside a call of this one.
    pub(crate) fn read_kept() -> String {
        KEPT.with(|k| {
            k.borrow().as_ref().map_or_else(
                || "none".to_owned(),
                |(object, store)| {
                    format!(
                        "{} {}",
                        object.can_run_now(),
                        store.label().unwrap_or_else(|e| e)
                    )
                },
            )
        })
    }

    /// Keeps `f` for Rust.
    pub(crate) fn keep_fn(f: Callback<f64, String>) {
        KEPT_FN.with(|k| *k.borrow_mut() = Some(f));
    }

    /// A count Rust gives JS (a read's JS calling Rust).
    pub(crate) fn seven() -> i64 {
        7
    }

    /// Reads `object`'s label when it drops, answering under `label`.
    pub(crate) fn watch(&mut self, label: String, object: JsHandle) {
        self.label = label;
        self.watched = Some(object);
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/between-turns");
    r.interface::<JsStore>().class::<Holder>();
    r
}

fn host_with(limits: Limits) -> Host {
    Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        limits,
        ..HostOptions::default()
    })
    .unwrap()
}

fn host() -> Host {
    host_with(Limits::default())
}

/// What every test's store starts from.
const STORE: &str = r#"
globalThis.counted = 0;
globalThis.store = {
    label: "ann",
    get reads() { return ++counted; },
    name() { return "store"; },
    notes: [],
    note(text) { this.notes.push(text); },
    nested: { size: 3 },
};
"#;

/// Runs `code` (a module importing the class, after [`STORE`]); the texts
/// it shows with `show(...)`.
fn run(host: &Host, code: &str) -> Result<Vec<String>, String> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Holder }} from \"test/between-turns\";\nglobalThis.Holder = Holder;\n{code}"
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

/// A host whose store `Holder.keep` kept: the object and its stand-in.
fn kept_store(host: &Host) -> (JsHandle, JsStore) {
    run(host, STORE).unwrap();
    run(host, "Holder.keep(store);").unwrap();
    KEPT.with(|k| k.borrow_mut().take()).expect("kept")
}

#[test]
fn a_getter_is_read_between_turns() {
    let host = host();
    let (object, store) = kept_store(&host);
    // No JS runs: Rust reads as a task would.
    assert!(object.can_run_now());
    assert!(!object.can_call_now());
    assert_eq!(store.label(), Ok("ann".to_owned()));
    // An accessor runs each time.
    assert_eq!(store.reads(), Ok(1.0));
    assert_eq!(store.reads(), Ok(2.0));
    // An optional getter the object lacks runs the default.
    assert_eq!(store.limit(), None);
    run(&host, "store.limit = 5;").unwrap();
    assert_eq!(store.limit(), Some(5.0));
    assert!(!host.has_exception());
}

#[test]
fn a_method_returning_a_value_runs_between_turns_and_one_returning_unit_is_posted() {
    let host = host();
    let (object, store) = kept_store(&host);
    assert_eq!(store.name(), Ok("store".to_owned()));
    store.note("later".to_owned());
    assert_eq!(object.get::<Vec<String>>("notes"), Ok(Vec::new()));
    host.run_posted_without_core();
    assert_eq!(
        object.get::<Vec<String>>("notes"),
        Ok(vec!["later".to_owned()])
    );
}

#[test]
fn a_function_is_called_between_turns() {
    let host = host();
    run(&host, STORE).unwrap();
    run(
        &host,
        r#"Holder.keepFn((n) => { if (n < 0) throw new Error("negative"); return `${n * Holder.seven()}`; });"#,
    )
    .unwrap();
    let f = KEPT_FN.with(|k| k.borrow_mut().take()).expect("kept");
    assert!(f.can_run_now());
    assert!(!f.can_call_now());
    assert_eq!(f.call(2.0), Ok("14".to_owned()));
    assert_eq!(f.call(-1.0), Err("negative".to_owned()));
    assert!(!host.has_exception());
    drop(host);
    assert!(!f.can_run_now());
    assert!(f.call(1.0).is_err());
}

#[test]
fn a_kept_object_s_property_is_read_between_turns() {
    let host = host();
    let (object, _) = kept_store(&host);
    assert_eq!(object.get::<String>("label"), Ok("ann".to_owned()));
    assert_eq!(object.get::<f64>("reads"), Ok(1.0));
    assert_eq!(object.get::<Option<f64>>("missing"), Ok(None));
    let nested = object.get::<JsHandle>("nested").unwrap();
    assert_eq!(nested.get::<i64>("size"), Ok(3));
    // The same object back: a kept handle of it.
    assert_eq!(object.get::<JsHandle>("nested"), Ok(nested));
    assert_eq!(
        object.get::<f64>("label").map_err(|e| e.message),
        Err("`label`: expected a number, got \"ann\"".to_owned())
    );
}

#[test]
fn what_a_getter_throws_is_the_err() {
    let host = host();
    let (object, store) = kept_store(&host);
    run(
        &host,
        r#"Object.defineProperty(store, "label", { get() { throw new Error("no label"); } });"#,
    )
    .unwrap();
    assert_eq!(store.label(), Err("no label".to_owned()));
    assert_eq!(
        object.get::<String>("label").map_err(|e| e.message),
        Err("no label".to_owned())
    );
    assert!(!host.has_exception());
    // The VM goes on.
    assert_eq!(store.reads(), Ok(1.0));
}

#[test]
fn a_read_s_js_may_call_rust() {
    let host = host();
    let (_, store) = kept_store(&host);
    run(
        &host,
        r#"Object.defineProperty(store, "label", { get() { return `n${Holder.seven()}`; } });"#,
    )
    .unwrap();
    assert_eq!(store.label(), Ok("n7".to_owned()));
}

#[test]
fn reads_inside_a_call_from_js_run_on_the_spot() {
    let host = host();
    run(&host, STORE).unwrap();
    assert_eq!(
        run(&host, "show(Holder.readHere(store));").unwrap(),
        ["true ann None ann"]
    );
}

#[test]
fn a_read_that_overruns_its_budget_is_interrupted_and_the_vm_goes_on() {
    let host = host_with(Limits {
        call_budget: Duration::from_millis(50),
        ..Limits::default()
    });
    let (object, store) = kept_store(&host);
    run(
        &host,
        r#"Object.defineProperty(store, "label", { get() { for (;;) {} } });"#,
    )
    .unwrap();
    let start = std::time::Instant::now();
    let read = store.label();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(read.is_err(), "{read:?}");
    assert!(host.fatal().is_none());
    assert!(!host.has_exception());
    assert_eq!(store.reads(), Ok(1.0));
    assert!(object.get::<JsHandle>("nested").is_ok());
    assert_eq!(run(&host, "show(String(counted));").unwrap(), ["1"]);
}

#[test]
fn no_read_in_a_finalizer() {
    let host = host();
    run(&host, STORE).unwrap();
    // Holders freed by their count inside a call, and by the collector
    // between turns: each drop asks about a live object.
    run(
        &host,
        r#"
        let freed = new Holder();
        freed.watch("count", store);
        freed = null;
        for (let i = 0; i < 20; i++) {
            const h = new Holder();
            h.watch("cycle", { h });
        }
        "#,
    )
    .unwrap();
    host.collect_garbage();
    let drops = DROPS.with(|d| std::mem::take(&mut *d.borrow_mut()));
    assert!(drops.contains(&"count false false".to_owned()), "{drops:?}");
    assert!(
        drops.iter().filter(|d| d.starts_with("cycle")).count() >= 19,
        "{drops:?}"
    );
    assert!(
        drops.iter().all(|d| d.ends_with(" false false")),
        "{drops:?}"
    );
    assert!(!host.has_exception());
}

#[test]
fn no_read_once_the_vm_is_gone() {
    let host = host();
    let (object, store) = kept_store(&host);
    drop(host);
    assert!(!object.can_run_now());
    assert!(object.get::<String>("label").is_err());
    assert_eq!(
        store.label(),
        Err(
            "Store.label: called while JS runs on this thread without a call from it to Rust (a \
             finalizer, another VM), or once the VM is gone"
                .to_owned()
        )
    );
}

#[test]
fn no_read_of_another_vm_s_object_while_js_runs() {
    let first = host();
    let (object, store) = kept_store(&first);
    KEPT.with(|k| *k.borrow_mut() = Some((object, store)));
    let second = host();
    run(&second, STORE).unwrap();
    assert_eq!(
        run(
            &second,
            "show(Holder.readKept()); show(Holder.readHere(store));"
        )
        .unwrap(),
        [
            "false Store.label: called while JS runs on this thread without a call from it to \
             Rust (a finalizer, another VM), or once the VM is gone",
            "true ann None ann"
        ]
    );
    drop(second);
    // Between the turns of both, the first VM's object reads again.
    let (object, store) = KEPT.with(|k| k.borrow_mut().take()).expect("kept");
    assert_eq!(store.label(), Ok("ann".to_owned()));
    drop(first);
    drop((object, store));
}
