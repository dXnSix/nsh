//! The app's end ([`super::on_terminate`]; TS: `Lifecycle.on("terminate",
//! ...)`).
//!
//! The platform tells the engine that the app is about to terminate; the
//! engine has the core run the listeners ([`crate::Core::will_terminate`])
//! on its thread, once, while the platform waits for it, then runs what
//! they started and writes the app's storage out. The listeners of JS's
//! side are a task of its module, which waits for [`terminating`].

use super::{AnswerSlot, Owner, task::Answer};

/// What runs as the app terminates, under the owner that added it.
type Listener = (Owner, Box<dyn FnOnce()>);

/// The app's terminate listeners, in the order they were added.
#[derive(Default)]
pub(crate) struct TerminateListeners {
    next: u64,
    list: Vec<(u64, Listener)>,
    /// The tasks waiting for the end ([`terminating`]).
    waiting: Vec<AnswerSlot<()>>,
    /// The listeners ran: nothing runs again.
    terminated: bool,
}

impl TerminateListeners {
    pub(crate) fn add(&mut self, owner: Owner, listener: Box<dyn FnOnce()>) -> u64 {
        self.next += 1;
        if !self.terminated {
            self.list.push((self.next, (owner, listener)));
        }
        self.next
    }

    /// Removes listener `id`; returns it so it is dropped outside the
    /// borrow (it may own what removes others).
    pub(crate) fn remove(&mut self, id: u64) -> Option<Box<dyn FnOnce()>> {
        let i = self.list.iter().position(|(i, _)| *i == id)?;
        let (_, (_, listener)) = self.list.remove(i);
        Some(listener)
    }

    /// The app terminates: its listeners, oldest first, to run once, and
    /// the tasks waiting for it woken. Empty from then on.
    pub(crate) fn take(&mut self) -> Vec<Listener> {
        if std::mem::replace(&mut self.terminated, true) {
            return Vec::new();
        }
        for slot in self.waiting.drain(..) {
            slot.borrow_mut().deliver(());
        }
        self.list.drain(..).map(|(_, listener)| listener).collect()
    }
}

/// Resolves when the app is about to terminate, at once when it did; to
/// await in a task (JS's listeners: `runtime/lifecycle.rs`).
pub(crate) fn terminating() -> Answer<()> {
    let (answer, slot) = Answer::new();
    let listeners = super::ambient::executor().terminate.clone();
    let mut listeners = listeners.borrow_mut();
    if listeners.terminated {
        slot.borrow_mut().deliver(());
    } else {
        listeners.waiting.push(slot);
    }
    answer
}
