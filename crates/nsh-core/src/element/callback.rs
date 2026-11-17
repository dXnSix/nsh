//! JS functions Rust takes ([`Callback`]): called on the spot inside a
//! synchronous call from JS, or queued for the next JS turn from anywhere
//! on the engine thread ([`Callback::post`], [`Callback::call_async`]: the
//! engine hands the queue, [`super::Posts`], to the script after the
//! core's work). A callback an instance keeps is reported to QuickJS's
//! collector through the class's [`Trace`], so a cycle through the
//! instance's JS object is collected.

use std::{
    any::Any,
    cell::RefCell,
    collections::{BTreeMap, HashMap, VecDeque},
    fmt,
    marker::PhantomData,
    rc::Rc,
};

use super::{
    JsValue, PropError, TsTypes,
    class::{Arg, Class, ClassDef, Instance, Ret},
    js::{JsHost, JsIn, JsOwned},
    task::Answer,
};

/// A JS function as Rust keeps it (`nsh-js` implements it).
#[doc(hidden)]
pub trait CallbackHandle: std::any::Any {
    /// Runs `call` now: inside a synchronous call from JS.
    ///
    /// # Panics
    ///
    /// Outside one (in a task): JS is not running there.
    fn call_now(&self, call: JsCall);
    /// Queues `call` for the next JS turn.
    fn post(&self, call: JsCall);
    /// The value it keeps, as an owned value of `host` (a new reference the
    /// caller frees); `None` once the VM is gone, or for a host of another
    /// VM.
    fn value(&self, _host: &dyn JsHost) -> Option<super::JsRaw> {
        None
    }
    /// What tells the object it keeps apart from every other live object
    /// (a [`super::JsHandle`]'s equality): the same for two handles of one
    /// object. `None`: the handle itself does (a host keeping one handle
    /// per object).
    fn identity(&self) -> Option<(usize, usize)> {
        None
    }
    /// Whether a call made now runs on the spot ([`CallbackHandle::call_now`]):
    /// inside a synchronous call from JS, while the VM lives and does not
    /// collect garbage.
    fn in_call(&self) -> bool {
        false
    }
    /// Whether a call made now runs on the spot between JS's turns
    /// ([`CallbackHandle::call_between_turns`]): on the engine thread while
    /// no JS of the thread runs (a task, a component, a factory Rust runs
    /// there), the VM lives and does not collect garbage. `false` inside a
    /// synchronous call from JS, where [`CallbackHandle::in_call`] answers.
    fn between_turns(&self) -> bool {
        false
    }
    /// Runs `call` (a function's call, a member's, a property's read) now,
    /// between JS's turns, as a call into JS of its own within the VM's
    /// budget. Where
    /// [`CallbackHandle::between_turns`] is `false` the call is dropped
    /// (its reply says it was not run).
    fn call_between_turns(&self, call: JsCall) {
        drop(call);
    }
}

/// Runs `call` on the spot where JS can run now: inside a synchronous call
/// from JS ([`CallbackHandle::in_call`]), or between JS's turns
/// ([`CallbackHandle::between_turns`]); the call back where neither holds.
pub(crate) fn run_now(handle: &Rc<dyn CallbackHandle>, call: JsCall) -> Result<(), JsCall> {
    if handle.in_call() {
        handle.call_now(call);
        Ok(())
    } else if handle.between_turns() {
        handle.call_between_turns(call);
        Ok(())
    } else {
        Err(call)
    }
}

/// What receives a call's result.
pub(crate) type ReplyFn = Box<dyn for<'a> FnOnce(Result<JsIn<'a>, Exception>)>;

/// What a call does with the value its handle keeps.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsTarget {
    /// Calls it: a function ([`Callback`]).
    Call,
    /// Calls its method of this name, the value as `this` (an interface's
    /// method).
    Method(&'static str),
    /// Reads its property of this name (an interface's `#[getter]`).
    Get(&'static str),
}

/// The arguments of a call: made into JS values when it runs, or handed
/// back to Rust when the member it calls is missing.
trait PendingArgs {
    fn into_js<'a>(self: Box<Self>, host: &'a dyn JsHost) -> Result<Vec<JsOwned<'a>>, PropError>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<A: CallbackArgs + 'static> PendingArgs for A {
    fn into_js<'a>(self: Box<Self>, host: &'a dyn JsHost) -> Result<Vec<JsOwned<'a>>, PropError> {
        (*self).into_js_args(host)
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// What runs when the member a call names is missing: its arguments back.
type MissingFn = Box<dyn FnOnce(Box<dyn Any>)>;

/// What a JS function threw ([`Callback::call_async_traced`]): its message
/// and, for an `Error`, the stack where it was made (`    at f
/// (bundle.js:12:5)` lines, the bundle's positions).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Exception {
    /// The message (an `Error`'s `message`, else the value as a string).
    pub message: String,
    /// The stack, for an `Error`.
    pub stack: Option<String>,
    /// What was thrown, as JSON, for checks of it (`nsh/test`'s `toThrow`,
    /// `toMatchObject`): an object's own enumerable properties (an
    /// `Error`'s `code`, ...) with its `name` and `message`, any other
    /// value as itself; `None` when Rust made the error (a conversion's),
    /// and for a call that is not traced ([`JsCall::traced`]).
    pub thrown: Option<crate::JsonValue>,
}

impl From<String> for Exception {
    fn from(message: String) -> Self {
        Self {
            message,
            stack: None,
            thrown: None,
        }
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Its message: what an interface's method returning `Result<T, String>`
/// gets for what JS threw.
impl From<Exception> for String {
    fn from(exception: Exception) -> Self {
        exception.message
    }
}

/// A call of a JS function (or of a JS object's member): its arguments,
/// made when it runs, and what receives its result.
#[doc(hidden)]
pub struct JsCall {
    callback: Rc<dyn CallbackHandle>,
    target: JsTarget,
    args: Option<Box<dyn PendingArgs>>,
    reply: Option<ReplyFn>,
    /// The member may be missing: this gets the arguments back.
    missing: Option<MissingFn>,
    /// The reply waits for a promise the function returns.
    settles: bool,
    /// The reply wants what was thrown whole ([`Exception::thrown`]).
    traced: bool,
}

impl JsCall {
    fn new<A: CallbackArgs + 'static>(
        callback: Rc<dyn CallbackHandle>,
        args: A,
        reply: Option<ReplyFn>,
        settles: bool,
        traced: bool,
    ) -> Self {
        Self {
            callback,
            target: JsTarget::Call,
            args: Some(Box::new(args)),
            reply,
            missing: None,
            settles,
            traced,
        }
    }

    /// A call of member `target` of the object `callback` keeps: `missing`
    /// runs instead (with `args`) when the object has none (`undefined`).
    pub(crate) fn member<A: CallbackArgs + 'static>(
        callback: Rc<dyn CallbackHandle>,
        target: JsTarget,
        args: A,
        reply: Option<ReplyFn>,
        missing: Option<Box<dyn FnOnce(A)>>,
        settles: bool,
    ) -> Self {
        let missing = missing.map(|missing| {
            Box::new(move |args: Box<dyn Any>| {
                if let Ok(args) = args.downcast::<A>() {
                    missing(*args);
                }
            }) as MissingFn
        });
        Self {
            callback,
            target,
            args: Some(Box::new(args)),
            reply,
            missing,
            settles,
            traced: false,
        }
    }

    /// The function to call, or the object whose member to call.
    pub fn callback(&self) -> &Rc<dyn CallbackHandle> {
        &self.callback
    }

    /// What the call does with the value: calls it, or its member.
    pub fn target(&self) -> JsTarget {
        self.target
    }

    /// Whether the member may be missing: one that is `undefined` hands
    /// the call back to Rust ([`JsCall::reply_missing`]) rather than
    /// failing.
    pub fn optional(&self) -> bool {
        self.missing.is_some()
    }

    /// The arguments, as values of `host`.
    pub fn args<'a>(&mut self, host: &'a dyn JsHost) -> Result<Vec<JsOwned<'a>>, PropError> {
        match self.args.take() {
            Some(args) => args.into_js(host),
            None => Ok(Vec::new()),
        }
    }

    /// Hands the call back to Rust: the object has no such member (a
    /// missing optional one's default runs). A call whose member may not be
    /// missing fails instead.
    pub fn reply_missing(mut self) {
        let Some(missing) = self.missing.take() else {
            let name = match self.target {
                JsTarget::Method(name) | JsTarget::Get(name) => name,
                JsTarget::Call => "the function",
            };
            self.reply(Err(format!("`{name}` is missing").into()));
            return;
        };
        // The default answers in place of the reply.
        self.reply = None;
        let args = match self.args.take() {
            Some(args) => args.into_any(),
            None => Box::new(()),
        };
        missing(args);
    }

    /// Whether the result is wanted (`call`, `call_async`).
    pub fn wants_reply(&self) -> bool {
        self.reply.is_some()
    }

    /// Whether the reply waits for a promise the function returns
    /// (`call_async`).
    pub fn settles(&self) -> bool {
        self.settles
    }

    /// Whether the reply wants the value thrown as JSON
    /// ([`Exception::thrown`]: `call_traced`, `call_async_traced`); other
    /// calls' exceptions carry their message and stack only, so an
    /// exception costs no conversion where nothing checks it.
    pub fn traced(&self) -> bool {
        self.traced
    }

    /// Delivers the result (or what the function threw).
    pub fn reply(mut self, result: Result<JsIn<'_>, Exception>) {
        if let Some(reply) = self.reply.take() {
            reply(result);
        }
    }
}

impl Drop for JsCall {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            reply(Err("the JS function was not called (JS stopped)"
                .to_owned()
                .into()));
        }
    }
}

impl fmt::Debug for JsCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsCall")
            .field("reply", &self.reply.is_some())
            .finish_non_exhaustive()
    }
}

/// A JS function a class's member or a function takes (TS:
/// `Callback<f64>` is `(value: number) => void`, `Callback<(String, f64),
/// bool>` is `(a: string, b: number) => boolean`; `#[args(name, amount)]`
/// on the parameter names them, `(name: string, amount: number) =>
/// boolean`):
/// `A` is its argument (a [`JsValue`] type, an instance, a tuple of them
/// for several, `()` for none), `R` what it returns. Cheap to clone; the JS
/// function is released when the last clone goes. Kept in an instance's
/// field, it is reported to QuickJS's collector, so a cycle through the
/// instance's JS object is collected; kept elsewhere, it keeps the
/// function alive until dropped.
pub struct Callback<A = (), R = ()> {
    handle: Rc<dyn CallbackHandle>,
    _types: PhantomData<fn(A) -> R>,
}

impl<A, R> Clone for Callback<A, R> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            _types: PhantomData,
        }
    }
}

impl<A, R> fmt::Debug for Callback<A, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Callback")
    }
}

impl<A, R> Callback<A, R> {
    /// The JS function it keeps (two callbacks of one function share it).
    pub(crate) fn handle(&self) -> &Rc<dyn CallbackHandle> {
        &self.handle
    }

    /// Whether a call made now runs inside a synchronous call from JS.
    /// `true` inside one of the function's own VM (a `#[class]` method, a
    /// function or a constructor JS called, also nested: JS that a
    /// [`Callback::call`] runs calling back into Rust), where every
    /// non-`async` member of an interface's stand-in (`Js<Trait>`) calls its
    /// JS method on the spot, one returning `()` included.
    ///
    /// `false` everywhere else: after that call returned (a task, an
    /// `async` member's body, a timer), in a finalizer (a value QuickJS
    /// frees, by its count or its collector), once the VM is gone, and
    /// while only another VM on the thread is in a call. Between JS's turns
    /// (a task, a timer) [`Callback::call`] still runs the function on the
    /// spot ([`Callback::can_run_now`] says where), and a stand-in's member
    /// returning `()` is posted for the next JS turn, in order with the
    /// other posts.
    ///
    /// `true` does not promise that the call succeeds: the function may
    /// throw, using an instance the call borrows throws, a member may be
    /// missing. Asking runs no JS, so it is safe anywhere on the engine
    /// thread, in `Drop` and in a finalizer too.
    pub fn can_call_now(&self) -> bool {
        self.handle.in_call()
    }

    /// Whether [`Callback::call`] runs the function on the spot now:
    /// inside a synchronous call from JS ([`Callback::can_call_now`]), and
    /// between JS's turns, where no JS of the thread runs: Rust code on the
    /// engine thread in a task, a component, a timer or a factory it runs
    /// there. `false` in a finalizer, once the VM is gone, and while JS of
    /// another VM on the thread runs; use [`Callback::post`] or
    /// [`Callback::call_async`] there. Asking runs no JS.
    pub fn can_run_now(&self) -> bool {
        self.handle.in_call() || self.handle.between_turns()
    }
}

impl<R: CallbackRet + 'static> Callback<(), R> {
    /// A stand-in for a function that returns `value`: called with
    /// [`Callback::call_async`] (or `call_async_traced`), it answers
    /// `value`, a promise what it settles to (`nsh/test`'s
    /// `expect(promise).rejects`). Called on the spot ([`Callback::call`])
    /// it throws, as calling what is no function does. A promise is
    /// handled from then on: its rejection comes here, not reported as
    /// unhandled. `None` where the host keeps no values
    /// ([`JsHost::retain`]).
    pub fn settling(value: JsIn<'_>) -> Option<Self> {
        value.host().retain(value.raw()).map(Callback::of)
    }
}

impl<A: CallbackArgs + 'static, R: CallbackRet + 'static> Callback<A, R> {
    fn of(handle: Rc<dyn CallbackHandle>) -> Self {
        Self {
            handle,
            _types: PhantomData,
        }
    }

    /// Calls the function now, as `Array.prototype.sort` calls its
    /// comparison: its result, or what it threw (its message). Inside a
    /// synchronous call from JS (a method, a function, the constructor)
    /// the instance the call is on stays borrowed meanwhile: the function
    /// using it again throws. Between JS's turns (a task, a factory Rust
    /// runs there) it is a call into JS of its own, within the VM's budget,
    /// and runs before the calls posted earlier that have not run yet; what
    /// its JS posts or sends runs in JS's next turn.
    ///
    /// # Panics
    ///
    /// Where JS of the thread runs without a call from it to Rust (another
    /// VM's): use [`Callback::post`] or [`Callback::call_async`] there. It
    /// fails (the `Err`) in a finalizer and once the VM is gone.
    /// [`Callback::can_run_now`] asks first.
    pub fn call(&self, args: A) -> Result<R, String> {
        self.call_now(args, false).map_err(|e| e.message)
    }

    /// [`Callback::call`], with what the function threw whole: the
    /// message, the stack and the value thrown ([`Exception::thrown`]).
    ///
    /// # Panics
    ///
    /// As [`Callback::call`]: [`Callback::can_run_now`] asks first.
    pub fn call_traced(&self, args: A) -> Result<R, Exception> {
        self.call_now(args, true)
    }

    fn call_now(&self, args: A, traced: bool) -> Result<R, Exception> {
        let out: Rc<RefCell<Option<Result<R, Exception>>>> = Rc::default();
        let slot = out.clone();
        let call = JsCall::new(
            self.handle.clone(),
            args,
            Some(Box::new(move |result| {
                *slot.borrow_mut() =
                    Some(result.and_then(|v| R::from_js(v).map_err(|e| e.0.into())));
            })),
            false,
            traced,
        );
        if let Err(call) = run_now(&self.handle, call) {
            // The host fails it (a finalizer, the VM gone) or panics.
            self.handle.call_now(call);
        }
        let result = out.borrow_mut().take();
        result.unwrap_or_else(|| Err("the JS function was not called".to_owned().into()))
    }

    /// Calls the function in the next JS turn, in order with the other
    /// posts, forgetting its result (what it throws is reported as
    /// uncaught). From anywhere on the engine thread.
    pub fn post(&self, args: A) {
        self.handle
            .post(JsCall::new(self.handle.clone(), args, None, false, false));
    }

    /// Calls the function in the next JS turn: its result (a promise it
    /// returns, settled), or the message of what it threw.
    pub fn call_async(&self, args: A) -> Answer<Result<R, String>> {
        let (answer, slot) = Answer::new();
        self.handle.post(JsCall::new(
            self.handle.clone(),
            args,
            Some(Box::new(move |result| {
                let value = result
                    .map_err(|e| e.message)
                    .and_then(|v| R::from_js(v).map_err(|e| e.0));
                slot.borrow_mut().deliver(value);
            })),
            true,
            false,
        ));
        answer
    }

    /// [`Callback::call_async`], with what the function threw (or its
    /// promise rejected with) whole: the message and the stack, which says
    /// where it was thrown (a test's failing line).
    pub fn call_async_traced(&self, args: A) -> Answer<Result<R, Exception>> {
        let (answer, slot) = Answer::new();
        self.handle.post(JsCall::new(
            self.handle.clone(),
            args,
            Some(Box::new(move |result| {
                let value = result.and_then(|v| R::from_js(v).map_err(|e| e.0.into()));
                slot.borrow_mut().deliver(value);
            })),
            true,
            true,
        ));
        answer
    }
}

/// The arguments of a [`Callback`]: one value, `()` for none, or a tuple.
pub trait CallbackArgs {
    /// How many there are.
    const ARITY: usize;

    /// Its TypeScript parameters, named `names` (`name: string, amount:
    /// number`), or without names `value` for one and `a`, `b`, ... for
    /// several.
    fn ts_params(types: &mut TsTypes, names: &[&str]) -> String;
    /// The arguments as JS values.
    fn into_js_args<'a>(self, host: &'a dyn JsHost) -> Result<Vec<JsOwned<'a>>, PropError>;
    /// Adds the classes the arguments name.
    #[doc(hidden)]
    fn classes(out: &mut Vec<&'static ClassDef>);
}

impl<T: Ret> CallbackArgs for T {
    const ARITY: usize = if T::UNIT { 0 } else { 1 };

    fn ts_params(types: &mut TsTypes, names: &[&str]) -> String {
        if T::UNIT {
            String::new()
        } else {
            format!(
                "{}: {}",
                names.first().copied().unwrap_or("value"),
                T::ts_type(types)
            )
        }
    }

    fn into_js_args<'a>(self, host: &'a dyn JsHost) -> Result<Vec<JsOwned<'a>>, PropError> {
        if T::UNIT {
            Ok(Vec::new())
        } else {
            Ok(vec![Ret::into_js(self, host)?])
        }
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        T::classes(out);
    }
}

macro_rules! callback_tuple {
    ($(($($t:ident $v:ident $n:literal $i:literal),*)),*) => {$(
        impl<$($t: Ret),*> CallbackArgs for ($($t,)*) {
            const ARITY: usize = [$($n),*].len();

            fn ts_params(types: &mut TsTypes, names: &[&str]) -> String {
                let defaults = [$($n),*];
                [$($t::ts_type(types)),*]
                    .iter()
                    .enumerate()
                    .map(|(i, ts)| format!("{}: {ts}", names.get(i).copied().unwrap_or(defaults[i])))
                    .collect::<Vec<_>>()
                    .join(", ")
            }

            fn into_js_args<'a>(self, host: &'a dyn JsHost) -> Result<Vec<JsOwned<'a>>, PropError> {
                let ($($v,)*) = self;
                Ok(vec![$(Ret::into_js($v, host)?),*])
            }

            fn classes(out: &mut Vec<&'static ClassDef>) {
                $($t::classes(out);)*
            }
        }
    )*};
}

callback_tuple!(
    (A a "a" 0, B b "b" 1),
    (A a "a" 0, B b "b" 1, C c "c" 2),
    (A a "a" 0, B b "b" 1, C c "c" 2, D d "d" 3),
    (A a "a" 0, B b "b" 1, C c "c" 2, D d "d" 3, E e "e" 4),
    (A a "a" 0, B b "b" 1, C c "c" 2, D d "d" 3, E e "e" 4, F f "f" 5),
    (A a "a" 0, B b "b" 1, C c "c" 2, D d "d" 3, E e "e" 4, F f "f" 5, G g "g" 6),
    (A a "a" 0, B b "b" 1, C c "c" 2, D d "d" 3, E e "e" 4, F f "f" 5, G g "g" 6, H h "h" 7)
);

/// What a [`Callback`] returns: a [`JsValue`] type, an instance of a class
/// (`Instance<C>`, `Option<Instance<C>>`), or `()` (its result ignored).
pub trait CallbackRet: Sized {
    /// Its TypeScript type.
    fn ts_type(types: &mut TsTypes) -> String;
    /// The value the function returned.
    fn from_js(value: JsIn<'_>) -> Result<Self, PropError>;
    /// Adds the classes the type names.
    #[doc(hidden)]
    fn classes(_out: &mut Vec<&'static ClassDef>) {}
    /// The value of `()`, which needs no JS value (`None` for any other
    /// type): what a posted interface method gives.
    #[doc(hidden)]
    fn unit() -> Option<Self> {
        None
    }
}

impl<T: JsValue> CallbackRet for T {
    fn ts_type(types: &mut TsTypes) -> String {
        T::ts_type(types)
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        T::from_js(value)
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        T::classes(out);
    }
}

impl CallbackRet for () {
    fn ts_type(_types: &mut TsTypes) -> String {
        "void".into()
    }

    fn from_js(_value: JsIn<'_>) -> Result<Self, PropError> {
        Ok(())
    }

    fn unit() -> Option<Self> {
        Some(())
    }
}

/// The instance a JS object of `C` holds.
impl<C: Class> CallbackRet for Instance<C> {
    fn ts_type(types: &mut TsTypes) -> String {
        <Instance<C> as Arg>::ts_type(types)
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        <Instance<C> as Arg>::from_js(value)
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        out.push(C::DEF);
    }
}

/// `null` or `undefined` is `None`.
impl<C: Class> CallbackRet for Option<Instance<C>> {
    fn ts_type(types: &mut TsTypes) -> String {
        <Option<Instance<C>> as Arg>::ts_type(types)
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        <Option<Instance<C>> as Arg>::from_js(value)
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        out.push(C::DEF);
    }
}

/// The TypeScript type of a callback whose arguments are named `names`
/// (none: the defaults): `(value: string) => void`.
fn callback_ts<A: CallbackArgs, R: CallbackRet>(types: &mut TsTypes, names: &[&str]) -> String {
    format!("({}) => {}", A::ts_params(types, names), R::ts_type(types))
}

/// A parameter whose JS function's arguments `#[args(..)]` names: a
/// [`Callback`], or an `Option` of one.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`#[args(..)]` names the arguments of a `Callback`: `{Self}` is not one",
    label = "`#[args(..)]` is for a `Callback<A, R>` parameter (or an `Option` of one)"
)]
pub trait NamedArgs {
    /// How many arguments the function takes.
    const ARITY: usize;
    /// The parameter's TypeScript type, its function's arguments named
    /// `names`.
    fn ts_named(types: &mut TsTypes, names: &[&str]) -> String;
}

impl<A: CallbackArgs + 'static, R: CallbackRet + 'static> NamedArgs for Callback<A, R> {
    const ARITY: usize = A::ARITY;

    fn ts_named(types: &mut TsTypes, names: &[&str]) -> String {
        callback_ts::<A, R>(types, names)
    }
}

impl<A: CallbackArgs + 'static, R: CallbackRet + 'static> NamedArgs for Option<Callback<A, R>> {
    const ARITY: usize = A::ARITY;

    fn ts_named(types: &mut TsTypes, names: &[&str]) -> String {
        format!("({}) | null", callback_ts::<A, R>(types, names))
    }
}

impl<A: CallbackArgs + 'static, R: CallbackRet + 'static> Arg for Callback<A, R> {
    fn ts_type(types: &mut TsTypes) -> String {
        callback_ts::<A, R>(types, &[])
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        value
            .host()
            .callback(value.raw())
            .map(Callback::of)
            .ok_or_else(|| value.expected("a function"))
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        A::classes(out);
    }
}

/// `null` or `undefined` is `None`.
impl<A: CallbackArgs + 'static, R: CallbackRet + 'static> Arg for Option<Callback<A, R>> {
    fn ts_type(types: &mut TsTypes) -> String {
        format!("({}) | null", callback_ts::<A, R>(types, &[]))
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        if value.is_nullish() {
            return Ok(None);
        }
        Callback::from_js(value).map(Some)
    }

    fn classes(out: &mut Vec<&'static ClassDef>) {
        A::classes(out);
    }
}

// --- Tracing ----------------------------------------------------------------------

/// Where a class reports the callbacks its value keeps, for QuickJS's
/// collector: a callback whose function reaches the instance's own JS
/// object would keep both alive forever otherwise.
pub struct Tracer<'a> {
    mark: &'a mut dyn FnMut(&dyn CallbackHandle),
}

impl<'a> Tracer<'a> {
    /// Reports callbacks to `mark`.
    #[doc(hidden)]
    pub fn new(mark: &'a mut dyn FnMut(&dyn CallbackHandle)) -> Self {
        Self { mark }
    }

    /// Reports `handle` (a value Rust keeps through this one alone).
    pub(super) fn report(&mut self, handle: &dyn CallbackHandle) {
        (self.mark)(handle);
    }
}

impl fmt::Debug for Tracer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tracer")
    }
}

/// What reports the callbacks a value keeps, for QuickJS's collector.
/// `#[class]` on a struct implements it from the fields whose types
/// implement it (`Callback`, `JsHandle`, `Instance`, and `Option`, `Box`, `RefCell`,
/// `Vec`, `VecDeque`, `HashMap` / `BTreeMap` values and pairs of them);
/// other fields are not reported, which is always safe: what they keep
/// stays alive while they do.
///
/// # Safety
///
/// The collector counts each reported function as one reference held by
/// the instance's JS object. An implementation reports, on each call, a
/// callback at most once, and only one held through this value alone (no
/// other value or clone keeps it, as the impls of `Callback` and
/// `Instance` check with their strong counts): reporting one twice, or one
/// shared (an `Rc` reached from two instances), lets the collector free a
/// function still in use. It never panics, allocates JS values or runs
/// JS: it is called from the collector's mark phase, which a panic would
/// leave half done (the runtime catches one and logs it, and what was not
/// reported stays alive, but a panic midway may already have reported part
/// of a value). It reads only, without borrowing what the app may hold
/// borrowed (`RefCell::try_borrow`: a value borrowed elsewhere is
/// skipped).
pub unsafe trait Trace {
    /// Reports the callbacks to `tracer`.
    fn trace(&self, tracer: &mut Tracer<'_>);
}

/// A callback held only here: shared ones are kept alive by whoever else
/// holds them (reporting those would let the collector free a function
/// still in use).
unsafe impl<A, R> Trace for Callback<A, R> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if Rc::strong_count(&self.handle) == 1 {
            (tracer.mark)(&*self.handle);
        }
    }
}

/// An instance held only here: the callbacks its value keeps.
unsafe impl<C: Class> Trace for Instance<C> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if self.only_handle() {
            self.trace_value(tracer);
        }
    }
}

unsafe impl<T: Trace> Trace for Option<T> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(v) = self {
            v.trace(tracer);
        }
    }
}

unsafe impl<T: Trace> Trace for Box<T> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        (**self).trace(tracer);
    }
}

unsafe impl<T: Trace> Trace for RefCell<T> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Ok(v) = self.try_borrow() {
            v.trace(tracer);
        }
    }
}

unsafe impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in self {
            v.trace(tracer);
        }
    }
}

unsafe impl<T: Trace> Trace for VecDeque<T> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in self {
            v.trace(tracer);
        }
    }
}

unsafe impl<K, T: Trace, S> Trace for HashMap<K, T, S> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in self.values() {
            v.trace(tracer);
        }
    }
}

unsafe impl<K, T: Trace> Trace for BTreeMap<K, T> {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in self.values() {
            v.trace(tracer);
        }
    }
}

unsafe impl<A: Trace, B: Trace> Trace for (A, B) {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.0.trace(tracer);
        self.1.trace(tracer);
    }
}
