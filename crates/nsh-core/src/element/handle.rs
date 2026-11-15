//! JS objects Rust keeps ([`JsHandle`]): any object or function JS passes,
//! stored by Rust for as long as it likes and given back to JS as the very
//! same object. They are kept in the JS host's table of the values Rust
//! holds, as [`super::Callback`]s' functions are, so a `#[class]` field
//! holding one is reported to QuickJS's collector the same way.

use std::{fmt, hash, rc::Rc};

use super::{
    JsValue, PropError, TsTypes,
    callback::{CallbackHandle, Trace, Tracer},
    class::{ClassDef, Constructor},
    js::{JsHost, JsIn, JsKind, JsOwned},
};

/// A JS object Rust keeps: any object, array or function JS passes (TS
/// `object`), held as long as Rust holds the handle and given back to JS
/// as the same object (`===`). Cheap to clone; two handles are equal (and
/// hash alike) when they keep the same object, so a handle is a key
/// (a `HashMap<JsHandle, _>` of the objects an app registered).
///
/// It belongs to the engine thread (it is not `Send`), and to its VM: the
/// object is released when the last clone drops, and once the VM is gone
/// the handle is dead (giving it back to JS fails; equality still holds).
/// Kept in a `#[class]`'s field, it is reported to QuickJS's collector as
/// a [`super::Callback`] is, so an object that reaches the instance's own
/// JS object (a listener holding its owner) is collected with it; kept
/// elsewhere (a static, a task, an `Rc` two instances share), it is a root
/// until dropped.
#[derive(Clone)]
pub struct JsHandle {
    handle: Rc<dyn CallbackHandle>,
    /// What tells its object apart (computed once: equality and hashing
    /// read it, also after the VM is gone).
    identity: (usize, usize),
    /// The Rust class the object is the constructor of, if it is one.
    class: Option<&'static ClassDef>,
}

impl JsHandle {
    fn of(handle: Rc<dyn CallbackHandle>, class: Option<&'static ClassDef>) -> Self {
        // A host keeping one handle per object needs no identity of its
        // own: the handle is the object's.
        let identity = handle
            .identity()
            .unwrap_or((0, Rc::as_ptr(&handle) as *const () as usize));
        Self {
            handle,
            identity,
            class,
        }
    }

    /// The Rust class whose constructor the object is, if it is one (a
    /// token TS passes may be a Rust class or anything else): what a
    /// [`Constructor`] parameter would have taken.
    pub fn constructor(&self) -> Option<Constructor> {
        self.class.map(Constructor::from_def)
    }

    /// The value it keeps, as the host holds it.
    #[doc(hidden)]
    pub fn handle(&self) -> &Rc<dyn CallbackHandle> {
        &self.handle
    }
}

impl PartialEq for JsHandle {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for JsHandle {}

impl hash::Hash for JsHandle {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl fmt::Debug for JsHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JsHandle")
    }
}

/// Any object, array or function (TS `object`); `null`, `undefined` and
/// primitives are refused (`Option<JsHandle>` takes `null` and `undefined`
/// as `None`).
impl JsValue for JsHandle {
    fn ts_type(_types: &mut TsTypes) -> String {
        "object".into()
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        if !matches!(
            value.kind(),
            JsKind::Object | JsKind::Array | JsKind::Function
        ) {
            return Err(value.expected("an object"));
        }
        let host = value.host();
        // A constructor stays one: told once, while the host is at hand.
        let class = host.class_of(value.raw());
        host.object(value.raw())
            .map(|handle| Self::of(handle, class))
            .ok_or_else(|| PropError("JS objects cannot be kept here".into()))
    }

    /// The object it keeps: the same one JS passed.
    fn to_js<'a>(&self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        self.handle
            .value(host)
            .map(|raw| JsOwned::new(host, raw))
            .ok_or_else(|| PropError("the object belongs to a VM that is gone, or another".into()))
    }
}

/// A handle held only here: shared ones are kept alive by whoever else
/// holds them (reporting those would let the collector free an object
/// still in use).
unsafe impl Trace for JsHandle {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if Rc::strong_count(&self.handle) == 1 {
            tracer.report(&*self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        element::{JsHost, JsIn, Registry, declarations},
        prelude::*,
    };

    /// Keeps the objects JS gives it.
    #[class]
    struct Keeper {
        kept: Vec<JsHandle>,
    }

    #[class]
    impl Keeper {
        /// Keeps `object`.
        pub(crate) fn keep(&mut self, object: JsHandle) {
            self.kept.push(object);
        }

        /// The first object kept, if any.
        pub(crate) fn first(&self) -> Option<JsHandle> {
            self.kept.first().cloned()
        }

        /// How many different objects `objects` holds.
        pub(crate) fn distinct(objects: Vec<JsHandle>) -> usize {
            objects.into_iter().collect::<HashSet<_>>().len()
        }

        /// Calls `make` and keeps what it returns.
        pub(crate) fn make(&mut self, make: Callback<(), JsHandle>) -> Result<(), String> {
            self.kept.push(make.call(())?);
            Ok(())
        }
    }

    #[test]
    fn kept_objects_are_declared_as_objects() {
        let mut registry = Registry::ui_set("test/ui");
        registry.class::<Keeper>();
        assert_eq!(
            declarations(&registry, &[]),
            r#"/** Keeps the objects JS gives it. */
export declare class Keeper {
  /** Made by the class's members only. */
  private constructor();
  /** Keeps `object`. */
  keep(object: object): void;
  /** The first object kept, if any. */
  first(): object | null;
  /** How many different objects `objects` holds. */
  static distinct(objects: (object)[]): number;
  /** Calls `make` and keeps what it returns. */
  make(make: () => object): void;
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}

"#
        );
    }

    #[test]
    fn hosts_that_keep_no_values_refuse_objects() {
        let host = crate::element::json_host::JsonHost::default();
        let object = host.value(serde_json::json!({}));
        let error = JsHandle::from_js(JsIn::new(&host, object)).unwrap_err();
        assert_eq!(error.0, "JS objects cannot be kept here");
        let number = host.value(serde_json::json!(3));
        let error = JsHandle::from_js(JsIn::new(&host, number)).unwrap_err();
        assert_eq!(error.0, "expected an object, got 3");
        assert_eq!(
            <Option<JsHandle>>::from_js(JsIn::new(&host, JsHost::null(&host))).unwrap(),
            None
        );
    }
}
