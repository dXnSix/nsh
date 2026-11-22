//! The Rust owners tied to JS owners: where a synchronous call from JS runs
//! its Rust reactive code (`CallCx::with_owner`), and what the Rust
//! components JS makes see the context of.
//!
//! JS and Rust components each have their own reactive graph (JS's in this
//! VM, Rust's in the engine's core). A call under a JS owner (a component's
//! body, an effect, a root) gets a Rust owner of its own, made the first
//! time a call under that JS owner asks for it and disposed when the JS
//! owner goes (a JS cleanup on it). Its context's parent is the nearest
//! tied owner above (found by walking the JS owners up), so what one call
//! provides is seen by the calls under the JS owners below it; a Rust
//! component JS makes is made under the nearest one too.
//!
//! The JS children of a Rust component are made under a JS owner the
//! component's reader marks as their place. Lazy children are made once the
//! component placed them: the calls in them find the owner it placed them
//! under, its context. Eager children are made with the component, before
//! its view: the calls in them see the context around the component, and
//! the Rust components in them are left to the core, which makes their
//! views where the component placed them.

use std::{
    cell::{Cell, RefCell},
    ffi::c_int,
};

use nsh_core::{
    NodeId,
    element::{Owner, RuntimeHandle, create_root, reactive as rust},
};
use rquickjs::qjs;
use rustc_hash::FxHashMap;

use crate::reactive;

type JsContext = qjs::JSContext;
type JsValue = qjs::JSValue;

const UNDEFINED: JsValue = qjs::JS_UNDEFINED;

/// Walks up the JS owners at most this far (a guard: a root made under an
/// owner whose slot was reused could name it).
const MAX_DEPTH: usize = 4096;

/// A JS owner: its node and the node's generation.
type Key = (u32, u32);

/// A tied owner, and whether it was made in the eager children of a Rust
/// component (whose view was not made then): the Rust components made under
/// it are left to the core.
#[derive(Clone, Copy)]
struct Tie {
    owner: Owner,
    in_eager: bool,
}

/// The VM's ties: the core's Rust graph, the tied owners and the JS owners
/// the children of Rust components are made under.
#[derive(Default)]
pub(crate) struct Ties {
    runtime: RefCell<Option<RuntimeHandle>>,
    /// A call ran `with_owner` in this VM: from then on the children of
    /// Rust components mark their place (until then, nothing is tied and
    /// nothing pays for it).
    used: Cell<bool>,
    owners: RefCell<FxHashMap<Key, Tie>>,
    /// The JS owner a Rust component's children are made under, with the
    /// component's host.
    places: RefCell<FxHashMap<Key, NodeId>>,
}

impl Ties {
    /// Frees what is held (the VM goes; the core disposes its graph itself).
    pub(crate) fn free_all(&self) {
        self.owners.borrow_mut().clear();
        self.places.borrow_mut().clear();
        self.runtime.borrow_mut().take();
    }

    fn is_empty(&self) -> bool {
        self.owners.borrow().is_empty() && self.places.borrow().is_empty()
    }

    fn enter(&self) -> Option<nsh_core::element::reactive::Entered> {
        self.runtime
            .borrow()
            .as_ref()
            .and_then(RuntimeHandle::enter_call)
    }
}

fn ties<'a>(cx: *mut JsContext) -> &'a Ties {
    // SAFETY: a projection of the state's field, by shared reference.
    unsafe { &(*crate::state::state(cx)).ties }
}

/// [`ties`], unless the VM's state is gone (a cleanup run at its end).
fn try_ties<'a>(cx: *mut JsContext) -> Option<&'a Ties> {
    let state = crate::state::state(cx);
    // SAFETY: a projection of a live state's field, by shared reference.
    (!state.is_null()).then(|| unsafe { &(*state).ties })
}

/// The core's graph synchronous calls run their Rust reactive code in.
pub(crate) fn set_runtime(cx: *mut JsContext, runtime: Option<RuntimeHandle>) {
    *ties(cx).runtime.borrow_mut() = runtime;
}

/// The key of JS owner `id`.
fn key(cx: *mut JsContext, id: u32) -> Key {
    // SAFETY: a short read of the graph.
    (id, unsafe { reactive::graph(cx) }.generation(id))
}

/// Runs `f` in the core's Rust graph, under the Rust owner tied to the JS
/// owner of what runs now (made if it has none), else under no owner;
/// without a core's graph, as it is.
pub(crate) fn with_owner(cx: *mut JsContext, f: &mut dyn FnMut()) {
    let ties = ties(cx);
    let Some(_entered) = ties.enter() else {
        f();
        return;
    };
    ties.used.set(true);
    let owner = if reactive::has_owner(cx) {
        tied(cx, reactive::current_owner(cx)).owner
    } else {
        Owner::detached()
    };
    owner.run(f);
}

/// The nearest owner at or above JS owner `id` that the Rust code under it
/// sees the context of: a tied owner, or where a Rust component placed the
/// children made under it. Eager children of a Rust component (its place
/// known, its view not made yet) are passed, and the result says so
/// (`in_eager`); with none found, no owner. In a call's entry.
fn nearest(cx: *mut JsContext, mut id: u32) -> Option<Tie> {
    let ties = ties(cx);
    let mut in_eager = false;
    for _ in 0..MAX_DEPTH {
        // SAFETY: short reads of the graph.
        if !unsafe { reactive::graph(cx) }.is_live_owner(id) {
            break;
        }
        let k = key(cx, id);
        let tie = ties.owners.borrow().get(&k).copied();
        if let Some(tie) = tie.filter(|t| t.owner.is_live()) {
            return Some(Tie {
                owner: tie.owner,
                in_eager: tie.in_eager || in_eager,
            });
        }
        let place = ties.places.borrow().get(&k).copied();
        if let Some(host) = place {
            match rust::children_owner(host) {
                Some(owner) => return Some(Tie { owner, in_eager }),
                None => in_eager = true,
            }
        }
        // SAFETY: a short read of the graph.
        id = unsafe { reactive::graph(cx) }.parent(id);
    }
    in_eager.then(|| Tie {
        owner: Owner::detached(),
        in_eager,
    })
}

/// The Rust owner tied to JS owner `id` (live), made under the nearest
/// tied owner above it the first time. In a call's entry.
fn tied(cx: *mut JsContext, id: u32) -> Tie {
    let k = key(cx, id);
    let kept = ties(cx).owners.borrow().get(&k).copied();
    if let Some(tie) = kept.filter(|t| t.owner.is_live()) {
        return tie;
    }
    // From `id` itself: it may be where a component's children are made.
    let parent = nearest(cx, id).unwrap_or(Tie {
        owner: Owner::detached(),
        in_eager: false,
    });
    let (owner, ()) = parent.owner.run(|| create_root(|| ()));
    let tie = Tie {
        owner,
        in_eager: parent.in_eager,
    };
    ties(cx).owners.borrow_mut().insert(k, tie);
    // It goes with the JS owner (the one current: `id`).
    add_cleanup(cx, untie, k);
    tie
}

/// The native function a cleanup of [`add_cleanup`] runs.
type CleanupFn = unsafe extern "C" fn(
    *mut JsContext,
    JsValue,
    c_int,
    *mut JsValue,
    c_int,
    *mut JsValue,
) -> JsValue;

/// Adds `f`, with `k` as its data, to the cleanups of the JS owner current.
fn add_cleanup(cx: *mut JsContext, f: CleanupFn, k: Key) {
    let mut data = [
        qjs::JS_NewFloat64(f64::from(k.0)),
        qjs::JS_NewFloat64(f64::from(k.1)),
    ];
    // SAFETY: the function keeps dups of `data` (numbers); the owner takes
    // it.
    let cleanup = unsafe { qjs::JS_NewCFunctionData(cx, Some(f), 0, 0, 2, data.as_mut_ptr()) };
    reactive::add_cleanup(cx, cleanup);
}

/// The key a cleanup of [`add_cleanup`] holds.
///
/// # Safety
///
/// `data` holds two numbers.
unsafe fn cleanup_key(cx: *mut JsContext, data: *mut JsValue) -> Key {
    let (mut id, mut generation) = (0.0, 0.0);
    // SAFETY: as the caller promises.
    unsafe {
        qjs::JS_ToFloat64(cx, &mut id, *data);
        qjs::JS_ToFloat64(cx, &mut generation, *data.add(1));
    }
    (id as u32, generation as u32)
}

/// A JS owner goes: its tied owner goes too.
unsafe extern "C" fn untie(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
    _magic: c_int,
    data: *mut JsValue,
) -> JsValue {
    // SAFETY: `add_cleanup`'s data.
    let k = unsafe { cleanup_key(cx, data) };
    let Some(ties) = try_ties(cx) else {
        return UNDEFINED;
    };
    let tie = ties.owners.borrow_mut().remove(&k);
    if let (Some(tie), Some(_entered)) = (tie, ties.enter()) {
        tie.owner.dispose();
    }
    UNDEFINED
}

/// The Rust component on `host` is made now, under the JS owner current:
/// its view is made under the nearest tied owner, if there is one (not in
/// the eager children of another Rust component: the core places those).
pub(crate) fn place_component(cx: *mut JsContext, host: NodeId) {
    let ties = ties(cx);
    if ties.is_empty() || !reactive::has_owner(cx) {
        return;
    }
    let Some(_entered) = ties.enter() else {
        return;
    };
    if let Some(tie) = nearest(cx, reactive::current_owner(cx))
        && !tie.in_eager
        && tie.owner.is_live()
    {
        rust::set_component_owner(host, tie.owner);
    }
}

/// The reader of the JS children of the Rust component on `host` (owned):
/// `reader` (borrowed), run with the JS owner it runs under marked as the
/// place of the component's children. `lazy` children (made once the
/// component placed them) are always marked, whenever the calls that read
/// the place come, as they are what sees the component's context; eager
/// ones only once a call of this VM ran `with_owner` (`None` before), as
/// they only keep the Rust components made in them with the core's
/// placement, so a VM whose calls never use owners pays nothing for its
/// components' eager children.
pub(crate) fn placed_reader(
    cx: *mut JsContext,
    host: NodeId,
    reader: JsValue,
    lazy: bool,
) -> Option<JsValue> {
    if !lazy && !ties(cx).used.get() {
        return None;
    }
    let mut data = [reader, qjs::JS_NewFloat64(f64::from(host))];
    // SAFETY: the function keeps dups of `data` (the borrowed reader, a
    // number).
    Some(unsafe { qjs::JS_NewCFunctionData(cx, Some(read_placed), 0, 0, 2, data.as_mut_ptr()) })
}

/// Reads a component's children ([`placed_reader`]).
unsafe extern "C" fn read_placed(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
    _magic: c_int,
    data: *mut JsValue,
) -> JsValue {
    // SAFETY: two data values (`placed_reader`).
    let (reader, host) = unsafe { (*data, *data.add(1)) };
    if reactive::has_owner(cx) {
        let mut id = 0.0;
        // SAFETY: a number.
        unsafe { qjs::JS_ToFloat64(cx, &mut id, host) };
        let k = key(cx, reactive::current_owner(cx));
        let fresh = ties(cx)
            .places
            .borrow_mut()
            .insert(k, id as NodeId)
            .is_none();
        if fresh {
            add_cleanup(cx, unplace, k);
        }
    }
    // SAFETY: calls the children's reader.
    unsafe { qjs::JS_Call(cx, reader, UNDEFINED, 0, std::ptr::null_mut()) }
}

/// The JS owner a component's children were made under goes.
unsafe extern "C" fn unplace(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
    _magic: c_int,
    data: *mut JsValue,
) -> JsValue {
    // SAFETY: `add_cleanup`'s data.
    let k = unsafe { cleanup_key(cx, data) };
    if let Some(ties) = try_ties(cx) {
        ties.places.borrow_mut().remove(&k);
    }
    UNDEFINED
}
