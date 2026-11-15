//! Rust classes passed as values (`Constructor`), headless: a registered
//! class's constructor tells Rust which class it is, any other value is
//! refused, and a constructor Rust gives back is the class itself.

use std::collections::HashMap;

use nsh_core::{element::Registry, prelude::*};

use crate::{Host, HostOptions};

/// A service.
#[class]
pub(crate) struct Clock;

#[class]
impl Clock {
    pub(crate) fn new() -> Self {
        Clock
    }
}

/// Another service, made by its members only.
#[class]
pub(crate) struct Ledger;

#[class]
impl Ledger {
    /// A ledger.
    pub(crate) fn open() -> Ledger {
        Ledger
    }
}

/// A class no signature names and no set registers.
#[class]
pub(crate) struct Hidden;

#[class]
impl Hidden {}

/// Finds services by their class.
#[class]
pub(crate) struct Services {
    names: HashMap<Constructor, String>,
}

#[class]
impl Services {
    pub(crate) fn new() -> Self {
        Services {
            names: HashMap::new(),
        }
    }

    /// The name of `service`'s class.
    pub(crate) fn name_of(service: Constructor) -> String {
        service.def().name.to_owned()
    }

    /// Whether `service` is `Clock`.
    pub(crate) fn is_clock(service: Constructor) -> bool {
        service.is::<Clock>()
    }

    /// Names `service`.
    pub(crate) fn add(&mut self, service: Constructor, name: String) {
        self.names.insert(service, name);
    }

    /// The name `service` was given.
    pub(crate) fn get(&self, service: Constructor) -> Option<String> {
        self.names.get(&service).cloned()
    }

    /// `service` back.
    pub(crate) fn echo(service: Constructor) -> Constructor {
        service
    }

    /// The class `Hidden`, which nothing registered.
    pub(crate) fn hidden() -> Constructor {
        Constructor::of::<Hidden>()
    }

    /// What `token` is: a Rust class's name, or `object`.
    pub(crate) fn token(token: JsHandle) -> String {
        match token.constructor() {
            Some(class) => format!("class {}", class.def().name),
            None => "object".to_owned(),
        }
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/constructors");
    r.class::<Clock>().class::<Ledger>().class::<Services>();
    r
}

fn host() -> Host {
    Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        ..HostOptions::default()
    })
    .unwrap()
}

/// Runs `code` (a module importing the classes); the texts it shows with
/// `show(...)`.
fn run(host: &Host, code: &str) -> Result<Vec<String>, String> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Clock, Ledger, Services }} from \"test/constructors\";\n{code}"
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
fn a_constructor_names_its_class() {
    let host = host();
    let shown = run(
        &host,
        r#"
        show(`${Services.nameOf(Clock)} ${Services.nameOf(Ledger)} ${Services.nameOf(Services)}`);
        show(`${Services.isClock(Clock)} ${Services.isClock(Ledger)}`);
        const s = new Services();
        s.add(Clock, "clock");
        s.add(Ledger, "ledger");
        s.add(Clock, "the clock");
        show(`${s.get(Clock)} ${s.get(Ledger)} ${s.get(Services)}`);
        show(`${Services.echo(Clock) === Clock} ${Services.echo(Ledger) === Ledger}`);
        show(`${Services.token(Clock)} ${Services.token(new Clock())} ${Services.token({})}`);
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "Clock Ledger Services",
            "true false",
            "the clock ledger null",
            "true true",
            "class Clock object object",
        ]
    );
}

#[test]
fn other_values_are_refused() {
    let host = host();
    let shown = run(
        &host,
        r#"
        const msg = (f) => { try { return String(f()); } catch (e) { return `${e.name}: ${e.message}`; } };
        class Mine {}
        class Later extends Clock {}
        show(msg(() => Services.nameOf(Mine)));
        show(msg(() => Services.nameOf(Later)));
        show(msg(() => Services.nameOf(() => 1)));
        show(msg(() => Services.nameOf(new Clock())));
        show(msg(() => Services.nameOf(null)));
        show(Services.token(Later));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "TypeError: Services.nameOf: `service`: expected a Rust class, got a function",
            "TypeError: Services.nameOf: `service`: expected a Rust class, got a function",
            "TypeError: Services.nameOf: `service`: expected a Rust class, got a function",
            "TypeError: Services.nameOf: `service`: expected a Rust class, got a Clock",
            "TypeError: Services.nameOf: `service`: expected a Rust class, got null",
            "object",
        ]
    );
}

#[test]
fn a_class_rust_gives_back_is_made_on_first_use() {
    let host = host();
    let shown = run(
        &host,
        r#"
        const Hidden = Services.hidden();
        show(`${Hidden.name} ${Services.hidden() === Hidden} ${Services.nameOf(Hidden)}`);
        "#,
    )
    .unwrap();
    assert_eq!(shown, ["Hidden true Hidden"]);
}
