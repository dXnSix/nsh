//! Class extensions: instance methods a crate adds to a `#[class]` it does
//! not own (`builder.addI18n(..)` on another crate's `AppBuilder`), written
//! with `#[class_extension(C)]` on an `impl` block of a type of its own and
//! registered with [`super::Registry::class_extension`].

use std::{any::TypeId, fmt};

use super::{
    TsTypes,
    class::{ClassDef, Member, MemberSpec, member_specs},
};

/// Instance methods a crate adds to class `C`, a `#[class]` of another
/// crate (or of another UI set): `#[class_extension(C)]` on an `impl`
/// block implements it.
///
/// ```ignore
/// /// Methods the i18n crate adds to the app builder.
/// pub struct I18nBuilder;
///
/// #[class_extension(AppBuilder)]
/// impl I18nBuilder {
///     /// Registers the i18n service, configured by `configure`.
///     pub fn add_i18n(
///         this: Instance<AppBuilder>,
///         #[args(options)] configure: Option<Callback<Instance<I18nOptions>>>,
///     ) -> Result<Instance<AppBuilder>, String> {
///         // ...
///         Ok(this)
///     }
/// }
///
/// registry.class_extension::<I18nBuilder>();
/// ```
///
/// TS then calls `builder.addI18n((o) => ...)` on every `AppBuilder`, as
/// it calls the class's own methods.
///
/// - **The holder.** The `impl` block is of a type of the extending crate
///   (`I18nBuilder`): a unit struct is enough. It is never a JS value and
///   needs no `#[class]`. Its functions stay callable from Rust
///   (`I18nBuilder::add_i18n(builder, None)`).
/// - **The class** is named by its own name (a path to it, not an alias:
///   messages name it as JS knows it, and the macro refuses an alias).
/// - **The methods.** The block's functions with a visibility (`pub fn`,
///   `pub(crate) fn`) are methods of `C`; those without one are helpers,
///   Rust's only. Each takes `this: Instance<C>` first (borrow the value
///   with `this.with(..)` / `this.with_mut(..)`, return `this` to chain).
///   Parameters and results are a `#[class]` method's: values, instances,
///   `Callback`s (`#[args(..)]` names their arguments), trailing `Option`s
///   optional in TS, `System`, `CallCx` and borrowed `&Other` / `&mut
///   Other` in a synchronous method; `T`, `Result<T, E>` (an `Err`
///   throws), `Later<T>` (a promise of the task). An `async fn` returns a
///   promise and runs as a task of the core. `#[ts("…")]` and
///   `#[overload(..)]` declare a method's TypeScript signature as on a
///   class's member. The JS name is the camelCase of the Rust name; doc
///   comments are the TS docs.
/// - **Instance methods only**: no constructor, statics, getters or
///   setters, and no `&self` (the holder is not the class). `dispose` and
///   `constructor` are every class's own.
/// - **When the methods reach JS.** On the prototype of `C`, once a UI set
///   the host serves registers the extension: `C`'s own set, another set,
///   or a submodule of one. They are there however JS reaches the class:
///   from any set that exports it, or from a member that returns an
///   instance. Registering the extension does not export `C`; the classes
///   and types its methods' signatures name are registered with it.
/// - **Refused** (a panic: a mistake in the code, caught at the app's first
///   run or when the toolchain describes the set): a method named like an
///   instance member of `C` (a method, getter or setter; a static is no
///   clash), and two extensions adding one name to one class, in one set
///   ([`super::Registry::class_extension`]) or in two sets a host serves
///   ([`super::Registry::class_extensions_in`]).
/// - **Declared** with the class: a set that registers `C` and the
///   extension lists the methods in `C`'s declaration; `.nsh/types` adds
///   them, as an `interface` TypeScript merges with the class, to every
///   other declaration of `C` among the sets an app's hosts serve.
pub trait ClassExtension: 'static {
    /// The extension: its class and methods.
    const DEF: &'static ClassExtensionDef;
}

/// What a class extension is: its class and the methods it adds
/// (`#[class_extension(C)]` writes it; [`ClassExtension`]).
pub struct ClassExtensionDef {
    /// The class it adds methods to.
    pub class: &'static ClassDef,
    /// The holder type, telling two extensions apart.
    pub type_id: fn() -> TypeId,
    /// The holder's Rust path, for messages (`app_i18n::I18nBuilder`).
    pub type_name: fn() -> &'static str,
    /// Its methods: instance methods of the class, each taking
    /// `this: Instance<C>`.
    pub members: &'static [Member],
}

impl ClassExtensionDef {
    /// The specs of its methods, adding the named types they use to
    /// `types`.
    pub(crate) fn member_specs(&self, types: &mut TsTypes) -> Vec<MemberSpec> {
        member_specs(self.members, types)
    }

    /// Whether it adds methods to the class `def` is.
    pub(crate) fn extends(&self, def: &ClassDef) -> bool {
        (self.class.type_id)() == (def.type_id)()
    }
}

impl fmt::Debug for ClassExtensionDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassExtensionDef")
            .field("class", &self.class.name)
            .field("type_name", &(self.type_name)())
            .finish_non_exhaustive()
    }
}
