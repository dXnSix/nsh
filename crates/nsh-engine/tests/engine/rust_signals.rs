//! A JS signal Rust makes and writes (`CallCx::signal`, `JsSignal`),
//! through an engine: a synchronous call returns it, JS reads it tracked
//! (an effect follows it), and Rust writes it inside a call (on the spot),
//! from a core task between JS's turns, and from the drop of a value JS
//! disposes (`dispose()` runs as a call from JS: what the drop calls back
//! runs on the spot).

use std::{cell::RefCell, time::Duration};

use nsh_core::{element::Registry, prelude::*};

use crate::rust_classes::{shown, start};

thread_local! {
    /// The signal `Counter.make` made.
    static COUNT: RefCell<Option<JsSignal<f64>>> = const { RefCell::new(None) };
}

fn write(value: f64) {
    COUNT.with_borrow(|count| {
        if let Some(count) = count {
            count.set(value);
        }
    });
}

/// A count Rust keeps for JS.
#[module]
pub(crate) struct Counter;

#[module]
impl Counter {
    /// A new signal holding `start`, kept to write.
    pub(crate) fn make(start: f64, cx: CallCx) -> Result<JsSignal<f64>, String> {
        let count = cx.signal(start).map_err(|error| error.0)?;
        COUNT.with_borrow_mut(|kept| *kept = Some(count.clone()));
        Ok(count)
    }

    /// Writes `value` inside this call.
    pub(crate) fn set_now(value: f64) {
        write(value);
    }

    /// Writes `value` from a core task, after a timer.
    pub(crate) fn set_later(value: f64) {
        let started = start_task(async move {
            timer(Duration::from_millis(1)).await;
            write(value);
        });
        assert!(started.is_ok(), "the task starts in a call");
    }
}

/// Writes the count when it goes.
#[class]
pub(crate) struct Writer {
    value: f64,
}

impl Drop for Writer {
    fn drop(&mut self) {
        write(self.value);
    }
}

#[class]
impl Writer {
    /// A writer of `value`, written when it goes.
    pub(crate) fn new(value: f64) -> Self {
        Self { value }
    }
}

fn set() -> Registry {
    let mut r = Registry::ui_set("test/rust-signals");
    r.module::<Counter>().class::<Writer>();
    r
}

const APP: &str = r#"import { effect, mount, signal, Text } from "nsh";
import { Counter, Writer } from "test/rust-signals";

const shown = signal("waiting");
const seen = [];
const count = Counter.make(1);
effect(() => seen.push(count()));
Counter.setNow(2);
Counter.setLater(3);
setTimeout(() => {
  new Writer(4).dispose();
  const disposed = count();
  shown.set(`done ${seen.join(",")} ${disposed}`);
}, 50);
mount(() => <Text>{shown()}</Text>);
"#;

#[test]
fn rust_writes_a_signal_it_made_for_js() {
    let (_engine, rx) = start(APP, vec![set()]);
    assert_eq!(shown(&rx, "done"), "done 1,2,3,4 4");
}
