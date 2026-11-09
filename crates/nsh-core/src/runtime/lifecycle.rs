//! `nsh`'s `Lifecycle`: the app's end (`terminate`), beside `lifecycle()`,
//! whether it is in use. Needs no capability.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use crate::element::{Callback, JsError, Local, PropEnum, Stop, System, module};

/// The app's lifecycle events.
#[module]
pub(crate) struct Lifecycle;

#[module]
impl Lifecycle {
    /// Listens to `event`. `terminate`: the app is about to terminate, the
    /// last chance to save what the user has not saved. The listeners run
    /// once, synchronously, in the order they were added; a promise they
    /// start settles only if it does at once, as `Storage`'s calls do,
    /// which reach the device before the app ends. The system does not
    /// always say it ends an app (one suspended in the background is ended
    /// without a word), so save when `lifecycle()` is `"background"` too.
    /// Returns a function that stops listening; it also stops when what
    /// called `on` goes (a component).
    pub(crate) fn on(
        event: LifecycleEvent,
        listener: Callback<()>,
        sys: System,
        cx: crate::element::CallCx<'_>,
    ) -> Result<Stop, JsError> {
        let LifecycleEvent::Terminate = event;
        let listeners = sys.locals.get::<Listeners>();
        let id = listeners.next.get();
        listeners.next.set(id.wrapping_add(1));
        listeners.terminate.borrow_mut().push((id, listener));
        if !listeners.started.replace(true) {
            cx.start(deliver(Rc::downgrade(&listeners)));
        }
        let listeners = Rc::downgrade(&listeners);
        Ok(Stop::new(move || {
            if let Some(listeners) = listeners.upgrade() {
                listeners.terminate.borrow_mut().retain(|(i, _)| *i != id);
            }
        }))
    }
}

/// `Lifecycle.on`'s events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PropEnum)]
pub(crate) enum LifecycleEvent {
    /// The app is about to terminate.
    #[default]
    Terminate,
}

/// The VM's `terminate` listeners.
#[derive(Default)]
struct Listeners {
    terminate: RefCell<Vec<(u32, Callback<()>)>>,
    next: Cell<u32>,
    /// The task waiting for the app's end started; it lasts as the VM does.
    started: Cell<bool>,
}

impl Local for Listeners {
    fn close(&self) {
        self.terminate.borrow_mut().clear();
    }
}

/// Calls the listeners when the app terminates, in JS's turn right after
/// the Rust listeners ran ([`crate::Core::will_terminate`]).
async fn deliver(listeners: Weak<Listeners>) {
    crate::element::terminate::terminating().await;
    let Some(listeners) = listeners.upgrade() else {
        return;
    };
    let terminate = std::mem::take(&mut *listeners.terminate.borrow_mut());
    for (_, listener) in terminate {
        listener.post(());
    }
}
