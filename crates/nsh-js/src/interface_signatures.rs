//! `#[ts("…")]` on an `#[interface]`'s methods, headless: the interface
//! declares what JS returns as written, and an `async fn` Rust awaits from
//! a task settles with a plain value as it does with a promise's.

use std::{
    cell::RefCell,
    future::Future,
    pin::{Pin, pin},
    task::{Context, Poll, Waker},
};

use nsh_core::{
    element::{Registry, declarations},
    prelude::*,
};

use crate::{Host, HostOptions};

thread_local! {
    /// The service `Startups.keep` keeps.
    static KEPT: RefCell<Option<JsStartable>> = const { RefCell::new(None) };
    /// The context `Startups.keepContext` keeps.
    static CONTEXT: RefCell<Option<JsHandle>> = const { RefCell::new(None) };
}

/// A service that starts with the app.
#[interface]
pub(crate) trait Startable {
    /// Whether it has started; awaited, though TS returns a plain value.
    #[ts("(): boolean")]
    async fn is_initialized(&self) -> Result<bool, String>;
    /// Starts it with `context`.
    #[ts("(context: object): Promise<void>")]
    async fn initialize(&self, context: JsHandle) -> Result<(), String>;
    /// How long it may take, if it says.
    #[ts("(): number | undefined")]
    async fn initialization_timeout(&self) -> Result<Option<f64>, String> {
        Ok(None)
    }
    /// The services it waits for.
    #[getter]
    #[ts("(): readonly string[]")]
    async fn depends_on(&self) -> Result<Vec<String>, String>;
}

/// Keeps a service for Rust.
#[class]
pub(crate) struct Startups;

#[class]
impl Startups {
    /// Keeps `service`.
    pub(crate) fn keep(service: JsStartable) {
        KEPT.with(|k| *k.borrow_mut() = Some(service));
    }

    /// Keeps `context`, for a service's `initialize`.
    pub(crate) fn keep_context(context: JsHandle) {
        CONTEXT.with(|c| *c.borrow_mut() = Some(context));
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/startup");
    r.interface::<JsStartable>().class::<Startups>();
    r
}

/// `future`, polled once.
fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn interface_members_are_declared_as_written() {
    let ts = declarations(&registry(), &[]);
    assert!(
        ts.contains(
            "export interface Startable {
  /** Whether it has started; awaited, though TS returns a plain value. */
  isInitialized(): boolean;
  /** Starts it with `context`. */
  initialize(context: object): Promise<void>;
  /** How long it may take, if it says. */
  initializationTimeout?(): number | undefined;
  /** The services it waits for. */
  readonly dependsOn: readonly string[];
}"
        ),
        "{ts}"
    );
}

#[test]
fn an_awaited_member_settles_with_a_plain_value_or_a_promise() {
    let host = Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        ..HostOptions::default()
    })
    .unwrap();
    let keep = |code: &str| {
        host.eval_source(&format!(
            "import {{ Startups }} from \"test/startup\";\nStartups.keep({code});"
        ))
        .unwrap();
        KEPT.with(|k| k.borrow_mut().take()).unwrap()
    };
    // Plain values, as the TS API declares them; no timeout member.
    let service = keep(
        r#"{
            started: false,
            isInitialized() { return this.started; },
            async initialize(context) { this.started = context.ready; },
            get dependsOn() { return ["config", "i18n"]; },
        }"#,
    );
    host.eval_source(
        "import { Startups } from \"test/startup\";
Startups.keepContext({ ready: true });",
    )
    .unwrap();
    let context = CONTEXT.with(|c| c.borrow_mut().take()).unwrap();
    let mut before = pin!(service.is_initialized());
    let mut init = pin!(service.initialize(context));
    let mut timeout = pin!(service.initialization_timeout());
    let mut depends = pin!(service.depends_on());
    // Each posts its call when first polled, and JS answers in its turn.
    assert!(poll(before.as_mut()).is_pending());
    assert!(poll(init.as_mut()).is_pending());
    assert!(poll(timeout.as_mut()).is_pending());
    assert!(poll(depends.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(before.as_mut()), Poll::Ready(Ok(false)));
    assert_eq!(poll(init.as_mut()), Poll::Ready(Ok(())));
    assert_eq!(poll(timeout.as_mut()), Poll::Ready(Ok(None)));
    assert_eq!(
        poll(depends.as_mut()),
        Poll::Ready(Ok(vec!["config".to_owned(), "i18n".to_owned()]))
    );
    let mut after = pin!(service.is_initialized());
    assert!(poll(after.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(after.as_mut()), Poll::Ready(Ok(true)));
    // Promises settle the same; `undefined` and a number for the timeout.
    let service = keep(
        r#"{
            async isInitialized() { await null; return true; },
            initialize() {},
            initializationTimeout() { return 5000; },
            dependsOn: [],
        }"#,
    );
    let mut done = pin!(service.is_initialized());
    let mut timeout = pin!(service.initialization_timeout());
    assert!(poll(done.as_mut()).is_pending());
    assert!(poll(timeout.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(done.as_mut()), Poll::Ready(Ok(true)));
    assert_eq!(poll(timeout.as_mut()), Poll::Ready(Ok(Some(5000.0))));
    let undefined = keep(
        r#"{
            isInitialized() { return true; },
            initialize() {},
            initializationTimeout() { return undefined; },
            dependsOn: [],
        }"#,
    );
    let mut timeout = pin!(undefined.initialization_timeout());
    assert!(poll(timeout.as_mut()).is_pending());
    host.run_posted_without_core();
    assert_eq!(poll(timeout.as_mut()), Poll::Ready(Ok(None)));
}
