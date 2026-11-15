//! JS objects Rust keeps (`JsHandle`), headless: taken from JS, compared
//! by identity, given back as the same object, traced like callbacks and
//! dead once their VM is gone.

use std::cell::{Cell, RefCell};

use nsh_core::{element::Registry, prelude::*};

use crate::{Host, HostOptions};

thread_local! {
    /// The `Keeper`s dropped so far on this thread.
    static DROPPED: Cell<usize> = const { Cell::new(0) };
    /// An object kept outside any instance (a root).
    static STASHED: RefCell<Option<JsHandle>> = const { RefCell::new(None) };
}

fn dropped() -> usize {
    DROPPED.with(Cell::get)
}

/// Keeps the objects JS gives it.
#[class]
pub(crate) struct Keeper {
    kept: Vec<JsHandle>,
}

impl Drop for Keeper {
    fn drop(&mut self) {
        DROPPED.with(|d| d.set(d.get() + 1));
    }
}

#[class]
impl Keeper {
    pub(crate) fn new() -> Self {
        Keeper { kept: Vec::new() }
    }

    /// Keeps `object`; how many objects it keeps.
    pub(crate) fn keep(&mut self, object: JsHandle) -> usize {
        self.kept.push(object);
        self.kept.len()
    }

    /// Whether `object` is one it keeps.
    pub(crate) fn has(&self, object: JsHandle) -> bool {
        self.kept.contains(&object)
    }

    /// The object kept at `index`.
    pub(crate) fn at(&self, index: u32) -> Option<JsHandle> {
        self.kept.get(index as usize).cloned()
    }

    /// How many different objects `objects` holds.
    pub(crate) fn distinct(objects: Vec<JsHandle>) -> usize {
        objects
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    /// `object`, or `null`.
    pub(crate) fn maybe(object: Option<JsHandle>) -> Option<JsHandle> {
        object
    }

    /// Calls `make` and keeps the object it returns.
    pub(crate) fn make(&mut self, make: Callback<(), JsHandle>) -> Result<usize, String> {
        let made = make.call(())?;
        Ok(self.keep(made))
    }

    /// Keeps `object` outside any instance.
    pub(crate) fn stash(object: JsHandle) {
        STASHED.with(|s| *s.borrow_mut() = Some(object));
    }

    /// The object kept outside any instance.
    pub(crate) fn stashed() -> Option<JsHandle> {
        STASHED.with(|s| s.borrow().clone())
    }

    /// Lets go of the object kept outside any instance.
    pub(crate) fn unstash() {
        STASHED.with(|s| s.borrow_mut().take());
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/handles");
    r.class::<Keeper>();
    r
}

fn host() -> Host {
    Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        ..HostOptions::default()
    })
    .unwrap()
}

/// Runs `code` (a module importing `Keeper`); the texts it shows with
/// `show(...)`.
fn run(host: &Host, code: &str) -> Result<Vec<String>, String> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Keeper }} from \"test/handles\";\n{code}"
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

#[test]
fn objects_come_back_as_themselves_and_compare_by_identity() {
    let host = host();
    let shown = run(
        &host,
        r#"
        const msg = (f) => { try { return String(f()); } catch (e) { return `${e.name}: ${e.message}`; } };
        const k = new Keeper();
        const o = { name: "o" }, f = () => 1, a = [1];
        k.keep(o); k.keep(f); k.keep(a); k.keep(k);
        show(`${k.at(0) === o} ${k.at(1) === f} ${k.at(2) === a} ${k.at(3) === k}`);
        show(`${k.has(o)} ${k.has({ name: "o" })} ${k.has(f)}`);
        show(String(Keeper.distinct([o, o, f, a, o, {}])));
        show(`${Keeper.maybe(o) === o} ${Keeper.maybe(null)} ${Keeper.maybe(undefined)}`);
        k.make(() => o);
        show(`${k.at(4) === o} ${k.has(k.at(4))}`);
        show(msg(() => k.keep(3)));
        show(msg(() => k.keep("o")));
        show(msg(() => k.keep(null)));
        show(msg(() => k.make(() => 3)));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "true true true true",
            "true false true",
            "4",
            "true null null",
            "true true",
            "TypeError: Keeper.keep: `object`: expected an object, got 3",
            "TypeError: Keeper.keep: `object`: expected an object, got \"o\"",
            "TypeError: Keeper.keep: `object`: expected an object, got null",
            "Error: expected an object, got 3",
        ]
    );
}

#[test]
fn a_cycle_through_a_kept_object_is_collected() {
    let host = host();
    let before = dropped();
    run(
        &host,
        r#"
        (() => {
            const k = new Keeper();
            k.keep({ owner: k });
        })();
        "#,
    )
    .unwrap();
    assert_eq!(dropped(), before, "the cycle waits for the collector");
    host.collect_garbage();
    assert_eq!(dropped(), before + 1, "the instance's object was reported");
    // Kept outside any instance, an object is a root.
    run(
        &host,
        r#"
        (() => {
            const k = new Keeper();
            Keeper.stash({ owner: k });
        })();
        "#,
    )
    .unwrap();
    host.collect_garbage();
    assert_eq!(
        dropped(),
        before + 1,
        "a stashed object keeps what it reaches"
    );
    run(&host, "Keeper.unstash();").unwrap();
    host.collect_garbage();
    assert_eq!(dropped(), before + 2);
}

#[test]
fn an_object_lives_while_rust_keeps_it() {
    let host = host();
    run(
        &host,
        r#"
        Keeper.stash({ label: "kept" });
        "#,
    )
    .unwrap();
    host.collect_garbage();
    let shown = run(&host, "show(Keeper.stashed().label);").unwrap();
    assert_eq!(shown, ["kept"]);
    run(&host, "Keeper.unstash();").unwrap();
}

#[test]
fn a_handle_is_its_vms_only_and_dead_after_it() {
    let first = host();
    run(&first, "globalThis.o = {}; Keeper.stash(o);").unwrap();
    let kept = STASHED.with(|s| s.borrow().clone()).unwrap();
    // Another VM on the thread cannot get the object.
    let second = host();
    let shown = run(
        &second,
        r#"
        try { Keeper.stashed(); show("no throw"); } catch (e) { show(`${e.name}: ${e.message}`); }
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        ["Error: the object belongs to a VM that is gone, or another"]
    );
    drop(second);
    // Once its VM is gone the handle is dead, and still equal to itself.
    drop(first);
    let again = STASHED.with(|s| s.borrow_mut().take()).unwrap();
    assert_eq!(kept, again);
    drop((kept, again));
}
