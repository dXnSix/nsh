//! A JS signal Rust makes and writes ([`JsSignal`]): a synchronous call from
//! JS makes it ([`super::CallCx::signal`]) and returns it, JS reads it by
//! calling it (tracked, as any signal of its graph), and Rust writes it
//! later on the engine thread.

use std::{fmt, marker::PhantomData};

use super::{Arg, Callback, ClassDef, JsHandle, JsHost, JsOwned, JsValue, PropError, Ret, TsTypes};

/// A signal of the call's VM that Rust writes: what a synchronous call from
/// JS returns, for JS to read (`accessor()`, tracked; TS
/// `import("nsh").Accessor<T>`: the declarations give JS no `set`), and
/// keeps to write later. Writing it ([`Self::set`]) runs on the spot where
/// JS can run now (a synchronous call from JS, a task between JS's turns),
/// else in the VM's next turn; once the VM is gone it does nothing. A clone
/// is the same signal. It lives on the engine thread: it is not `Send`.
pub struct JsSignal<T> {
    object: JsHandle,
    set: Callback<T>,
    value: PhantomData<fn(T)>,
}

impl<T> Clone for JsSignal<T> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            set: self.set.clone(),
            value: PhantomData,
        }
    }
}

impl<T> fmt::Debug for JsSignal<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JsSignal")
    }
}

impl<T: JsValue + 'static> JsSignal<T> {
    /// A new signal of `host`'s VM holding `value`.
    pub(crate) fn new(host: &dyn JsHost, value: T) -> Result<Self, PropError> {
        let value = value.to_js(host)?;
        let object = JsOwned::new(host, host.new_signal(value.get().raw())?);
        let setter = JsOwned::new(host, host.field(object.get().raw(), "set")?);
        Ok(Self {
            object: <JsHandle as JsValue>::from_js(object.get())?,
            set: <Callback<T> as Arg>::from_js(setter.get())?,
            value: PhantomData,
        })
    }

    /// Writes `value` (JS's readers run as for any write of the signal):
    /// now where JS can run now, else in the VM's next turn.
    pub fn set(&self, value: T) {
        if self.set.can_run_now() {
            if let Err(error) = self.set.call(value) {
                log::warn!(target: "js", "a signal Rust writes was not written: {error}");
            }
        } else {
            self.set.post(value);
        }
    }
}

impl<T: JsValue + 'static> Ret for JsSignal<T> {
    fn ts_type(types: &mut TsTypes) -> String {
        format!("import(\"nsh\").Accessor<{}>", T::ts_type(types))
    }

    fn into_js<'a>(self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        self.object.to_js(host)
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        T::classes(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signal_rust_writes_is_an_accessor_in_ts() {
        let mut types = TsTypes::default();
        assert_eq!(
            <JsSignal<f64> as Ret>::ts_type(&mut types),
            "import(\"nsh\").Accessor<number>"
        );
    }
}
