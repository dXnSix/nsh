//! Where Rust code on the engine thread runs, and starting core tasks from
//! anywhere there: [`runtime_entered`] tells whether NSH's reactive
//! runtime is entered (signals, [`super::spawn`]), and [`start_task`]
//! starts a task of the core from inside that runtime or from a
//! synchronous call from JS, where no [`super::CallCx`] is at hand.

use std::{
    cell::RefCell,
    fmt,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
};

use super::{TaskStart, ToJs};

/// Whether NSH's reactive runtime is entered on this thread: the engine
/// thread while it runs Rust code of the core (a task, a component, an
/// `async` member's body, an op), where signals, [`super::spawn`] and the
/// other free functions work. `false` inside a synchronous call from JS (a
/// `#[class]` member or a module function that is not `async`), off the
/// engine thread, and outside an engine (a unit test of a model). Asking
/// costs a thread-local read.
pub fn runtime_entered() -> bool {
    super::reactive::try_with_data(|_| ()).is_some()
}

/// A task [`start_task`] could not start, given back (to queue it some
/// other way).
pub struct NotStarted(pub Pin<Box<dyn Future<Output = ()>>>);

impl fmt::Debug for NotStarted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NotStarted")
    }
}

impl fmt::Display for NotStarted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "no task of the core can start here: off the engine thread, or outside both NSH's \
             reactive runtime and a synchronous call from JS",
        )
    }
}

impl std::error::Error for NotStarted {}

/// Starts `work` as a task of the core: a root task (it ends when `work`
/// does, or with the engine; no owner ends it, and nothing waits for it).
///
/// - Where [`runtime_entered`] is `true` (a task, a component), it is
///   spawned on the spot, and first polled at the executor's next run, as
///   [`super::spawn`]'s are.
/// - Inside a synchronous call from JS (a `#[class]` member, a module
///   function that is not `async`), it starts as `CallCx::start` starts
///   one: in order with what JS sent before the call, when the core takes
///   this turn's ops, with no extra turn.
/// - Elsewhere (off the engine thread, in a finalizer, once the VM is
///   gone, outside an engine) it is not started: `Err` gives `work` back.
///
/// A panic in `work` ends the task and is logged, as a task `CallCx::start`
/// starts.
pub fn start_task(work: impl Future<Output = ()> + 'static) -> Result<(), NotStarted> {
    start_boxed(Box::pin(work))
}

fn start_boxed(work: Pin<Box<dyn Future<Output = ()>>>) -> Result<(), NotStarted> {
    if let Some(executor) = super::ambient::try_executor() {
        super::task::spawn_raw(
            &executor,
            crate::ROOT_ID,
            Box::pin(async move {
                if let Err(e) = super::module::catching(Box::pin(async move {
                    work.await;
                    Ok(Box::new(()) as Box<dyn ToJs>)
                }))
                .await
                {
                    log::warn!(target: "js", "a task Rust started failed: {e}");
                }
            }),
        );
        return Ok(());
    }
    match in_call() {
        Some(tasks) => {
            tasks.start_task(Box::new(move || {
                Box::pin(async move {
                    work.await;
                    Ok(Box::new(()) as Box<dyn ToJs>)
                })
            }));
            Ok(())
        }
        None => Err(NotStarted(work)),
    }
}

/// A JS host's way to start core tasks from inside a synchronous call from
/// JS of its VM, registered once per VM ([`register_call_tasks`]): `nsh-js`
/// and the web player implement it.
#[doc(hidden)]
pub trait CallTasks {
    /// Whether a synchronous call from JS of this VM runs now (a task
    /// started now goes in order with what JS sent before it).
    fn in_call(&self) -> bool;
    /// Starts `task` as [`super::CallCx::start`] does: an op after what JS
    /// sent before this call.
    fn start_task(&self, task: TaskStart);
}

thread_local! {
    /// The JS hosts of this thread's VMs ([`register_call_tasks`]).
    static CALL_TASKS: RefCell<Vec<Weak<dyn CallTasks>>> = const { RefCell::new(Vec::new()) };
}

/// Lets [`start_task`] start tasks through `tasks` inside a synchronous
/// call from JS of its VM, for as long as it lives.
#[doc(hidden)]
pub fn register_call_tasks(tasks: Weak<dyn CallTasks>) {
    CALL_TASKS.with(|c| {
        let mut c = c.borrow_mut();
        c.retain(|t| t.strong_count() > 0);
        c.push(tasks);
    });
}

/// The VM a synchronous call from JS runs in now: the one VM in a call
/// (two VMs in calls at once, one's JS calling the other's Rust, leave it
/// open which one the task belongs to: none).
fn in_call() -> Option<Rc<dyn CallTasks>> {
    let hosts: Vec<Rc<dyn CallTasks>> =
        CALL_TASKS.with(|c| c.borrow().iter().filter_map(Weak::upgrade).collect());
    let mut calling = hosts.into_iter().filter(|t| t.in_call());
    let one = calling.next()?;
    calling.next().is_none().then_some(one)
}
