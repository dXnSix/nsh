//! Interfaces: Rust traits TS objects implement, written with `#[interface]`
//! on the trait and registered with [`super::Registry::interface`]. TS gets
//! an `export interface` of the trait's name; Rust gets the trait's
//! stand-in (`Js<Trait>`), a struct over a [`JsHandle`] that implements the
//! trait by calling the JS object's members.
//!
//! A member runs where Rust calls it: a method that is not `async` calls
//! the JS method on the spot inside a synchronous call from JS (as a
//! [`super::Callback`]'s `call` does), and outside one (in a task) posts a
//! method returning `()` for the next JS turn, in order with the other
//! posts, and runs the others (getters too) on the spot between JS's turns
//! (a task, a factory: [`JsHandle::can_run_now`]), failing only where JS of
//! the thread runs without a call to Rust (a finalizer, another VM) or the
//! VM is gone ([`JsHandle::can_call_now`] says whether a call from JS is in
//! progress); an `async` method is posted and awaited (`call_async`:
//! a promise it returns settles it). What JS throws (or rejects with) is
//! the error of a method returning `Result<T, E>` (`E: From<Exception>`);
//! a method returning a plain value panics with it, which the JS call
//! around it throws. A member with a default body in the trait is optional
//! in TS: an object without it (`undefined`) runs the default.

use std::{any::TypeId, cell::RefCell, fmt, rc::Rc};

use super::{
    JsValue, PropError, TsType, TsTypes,
    callback::{CallbackArgs, CallbackRet, Exception, JsCall, JsTarget, run_now},
    class::{ClassDef, ClassRefs, MemberKind},
    handle::JsHandle,
    js::{JsIn, JsKind},
    module::ModuleParam,
    task::Answer,
};

/// A Rust trait's stand-in: the struct `#[interface]` makes over a
/// [`JsHandle`] (`Js<Trait>`), which implements the trait by calling the
/// JS object's members. Registered with [`super::Registry::interface`].
pub trait Interface: JsValue + 'static {
    /// The interface: its TS name (the trait's), documentation and members.
    const DEF: &'static InterfaceDef;
}

/// What an interface is: its name and members (`#[interface]` writes it).
pub struct InterfaceDef {
    /// Its TS name: the trait's (`AppContract`).
    pub name: &'static str,
    /// Documentation (the trait's doc comment).
    pub doc: &'static str,
    /// The stand-in's type, telling two interfaces of the same name apart.
    pub type_id: fn() -> TypeId,
    /// The stand-in's Rust path, for messages.
    pub type_name: fn() -> &'static str,
    /// Its members, in the trait's order.
    pub members: &'static [InterfaceMember],
}

impl fmt::Debug for InterfaceDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InterfaceDef")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// A member of an interface (`#[interface]` makes them).
pub struct InterfaceMember {
    /// Its JS name (`start`; a getter's is the property's).
    pub name: &'static str,
    /// Documentation.
    pub doc: &'static str,
    /// [`MemberKind::Method`], or [`MemberKind::Getter`] for a property
    /// (`#[getter]`).
    pub kind: MemberKind,
    /// The parameters Rust passes.
    pub params: &'static [ModuleParam],
    /// The TypeScript type of what JS returns (or the property holds).
    pub returns: TsType,
    /// An `async` method: Rust awaits it, so TS returns a promise (or a
    /// value, which Rust awaits as one already settled).
    pub is_async: bool,
    /// The trait has a default body: TS may leave it out.
    pub optional: bool,
    /// The signatures TS declares instead of the one above, written in
    /// TypeScript (`#[ts("(): boolean")]` on the trait's method): what JS
    /// returns, declared as written (no `Promise` added).
    pub overloads: &'static [super::Overload],
    /// The classes its parameters and result name.
    pub refs: ClassRefs,
}

/// What an interface's method returns: what JS gives back (a
/// [`CallbackRet`] type, `()` for nothing), or a `Result` of it whose `Err`
/// is what JS threw.
pub trait InterfaceRet: Sized + 'static {
    /// What JS gives back.
    type Value: CallbackRet + 'static;
    /// The result of the call (`what`: `AppContract.start`, for a panic).
    fn from_result(result: Result<Self::Value, Exception>, what: &str) -> Self;
    /// What a method returning it gives when it is posted, outside a
    /// synchronous call from JS: only `()` is (`None`: it fails there).
    fn posted() -> Option<Self>;
}

/// A plain value: what JS throws panics (the JS call around it throws it).
impl<T: CallbackRet + 'static> InterfaceRet for T {
    type Value = T;

    #[track_caller]
    fn from_result(result: Result<T, Exception>, what: &str) -> Self {
        match result {
            Ok(value) => value,
            Err(e) => panic!("{what}: {e}"),
        }
    }

    fn posted() -> Option<Self> {
        T::unit()
    }
}

/// What JS throws is the `Err`.
impl<T: CallbackRet + 'static, E: From<Exception> + 'static> InterfaceRet for Result<T, E> {
    type Value = T;

    fn from_result(result: Result<T, Exception>, _what: &str) -> Self {
        result.map_err(E::from)
    }

    fn posted() -> Option<Self> {
        None
    }
}

/// The TS declaration of interface `def`: its name, declared once in
/// `types`.
fn declare(def: &'static InterfaceDef, types: &mut TsTypes) -> String {
    use std::fmt::Write as _;
    types.declare(def.name, |types| {
        let mut out = String::new();
        if !def.doc.is_empty() {
            let _ = writeln!(out, "/** {} */", def.doc);
        }
        let _ = writeln!(out, "export interface {} {{", def.name);
        for m in def.members {
            if !m.doc.is_empty() {
                let _ = writeln!(out, "  /** {} */", m.doc);
            }
            let optional = if m.optional { "?" } else { "" };
            if !m.overloads.is_empty() {
                for (i, o) in m.overloads.iter().enumerate() {
                    if i > 0 && !m.doc.is_empty() {
                        let _ = writeln!(out, "  /** {} */", m.doc);
                    }
                    let returns = o.returns.map_or_else(|| "void".to_owned(), |ts| ts(types));
                    if m.kind == MemberKind::Getter {
                        let _ = writeln!(out, "  readonly {}{optional}: {returns};", m.name);
                        continue;
                    }
                    let params = o
                        .params
                        .iter()
                        .map(|(name, ts, opt)| {
                            format!("{name}{}: {}", if *opt { "?" } else { "" }, ts(types))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let _ = writeln!(
                        out,
                        "  {}{optional}{}({params}): {returns};",
                        m.name, o.generics
                    );
                }
                continue;
            }
            let returns = (m.returns)(types);
            if m.kind == MemberKind::Getter {
                let _ = writeln!(out, "  readonly {}{optional}: {returns};", m.name);
                continue;
            }
            let params = m
                .params
                .iter()
                .map(|(name, ts, _)| format!("{name}: {}", ts(types)))
                .collect::<Vec<_>>()
                .join(", ");
            let returns = if m.is_async {
                format!("Promise<{returns}>")
            } else {
                returns
            };
            let _ = writeln!(out, "  {}{optional}({params}): {returns};", m.name);
        }
        out.push('}');
        out
    })
}

/// What a member's call came to: what JS gave (or threw), or the object
/// has no such member (its arguments back, for the default).
enum Outcome<A, T> {
    Done(Result<T, Exception>),
    Missing(A),
}

/// Where a call made on the spot leaves what it came to.
type Slot<A, T> = Rc<RefCell<Option<Outcome<A, T>>>>;

/// How a member's call answers.
#[derive(Clone, Copy)]
struct How {
    /// Its result is wanted.
    reply: bool,
    /// The object may lack the member (an optional one's).
    missing: bool,
    /// The reply waits for a promise the member returns.
    settles: bool,
}

/// The call of member `index` of `def` on `object` with `args`, delivering
/// what it comes to as `how` says.
fn member_call<A: CallbackArgs + 'static, T: CallbackRet + 'static>(
    object: &JsHandle,
    def: &'static InterfaceDef,
    index: usize,
    args: A,
    how: How,
    deliver: impl FnOnce(Outcome<A, T>) + 'static,
) -> JsCall {
    let member = &def.members[index];
    let target = match member.kind {
        MemberKind::Getter => JsTarget::Get(member.name),
        _ => JsTarget::Method(member.name),
    };
    // One of the two delivers.
    let deliver = Rc::new(RefCell::new(Some(deliver)));
    let on_missing = deliver.clone();
    let reply = how.reply.then(|| {
        Box::new(move |result: Result<JsIn<'_>, Exception>| {
            let value = result.and_then(|v| {
                T::from_js(v)
                    .map_err(|e| Exception::from(format!("{}.{}: {}", def.name, member.name, e.0)))
            });
            let deliver = deliver.borrow_mut().take();
            if let Some(deliver) = deliver {
                deliver(Outcome::Done(value));
            }
        }) as Box<dyn for<'a> FnOnce(Result<JsIn<'a>, Exception>)>
    });
    let missing = how.missing.then(|| {
        Box::new(move |args: A| {
            let deliver = on_missing.borrow_mut().take();
            if let Some(deliver) = deliver {
                deliver(Outcome::Missing(args));
            }
        }) as Box<dyn FnOnce(A)>
    });
    JsCall::member(
        object.handle().clone(),
        target,
        args,
        reply,
        missing,
        how.settles,
    )
}

/// `Interface.member`, for messages.
fn what(def: &'static InterfaceDef, index: usize) -> String {
    format!("{}.{}", def.name, def.members[index].name)
}

/// A member that is not `async`, called with `args`: now inside a
/// synchronous call from JS; outside one, a method returning `()` is
/// posted, and the others (getters too) run now between JS's turns, failing
/// where JS cannot run now. `default` runs when the object has no such
/// member (an optional one's).
pub(crate) fn call_sync<A, R, F>(
    object: &JsHandle,
    def: &'static InterfaceDef,
    index: usize,
    args: A,
    default: Option<F>,
) -> R
where
    A: CallbackArgs + 'static,
    R: InterfaceRet,
    F: FnOnce(A) -> R + 'static,
{
    let member = &def.members[index];
    let handle = object.handle();
    // Outside a call from JS, a method returning `()` keeps its place among
    // the posts; the rest run between JS's turns.
    let posted = if member.kind == MemberKind::Method && !handle.in_call() {
        R::posted()
    } else {
        None
    };
    if posted.is_none() && (handle.in_call() || handle.between_turns()) {
        let out: Slot<A, R::Value> = Rc::default();
        let slot = out.clone();
        let how = How {
            reply: true,
            missing: default.is_some(),
            settles: false,
        };
        let call = member_call(object, def, index, args, how, move |outcome| {
            *slot.borrow_mut() = Some(outcome);
        });
        if let Err(call) = run_now(handle, call) {
            // JS started or stopped meanwhile (it cannot): not run.
            drop(call);
        }
        let outcome = out.borrow_mut().take();
        return match (outcome, default) {
            (Some(Outcome::Done(result)), _) => R::from_result(result, &what(def, index)),
            (Some(Outcome::Missing(args)), Some(default)) => default(args),
            _ => R::from_result(
                Err(Exception::from(format!(
                    "{}: the JS method was not called",
                    what(def, index)
                ))),
                &what(def, index),
            ),
        };
    }
    if let Some(done) = posted {
        // In the next JS turn, in order with the other posts; what it
        // throws is reported as uncaught, as a posted callback's is.
        let how = How {
            reply: false,
            missing: default.is_some(),
            settles: false,
        };
        let call = member_call::<A, R::Value>(object, def, index, args, how, move |outcome| {
            if let (Outcome::Missing(args), Some(default)) = (outcome, default) {
                drop(default(args));
            }
        });
        handle.post(call);
        return done;
    }
    R::from_result(
        Err(Exception::from(format!(
            "{}: called while JS runs on this thread without a call from it to Rust (a \
             finalizer, another VM), or once the VM is gone",
            what(def, index)
        ))),
        &what(def, index),
    )
}

/// A member called in the next JS turn, its result (a promise it returns,
/// settled) awaited; `Err(args)` when the object has no such member and
/// `optional`.
async fn call_later<A, R>(
    object: &JsHandle,
    def: &'static InterfaceDef,
    index: usize,
    args: A,
    optional: bool,
) -> Result<R, A>
where
    A: CallbackArgs + 'static,
    R: InterfaceRet,
{
    let (answer, slot) = Answer::<Outcome<A, R::Value>>::new();
    let how = How {
        reply: true,
        missing: optional,
        settles: def.members[index].kind == MemberKind::Method,
    };
    let call = member_call(object, def, index, args, how, move |outcome| {
        slot.borrow_mut().deliver(outcome);
    });
    object.handle().post(call);
    match answer.await {
        Outcome::Done(result) => Ok(R::from_result(result, &what(def, index))),
        Outcome::Missing(args) => Err(args),
    }
}

/// The class's generated code's helpers.
#[doc(hidden)]
pub(crate) mod __private {
    use super::*;

    /// [`call_sync`].
    pub fn interface_sync<A, R, F>(
        object: &JsHandle,
        def: &'static InterfaceDef,
        index: usize,
        args: A,
        default: Option<F>,
    ) -> R
    where
        A: CallbackArgs + 'static,
        R: InterfaceRet,
        F: FnOnce(A) -> R + 'static,
    {
        call_sync(object, def, index, args, default)
    }

    /// An `async` member the object has (a required one).
    pub async fn interface_async<A, R>(
        object: &JsHandle,
        def: &'static InterfaceDef,
        index: usize,
        args: A,
    ) -> R
    where
        A: CallbackArgs + 'static,
        R: InterfaceRet,
    {
        match call_later::<A, R>(object, def, index, args, false).await {
            Ok(result) => result,
            // A required member's call never comes back missing.
            Err(_) => R::from_result(
                Err(Exception::from(format!("{} is missing", what(def, index)))),
                &what(def, index),
            ),
        }
    }

    /// An optional `async` member: `Err(args)` when the object has none.
    pub async fn interface_async_optional<A, R>(
        object: &JsHandle,
        def: &'static InterfaceDef,
        index: usize,
        args: A,
    ) -> Result<R, A>
    where
        A: CallbackArgs + 'static,
        R: InterfaceRet,
    {
        call_later(object, def, index, args, true).await
    }

    /// The TS type of what JS returns for a member returning `R`.
    pub const fn interface_ret_ts<R: InterfaceRet>() -> TsType {
        if crate::element::DECLARATIONS {
            <R::Value as CallbackRet>::ts_type
        } else {
            |_| String::new()
        }
    }

    /// The classes what JS returns names.
    pub fn interface_ret_classes<R: InterfaceRet>(out: &mut Vec<&'static ClassDef>) {
        <R::Value as CallbackRet>::classes(out);
    }

    /// Interface `def`'s TS type: its name, declared in `types`.
    pub fn interface_ts(def: &'static InterfaceDef, types: &mut TsTypes) -> String {
        if crate::element::DECLARATIONS {
            declare(def, types)
        } else {
            def.name.to_owned()
        }
    }

    /// The object of a JS value that implements interface `def`: an
    /// object whose required methods are functions.
    pub fn interface_from_js(
        value: JsIn<'_>,
        def: &'static InterfaceDef,
    ) -> Result<JsHandle, PropError> {
        if !matches!(value.kind(), JsKind::Object | JsKind::Function) {
            return Err(value.expected(&format!("a {}", def.name)));
        }
        for m in def
            .members
            .iter()
            .filter(|m| !m.optional && m.kind == MemberKind::Method)
        {
            let member = value.field(m.name)?;
            if member.get().kind() != JsKind::Function {
                return Err(PropError(format!(
                    "expected a {}, got an object whose `{}` is {}",
                    def.name,
                    m.name,
                    member.get().describe()
                )));
            }
        }
        <JsHandle as JsValue>::from_js(value)
    }

    thread_local! {
        /// The interfaces whose classes are being listed (one naming itself
        /// stops there).
        static LISTING: RefCell<Vec<TypeId>> = const { RefCell::new(Vec::new()) };
    }

    /// The classes interface `def`'s members name.
    pub fn interface_classes(def: &'static InterfaceDef, out: &mut Vec<&'static ClassDef>) {
        let id = (def.type_id)();
        let listing = LISTING.with(|l| {
            let mut l = l.borrow_mut();
            if l.contains(&id) {
                return false;
            }
            l.push(id);
            true
        });
        if !listing {
            return;
        }
        for m in def.members {
            (m.refs)(out);
        }
        LISTING.with(|l| l.borrow_mut().retain(|t| *t != id));
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        element::{Registry, declarations},
        prelude::*,
    };

    /// A count the service keeps.
    #[class]
    struct Counter;

    #[class]
    impl Counter {}

    /// A service the app writes in TS.
    #[interface]
    trait Startable {
        /// Starts as `name`; resolves once started.
        async fn start(&self, name: String, attempts: u32) -> Result<(), String>;
        /// Whether it started.
        fn is_started(&self) -> bool;
        /// Stops it (nothing by default).
        fn stop(&self) {}
        /// Its label.
        #[getter]
        fn label(&self) -> String;
        /// How long it may take, in ms (the registry's by default).
        #[getter]
        fn timeout(&self) -> Option<f64> {
            None
        }
        fn counter(&self) -> Instance<Counter>;
    }

    /// Runs services.
    #[class]
    struct Runner;

    #[class]
    impl Runner {
        /// Runs `service`.
        pub(crate) fn run(service: JsStartable) {
            let _ = service;
        }
    }

    const STARTABLE: &str = r#"/** A service the app writes in TS. */
export interface Startable {
  /** Starts as `name`; resolves once started. */
  start(name: string, attempts: number): Promise<void>;
  /** Whether it started. */
  isStarted(): boolean;
  /** Stops it (nothing by default). */
  stop?(): void;
  /** Its label. */
  readonly label: string;
  /** How long it may take, in ms (the registry's by default). */
  readonly timeout?: number | null;
  counter(): Counter;
}

/** A count the service keeps. */
export declare class Counter {
  /** Made by the class's members only. */
  private constructor();
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}

"#;

    #[test]
    fn an_interface_is_declared_with_the_classes_it_names() {
        let mut registry = Registry::ui_set("test/ui");
        registry
            .interface::<JsStartable>()
            .interface::<JsStartable>();
        assert_eq!(declarations(&registry, &[]), STARTABLE);
        assert_eq!(registry.undocumented(), ["Startable.counter"]);
    }

    #[test]
    fn a_signature_naming_an_interface_declares_it() {
        let mut registry = Registry::ui_set("test/ui");
        registry.class::<Runner>();
        assert_eq!(
            declarations(&registry, &[]),
            format!(
                "{STARTABLE}/** Runs services. */
export declare class Runner {{
  /** Made by the class's members only. */
  private constructor();
  /** Runs `service`. */
  static run(service: Startable): void;
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}}

"
            )
        );
    }

    mod named_alike {
        use crate::prelude::*;

        /// A class named like the interface.
        #[class]
        pub(super) struct Startable;

        #[class]
        impl Startable {}
    }

    #[test]
    fn an_interface_and_a_class_share_no_name() {
        let class_first = std::panic::catch_unwind(|| {
            Registry::ui_set("test/ui")
                .class::<named_alike::Startable>()
                .interface::<JsStartable>();
        });
        assert!(class_first.is_err(), "an interface after a class");
        let interface_first = std::panic::catch_unwind(|| {
            Registry::ui_set("test/ui")
                .interface::<JsStartable>()
                .class::<named_alike::Startable>();
        });
        assert!(interface_first.is_err(), "a class after an interface");
    }
}
