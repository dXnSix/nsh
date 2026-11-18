# Classes and functions from Rust

Status: implemented (`nsh_core::element` `class.rs`, `callback.rs`, `call.rs`, `js.rs`; `nsh-macros` `class.rs`;
`nsh-js` `classes.rs`, `components.rs`); "Implementation notes" at the end says where it differs from the
proposal.

An app's Rust crate (`nsh.json` `"rust"`) exports components, modules (`#[module]`: functions) and classes
(`examples/rust-exports`: one of each, every member kind below, used from TSX and tested headless). This
describes classes: a Rust type that TypeScript uses as a class, with a constructor, methods, getters and setters, statics and
instances that keep their state in Rust; synchronous functions on the same footing; and how NSH's own standard
library shares the machinery.

```rust
/// A tally kept in Rust.
#[class]
pub struct Tally {
    count: i64,
    log: Vec<String>,
}

#[class]
impl Tally {
    /// Starts at `start` (0 when left out).
    pub fn new(start: Option<i64>) -> Self {
        Tally { count: start.unwrap_or(0), log: Vec::new() }
    }

    /// Adds `by`; the new count.
    pub fn add(&mut self, by: i64) -> i64 {
        self.count += by;
        self.log.push(format!("+{by}"));
        self.count
    }

    /// The count.
    #[getter]
    pub fn count(&self) -> i64 {
        self.count
    }

    #[setter]
    pub fn set_count(&mut self, count: i64) {
        self.count = count;
    }

    /// A tally from `"12"`; throws on anything else.
    pub fn parse(text: String) -> Result<Tally, String> {
        Ok(Tally::new(Some(text.trim().parse().map_err(|_| format!("not a number: {text}"))?)))
    }

    /// Saves the log; resolves when it is written.
    pub async fn save(this: Instance<Tally>, path: String) -> Result<(), String> {
        let lines = this.with(|t| t.log.join("\n"));
        blocking(move || std::fs::write(path, lines)).await.map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }
}

#[native_module("app/native")]
fn native(registry: &mut Registry) {
    registry.class::<Tally>();
}
```

```ts
// .nsh/types/app/native.d.ts, generated
/** A tally kept in Rust. */
export declare class Tally {
  /** Starts at `start` (0 when left out). */
  constructor(start?: number);
  /** Adds `by`; the new count. */
  add(by: number): number;
  /** The count. */
  get count(): number;
  set count(count: number);
  /** A tally from `"12"`; throws on anything else. */
  static parse(text: string): Tally;
  /** Saves the log; resolves when it is written. */
  save(path: string): Promise<void>;
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}
```

```tsx
import { Tally } from "app/native";
const tally = new Tally(3);
tally.add(2);            // 5, synchronously
tally.count = 10;
await tally.save("/tmp/log");
```

## Why synchronous

Module functions are ops: JS queues them and the core runs them after the turn, so they always return a promise.
That is fine for "do something" (show an alert) but not for something that looks like a class: a getter cannot be a
promise, and `tally.add(2) + 1` should be a number. JS and Rust components already run on one thread (the engine's),
so a class's methods can run directly inside the JS call, as the web classes nsh-js implements in Rust (`Blob`,
`Headers`, `URL`) do.

## The model

- **Instances live in Rust, owned by their JS object.** A JS object of the class holds the Rust value as its opaque
  (`Rc<RefCell<T>>`, the web classes' slot, `nsh-js` `web/mod.rs`); the object being collected drops it.
  `dispose()` (and `[Symbol.dispose]`, for `using`) drops it now (a value holding a file or a large buffer); any use
  after throws. `T: 'static`, not
  `Send`: it never leaves the engine thread.
- **Synchronous members run in the JS call.** `&self` / `&mut self` methods, getters and setters, statics and the
  constructor run inside the JS turn, outside the core: they compute and keep state, and cannot use the UI's free
  functions (signals, `dialog`, `theme()`): the reactive graph is not entered there, and calling one panics with a
  message saying so (as on any thread without it). `runtime_entered()` tells code that runs in both places which
  one it is in, and `start_task(future)` starts a task of the core from either (below). A method borrows its instance for its call; a re-entrant call
  (a method reaching the same instance again) throws instead of aliasing, as the web classes do.
- **`async` members are core tasks.** An `async fn` method gets `this: Instance<Self>` (a handle: `with` /
  `with_mut` borrow the value between awaits) and returns a promise; it runs as a task of the core, where the free
  functions work (`dialog`, `timer`, `blocking`, `background`, the clipboard), as an `async` module function does.
  `this` and the arguments are converted in the JS call, as a synchronous member's are (a wrong one rejects the
  promise with the same `TypeError`: a call returning a promise never throws), into the start of the task: a closure holding the typed values, the
  instance among them, that an op carries to the core ([below](#how-calls-cross)). The task keeps the instance
  alive until it ends.
- **One object per instance.** While both live, an instance has one JS object (`nsh-js` remembers it by the `Rc`'s
  address): a member returning an instance Rust keeps gives the object it has, so `a.badge === a.badge`.
- **Errors.** `Result<T, E: Display>` throws (`Error` with the message) or rejects; a wrong argument throws (or,
  for a member returning a promise, rejects with) a `TypeError` naming the member and the parameter (``Tally.add: `by`: expected a number, got "x"``), and so does a
disposed or busy instance passed for a borrowed parameter (``Account.transfer: `to`: Account is disposed``); a panic in
  Rust is caught and thrown as an `Error`, or rejects (the engine stays up) where Rust unwinds: the web runtime is
  built with `panic = "abort"` (`[profile.web]`), so there a panic ends the page's engine. Integer parameters take
  whole numbers in their range (a fraction, `NaN` or a value out of range is a `TypeError`, not rounded or
  clamped), and one call reads at most 2^20 array items and fields, as a JSON conversion does.

## Functions

Rust exports functions through `#[module]` (an object, `AlertDialog.show(..)`, or `#[module(flat)]`, top-level
functions, which a module named like one with `#[module(name = "…")]` gives properties: `test.skip(..)`;
`docs/dev/components.md`). Every call used to be an op ordered with the UI's, so every result was a promise. The class's split
applies to functions too, decided by the signature (`FunctionCall::Sync` / `Op` / `Async`) of each function with
a visibility (`pub fn`, `pub(crate) fn`; the others are the block's helpers):

- **`fn` without a context:** synchronous, called directly in the JS call like a class's method (`parse(text)`,
  `checksum(bytes)`): TS gets the value, not a promise.
- **`fn` taking `cx: &mut ModuleCx`:** an op, run in order with what JS set before it (a dialog, the status bar,
  focus, `cx.run_on_main_thread(f)` for APIs that only work on the platform's main thread). Its return type decides
  what TS gets:
  - returning `()`, it gives `void`, with no reply: JS calls it and moves on, the op runs in order with the UI at
    the commit, nothing answers it (`RustOp::call` 0, no `JsEvent::Reply`), and a wrong argument throws in the call;
  - returning `Result<(), E>`, it gives `Promise<void>`: resolved once the op ran, rejected with the `Err`'s message.
    It cannot throw instead, because the op runs at the commit, after the JS call returned. A call nobody awaits or
    catches is still seen: the rejection is logged as an unhandled one (`docs/dev/pitfalls.md`);
  - returning a value (`T`, `Result<T, E>`), it gives `Promise<T>`, and a wrong argument rejects it.
- **`async fn`:** a task of the core; it gives a Promise (`Promise<void>` for `()`, resolved when the task ends).

The same holds for a component's ref methods (`docs/dev/components.md`, "Methods"). An op returning `()` is
`void` because the runtime modules released theirs as `void` (`WindowRef.close`,
...) and a return type never changes (`docs/dev/compatibility.md`); a Rust function that can fail says so with
`Result<(), E>`, and TS gets a promise to see the error. `FunctionSpec::promises` / `MethodSpec::promises` decide
it for the declarations and the JS face alike; `-> ()` written out is the same as no return type.

A plain `fn` that returns a value (today a promise in TS) becomes synchronous: no runtime module has one (theirs
take `ModuleCx` or are `async`), and an app crate's are rebuilt with the app, whose types change with them.

## The standard library

What TSX imports from NSH is made two ways today:

- **Runtime modules in Rust** (`crates/nsh-core/src/runtime/`: `nsh`'s `Keyboard`, `Image`, `Clipboard`,
  `Storage` and accessibility functions, `nsh/mobile`, `nsh/desktop`) already use `#[module]` and `Registry`, the app crates'
  mechanism; they take the synchronous functions and classes as they come.
- **Hand-written in `nsh-js`:** the reactive graph and the renderer (`nsh`), `nsh`'s `AnimatedValue` and
  `AnimatedNode`, `nsh`'s `Path2D`, `CanvasGradient` and `CanvasRenderingContext2D`, the keymap, lists, and the
  web globals (`URL`, `Headers`, `Blob`, `fetch`, streams, events). Their classes already sit on one piece of
  machinery: a QuickJS class whose opaque is the Rust value in a `RefCell` (`nsh-js` `web/mod.rs`).

So the classes of this proposal are built on that same machinery: one implementation of "a JS class backed by a
Rust value", with `#[class]` its declarative front. Which standard classes move to `#[class]` depends on what they
need beyond the first version:

| Class | Needs | Moves |
|---|---|---|
| `Path2D`, `CanvasGradient` | plain values, called often | when calls convert QuickJS values directly (below) |
| `CanvasRenderingContext2D` | 17 500 calls in ~1 ms (`docs/dev/performance.md`) | stays recorded natively: a class front adds nothing |
| `AnimatedValue`, `AnimatedNode` | values the renderer reads from style props, ops ordered with the UI | stays: it is part of the renderer |
| `URL`, `TextEncoder` | WebIDL coercion, `new.target`, iterators | after subclassing and iterators |
| `Headers`, `Blob`, streams, `EventTarget` | callbacks, events, promises interleaved with JS | after callbacks and events |
| `nsh` itself (signals, `View`, `mount`) | the reactive graph and the JSX compiler's calls | stays: it is the runtime |

For that, two choices of the first version change:

- **Values convert directly**, QuickJS value to Rust type and back, not through `JsonValue`: a synchronous call is
  what makes these classes worth using, and a JSON round trip per call would cost more than the call (the canvas'
  recording is a native list for that reason). `JsValue` has only the direct conversions; what crosses to the
  core (ops, `async` calls, Rust components' props, events and methods) crosses typed too
  ([below](#how-calls-cross)).
- **The machinery is shared from the start**: `#[class]` instances are the web classes' slots, so `instanceof`,
  re-entrancy, finalization and (later) `new.target` behave the same for NSH's classes and an app's.

## What crosses

- **Arguments and results** are the module functions' types (`JsValue`: numbers, `bool`, strings, `Option`, `Vec`,
  `JsonValue`, `#[derive(JsObject)]` structs, `#[derive(PropEnum)]` enums), so classes and modules share one set of
  conversions and TypeScript types (`Arg` and `Ret` over them); a trailing `Option` parameter is optional.
  `Vec<u8>` is bytes: a `Uint8Array` (a `Buffer` too), copied in the call (`u8` is no `JsValue`, so it never
  overlaps the arrays); `SystemTime` is a `Date`. `#[prop(exact)]` on a `JsObject` struct refuses other own keys
  (options, where a typo must not pass), and `#[prop(skip_none)]` on an `Option` field leaves it out when `None`
  rather than `null` (`process`' exit `error`).
- **Interfaces**: `#[class(interface)]` on the struct makes a class whose objects come from members only: its
  module exports no value of it and TypeScript declares an `interface` (`fs/promises`' `Stats`); it has no
  constructor or statics. Its getters are read once when its object is made, as own enumerable data properties
  (so `Object.keys`, spread and `JSON.stringify` see them, as Node's `Stats`' fields), its methods stay on the
  prototype (`nsh-js` `classes.rs`). A result `Vec<Instance<T>>` is an array of objects.
- **The VM's `System`**: a parameter `sys: System` of a function or member (any kind) is not one JS passes: the
  call fills it in (an `async` one when it is made) with the app's `Access`, its data directory, the host's
  `TrustedPaths`, the VM's `Quotas` (counts of limited resources, `quotas.take(kind, max)` or
  `quotas.reserve(kind, amount, max)` giving a `Held` back when dropped) and the surface's `Os`. The host provides it (`JsHost::system`); one that configured nothing grants nothing.
- **Work started in the call**: a function or method that is not `async` and returns `Later<T>` (or
  `Result<Later<T>, E>`) runs in the JS call, in order with what JS sent before it, and the work it returns runs
  as a core task: TS `Promise<T>` (`FunctionCall::Start`, `MemberCall::Start`). What the call refuses (an
  argument, a grant, a quota) rejects the promise, as the task's errors do: a call returning a promise never
  throws (`docs/dev/api-naming.md` "Failures"). The system modules use it (`net`, `fs/promises`).
- **Errors of a chosen class**: `Result<T, JsError>` throws a `TypeError`, `RangeError` or `Error`
  (`JsError::Type` / `Range` / `Error`); any other `E: Display` throws an `Error` with its message. A `Later`
  task's (and an `async` function's) `Err(JsError)` rejects with the same, any other error with an `Error` of its
  message (the reply carries a `Thrown`). `JsError::with(key, value)` / `with_code(code)` add properties
  (`JsError::Fields`), so errors keep Node's shape: `code`, `syscall`, `path` (`error.code === "ENOENT"`);
  `JsError::from(io::Error)` and `io_code` name an I/O error's code from its OS error or kind. The system modules
  share it (`fs/promises`; `process` and `net` can). `JsError::access_denied(message, permission, resource)` is what
  a call the capabilities do not grant fails with (`ERR_ACCESS_DENIED`), and `nsh_core::element::codes` holds the
  codes NSH's calls use beside I/O's (`docs/dev/api-naming.md` "Failures").
- **What a synchronous call can do beyond computing**: a parameter `cx: CallCx` (filled in by the call, never in an
  `async` one) starts a task of the core in order with the ops (`cx.start(future)`, nothing waits for it) and ties
  work to what owns the call in JS (`cx.on_cleanup(f)`: the reactive owner, else the module's run under hot
  reload). `process.spawn` returns its child at once and delivers the child's events from such a task.
- **Starting a task without `cx`**: `start_task(future)` (`nsh_core::element`, prelude) starts a root task of the
  core (no owner ends it, nothing waits for it) from Rust code that has no `CallCx` at hand: inside a synchronous
  call from JS it goes as `cx.start` does, in order with what JS sent before the call, when the core takes the
  turn's ops; where the reactive runtime is entered (a task, a component) it is spawned on the spot, polled at the
  executor's next run. Elsewhere (off the engine thread, in a finalizer, once the VM is gone) it is not started:
  `Err(NotStarted(future))` gives it back. `runtime_entered()` says whether the reactive runtime is entered on this
  thread (the engine thread while the core runs Rust: a task, a component, an op), where signals and `spawn`
  work; `false` in a synchronous call from JS. The JS host registers itself per VM (`CallTasks`,
  `register_call_tasks`: `nsh-js`'s callbacks' table, the web player's bindings) so `start_task` finds the VM in a
  call; with two VMs in calls at once (one's JS calling the other's Rust) it starts none.
- **Theme writes in a call**: `set_modifier`, `set_color_scheme`, `update_tokens`, `reset_tokens` and
  `reset_all_tokens` (`element/ambient.rs` `write_theme`) are actions of the executor where the reactive runtime is
  entered; inside a synchronous call from JS, where it is not, they go through the same `CallTasks` of the VM
  (`push_op`) as the root's op JS's own write sends (`Prop::UiModifier`, `Prop::Appearance`, `Prop::UiThemeUpdate`,
  `Prop::UiThemeReset`), after what JS sent before the call. The core applies them with the turn's ops before the
  turn's commit, so a class extension that picks a theme context before `mount` (a framework's `addTheme`) is in
  the first frame; a task the call starts would first run only after it. Outside both they panic as before.
  `set_theme` stays out: JS's `setTheme` also keeps the VM's view of the active document (`turn_document`, the
  base document), which an op alone would not.
- **The VM's own state**: `sys.locals.get::<T: Local>()` is one `T` per VM, made on first use and closed
  (`Local::close`) when the VM goes, before its callbacks are released (`process`'s children).
- **Getters of a module**: `#[getter]` on a function without JS parameters makes it a property of the module's
  object (`process.env`), declared `readonly`, never a named export. A flat default module's object also carries
  the registry's native values (`Registry::native_values`: `process.nextTick`), and stays one object per VM, so
  a global made from it (`process`) is the default export.
- **`this` in a synchronous method**: `fn on(this: Instance<Self>, ..) -> Instance<Self>` gets the handle instead
  of a borrow, so it can return its own object (`child.on(..)` chains).
- **Overloads**: `#[overload(fn(event: ExitEvent, handler: Callback<ExitResult>) -> Instance<ChildProcess>)]`
  (repeatable) on a module function or a member declares those signatures, from their Rust types, instead of
  the one its own types give (the result is a promise when the function's is): what an argument selects
  (`on("exit", ..)`, `readFile(path, "utf8")`).
- **Signatures written in TypeScript**: `#[ts("<T>(token: { readonly prototype: T }): T")]` (repeatable, in order
  with any `#[overload]`) on a module function or a member declares that signature as written, generic ones too,
  where the Rust types are looser than the TS should be (`getRequiredService(token: JsHandle) -> JsHandle`, which TS
  gets as `getRequiredService<T>(token: { readonly prototype: T }): T`; `{ readonly prototype: T }` is how a class
  with a `private constructor()` is taken, as no `new (..) => T` takes it). The text is TypeScript's: type
  parameters, then the parameters, then `: Result` (or nothing for `void`); the result is a promise of it when the
  function's is. `nsh-macros` (`ts.rs`) checks it against the Rust function at compile time: its parameters are the
  ones JS passes (not `System` or `CallCx`), with their names in TS and in order; a trailing optional one may be left
  out, and only an optional one takes `?`; it gives a result exactly when the function returns a value; a
  constructor, getter or setter has no type parameters (and a getter's is `(): T`). The types themselves are left to
  TypeScript, and the classes the text names are not registered by it: the set registers them, or a Rust type of the
  signature names them.
- **Generic classes**: `#[class(generic = "T")]` on the struct declares the class generic over those TypeScript type
  parameters, written as TypeScript writes them (`T extends object = object, K`; `in` / `out` are refused): `export
  declare class ServiceToken<T>`. The class itself is one Rust type and one JS class; only its declaration is
  generic. TypeScript compares classes by their members, so a type parameter no member names would let a
  `ServiceToken<A>` pass for a `ServiceToken<B>`: the declaration keeps them in an ES private member, `#types: [T]`,
  which no code can read (it has no value at run time) and which makes them covariant. Its `#[ts]` signatures name
  them (`#[ts("<T>(name: string): ServiceToken<T>")]` on a static); a Rust type naming the class (`Instance<ServiceToken>`,
  a result `ServiceToken`) is `ServiceToken<any>`. A `#[class(interface)]` is not generic (an `interface` has no
  private member).
- **Type aliases the signatures share**: `registry.ts_alias(TsAlias { name: "ServiceKey<T>", ts: "{ readonly
  prototype: T } | ServiceToken<T>", doc: "…" })` exports `export type ServiceKey<T> = …;` with the set's named types,
  so `#[ts("<T>(token: ServiceKey<T>): T")]` signatures need not repeat the union. A declaration only (TS imports it
  with `import type`); its name is the set's like a class's (a class, interface, module or component of that name
  panics, as two of those do), and `Registry::undocumented` lists one without a doc.
- **Instances of classes**, by reference: a parameter `&Other` / `&mut Other` borrows another instance for the call
  (TS: `other: Other`), `Instance<Other>` keeps a handle (to store one instance in another), and a result `Other`
  or `Instance<Other>` is a JS object (`Self` from `new` and statics; `Option<Instance<Other>>` for none). An
  instance of another app's or set's class is refused with a `TypeError`. `async` members and module functions keep
  what they take: `Instance<Other>`, not `&Other`.
- **Classes themselves**: a parameter `Constructor` takes a Rust class's constructor, the value TS names the class
  by (`getRequiredService(I18nService)`), and tells Rust which `#[class]` it is (`def()`, `is::<I18nService>()`;
  `Eq` and `Hash` by the class, so it keys a table of services). Only constructors NSH made for Rust classes are
  taken, of any set the VM serves: a class TS declares, a TS subclass of a Rust class, any other function or value
  throws a `TypeError` (``Services.nameOf: `service`: expected a Rust class, got a function``). A result
  `Constructor` (`Constructor::of::<C>()`) gives JS the class's constructor, made on first use as an import makes
  it. TS: `Function`, since TypeScript assigns a class declared with `private constructor()` to no `new (...) => …`
  type. A `JsHandle` tells the same of the object it keeps (`handle.constructor()`), for a token that is a Rust
  class or anything else.
- **JS functions** (callbacks), below.
- **JS objects** Rust keeps (`JsHandle`), below, and **objects implementing a Rust trait** (`#[interface]`), below.
- **Not in the first version:** events emitted by an instance (`EventTarget`) and iterators. Constructors honor
  `new.target` as the web classes' do, so a TS class may extend a Rust one.

## Callbacks

A parameter `Callback<A, R = ()>` takes a JS function: `A` is its argument (a `JsValue` type, or a tuple for
several), `R` what it returns. TS: `Callback<f64>` is `(value: number) => void`, `Callback<(String, f64), bool>`
`(a: string, b: number) => boolean`; `#[args(..)]` on the parameter (a function's, a member's or an `#[overload]`'s)
names the function's arguments, checked at compile time to name each (`#[args(name, amount)] check: Callback<(String,
f64), bool>` is `check: (name: string, amount: number) => boolean`), in classes, modules and ref methods alike. The
attribute is the macros': it is removed from the function they leave.

```rust
#[class]
impl Downloader {
    /// Calls `progress` with each fraction done; resolves with the bytes' count.
    pub async fn fetch(this: Instance<Downloader>, url: String, progress: Callback<f64>) -> Result<u64, String> {
        // ...
        progress.post(0.5);                       // runs in JS after this step, in order
        progress.call_async(1.0).await?;          // runs in JS, its result back here
        Ok(bytes)
    }

    /// Sorts with the app's comparison, at once.
    pub fn sort_by(&mut self, compare: Callback<(String, String), f64>) -> Result<(), String> {
        self.items.sort_by(|a, b| {
            let order = compare.call((a.clone(), b.clone())).unwrap_or(0.0);
            order.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(())
    }
}
```

- **Calling it now** (`call(args) -> Result<R, String>`): inside a synchronous call from JS (a method, a function,
  the constructor), the JS function runs on the spot and its result comes back, as a JS callback passed to
  `Array.prototype.sort` does. Its exception is the `Err` (its message). The instance stays borrowed meanwhile:
  the callback reaching the same instance again throws, as any re-entrant call does. `call` also runs the function
  on the spot **between JS's turns**: Rust code on the engine thread while no JS of the thread runs (a task, an
  `async` member's body, an op, a component, a timer, a factory Rust runs there), as a call into JS of its own
  within the VM's budget of a call (one that overruns it fails and the VM goes on; running out of memory stops the
  VM, as in any call). It runs before the calls posted earlier that have not run yet (a task that posts a change
  and then calls reads the state from before it), and what its JS posts, sends or starts runs in JS's next turn.
  Where JS of the thread runs without a call to Rust (another VM's) `call` panics, saying to use `post` or
  `call_async`; in a finalizer and once the VM is gone it fails.
- **Asking first** (`can_call_now() -> bool`): whether a call made now runs on the spot. `true` inside a
  synchronous call from JS of the function's own VM, nested ones included (JS that a `call` runs calling Rust
  again) and calls from a posted turn; `false` after that call returned (a task, an `async` member's body, a timer),
  in a finalizer (a value the collector or a refcount free drops), once the VM is gone, and while only another VM on
  the thread is in a call. Where it is `false`, `call` panics (outside a call) or fails (in a finalizer, once JS
  stopped): use `post` or `call_async`. `true` does not promise success (the function may throw, a re-entrant use
  of the instance throws). Asking runs no JS, so it is safe anywhere, a `Drop` too. A `JsHandle` and an
  interface's stand-in (`stand_in.object().can_call_now()`) answer the same for their object.
  `can_run_now() -> bool` answers where `call` runs on the spot at all: inside a call from JS and between JS's
  turns; `false` in a finalizer, once the VM is gone, and while JS of another VM on the thread runs (a
  `JsHandle`'s answers the same for `get` and its stand-in's members returning a value).
- **Calling it later** (`post(args)`, `call_async(args).await -> Result<R, String>`): from anywhere on the engine
  thread (an async member's task, a timer, a stored callback), the call is queued for JS and runs in the engine's
  next JS turn, in order with the other posts: `post` forgets the result, `call_async` waits for it (and settles a
  promise the function returns). The calls wait in one ordered queue (`Posts`, made by the engine and handed to
  `nsh-js` and the core: `HostOptions::posts`, `Core::set_posts`), which the engine runs at each commit round
  before the core's events (`Host::run_posted`), so a task's posts reach JS before its result and cost one turn,
  not a thread hop. They run in the order they were posted, keeping JS's program order: a post made inside a
  synchronous call from JS is parked in its place until an op JS sends after it is applied
  (`RustCall::Post`, whose place is released when the op goes by any path, applied or dropped, so a lost op
  never holds back later posts), and what the ops JS sent before it post while the core applies them goes before
  it. So an op function posting and then a synchronous one posting arrive in that order, and a task posting while
  JS is still busy after the synchronous call arrives after it. Once the VM stops, queued calls and those posted later are
  dropped and their replies say so. `call_async_traced` is `call_async` (and `call_traced` is `call`) with what was
  thrown whole (`Exception`: its message, for an `Error` its stack, and the value as JSON, `thrown`: an object's own
  enumerable properties with its `name` and `message`, `None` when converting it throws), for a caller that says
  where it was thrown or checks it (`nsh test`'s failures, its `toThrow` and `toMatchObject`).
  `Callback::settling(value)` keeps a value in place of a function: `call_async` of it answers the value, a
  promise what it settles to (`nsh/test`'s `expect(promise)`).
  `call` rethrows an uncatchable exception (the CPU budget's interrupt, running
  out of memory) when the synchronous call around it returns, whatever Rust made of its `Err`.
- **In a finalizer:** a value whose JS object goes drops inside QuickJS's finalizer, where no JS may run. That is
  every finalizer, not only a collection's sweep: an object freed because its last reference went (a refcount
  free, at any point JS releases a value) finalizes the same way. `nsh-js` raises a counter around every
  finalizer's drop (`web::finalizing`); while it is raised, a callback's `call` fails (`Err`: called while JS
  collects garbage, use `post`) and `post` is not parked in program order: it goes to the end of the queue, without
  an op, and runs in the next turn. Both are about callbacks the dropping value does not own, since its own were
  released first (below).
- **Keeping it:** a `Callback` can be stored (in an instance's field, a `Vec`, a global) and called any number of
  times; dropping the last clone releases the JS function. A callback an instance keeps is reported to QuickJS's
  collector through the class's mark hook, as the web classes report the JS values they hold. `#[class]` traces
  every field whose type implements the `unsafe trait Trace` (`Callback`, `JsHandle`, `Instance`, and `Option`, `Box`,
  `RefCell`, `Vec`, `VecDeque`, map values and pairs of them; chosen by type, so aliases count and other types are
  skipped) and reports them while the object is the instance's only owner and the callback has no other clone,
  so a callback that captures the object owning the instance does not keep both alive forever. Callbacks held
  elsewhere (an `Rc` shared by two instances included) are roots until dropped. The functions belong to the VM:
  freed with it, a callback Rust keeps longer does nothing. When the collector frees an object that owns its
  instance alone, the instance's callbacks are released before its value drops, so a `Drop` calling or posting
  one finds it gone instead of a function the same collection frees. That includes the callbacks of instances
  it alone holds: a `Drop` that moves such an instance out (into a static, a task) keeps the instance, but its
  callbacks are dead (calling fails, posting does nothing). `Trace` never panics, allocates JS values or runs JS
  (it runs in the collector's mark phase), part of its safety contract.
- **Functions take them too:** `fn on(event: String, listener: Callback<String>) -> Stop` (`listener` kept and posted
  later), and `async` functions and members can `call_async`.
- **Listeners**: an instance keeps them in a field (`listeners: Vec<Callback<String>>`, added by
  `on(&mut self, event: String, listener: Callback<String>) -> Stop`, below) and its tasks post to them (`Later` work cloning the field); the
  field is traced like any other, so no event machinery is needed for `child.on("stdout", ..)`.
- **Stopping**: a function or method that registers a listener (`on(event, listener)`, `docs/dev/api-naming.md`
  "Listening") returns `Stop` (`nsh_core::prelude`, `element/system.rs`; `Stop::new(f)`), which crosses as a JS
  function (`() => void`). `f` runs once: when JS calls it, or when the owner of the call in JS goes (the reactive
  owner it runs under, or the module's run under hot module replacement: `JsHost::on_cleanup`), whichever comes
  first; calling it unregisters the cleanup, so a long-lived owner does not collect stopped listeners. The
  Node-shaped standard modules keep Node's `on` returning the instance (`child.on(..)` chains).

## JS objects

A parameter `JsHandle` takes any JS object, array or function (TS `object`) and keeps it: Rust stores it as long as
it likes (a field, a map, a task) and gives it back to JS as the very same object (`===`). It is how Rust keeps what
TS made, a service or a token, without converting it.

```rust
#[class]
impl Services {
    /// Keeps `service` under `token`.
    pub fn add(&mut self, token: JsHandle, service: JsHandle) {
        self.by_token.insert(token, service);
    }

    /// What `token` was given, or `null`.
    pub fn get(&self, token: JsHandle) -> Option<JsHandle> {
        self.by_token.get(&token).cloned()
    }
}
```

- **Identity.** Two handles are equal (`Eq`, `Hash`) when they keep the same object, so a handle is a map key: the
  same object converted twice gives equal handles, two objects with the same fields do not.
- **Wherever a value crosses.** It is a `JsValue`, so `Option<JsHandle>` (`object | null`), `Vec<JsHandle>`, a
  callback's result (`Callback<(), JsHandle>`, a factory TS passes) and a field of a `#[derive(JsObject)]` struct
  take it too. `null`, `undefined` and primitives are refused with a `TypeError` (``Keeper.keep: `object`: expected
  an object, got 3``).
- **Kept as a callback is.** The object sits in the table of the values Rust keeps, where callbacks' functions are
  (`nsh-js` `classes.rs`; the web runtime's handles), and is released when the last clone drops. A handle a
  `#[class]` field holds alone is reported to the collector through the class's `Trace`, so an object that reaches
  the instance's own JS object (a service holding its registry) is collected with it; one held elsewhere (a static,
  a task, an `Rc` two instances share) is a root until dropped.
- **Its VM's, on the engine thread.** It is not `Send`. A host of another VM (two engines on one thread) refuses it
  (`the object belongs to a VM that is gone, or another`), and once its VM goes it is dead: giving it back to JS
  fails, its equality still holds.
- **Whether it can be called now.** `can_call_now()` and `can_run_now()` answer for the object what a callback's
  do (see "Asking first" under Callbacks): whether a call from JS is in progress (where every interface member runs
  on the spot), and whether JS runs on the spot now at all (`get`, getters, members returning a value).
- **Reading its properties.** `handle.get::<T>("name")` reads the object's property as JS reads it (a getter it
  defines runs) and converts it (`Option<T>` for one that may be `undefined`; a `JsHandle` for an object in it,
  `Result<T, Exception>`: what a getter throws, or a value that does not convert). It reads on the spot inside a
  synchronous call from JS and also **between JS's turns**: Rust code on the engine thread while no JS of the
  thread runs (a task, a component, a timer, a factory Rust runs there). `can_run_now()` asks first: `false` in a
  finalizer, once the VM is gone, and while JS of another VM on the thread runs. Between turns the read is a call into JS of its own, within the VM's budget of a call (one that
  overruns it fails, and the VM goes on; running out of memory stops the VM, as in any call), and runs before the
  calls posted earlier that have not run yet: a task that posts a change and then reads the value reads it from
  before the change. What the read's JS posts, sends or starts runs in JS's next turn, as a posted call's does
  (`CallbackHandle::between_turns` / `call_between_turns`: `nsh-js` counts the calls into JS of the thread's VMs,
  `host.rs` `Entered`, and runs the read under the host's budget, `BetweenTurns`; the web player reads when no call
  from the realm is in progress).

## Interfaces

`#[interface]` on a Rust trait makes it a TS interface that TS objects implement for Rust to call: the set's
declarations get an `export interface` of the trait's name, and Rust gets the trait's stand-in, `Js<Trait>`, a
struct over a `JsHandle` that implements the trait by calling the JS object's members. Rust code holding a
`JsStartable` uses it as any implementation of `Startable`.

```rust
/// A service the app writes in TS.
#[interface]
pub trait Startable {
    /// Starts as `name`; resolves once started.
    async fn start(&self, name: String) -> Result<(), String>;
    /// Whether it started.
    fn is_started(&self) -> bool;
    /// How long it may take, in ms (the registry's by default).
    #[getter]
    fn timeout(&self) -> Option<f64> {
        None
    }
}

#[native_module("app/native")]
fn native(registry: &mut Registry) {
    registry.interface::<JsStartable>();
}
```

```ts
// .nsh/types/app/native.d.ts, generated
/** A service the app writes in TS. */
export interface Startable {
  /** Starts as `name`; resolves once started. */
  start(name: string): Promise<void>;
  /** Whether it started. */
  isStarted(): boolean;
  /** How long it may take, in ms (the registry's by default). */
  readonly timeout?: number | null;
}
```

- **Members.** A method takes `&self` and owned values Rust passes to JS (what a class's member returns: strings,
  numbers, `JsHandle`, `Instance<T>`, `#[derive(JsObject)]` structs; up to eight), and returns what JS gives back (a
  `JsValue` type, `Instance<T>`, `()`) or a `Result` of it; its TS name is camel case, as a class member's.
  `#[getter]` on a method without parameters reads the property of its name (`readonly timeout`). A member with a
  default body is optional in TS (`stop?()`, `readonly timeout?`): an object without it (`undefined`) runs the
  default, with the same arguments. An interface has no generics, associated items or supertraits: its stand-in
  implements this trait alone.
- **When a member runs.** A method that is not `async` calls the JS method on the spot inside a synchronous call from
  JS, as a callback's `call` does (a class's method calling the service it was given). Outside one (a task, a
  timer), one returning `()` is posted for JS's next turn, in order with the other posts (`Callback::post`), and one
  returning a value runs on the spot between JS's turns, as `Callback::call` does there (before the posts not run
  yet). A getter that is not `async` is read on the spot in both places (see "Reading its properties" under JS
  objects), so Rust reads the state a TS object publishes, and calls it, wherever it holds the object. They fail
  (the `Err` of a `Result`) only where JS of the thread runs without a call to Rust (a finalizer, another VM) or the
  VM is gone. `stand_in.object().can_call_now()` says whether a call from JS is in progress and
  `can_run_now()` whether JS runs now at all (see "Asking first" under Callbacks). An `async fn` (and an `async`
  getter) is posted and awaited (`call_async`), a promise the JS method returns settled.
- **Their TS signatures.** NSH declares a member from its Rust types, an `async fn`'s result as a `Promise`. An
  awaited member also settles with a plain value JS returns, so `#[ts("(): boolean")]` on the trait's method (as
  on a class's member, checked against its parameters and its result) declares what JS returns as written, with no
  `Promise` added: `async fn is_initialized(&self) -> Result<bool, String>` with `#[ts("(): boolean")]` is TS
  `isInitialized(): boolean`, which a task can await. Where Rust wants nothing, any result is let go
  (`#[ts("(context: Context): Promise<void>")]`); a method that is not `async` takes what JS returns at once, so
  a `Promise` result is refused there.
- **Failures.** What JS throws or rejects with is the `Err` of a member returning `Result<T, E>` (`E:
  From<Exception>`: `String` takes the message, `Exception` the stack too), and so is a value that does not convert
  (``Greeter.greet: expected a string, got 3``) or a member that is no function. A member returning a plain value
  panics with it, which the JS call around it throws as an `Error`: a member Rust calls from a task returns a
  `Result`.
- **As a value.** The stand-in is a `JsValue` (TS: the interface). A parameter `JsStartable` takes a JS object whose
  required methods are functions and refuses any other value with a `TypeError` (``Runner.run: `service`: expected a
  Startable, got an object whose `start` is undefined``); a result gives back the same object.
  `JsStartable::new(handle)` (or `From<JsHandle>`) makes one from a kept object, unchecked. It is traced as its
  `JsHandle` is, and equal to another of the same object.
- **Declared.** `Registry::interface::<JsStartable>()` declares the interface and registers the classes its members
  name; a signature naming the stand-in declares it too. An interface, a class, a module and a component of one set
  share no name. JS has no value of it: TS imports it as a type (`import type { Startable } from "app/native"`).
- **Its defaults.** `#[interface]` moves a default body to a hidden method of the trait (`__nsh_default_<name>`),
  which the method's own default calls: a Rust implementor sees no change, and the stand-in runs it for an object
  without the member.

## Class extensions

A crate adds instance methods to a `#[class]` of another crate with `#[class_extension(C)]` on an `impl` block of a
type of its own (the holder), and its UI set registers them. A service crate adds `builder.addI18n(..)` to the app
builder its host crate exports, as a Rust extension trait adds `.add_i18n(..)`:

```rust
/// Methods the i18n crate adds to the app builder.
pub struct I18nBuilder;

#[class_extension(AppBuilder)]
impl I18nBuilder {
    /// Registers the i18n service, configured by `configure`.
    pub fn add_i18n(
        this: Instance<AppBuilder>,
        #[args(options)] configure: Option<Callback<Instance<I18nOptions>>>,
    ) -> Result<Instance<AppBuilder>, String> {
        let options = Instance::new(I18nOptions::default());
        if let Some(configure) = configure {
            configure.call(options.clone())?;
        }
        this.with_mut(|b| b.add_service(I18n::new(options)));
        Ok(this)
    }
}

#[native_module("app-i18n/native")]
fn native(registry: &mut Registry) {
    registry.class_extension::<I18nBuilder>();
}
```

```ts
const builder = new AppBuilder().addStorage().addI18n((o) => { o.locale = "hr"; });
```

- **The holder.** The block's type (a unit struct is enough) carries the methods; it is never a JS value and needs
  no `#[class]`. Its functions stay callable from Rust (`I18nBuilder::add_i18n(builder, None)`).
- **The methods.** The block's functions with a visibility are methods of `C`, those without one helpers. Each takes
  `this: Instance<C>` first: it borrows the value with `this.with(..)` / `this.with_mut(..)` and returns `this` to
  chain (`===` the object it was called on). The rest is a `#[class]` method's: parameters and results, `async fn`
  and `Later<T>` (promises), `#[args(..)]`, `#[ts("…")]`, `#[overload(..)]`, `#[cfg]`, camelCase names and doc
  comments. An extension adds instance methods only: a receiver (the holder is not the class), a function without
  `this` (a static) or named `new`, `#[getter]` / `#[setter]`, `this` of another type, `dispose` and
  `constructor`, `ModuleCx`, generics, a trait `impl` and a doc comment on the block are compile errors.
- **When they reach JS.** `Registry::class_extension::<E>()` (`ClassExtension`, `ClassExtensionDef`) puts the
  methods on `C`'s prototype wherever a host serves the set, native (nsh-js) or the web player: `C`'s own set, another set, or a submodule of one. They
  are there however JS gets the class (an import from any set that exports it, an instance a member returns), as
  the class's own methods are: functions named and sized like them, writable, configurable, not enumerable. A host
  serving no set that registers the extension has no such methods. Registering an extension does not export `C`:
  it adds to the class wherever the class is exported. The classes and named types its methods name are
  registered with it.
- **Refused.** A method named like an instance member of `C` (a method, getter or setter; a static lives on the
  constructor and is no clash) panics where the set registers the extension (``class extension
  `app_i18n::I18nBuilder` adds `addStorage` to class `AppBuilder` (`app::AppBuilder`), which has a member of that
  name: rename one``), and so does a second extension adding a name another extension of `C` in the set adds
  (``class extensions `app_i18n::I18nBuilder` and `app_l10n::L10nBuilder` both add `addI18n` to class `AppBuilder`
  (`app::AppBuilder`) in `app/native`: rename one``). Two sets adding one name to one class panic where a host takes
  its sets (`Registry::class_extensions_in`, which `Registry::merged` and `nsh-js`'s `Host::with_options` call), "by
  two UI sets", as two components of one name are. One extension registered by two sets is one extension. The
  attribute names `C` by its own name, the one JS knows: an alias (`use app::AppBuilder as Builder`) is a compile
  error.
- **Declared.** A set that registers both `C` and the extension lists the methods in `C`'s declaration, after its
  own members and before `dispose()`; the named types they use are the set's. `.nsh/types` adds them to every other
  file that declares `C` (the same Rust class, not another of its name) among the sets an app's hosts serve, as
  `export interface C<its type parameters> { … }`, which TypeScript merges with the class; the types the methods
  name are the extending set's, imported as usual, or written `import("./app-i18n/native").Options` where the name
  would resolve to another type ([tooling.md](tooling.md)). Without a set registering the extension, nothing is
  added. `Registry::undocumented`
  lists an undocumented method as `C.<method>`.

## How calls cross

Nothing crosses between JS and the core as JSON for a Rust component, module or class (`nsh_core::element::call`):

- **Arguments are converted in the JS call** by the generated decoders (`JsValue::from_js`), for every kind of
  call. A synchronous member or function runs on them; the other kinds (a function with `cx`, an `async` function
  or member, a component's ref method) are carried to the core as typed ops and run in order with the UI's ops. A
  value of the wrong type throws a `TypeError` in the call, or rejects the promise of a call that returns one, with
  the same message whatever the kind (``Ints.op: `n`: expected an integer that fits u32, got 1.5``).
- **Results come back in the core's event stream** (`JsEvent::Reply`, `Core::take_events`), in order with the
  nodes' events, and `nsh-js` converts them when it settles the promise (`Host::deliver`). Every path that drops a
  call settles its promise: a panic or an error rejects it, a method of a node hosting no component rejects, and the
  ops the engine drops when the script stops reject theirs (`Core::discard_ops`). Panics in the app's conversion
  code on the way back are caught (rejected, logged or thrown at the assignment).
- **A Rust component's props** are decoded when JS sets them (`PropSpec::decode`, generated by `derive(Props)`). A
  value of the wrong type is a warning, as a host element's bad value is (`Gauge.level: expected an integer that
  fits u32, got 1.5`): the prop keeps its value (its default in the first render) and every other prop is set;
  `undefined` resets the prop to its default.
- **A Rust component's events** reach JS as the event itself (`JsEvent::Component`), and whether its lazy children
  are placed as `JsEvent::Children`.
- **Platform native components** (Java, Swift, the embedder's GPUI components) keep JSON (`Prop::ComponentProp`,
  `MountItem::ComponentProps`, `EventPayload::Native`, `Command::Native`): their values cross into another
  language. The two kinds stay apart.
- **A class a signature names is registered with it** (`ClassRefs`), so its objects have a class in JS,
  `import { Other }` and `instanceof` work, and the declarations declare it, in every build. Registering a class
  again does nothing.

## The pieces

- **`nsh-macros`:** `#[class]` on the struct (a marker; the type's name is the JS name) and on its `impl`. The
  block's functions with a visibility (`pub fn`, `pub(crate) fn`) are exported; those without one are helpers,
  Rust's only, as in `#[module]` and `#[methods]` blocks (a `#[getter]` / `#[setter]` without one is a compile
  error: it can only be TS's). `new`
  is the constructor (a class without one cannot be made from JS, only returned); `&self` / `&mut self` methods,
  `#[getter]` / `#[setter]` (one name: `count` and `set_count` are the property `count`), functions without `self`
  are statics, `async fn` with `this: Instance<Self>` are async methods. A raw identifier is its name in TS
  (`r#type` is `type`). Doc comments become the declarations':
  the struct's documents the class, as a module's struct (`#[module]` on it and on its `impl`, `ModuleDoc`) does
  the module and a component's `#[component] fn` the component; one on a `#[class]` or `#[module]` `impl` block is a
  compile error (TS would not show it). TS gets a doc comment up to its first heading (`nsh-macros` `docs`, outside
  code fences): what follows `# Rust` (or rustdoc's `# Panics`, `# Examples`) is for Rust callers only, the place
  for notes about `&self`, `cx` or how a call crosses. `Registry::undocumented` lists the exports TS sees without a doc comment;
  the command line warns of an app's when it writes `.nsh/types`, and nsh's own sets are tested to have none.
  It implements `element::Class` (`Class::DEF`, a `ClassDef`: name, doc, members, each a `Member` with its
  `MemberCall`), as `#[module]` implements `Module`; on the struct it implements `Trace` (the callbacks the fields
  keep) and `Ret` (a value of the struct is a new instance). `#[interface]` on a trait (`nsh-macros`
  `interface.rs`) writes the stand-in and its `element::Interface` (`Interface::DEF`, an `InterfaceDef`: name, doc,
  members, each an `InterfaceMember`). `#[class_extension(C)]` on an `impl` block (`class_extension.rs`) makes its
  methods with `#[class]`'s member code (`class.rs`' `member_def`, `this` an `Instance<C>`) and implements
  `element::ClassExtension` (`ClassExtension::DEF`, a `ClassExtensionDef`: the class, the holder's type and the
  methods' `Member`s).
- **`nsh-core`:** `element::Class`, `ClassSpec` (for the declarations), `Instance<T>`, `Constructor`, `Callback<A, R>`, `JsHandle` (both with `can_call_now()` and `can_run_now()`; `JsHandle::get` too), `Posts`, `runtime_entered`, `start_task` (`engine_thread.rs`),
  `Registry::class::<C>()`, `ClassExtension` and `Registry::class_extension::<E>()` / `class_extensions()` /
  `class_extensions_in(sets)` (`class_extension.rs`, the clash checks in `component.rs`), `TsAlias` and `Registry::ts_alias` (`ts_alias.rs`), `Interface` and `Registry::interface::<I>()` (`interface.rs`: the stand-in's calls, a
  `JsCall` whose `JsTarget` is the object's method or property); the direct conversions (`JsHost`, `JsIn`, `JsOwned`; `JsValue`'s `from_js` /
  `to_js`); the typed calls (`call.rs`: `RustOp`, `RustCall`, `JsEvent`, `AnyValue`, `ToJs`); the core runs
  them in order with the ops (`Core::run_rust`).
- **`nsh-js`:** `classes.rs` makes each class's constructor and prototype from its `ClassDef` when a bundle imports
  the set or a member first returns one of its values: C closures calling the members through `CallHost` (its
  `JsHost`); calls that become ops convert their values and push the op (`classes::send`), returning the promise
  `components.rs` settles when the core's events bring the result (`components::deliver`); the object is the web
  classes' (`web/mod.rs`), its slot holding the instance, and its mark hook reports the instance's callbacks.
  `ModuleTable` keeps the served sets' class extensions (`Registry::class_extensions_in`), and `make` defines each
  extension's methods on its class's prototype after the class's own, through `call_extension` (a C closure whose
  opaque is the `ClassExtensionDef`, its magic the method's index), which runs the member as `call` runs a
  class's.
- **The web player** (`nsh-platform-web`): `Modules::describe` (op 200) gives each set's class extensions with its
  classes, `web/browser/modules.js` defines their methods on the class's prototype once both are described (an
  extension waits for a class a later gate brings), and a call goes to Rust as op 210, which runs the member as a
  class's own (op 202) is ([web.md](web.md)).
- **Declarations:** `nsh_core::element::declarations` writes `export declare class` for each, with the types the members
  use, next to the set's components and modules, and the methods of the set's extensions of the class
  (`class_declaration`); the declarations test checks that what exists is declared. `.nsh/types`
  (`nsh_js::type_definitions`) adds the methods of the other served sets' extensions to each file that exports the
  class, as an interface TypeScript merges with it (`ClassExtensionDescription`, the set description's
  `classExtensions`).
- **The CLI** needs no change: classes are part of the app's set like its modules.

## Costs

A synchronous call converts its arguments from QuickJS values to the Rust types directly (no `JsonValue`, no op, no
commit, no promise), as the web classes' methods do. `bench_calls` (`docs/dev/performance.md`): a class method with
a number costs what a JS method call does (~34ns), with a `{ x, y }` object ~139ns, a module function without a
context ~28ns; an awaited op, async function or async member ~0.56–0.63µs (1.6–2.15µs when they crossed as
JSON).

## Decisions

1. **Synchronous or a promise is the Rust signature's `async`**: a member or function that is not `async` runs in
   the JS call and returns its value (it computes and keeps state, without the UI's free functions); an `async`
   one returns a promise and runs as a core task, with them. The one exception, for functions only: one taking
   `cx: &mut ModuleCx` stays an op ordered with the UI (the status bar, focus), as now.
2. **`dispose()` and `[Symbol.dispose]`** besides collection.
3. **Callbacks in the first version** (above); events and iterators later.
4. **Functions follow the same rule**, so a plain `fn` returning a value becomes synchronous in TS.
5. **The standard library's classes move to `#[class]`** as the mechanism gains what each needs (the table above);
   the renderer's own classes stay hand-written. Their TS interfaces stay as they are: names, members and
   signatures are generated alike; arguments are checked by type rather than coerced (`moveTo("10", 0)` throws
   instead of reading 10), and errors read as the generated ones do.

## Implementation notes

- **Posted calls** (`Callback::post` / `call_async`) go through a queue the engine shares (`Posts`), run at each
  commit round before the core's events are dispatched.
- **Results by value.** `Self` and another class's value are results through the `Ret` impl `#[class]` writes for
  the struct; `Option<Other>` by value cannot be one (Rust's coherence rules), `Option<Instance<Other>>` can.
- **Subclassing works**: constructors honor `new.target`, so a TS subclass's objects are its own.
- **A class without `new`** is declared with `private constructor()`; JS calling it throws a `TypeError`. Members
  named `dispose` or `constructor`, and statics named `prototype`, `name` or `length`, are compile errors (the
  object has those already).
- **Panics in `async` module functions** reject their promise, as an `async` member's do.
- **Module functions take instances and callbacks** in every kind.
- **A context makes at most 32768 classes** and a class at most 65535 members: past them, making the class throws a
  `RangeError`.
- **The standard library's classes** (the table above) have not moved yet; the machinery is shared, so each moves
  when it needs nothing more.

## Known limits

- **Deep chains of instances.** Dropping an instance that holds another (`Instance<Node>` in a field, as a linked
  list) drops the chain recursively, and the mark hook traces it recursively: a chain of very many instances can
  overflow the stack. Keep long sequences in a `Vec`.
- **A promise that never settles.** `call_async` on a function returning a promise that never settles waits for
  ever; a cycle through such a promise (the promise's reactions holding the callback whose reply holds it) keeps
  both alive until the VM goes.
