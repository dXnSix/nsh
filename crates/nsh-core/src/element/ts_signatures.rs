//! `#[ts("…")]`: the TypeScript signatures of members and module functions
//! written in TypeScript, generic ones too, declared instead of the ones
//! their Rust types give.

use crate::{
    element::{Registry, declarations},
    prelude::*,
};

/// Keeps services by the class that names them.
#[class]
struct Provider;

#[class]
impl Provider {
    /// A provider under `parent`.
    #[ts("(parent?: Provider | null)")]
    pub(crate) fn new(parent: Option<Instance<Provider>>) -> Self {
        let _ = parent;
        Provider
    }

    /// The service `token` names; throws without one.
    #[ts("<T>(token: { readonly prototype: T }): T")]
    pub(crate) fn get_required_service(&self, token: JsHandle) -> JsHandle {
        token
    }

    /// The service `token` names, or `null`.
    #[ts("<T>(token: { readonly prototype: T }): T | null")]
    pub(crate) fn get_service(&self, token: JsHandle) -> Option<JsHandle> {
        Some(token)
    }

    /// Adds the service `factory` makes, once.
    #[ts("<T>(token: { readonly prototype: T }, factory: (provider: Provider) => T): this")]
    pub(crate) fn add_singleton(
        this: Instance<Self>,
        token: Constructor,
        factory: Callback<Instance<Provider>, JsHandle>,
    ) -> Instance<Self> {
        let _ = (token, factory);
        this
    }

    /// A service by its name too; the optional name may be left out.
    #[ts("<T>(token: { readonly prototype: T }, name: string): T")]
    #[ts("<T>(token: { readonly prototype: T }): T")]
    pub(crate) fn named(&self, token: JsHandle, name: Option<String>) -> JsHandle {
        let _ = name;
        token
    }

    /// An `async` member's result is a promise of the one written.
    #[ts("<T>(token: { readonly prototype: T }): T")]
    pub(crate) async fn create(this: Instance<Self>, token: JsHandle) -> JsHandle {
        let _ = this;
        token
    }

    /// A static.
    #[ts("<T extends object>(value: T): Provider")]
    pub(crate) fn of(value: JsHandle) -> Provider {
        let _ = value;
        Provider
    }

    /// The parent.
    #[getter]
    #[ts("(): Provider | null")]
    pub(crate) fn parent(&self) -> Option<Instance<Provider>> {
        None
    }

    #[setter]
    #[ts("(parent: Provider | null)")]
    pub(crate) fn set_parent(&mut self, parent: Option<Instance<Provider>>) {
        let _ = parent;
    }

    /// Returns nothing: `void`.
    #[ts("(token: object)")]
    pub(crate) fn remove(&self, token: JsHandle) {
        let _ = token;
    }

    /// The parameters JS does not pass are not the signature's.
    #[ts("(token: object): boolean")]
    pub(crate) fn has(&self, token: JsHandle, sys: System) -> bool {
        let _ = (token, sys);
        true
    }

    /// Mixed with an overload of Rust types, in their order.
    #[overload(fn(token: String) -> bool)]
    #[ts("(token: object): boolean")]
    pub(crate) fn knows(&self, token: JsHandle) -> bool {
        let _ = token;
        false
    }
}

/// What a lookup found.
#[class(interface)]
struct Found;

#[class]
impl Found {
    /// The value, or `fallback`.
    #[ts("<T>(fallback: T): T")]
    pub(crate) fn value_or(&self, fallback: JsHandle) -> JsHandle {
        fallback
    }

    /// The token it was found by.
    #[getter]
    #[ts("(): object")]
    pub(crate) fn token(&self) -> Option<JsHandle> {
        None
    }
}

/// Services by token.
#[module]
struct Services;

#[module]
impl Services {
    /// The service `token` names.
    #[ts("<T>(token: { readonly prototype: T }): T")]
    pub(crate) fn resolve(token: JsHandle) -> JsHandle {
        token
    }

    /// An `async` function's result is a promise of the one written.
    #[ts("<T>(token: { readonly prototype: T }): T")]
    pub(crate) async fn load(token: JsHandle) -> JsHandle {
        token
    }

    /// The names it has.
    #[getter]
    #[ts("(): readonly string[]")]
    pub(crate) fn names() -> Vec<String> {
        Vec::new()
    }
}

/// Lookups, one by one.
#[module]
struct Lookups;

#[module(flat)]
impl Lookups {
    /// The first of `values`.
    #[ts("<T>(values: readonly T[]): T | undefined")]
    pub(crate) fn first(values: Vec<JsHandle>) -> Option<JsHandle> {
        values.into_iter().next()
    }
}

#[test]
fn members_and_functions_are_declared_with_the_signatures_written() {
    let mut registry = Registry::ui_set("test/ts");
    registry
        .class::<Provider>()
        .class::<Found>()
        .module::<Services>()
        .module::<Lookups>();
    assert_eq!(
        declarations(&registry, &[]),
        r#"/** The first of `values`. */
export declare function first<T>(values: readonly T[]): T | undefined;

/** Services by token. */
export interface ServicesModule {
  /** The service `token` names. */
  resolve<T>(token: { readonly prototype: T }): T;
  /** An `async` function's result is a promise of the one written. */
  load<T>(token: { readonly prototype: T }): Promise<T>;
  /** The names it has. */
  readonly names: readonly string[];
}

/** Services by token. */
export declare const Services: ServicesModule;

/** What a lookup found. */
export interface Found {
  /** The value, or `fallback`. */
  valueOr<T>(fallback: T): T;
  /** The token it was found by. */
  readonly token: object;
}

/** Keeps services by the class that names them. */
export declare class Provider {
  /** A provider under `parent`. */
  constructor(parent?: Provider | null);
  /** The service `token` names; throws without one. */
  getRequiredService<T>(token: { readonly prototype: T }): T;
  /** The service `token` names, or `null`. */
  getService<T>(token: { readonly prototype: T }): T | null;
  /** Adds the service `factory` makes, once. */
  addSingleton<T>(token: { readonly prototype: T }, factory: (provider: Provider) => T): this;
  /** A service by its name too; the optional name may be left out. */
  named<T>(token: { readonly prototype: T }, name: string): T;
  /** A service by its name too; the optional name may be left out. */
  named<T>(token: { readonly prototype: T }): T;
  /** An `async` member's result is a promise of the one written. */
  create<T>(token: { readonly prototype: T }): Promise<T>;
  /** A static. */
  static of<T extends object>(value: T): Provider;
  /** The parent. */
  get parent(): Provider | null;
  set parent(parent: Provider | null);
  /** Returns nothing: `void`. */
  remove(token: object): void;
  /** The parameters JS does not pass are not the signature's. */
  has(token: object): boolean;
  /** Mixed with an overload of Rust types, in their order. */
  knows(token: string): boolean;
  /** Mixed with an overload of Rust types, in their order. */
  knows(token: object): boolean;
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}

"#
    );
}

#[test]
fn the_signatures_name_no_class_the_rust_types_do_not() {
    // The text is TypeScript's: the classes it names are the set's to
    // register. The Rust types' classes are registered with the member.
    let mut registry = Registry::ui_set("test/ts");
    registry.class::<Provider>();
    let names: Vec<&str> = registry.class_specs().iter().map(|c| c.name).collect();
    assert_eq!(names, ["Provider"]);
    assert!(
        registry.undocumented().is_empty(),
        "{:?}",
        registry.undocumented()
    );
}
