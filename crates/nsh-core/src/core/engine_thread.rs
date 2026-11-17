//! Rust code on the engine thread, in the core: [`runtime_entered`] is
//! `true` where the core runs Rust (an op, a task) and `false` outside
//! it, and [`start_task`] there spawns a root task no owner ends; outside
//! an engine it gives the task back.

use std::{cell::RefCell, rc::Rc};

use super::{
    Core,
    tests::{FakeEngine, core},
};
use crate::{
    Op,
    element::{
        NotStarted, RustCall, RustOp, ToJs, create_root, runtime_entered, spawn, start_task,
    },
};

/// Runs `f` in the core, as a call into Rust that is not `async` does.
fn in_core(c: &mut Core<FakeEngine>, f: impl FnOnce() + 'static) {
    let f = RefCell::new(Some(f));
    c.apply(Op::Rust(RustOp {
        call: 0,
        run: RustCall::Op(Box::new(move |_| {
            (f.borrow_mut().take().expect("once"))();
            Ok(Box::new(()) as Box<dyn ToJs>)
        })),
    }))
    .unwrap();
}

/// Runs the core's tasks until `done` holds (at most ten runs).
fn run_until(c: &mut Core<FakeEngine>, done: impl Fn() -> bool) {
    for _ in 0..10 {
        if done() {
            return;
        }
        c.run_component_tasks();
    }
}

#[test]
fn the_runtime_is_entered_in_the_core_s_calls_and_tasks_only() {
    let mut c = core();
    assert!(!runtime_entered());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (in_op, in_task) = (seen.clone(), seen.clone());
    in_core(&mut c, move || {
        in_op.borrow_mut().push(runtime_entered());
        spawn(async move { in_task.borrow_mut().push(runtime_entered()) });
    });
    run_until(&mut c, || seen.borrow().len() == 2);
    assert_eq!(*seen.borrow(), [true, true]);
    assert!(!runtime_entered());
}

#[test]
fn a_task_started_in_the_runtime_outlives_the_owner_it_was_started_under() {
    let mut c = core();
    let ran = Rc::new(RefCell::new(Vec::new()));
    let noted = ran.clone();
    in_core(&mut c, move || {
        let (root, started) = create_root(move || {
            start_task(async move {
                crate::element::timer(std::time::Duration::ZERO).await;
                noted.borrow_mut().push("ran");
            })
            .is_ok()
        });
        assert!(started);
        // Not polled inside the code that started it.
        root.dispose();
    });
    assert!(ran.borrow().is_empty());
    run_until(&mut c, || !ran.borrow().is_empty());
    assert_eq!(*ran.borrow(), ["ran"]);
}

#[test]
fn a_panic_in_a_started_task_ends_it_alone() {
    let mut c = core();
    let ran = Rc::new(RefCell::new(0));
    let after = ran.clone();
    in_core(&mut c, move || {
        start_task(async { panic!("boom") }).unwrap();
        start_task(async move { *after.borrow_mut() += 1 }).unwrap();
    });
    run_until(&mut c, || *ran.borrow() == 1);
    assert_eq!(*ran.borrow(), 1);
    assert_eq!(c.components.executor.task_count(), 0);
}

#[test]
fn outside_an_engine_the_task_is_given_back() {
    let Err(NotStarted(work)) = start_task(async {}) else {
        panic!("started outside an engine");
    };
    drop(work);
    assert_eq!(
        NotStarted(Box::pin(async {})).to_string(),
        "no task of the core can start here: off the engine thread, or outside both NSH's \
         reactive runtime and a synchronous call from JS"
    );
}
