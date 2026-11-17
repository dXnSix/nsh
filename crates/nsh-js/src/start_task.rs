//! Starting a core task from a synchronous call from JS without a
//! `CallCx` (`nsh_core::element::start_task`), headless: inside the call it
//! goes as an op after what JS sent before it, as `CallCx::start`'s does;
//! outside one (no reactive runtime either), in a finalizer and once the
//! VM is gone it is given back. The reactive runtime is not entered in the
//! call (`runtime_entered`).

use std::{
    cell::{Cell, RefCell},
    pin::pin,
    task::{Context, Poll, Waker},
};

use nsh_core::{
    Op,
    element::{NotStarted, Registry, RustCall, RustOp},
    prelude::*,
};

use crate::{Host, HostOptions};

thread_local! {
    /// What the started tasks did, in order.
    static RAN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// What the starters' drops answered.
    static DROPS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// What `Starter.start` saw of the runtime.
    static ENTERED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// The task `label` names: it notes that it ran.
async fn noting(label: String) {
    RAN.with(|r| r.borrow_mut().push(label));
}

/// Starts tasks from its calls.
#[class]
pub(crate) struct Starter {
    label: String,
}

impl Drop for Starter {
    fn drop(&mut self) {
        if !self.label.is_empty() {
            let started = start_task(noting(self.label.clone())).is_ok();
            DROPS.with(|d| d.borrow_mut().push(format!("{} {started}", self.label)));
        }
    }
}

#[class]
impl Starter {
    /// A starter whose drop starts a task under `label` (none when empty).
    pub(crate) fn new(label: String) -> Self {
        Starter { label }
    }

    /// Starts the task `label` names; whether it started.
    pub(crate) fn start(label: String) -> bool {
        ENTERED.with(|e| e.set(Some(runtime_entered())));
        start_task(noting(label)).is_ok()
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("test/start-task");
    r.class::<Starter>();
    r
}

fn host() -> Host {
    Host::with_options(HostOptions {
        ui_sets: vec![registry()],
        ..HostOptions::default()
    })
    .unwrap()
}

/// Runs `code` (a module importing the class); the ops it sent.
fn run(host: &Host, code: &str) -> Vec<Op> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Starter }} from \"test/start-task\";\n{code}"
    ))
    .unwrap();
    let mut ops = Vec::new();
    host.take_ops(&mut ops);
    ops
}

/// Polls `future` once (the tasks here end at their first poll).
fn poll_once<F: Future>(future: F) -> Poll<F::Output> {
    let mut context = Context::from_waker(Waker::noop());
    pin!(future).poll(&mut context)
}

/// What `ops` hold, in order: a text's content, `task` for a task start
/// (its future run to its end, as the core does when it applies it).
fn applied(ops: Vec<Op>) -> Vec<String> {
    ops.into_iter()
        .filter_map(|op| match op {
            Op::CreateText { text, .. } => Some(text.to_string()),
            Op::Rust(RustOp {
                run: RustCall::Task(start),
                ..
            }) => {
                assert!(poll_once(start()).is_ready());
                Some("task".to_owned())
            }
            _ => None,
        })
        .collect()
}

fn ran() -> Vec<String> {
    RAN.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

#[test]
fn a_task_starts_in_order_with_what_js_sent_before_the_call() {
    let host = host();
    let ops = run(
        &host,
        r#"
        show("before");
        show(String(Starter.start("a")));
        show("after");
        "#,
    );
    // Not run yet: the core starts it when it applies the op.
    assert!(ran().is_empty());
    assert_eq!(ENTERED.with(Cell::get), Some(false));
    assert_eq!(applied(ops), ["before", "task", "true", "after"]);
    assert_eq!(ran(), ["a"]);
}

#[test]
fn outside_a_call_and_the_runtime_the_task_is_given_back() {
    let host = host();
    run(&host, "");
    assert!(!runtime_entered());
    let Err(NotStarted(work)) = start_task(noting("back".to_owned())) else {
        panic!("started outside a call");
    };
    assert!(ran().is_empty());
    assert!(poll_once(work).is_ready());
    assert_eq!(ran(), ["back"]);
    drop(host);
}

#[test]
fn not_in_a_finalizer_nor_once_the_vm_is_gone() {
    let host = host();
    let ops = run(
        &host,
        r#"
        Starter.start("call");
        let freed = new Starter("count");
        freed = null;
        "#,
    );
    assert_eq!(applied(ops), ["task"]);
    assert_eq!(ran(), ["call"]);
    assert_eq!(
        DROPS.with(|d| std::mem::take(&mut *d.borrow_mut())),
        ["count false"]
    );
    drop(host);
    assert!(start_task(noting("gone".to_owned())).is_err());
    assert!(ran().is_empty());
}

#[test]
fn a_task_of_the_vm_in_the_call_not_of_another() {
    let first = host();
    run(&first, "");
    let second = host();
    let ops = run(&second, r#"Starter.start("second");"#);
    assert_eq!(applied(ops), ["task"]);
    let mut ops = Vec::new();
    first.take_ops(&mut ops);
    assert!(applied(ops).is_empty());
    assert_eq!(ran(), ["second"]);
}
