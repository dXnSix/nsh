//! Rust's theme writes (`set_modifier`, `set_color_scheme`,
//! `update_tokens`, `reset_tokens`) inside a synchronous call from JS,
//! where no reactive runtime is entered: each goes as the root's op through
//! the VM's [`CallTasks`], the op JS's own write sends. Inside the runtime
//! they stay actions of the executor, and outside both they panic as
//! before.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::tests::core;
use crate::{
    Op, Prop, ROOT_ID,
    element::{
        CallTasks, Patch, PatchValues, RustCall, RustOp, TaskStart, ToJs, register_call_tasks,
        reset_all_tokens, reset_tokens, runtime_entered, set_color_scheme, set_modifier,
        update_tokens,
    },
    platform::Appearance,
};

/// A VM's host as the core sees it: in a call while `calling` is set,
/// keeping the ops it is handed.
#[derive(Default)]
struct FakeCall {
    calling: Cell<bool>,
    ops: RefCell<Vec<Op>>,
}

impl CallTasks for FakeCall {
    fn in_call(&self) -> bool {
        self.calling.get()
    }

    fn push_op(&self, op: Op) {
        self.ops.borrow_mut().push(op);
    }
}

/// A [`FakeCall`] registered for this thread.
fn registered() -> Rc<FakeCall> {
    let call = Rc::new(FakeCall::default());
    let tasks: Rc<dyn CallTasks> = call.clone();
    register_call_tasks(Rc::downgrade(&tasks));
    call
}

/// The root props of `ops`, in order (anything else: `None`).
fn root_props(ops: Vec<Op>) -> Vec<Option<Prop>> {
    ops.into_iter()
        .map(|op| match op {
            Op::SetProp { id: ROOT_ID, prop } => Some(prop),
            _ => None,
        })
        .collect()
}

#[test]
fn inside_a_call_each_write_is_the_root_op_js_sends() {
    let call = registered();
    call.calling.set(true);
    assert!(!runtime_entered());
    set_modifier("colorMode", "dark");
    set_color_scheme(Appearance::Dark);
    update_tokens(Patch::new().value("radius-card", 12.0));
    reset_tokens(["radius-card"]);
    reset_all_tokens();
    call.calling.set(false);
    let patch = Patch::new().value("radius-card", 12.0);
    assert_eq!(
        root_props(call.ops.take()),
        [
            Some(Prop::UiModifier("colorMode".into(), "dark".into())),
            Some(Prop::Appearance(Appearance::Dark)),
            Some(Prop::UiThemeUpdate(std::sync::Arc::new(patch))),
            Some(Prop::UiThemeReset(Some(vec!["radius-card".into()]))),
            Some(Prop::UiThemeReset(None)),
        ]
    );
}

#[test]
fn a_task_started_in_a_call_goes_as_an_op_through_the_same_host() {
    let call = registered();
    call.calling.set(true);
    let task: TaskStart = Box::new(|| Box::pin(async { Ok(Box::new(()) as Box<dyn ToJs>) }));
    CallTasks::start_task(&*call, task);
    call.calling.set(false);
    let ops = call.ops.take();
    assert!(
        matches!(
            ops.as_slice(),
            [Op::Rust(RustOp {
                call: 0,
                run: RustCall::Task(_)
            })]
        ),
        "{ops:?}"
    );
}

#[test]
fn inside_the_runtime_a_write_stays_the_executor_s() {
    let call = registered();
    // Even while a call says it runs: the runtime is asked first.
    call.calling.set(true);
    let mut c = core();
    c.apply(Op::Rust(RustOp {
        call: 0,
        run: RustCall::Op(Box::new(|_| {
            set_modifier("colorMode", "dark");
            Ok(Box::new(()) as Box<dyn ToJs>)
        })),
    }))
    .unwrap();
    call.calling.set(false);
    assert!(call.ops.borrow().is_empty());
}

#[test]
fn outside_a_call_and_the_runtime_a_write_panics_as_before() {
    let call = registered();
    let panicked = std::panic::catch_unwind(|| set_modifier("colorMode", "dark")).is_err();
    assert!(panicked);
    assert!(call.ops.borrow().is_empty());
}
