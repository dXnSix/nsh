//! Components: the [`Component`] trait, its [`Props`] and [`Event`]s
//! (derived), the [`Registry`] a surface renders from, and the JS glue and
//! TypeScript declarations generated from them ([`declarations`]).

use std::{
    any::{Any, TypeId},
    fmt,
    sync::Arc,
};

use rustc_hash::FxHashMap;

use super::{
    Children, IntoElement, PropError, Slot, TsType, TsTypes,
    builder::{ComponentElement, ElNode},
    call::{AnyValue, JsResult, MethodCall},
    class::{Class, ClassDef, ClassSpec, MemberKind, MemberSpec, Thrown},
    class_extension::{ClassExtension, ClassExtensionDef},
    js::{JsHost, JsIn, JsOwned},
    module::{FunctionSpec, Module, ModuleSpec},
};

/// A component written in Rust: a function that runs once ([`Self::view`])
/// and makes its view, binding what changes to signals. Its props come from
/// JS (or from the component whose view has it), its events go to JS.
/// `#[component]` on a function makes the type and this impl.
///
/// It is registered under its type's full path ([`std::any::type_name`]:
/// its crate, modules and name) and exported to JS as the type's own name
/// (the last segment), so neither is written by hand
/// ([`Registry::register`]).
///
/// ```ignore
/// #[component]
/// pub fn Button(props: ButtonProps, cx: Scope<ButtonEvent>) -> impl IntoElement {
///     view()
///         .px_4()
///         .py_3()
///         .bg(move || theme().primary) // nsh-base's theme
///         .on_press(move || cx.emit(ButtonEvent::Press))
///         .child(text(props.title))
/// }
/// ```
pub trait Component: 'static {
    /// Its props (`#[derive(Props)]`).
    type Props: Props;

    /// Its events (`#[derive(Event)]`; [`NoEvent`] for none).
    type Event: Event;

    /// The methods of its ref in JS: those of the [`Api`] its view
    /// registers (`Methods::of::<StepperApi>()`; none by default).
    const METHODS: Methods = Methods::NONE;

    /// Its documentation in TypeScript: the `#[component]` function's doc
    /// comment (none by default).
    const DOC: &'static str = "";

    /// What it shows: its elements, with bindings where values change (TS:
    /// a component's body). Runs once, under an owner of its own: what it
    /// makes (signals, effects, timers) goes with the component.
    fn view(props: Self::Props, cx: super::Scope<Self::Event>) -> impl IntoElement;

    /// This component inside another one's view, given all its props.
    /// A `#[component]` has a constructor of its own, which Rust views use
    /// (`button().disabled(..)`); this stays for components implemented
    /// by hand and for code generic over components.
    fn element(props: Self::Props) -> ComponentElement<Self>
    where
        Self: Sized,
    {
        ComponentElement::new(props)
    }
}

/// A component's code panicked: what an error boundary
/// ([`super::catch_error`]) shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentError {
    /// The component that failed (its registered name: its type's).
    pub component: &'static str,
    /// What its code panicked with.
    pub message: String,
}

impl fmt::Display for ComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}", self.component, self.message)
    }
}

/// Runs component code, catching a panic as its message.
pub(crate) fn caught<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(panic_message)
}

/// The message of a panic's payload.
pub(crate) fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(s) => *s,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(s) => (*s).to_owned(),
            Err(_) => "a panic".to_owned(),
        },
    }
}

/// A method JS calls on a component's ref: one of its API's
/// ([`Api`]; `#[methods]` makes them).
pub struct Method {
    /// Its JS name (`setValue`).
    pub name: &'static str,
    /// Documentation.
    pub doc: &'static str,
    /// Its parameters: JS names and TypeScript types.
    pub params: &'static [Param],
    /// The TypeScript type of its result (`None`: it returns nothing, and
    /// JS gets `undefined`: the call runs in order with the props set
    /// before it, with nothing to wait for).
    pub returns: Option<TsType>,
    /// The classes its parameters and result name.
    pub refs: super::ClassRefs,
    /// Its call, the arguments converted in the JS call: what the core
    /// runs with the API, in order with the props set before it. A wrong
    /// argument's error starts with the method's name; the host puts its
    /// component's before it (`Stepper.setValue: ...`).
    pub call: fn(&dyn JsHost) -> Result<MethodCall, Thrown>,
}

/// What a component's ref does in JS: a handle its view makes, holding the
/// signals its methods read and write, registered with
/// [`super::Scope::methods`]. `#[methods]` on its `impl` block implements
/// it.
pub trait Api: Sized + 'static {
    /// Its methods.
    const METHODS: &'static [Method];
}

/// The methods a component's ref has, for its spec and declarations:
/// [`Methods::of`] its [`Api`] (`#[component(methods = StepperApi)]`).
#[derive(Clone, Copy)]
pub struct Methods {
    specs: fn(&mut TsTypes) -> Vec<MethodSpec>,
    refs: super::ClassRefs,
}

impl Methods {
    /// None.
    pub const NONE: Self = Self {
        specs: |_| Vec::new(),
        refs: |_| {},
    };

    /// Those of `A`.
    pub const fn of<A: Api>() -> Self {
        Self {
            specs: specs_of::<A>,
            refs: |out| {
                for m in A::METHODS {
                    (m.refs)(out);
                }
            },
        }
    }
}

fn specs_of<A: Api>(types: &mut TsTypes) -> Vec<MethodSpec> {
    A::METHODS
        .iter()
        .map(|m| MethodSpec {
            name: m.name,
            doc: m.doc,
            params: m
                .params
                .iter()
                .map(|(n, ts, optional)| (*n, ts(types), *optional))
                .collect(),
            returns: m.returns.map(|ts| ts(types)),
            call: m.call,
        })
        .collect()
}

/// A method's parameter: its JS name, its TypeScript type, and whether JS
/// may leave it out (an `Option` after which every parameter is optional).
pub type Param = (&'static str, TsType, bool);

/// One method, for the glue and its declarations.
#[derive(Debug, Clone)]
pub struct MethodSpec {
    /// Its JS name.
    pub name: &'static str,
    /// Documentation.
    pub doc: &'static str,
    /// Its parameters: JS names, TypeScript types, and whether each is
    /// optional.
    pub params: Vec<(&'static str, String, bool)>,
    /// The TypeScript type of its result; `None`: nothing (`void`).
    pub returns: Option<String>,
    /// Its call (the arguments converted in the JS call).
    pub call: fn(&dyn JsHost) -> Result<MethodCall, Thrown>,
}

impl MethodSpec {
    /// Whether JS gets a `Promise` of the result: when the method returns
    /// a value (`Result<(), E>` too, rejected with an `Err`). One
    /// returning `()` gives `undefined` (`void`): the call runs in order
    /// with the UI and nothing answers it, and a wrong argument throws.
    pub fn promises(&self) -> bool {
        self.returns.is_some()
    }
}

/// What a prop is to the glue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    /// A value: decoded in the JS call ([`PropSpec::decode`]), set by the
    /// core.
    Value,
    /// A render function (`Option<Renderer<T>>`): decoded in the JS call
    /// as a value is (a function, or none), its content rendered by JS
    /// when the core asks for it ([`super::JsEvent::Render`]).
    Renderer,
    /// A named slot: JSX in a holder node.
    Slot,
    /// The JSX children.
    Children,
    /// Virtualized rows: in JS `each`, a row renderer and the data props of
    /// `List` (the glue makes the list, the component places it).
    Items,
}

/// What converts a JS value into a prop's Rust type (the prop's
/// [`super::JsValue::from_js`], type-erased): `nsh-js` calls it when JS
/// sets the prop.
pub type PropDecode = fn(JsIn<'_>) -> Result<AnyValue, PropError>;

/// One prop, for the glue and its declarations.
#[derive(Debug, Clone)]
pub struct PropSpec {
    /// Its JS name (`accessibilityLabel`).
    pub name: &'static str,
    /// Its TypeScript type.
    pub ts: String,
    /// It may be left out.
    pub optional: bool,
    /// Documentation.
    pub doc: &'static str,
    /// What it is.
    pub kind: PropKind,
    /// A value prop's value from JS (`None` for slots, children and rows).
    pub decode: Option<PropDecode>,
}

/// One event, for the glue and its declarations.
#[derive(Debug, Clone)]
pub struct EventSpec {
    /// Its name (`press`: the handler is `onPress`).
    pub name: &'static str,
    /// The name of the handler's one argument in TypeScript (`event`;
    /// unused without data).
    pub arg: &'static str,
    /// The TypeScript type of its data (`void` for none), one object
    /// (`docs/dev/api-naming.md` "Payloads").
    pub data: String,
    /// Documentation.
    pub doc: &'static str,
}

/// A component's props. Derive it: `#[derive(Props)]`.
pub trait Props: Clone + PartialEq + 'static {
    /// The JS name of the [`super::Items`] prop, if it has one.
    #[doc(hidden)]
    const ITEMS: Option<&'static str> = None;

    /// Its children are made only while the view places them
    /// (`#[prop(lazy)]`).
    #[doc(hidden)]
    const LAZY_CHILDREN: bool = false;

    /// TS declares the props generic over `T`, the type of the rows a prop
    /// takes (`#[props(generic)]`; a prop's type names it, as
    /// [`super::Rows`] does).
    #[doc(hidden)]
    const GENERIC: bool = false;

    /// The props (their named types declared in `types`).
    fn spec(types: &mut TsTypes) -> Vec<PropSpec>;

    /// Every prop at its default.
    fn initial() -> Self;

    /// Prop `name` (its JS name) from JS, as [`PropSpec::decode`] made it;
    /// `None` restores its default. Returns whether it changed.
    fn set(&mut self, name: &str, value: Option<AnyValue>) -> Result<bool, PropError>;

    /// Slot `name` filled (`Some`) or not; returns whether it changed.
    fn set_slot(&mut self, name: &str, slot: Option<Slot>) -> bool;

    /// Whether the app passed children; returns whether it changed.
    fn set_children(&mut self, children: Children) -> bool;

    /// The props an instance keeps: each [`super::Prop`] field held in a
    /// signal of the current owner, so new values reach only what read it.
    #[doc(hidden)]
    fn held(&self) -> Self {
        self.clone()
    }

    /// An instance's props take `new`'s (a parent's region made the
    /// element again): held props update their readers; returns whether
    /// another field changed.
    #[doc(hidden)]
    fn update_from(&mut self, new: &Self) -> bool {
        if self == new {
            return false;
        }
        *self = new.clone();
        true
    }
}

/// The props of a component that has none (an app's root, say).
impl Props for () {
    fn spec(_: &mut TsTypes) -> Vec<PropSpec> {
        Vec::new()
    }

    fn initial() -> Self {}

    fn set(&mut self, name: &str, _: Option<AnyValue>) -> Result<bool, PropError> {
        Err(PropError(format!("unknown prop `{name}`")))
    }

    fn set_slot(&mut self, _: &str, _: Option<Slot>) -> bool {
        false
    }

    fn set_children(&mut self, _: Children) -> bool {
        false
    }
}

/// A component's events. Derive it: `#[derive(Event)]`. The element of a
/// `#[component]` emitting them gets a listener method per event, named
/// as its TSX handler in snake case (`onPress` is `.on_press(..)`): a
/// unit variant's takes `move || ..`, a one-field variant's the value
/// (`move |index: &u32| ..`), a named-field variant's the event itself
/// ([`ComponentBuilder`](super::ComponentBuilder)).
pub trait Event: 'static {
    /// The events (their named types declared in `types`).
    fn spec(types: &mut TsTypes) -> Vec<EventSpec>;

    /// This event's name.
    fn name(&self) -> &'static str;

    /// Its data as a JS value of `host` (converted when JS hears it).
    fn data_js<'a>(&self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError>;
}

/// The events of a component that has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEvent {}

impl Event for NoEvent {
    fn spec(_types: &mut TsTypes) -> Vec<EventSpec> {
        Vec::new()
    }

    fn name(&self) -> &'static str {
        match *self {}
    }

    fn data_js<'a>(&self, _host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        match *self {}
    }
}

/// What a component is to JS: its glue and declarations are generated
/// from it ([`declarations`]).
#[derive(Debug, Clone)]
pub struct ComponentSpec {
    /// Its registered name: its type's full path ([`std::any::type_name`]),
    /// unique within one binary.
    pub name: &'static str,
    /// Its JS export: the type's own name, the last segment of its path.
    pub export: String,
    /// Documentation.
    pub doc: &'static str,
    /// Its props, slots and children.
    pub props: Vec<PropSpec>,
    /// Its events.
    pub events: Vec<EventSpec>,
    /// The methods of its ref.
    pub methods: Vec<MethodSpec>,
    /// The named TypeScript types they use.
    pub types: TsTypes,
    /// JS makes the children only while the view places them
    /// (`#[prop(lazy)]` on its `Children`): the core tells it with a
    /// `$children` event on the host.
    pub lazy_children: bool,
    /// Its props are generic over `T`, the type of the rows one takes
    /// (`#[props(generic)]`), as a component with [`super::Items`] is.
    pub generic: bool,
}

/// The props JS sets on a component's host as on a view (TSX's
/// `ComponentHostProps` but `ref`): the app's classes and styles, its test
/// id, its tooltip, layout, file drop, mouse and context menu events.
pub const HOST_PROPS: &[&str] = &[
    "class",
    "classList",
    "style",
    "activeStyle",
    "hoverStyle",
    "focusStyle",
    "focusWithinStyle",
    "focusVisibleStyle",
    "disabledStyle",
    "testID",
    "tooltip",
    "onLayout",
    "onFileDragEnter",
    "onFileDragLeave",
    "onFileDrop",
    "onMouseEnter",
    "onMouseMove",
    "onMouseLeave",
    "onMouseUp",
    "onContextMenu",
];

/// What makes a component's props', events' and methods' specs.
type Specs = (
    fn(&mut TsTypes) -> Vec<PropSpec>,
    fn(&mut TsTypes) -> Vec<EventSpec>,
    fn(&mut TsTypes) -> Vec<MethodSpec>,
);

impl ComponentSpec {
    /// The [`HOST_PROPS`] JS sets on its host: those it does not declare
    /// itself. Its own prop or event of a host prop's name is the one apps
    /// get (`DataTable`'s `onContextMenu` reports a row and column).
    pub fn host_props(&self) -> impl Iterator<Item = &'static str> + '_ {
        HOST_PROPS.iter().copied().filter(|&name| {
            !self.props.iter().any(|p| p.name == name)
                && !self.events.iter().any(|e| handler_name(e.name) == name)
        })
    }

    pub(crate) fn of<C: Component>() -> Self {
        Self::build(
            name_of::<C>(),
            C::DOC,
            (C::Props::spec, C::Event::spec, C::METHODS.specs),
            C::Props::LAZY_CHILDREN,
            C::Props::GENERIC,
        )
    }

    /// [`Self::of`] past what depends on the component's type.
    #[inline(never)]
    fn build(
        name: &'static str,
        doc: &'static str,
        (props, events, methods): Specs,
        lazy_children: bool,
        generic: bool,
    ) -> Self {
        let mut types = TsTypes::default();
        let props = props(&mut types);
        let events = events(&mut types);
        let methods = methods(&mut types);
        Self {
            name,
            export: export_name(name).to_owned(),
            doc,
            props,
            events,
            methods,
            types,
            lazy_children,
            generic,
        }
    }

    /// Its named slots.
    pub fn slots(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.props
            .iter()
            .filter(|p| p.kind == PropKind::Slot)
            .map(|p| p.name)
    }

    /// Its render functions ([`PropKind::Renderer`]).
    pub fn renderers(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.props
            .iter()
            .filter(|p| p.kind == PropKind::Renderer)
            .map(|p| p.name)
    }

    /// Whether it takes JSX children.
    pub fn takes_children(&self) -> bool {
        self.props.iter().any(|p| p.kind == PropKind::Children)
    }

    /// Its [`PropKind::Items`] prop (one at most).
    pub fn items(&self) -> Option<&'static str> {
        self.props
            .iter()
            .find(|p| p.kind == PropKind::Items)
            .map(|p| p.name)
    }

    /// The nodes the glue makes for it, by name: slots and the list.
    pub(crate) fn holder(&self, name: &str) -> Option<&'static str> {
        self.props
            .iter()
            .find(|p| matches!(p.kind, PropKind::Slot | PropKind::Items) && p.name == name)
            .map(|p| p.name)
    }
}

/// The registered name of component `C`: its type's. It is only compared
/// within one binary, so the format of [`std::any::type_name`] may change
/// between compilers; the exports, which JS sees, are pinned by tests.
pub(crate) fn name_of<C: Component>() -> &'static str {
    std::any::type_name::<C>()
}

/// A type's own name, its JS export: the last segment of its path, without
/// a generic type's arguments (the test `components_are_named_after_their_types`).
pub(crate) fn export_name(type_name: &str) -> &str {
    let path = type_name.split('<').next().unwrap_or(type_name);
    path.rsplit("::").next().unwrap_or(path)
}

/// An instance: a component's props and view, their types erased.
pub(crate) trait AnyInstance {
    fn set_prop(&mut self, name: &str, value: Option<AnyValue>) -> Result<bool, PropError>;
    /// Props from the component whose view has this one.
    fn set_props(&mut self, props: &dyn Any);
    fn set_slot(&mut self, name: &'static str, filled: bool);
    fn set_children(&mut self, present: bool);
    /// The component goes away: what its view made goes with it.
    fn cleanup(&mut self);
    /// Its view, made once.
    fn view(&mut self) -> ElNode;
    /// Its registered name.
    fn name(&self) -> &'static str;
    /// A method of its ref (its arguments decoded); `None` when it has no
    /// ref of the method's type.
    fn call(&mut self, call: MethodCall) -> Option<JsResult>;
    /// Its view's owner, once its view is made.
    fn owner(&self) -> Option<super::Owner>;
    /// Where a failure of its code goes first: its owner, or where it is
    /// made before it has one.
    fn failure_owner(&self) -> super::Owner;
    /// Made by JS (where no owner is): its view will be made under
    /// `parent`, the owner of the nearest component above it in the tree,
    /// so it sees that one's context.
    fn adopt(&mut self, parent: super::Owner);
    /// It is the component of `host`, made where `parent` owns.
    fn place(&mut self, host: crate::NodeId, parent: super::Owner);
}

/// The root of a Rust app: makes the app's top component (on the engine
/// thread, where elements live). The engine mounts it as the content of the
/// surface (`nsh_engine::App::Rust`).
#[derive(Clone)]
pub struct Root {
    make: Arc<dyn Fn() -> super::AnyElement + Send + Sync>,
    theme: Option<Arc<str>>,
    theme_overrides: Option<Arc<str>>,
    fonts: Vec<Arc<[u8]>>,
    appearance: Option<crate::platform::Appearance>,
    app_id: Option<String>,
    links: crate::url_scheme::AppLinks,
    capabilities: Option<crate::access::Capabilities>,
    status_bar: Option<crate::StatusBarSetting>,
    main_window: Option<crate::desktop::MainWindowOptions>,
    application: Option<crate::desktop::ApplicationOptions>,
    translations: Option<crate::i18n::Translations>,
    app_info: Option<crate::platform::AppInfo>,
}

impl Root {
    /// Component `C` with the props `props` makes.
    pub fn new<C: Component>(props: impl Fn() -> C::Props + Send + Sync + 'static) -> Self {
        Self {
            make: Arc::new(move || super::IntoElement::into_element(C::element(props()))),
            theme: None,
            theme_overrides: None,
            fonts: Vec::new(),
            appearance: None,
            app_id: None,
            links: crate::url_scheme::AppLinks::default(),
            capabilities: None,
            status_bar: None,
            main_window: None,
            application: None,
            translations: None,
            app_info: None,
        }
    }

    /// The app's translations (its `locales/` files, which nsh's generated
    /// code compiles in): what [`t!`](crate::t) reads.
    pub fn translations(mut self, translations: crate::i18n::Translations) -> Self {
        self.translations = Some(translations);
        self
    }

    /// [`Root::translations`] from the JSON the build writes of the app's
    /// `locales/` files. A wrong one is logged and left out.
    #[doc(hidden)]
    pub fn translations_json(self, json: &str) -> Self {
        match crate::i18n::Translations::from_json(json) {
            Ok(translations) => self.translations(translations),
            Err(e) => {
                log::warn!("locales: {e}");
                self
            }
        }
    }

    /// The app's translations ([`Root::translations`]).
    pub fn translations_ref(&self) -> Option<&crate::i18n::Translations> {
        self.translations.as_ref()
    }

    /// The app's id (`nsh.json` `"id"`, which nsh's generated code sets):
    /// it names where the app keeps its data (`nsh/storage`).
    pub fn app_id(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }

    /// The app's id ([`Root::app_id`]).
    pub fn app_id_ref(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    /// The app as it was built ([`crate::platform::AppInfo`]), which
    /// nsh's generated code sets: what [`crate::element::app_info`]
    /// answers.
    pub fn app_info(mut self, info: crate::platform::AppInfo) -> Self {
        self.app_info = Some(info);
        self
    }

    /// [`Root::app_info`] from the JSON `appInfo()` reports, which nsh's
    /// generated code compiles in. JSON it cannot read is an app with
    /// nothing known (logged).
    #[doc(hidden)]
    pub fn app_info_json(self, json: &str) -> Self {
        let info = match serde_json::from_str(json) {
            Ok(value) => crate::platform::AppInfo::from_json(&value),
            Err(e) => {
                log::warn!("appInfo: {e}, ignored");
                crate::platform::AppInfo::default()
            }
        };
        self.app_info(info)
    }

    /// The app as it was built ([`Root::app_info`]).
    #[doc(hidden)]
    pub fn app_info_ref(&self) -> Option<&crate::platform::AppInfo> {
        self.app_info.as_ref()
    }

    /// The URL schemes the app claims (`nsh.json` `schemes`): the links the
    /// core lets through ([`crate::url_scheme`],
    /// [`crate::element::next_url_scheme_open`]). Replaces the schemes of
    /// [`Root::links`], keeping its web links.
    pub fn url_schemes(mut self, schemes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.links.set_schemes(schemes);
        self
    }

    /// The schemes the app claims ([`Root::url_schemes`]).
    pub fn url_schemes_ref(&self) -> &[String] {
        self.links.schemes()
    }

    /// The links the app claims (`nsh.json` `schemes` and `links`, which
    /// nsh's generated code sets): URLs of its schemes and `https` URLs of
    /// its web links, the links the core lets through ([`crate::url_scheme`],
    /// [`crate::element::next_url_scheme_open`]).
    pub fn links(mut self, links: crate::url_scheme::AppLinks) -> Self {
        self.links = links;
        self
    }

    /// [`Root::links`] from `nsh.json`'s `schemes` and `links` as JSON text
    /// (`{"schemes": [...], "links": [...]}`), which nsh's generated code
    /// compiles in (checked when it was generated). Read as a package's
    /// `app.json` is: what this runtime cannot claim is left out (logged).
    #[doc(hidden)]
    pub fn links_json(self, json: &str) -> Self {
        let links = match serde_json::from_str::<serde_json::Value>(json) {
            Ok(config) => {
                let (links, ignored) = crate::url_scheme::AppLinks::from_json_lenient(&config);
                for what in ignored {
                    log::warn!("nsh.json: {what}, ignored");
                }
                links
            }
            Err(e) => {
                log::warn!("nsh.json: links: {e}; none claimed");
                crate::url_scheme::AppLinks::default()
            }
        };
        self.links(links)
    }

    /// The links the app claims ([`Root::links`]).
    pub fn links_ref(&self) -> &crate::url_scheme::AppLinks {
        &self.links
    }

    /// What the app declares it may reach (`nsh.json` `capabilities`, which
    /// nsh's generated code sets): the engine checks the app's requests,
    /// URLs it opens, media, web views, clipboard, picker and the rest
    /// against them, as it does a TSX app's. Without a declaration the app
    /// reaches what an app without `capabilities` does.
    pub fn capabilities(mut self, capabilities: crate::access::Capabilities) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// [`Root::capabilities`] from `nsh.json`'s `capabilities` as JSON
    /// text, which nsh's generated code compiles in (checked when it was
    /// generated). Read as a package's are: what this runtime does not know
    /// is not granted (logged), and a declaration it cannot read grants
    /// nothing.
    #[doc(hidden)]
    pub fn capabilities_json(self, json: &str) -> Self {
        let read = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| e.to_string())
            .and_then(|v| crate::access::Capabilities::from_json_lenient(&v));
        let capabilities = match read {
            Ok((capabilities, dropped)) => {
                for what in dropped {
                    log::warn!("nsh.json: {what}, not granted");
                }
                capabilities
            }
            Err(e) => {
                log::warn!("nsh.json: capabilities: {e}; nothing granted");
                crate::access::Capabilities::default()
            }
        };
        self.capabilities(capabilities)
    }

    /// What the app declares it may reach ([`Root::capabilities`]); `None`:
    /// no declaration.
    pub fn capabilities_ref(&self) -> Option<&crate::access::Capabilities> {
        self.capabilities.as_ref()
    }

    /// The app's built-in theme document `json` (`nsh.json`'s `"theme"`):
    /// the active document from the first frame on, over the UI sets'
    /// ([`super::theme`](super::theme)). [`super::set_theme`] replaces it
    /// while the app runs, and `set_theme(None)` comes back to it. A text
    /// that is no document, or one of the old format (`legacy-format`), is logged
    /// and not activated.
    pub fn theme(mut self, json: impl Into<Arc<str>>) -> Self {
        self.theme = Some(json.into());
        self
    }

    /// The app's overrides `json` (`nsh.json`'s `"themeOverrides"`), a
    /// theme patch: the first entries of the app's one patch, which
    /// [`super::update_tokens`] merges into.
    pub fn theme_overrides(mut self, json: impl Into<Arc<str>>) -> Self {
        self.theme_overrides = Some(json.into());
        self
    }

    /// Registers a font file (TrueType / OpenType) before the first render;
    /// styles name it by its family name.
    pub fn font(mut self, data: impl Into<Arc<[u8]>>) -> Self {
        self.fonts.push(data.into());
        self
    }

    /// Shows the app in `appearance` whatever the system's (`nsh.json`
    /// `"appearance"`; later [`super::set_color_scheme`] calls
    /// change it).
    pub fn appearance(mut self, appearance: crate::platform::Appearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    /// The status bar's default (`nsh.json` `"statusBar"`), under what
    /// pages and dialogs set (`Screen::status_bar`, `Modal::status_bar`).
    pub fn status_bar(mut self, setting: crate::StatusBarSetting) -> Self {
        self.status_bar = Some(setting);
        self
    }

    /// The main window's launch options (`nsh.json` `"window"`, which nsh's
    /// generated code sets): the runners open the window by them, and the
    /// engine applies their attributes with the first frame and keeps the
    /// placement with `restore_bounds`, as for a TSX app's bundle.
    pub fn main_window(mut self, options: crate::desktop::MainWindowOptions) -> Self {
        self.main_window = Some(options);
        self
    }

    /// [`Root::main_window`] from `nsh.json`'s `"window"` object as JSON
    /// text, which nsh's generated code compiles in (checked when it was
    /// generated). A wrong one is logged and left out.
    #[doc(hidden)]
    pub fn main_window_json(self, json: &str) -> Self {
        let options = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| e.to_string())
            .and_then(|v| {
                crate::desktop::MainWindowOptions::from_json(&v).map_err(|e| e.to_string())
            });
        match options {
            Ok(options) => self.main_window(options),
            Err(e) => {
                log::warn!("nsh.json: window: {e}");
                self
            }
        }
    }

    /// The main window's launch options ([`Root::main_window`]).
    pub fn main_window_ref(&self) -> Option<&crate::desktop::MainWindowOptions> {
        self.main_window.as_ref()
    }

    /// The app's lifecycle options (`nsh.json` `"application"`, which nsh's
    /// generated code sets): the engine sets the quit mode before the app
    /// runs, as for a TSX app's bundle.
    pub fn application(mut self, options: crate::desktop::ApplicationOptions) -> Self {
        self.application = Some(options);
        self
    }

    /// [`Root::application`] from `nsh.json`'s `"application"` object as
    /// JSON text, which nsh's generated code compiles in (checked when it
    /// was generated). A wrong one is logged and left out.
    #[doc(hidden)]
    pub fn application_json(self, json: &str) -> Self {
        let options = serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| e.to_string())
            .and_then(|v| {
                crate::desktop::ApplicationOptions::from_json(&v).map_err(|e| e.to_string())
            });
        match options {
            Ok(options) => self.application(options),
            Err(e) => {
                log::warn!("nsh.json: application: {e}");
                self
            }
        }
    }

    /// The app's lifecycle options ([`Root::application`]).
    pub fn application_ref(&self) -> Option<&crate::desktop::ApplicationOptions> {
        self.application.as_ref()
    }

    /// The status bar's default ([`Root::status_bar`]).
    pub fn status_bar_ref(&self) -> Option<crate::StatusBarSetting> {
        self.status_bar
    }

    /// The appearance it starts in ([`Root::appearance`]; `None`: the
    /// system's).
    pub fn appearance_ref(&self) -> Option<crate::platform::Appearance> {
        self.appearance
    }

    /// The app's theme document ([`Root::theme`]).
    pub fn theme_ref(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    /// The app's overrides ([`Root::theme_overrides`]).
    pub fn theme_overrides_ref(&self) -> Option<&str> {
        self.theme_overrides.as_deref()
    }

    /// The font files to register.
    pub fn fonts(&self) -> &[Arc<[u8]>] {
        &self.fonts
    }

    /// Component `C` with default props.
    pub fn of<C: Component>() -> Self
    where
        C::Props: Default,
    {
        Self::new::<C>(C::Props::default)
    }

    /// The element of the root component.
    pub(crate) fn element(&self) -> super::AnyElement {
        (self.make)()
    }
}

impl fmt::Debug for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Root")
    }
}

/// The name of [`missing_instance`], which `nsh-cli` looks for in a web
/// runtime's symbol table to know the app makes a component by name (it
/// then keeps every `nsh_part_*` gate). A test below fails when the two
/// stop matching.
pub const BY_NAME_MARKER: &str = "missing_instance";

/// What a component element made by name ([`super::ComponentElement::by_name`])
/// has for its `make`: never called, the registry makes it. Never inlined, so
/// every by-name element's code references this one function.
#[inline(never)]
pub(crate) fn missing_instance() -> Box<dyn AnyInstance> {
    unreachable!("a component made by name is made by its registry")
}

/// What a component element made by name has for its `spec`: never called.
pub(crate) fn missing_spec() -> ComponentSpec {
    unreachable!("a component made by name is made by its registry")
}

/// A new instance of `C`, its props not set yet.
pub(crate) fn make_instance<C: Component>() -> Box<dyn AnyInstance> {
    Box::new(super::once::OnceInstance::<C>::new())
}

#[derive(Clone)]
struct ComponentType {
    spec: Arc<ComponentSpec>,
    make: fn() -> Box<dyn AnyInstance>,
    /// Tells two types of the same name apart (two versions of a crate).
    id: TypeId,
}

#[derive(Clone)]
struct ModuleType {
    spec: Arc<ModuleSpec>,
    /// The type that implements it.
    type_name: &'static str,
}

#[derive(Clone)]
struct ClassType {
    spec: Arc<ClassSpec>,
    /// The type that implements it.
    type_name: &'static str,
}

/// The bar of one scroll view or list that the core asks a UI set to make
/// ([`Registry::scrollbars_from`]).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct BuiltInScrollbar {
    /// The scroll view or list it scrolls.
    pub target: crate::NodeId,
    /// Whether that view scrolls sideways.
    pub horizontal: bool,
    /// The view's own theme (its `scrollbar` object, as JSON); `None`
    /// with no [`Self::theme_value`]: the app's.
    pub theme: Option<Arc<crate::JsonValue>>,
    /// The view's own theme as Rust gave it
    /// ([`ScrollbarSetting::ThemeValue`](crate::ScrollbarSetting::ThemeValue)),
    /// a value of the set's theme type.
    pub theme_value: Option<super::AnyValue>,
    /// The app's theme, as the set gave it
    /// ([`set_builtin_scrollbars`](super::set_builtin_scrollbars)).
    pub app_theme: Option<super::AnyValue>,
}

/// Makes the built-in bar of a scroll view or list: a component element,
/// which the core lays out over the view's viewport and keeps there as
/// it scrolls; `None` leaves the view the platform's indicator
/// ([`Registry::scrollbars_from`]).
pub type ScrollbarSource = fn(&BuiltInScrollbar) -> Option<super::AnyElement>;

/// A theme of one view's own given in Rust
/// ([`ScrollbarSetting::ThemeValue`](crate::ScrollbarSetting::ThemeValue)):
/// a value of the theme type of the UI set that makes bars, with the set's
/// maker, so a Rust app that never registered the set gets the bar too (as
/// with [`set_builtin_scrollbars`](super::set_builtin_scrollbars)). Two are
/// equal when they hold the same value.
#[derive(Clone)]
pub struct ScrollbarThemeValue {
    source: ScrollbarSource,
    theme: super::AnyValue,
}

impl ScrollbarThemeValue {
    /// `theme`, drawn by `source` (which reads it as
    /// [`BuiltInScrollbar::theme_value`]).
    pub fn new(source: ScrollbarSource, theme: super::AnyValue) -> Self {
        Self { source, theme }
    }

    /// What makes the bar.
    pub fn source(&self) -> ScrollbarSource {
        self.source
    }

    /// The theme.
    pub fn theme(&self) -> &super::AnyValue {
        &self.theme
    }
}

impl PartialEq for ScrollbarThemeValue {
    fn eq(&self, other: &Self) -> bool {
        self.theme == other.theme
    }
}

impl std::fmt::Debug for ScrollbarThemeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ScrollbarThemeValue")
            .field(&self.theme)
            .finish()
    }
}

/// What a UI set exports to JS: its components and modules, and what it
/// passes on from the runtime. Cheap to clone.
#[derive(Clone, Default)]
pub struct Registry {
    types: Arc<FxHashMap<&'static str, ComponentType>>,
    modules: Arc<FxHashMap<&'static str, ModuleType>>,
    /// By the specifier of their set and their name: the sets merged for
    /// the core may have classes of the same name.
    classes: Arc<FxHashMap<(Option<&'static str>, &'static str), ClassType>>,
    /// The interfaces TS objects implement for Rust, by name
    /// ([`Registry::interface`]): declarations only, JS has no value of
    /// them.
    interfaces: Arc<FxHashMap<&'static str, &'static super::InterfaceDef>>,
    /// The methods it adds to classes of other crates or sets
    /// ([`Registry::class_extension`]), in the order registered.
    extensions: Arc<Vec<&'static ClassExtensionDef>>,
    /// The TypeScript type aliases its declarations export, by name
    /// ([`Registry::ts_alias`]).
    aliases: Arc<FxHashMap<&'static str, super::TsAlias>>,
    /// Values and types of `nsh/_core` exported as they are.
    reexports: Arc<(Vec<&'static str>, Vec<&'static str>)>,
    /// Values `nsh-js` makes for the set (declared by hand, not generated
    /// from a spec).
    native_values: Arc<Vec<&'static str>>,
    /// The runtime module the functions belong to (`nsh/android`): JS
    /// calls them as `nsh/android:BackHandler.exitApp`, apart from any UI set's.
    namespace: Option<&'static str>,
    /// The JS module a UI set is imported as (`app/native`).
    ui_set: Option<&'static str>,
    /// The files its crates ship, with the targets they are for
    /// ([`Registry::assets_for`]; none: every target).
    assets: Arc<Vec<(&'static [crate::platform::Os], &'static [super::AssetFile])>>,
    /// The theme documents its sets register ([`Registry::theme_document`]),
    /// in set order; the ones refused whole are not kept.
    theme_documents: Arc<Vec<RegisteredTheme>>,
    /// The fallback documents ([`Registry::fallback_theme_document`]).
    fallback_documents: Arc<Vec<RegisteredTheme>>,
    /// The roles its components draw with ([`Registry::roles`]).
    roles: Arc<Vec<super::RoleSpec>>,
    /// The set that listed each of [`Self::roles`] (`None`: the app's
    /// components), for `role-conflict`'s message.
    role_sets: Arc<Vec<Option<&'static str>>>,
    /// Makes the views' built-in scroll bars ([`Registry::scrollbars_from`]).
    scrollbars: Option<ScrollbarSource>,
    /// The UI sets served under paths of this one's specifier
    /// ([`Registry::submodule`]).
    submodules: Arc<Vec<Registry>>,
    /// The UI sets served with this one under specifiers of their own
    /// ([`Registry::serve`]).
    served: Arc<Vec<Registry>>,
    /// What each engine serving the set runs once ([`Registry::setup`]).
    setups: Arc<Vec<Setup>>,
    /// The stand-ins its modules give headless runs, by name
    /// ([`Registry::fake`]).
    fakes: Arc<Vec<(&'static str, Arc<dyn crate::fake::Fake>)>>,
}

/// A function each engine serving a set runs once ([`Registry::setup`]).
type Setup = Arc<dyn Fn() + Send + Sync>;

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.types.keys()).finish()
    }
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers component `C` under its type's name, exported to JS as
    /// the type's own name ([`ComponentSpec::name`], [`ComponentSpec::export`]).
    ///
    /// # Panics
    ///
    /// When `C` is registered already, another type of the same name is
    /// (two versions of one crate), or another component of the set has the
    /// same export: a mistake in the code, caught by the app's first run.
    #[track_caller]
    pub fn register<C: Component>(&mut self) -> &mut Self {
        let spec = ComponentSpec::of::<C>();
        let mut refs = Vec::new();
        (C::METHODS.refs)(&mut refs);
        self.register_erased(spec, make_instance::<C>, TypeId::of::<C>(), refs)
    }

    /// [`Self::register`] past what depends on the component's type: once,
    /// not once per component.
    #[track_caller]
    #[inline(never)]
    fn register_erased(
        &mut self,
        spec: ComponentSpec,
        make: fn() -> Box<dyn AnyInstance>,
        id: TypeId,
        refs: Vec<&'static ClassDef>,
    ) -> &mut Self {
        let types = Arc::make_mut(&mut self.types);
        if let Some(old) = types.get(spec.name) {
            let set = self.ui_set.or(self.namespace);
            panic!("{}", same_name(spec.name, old.id, id, set));
        }
        if self.classes.keys().any(|(_, name)| *name == spec.export)
            || self.interfaces.contains_key(spec.export.as_str())
            || self.aliases.contains_key(spec.export.as_str())
        {
            panic!(
                "component `{}` is exported as `{}` like a class, an interface or a type alias{}: rename one",
                spec.name,
                spec.export,
                of_set(self.ui_set.or(self.namespace)),
            );
        }
        if let Some(other) = types.values().find(|t| t.spec.export == spec.export) {
            panic!(
                "components `{}` and `{}` are both exported as `{}`{}: rename one",
                other.spec.name,
                spec.name,
                spec.export,
                of_set(self.ui_set.or(self.namespace)),
            );
        }
        types.insert(
            spec.name,
            ComponentType {
                spec: Arc::new(spec),
                make,
                id,
            },
        );
        self.classes_named(refs)
    }

    /// Registers module `M`: its functions on a JS object named
    /// [`Module::NAME`] (merged into the component exported by that name, if
    /// the set has one).
    ///
    /// # Panics
    ///
    /// When the set has a module of that name already.
    #[track_caller]
    pub fn module<M: Module>(&mut self) -> &mut Self {
        let type_name = std::any::type_name::<M>();
        if self.classes.keys().any(|(_, name)| *name == M::NAME)
            || self.interfaces.contains_key(M::NAME)
            || self.aliases.contains_key(M::NAME)
        {
            panic!(
                "module `{type_name}` is named `{}` like a class, an interface or a type alias{}: rename one",
                M::NAME,
                of_set(self.ui_set.or(self.namespace))
            );
        }
        let modules = Arc::make_mut(&mut self.modules);
        if let Some(old) = modules.get(M::NAME) {
            panic!(
                "{}",
                same_module(
                    M::NAME,
                    old.type_name,
                    type_name,
                    self.ui_set.or(self.namespace)
                )
            );
        }
        modules.insert(
            M::NAME,
            ModuleType {
                spec: Arc::new(ModuleSpec::of::<M>()),
                type_name,
            },
        );
        let mut refs = Vec::new();
        for f in M::FUNCTIONS {
            (f.refs)(&mut refs);
        }
        self.classes_named(refs)
    }

    /// Registers class `C` (`#[class]`): a JS class named [`Class::DEF`]'s
    /// name, which JS makes with `new` (when it has a constructor) and gets
    /// from the members that return one. The classes its members'
    /// signatures name are registered with it, as are those a module's
    /// functions or a component's methods name ([`Registry::module`],
    /// [`Registry::register`]): their objects reach JS, so they are
    /// exported and declared. Registering a class again does nothing.
    /// Other crates' sets may add methods to it
    /// ([`Registry::class_extension`]).
    ///
    /// # Panics
    ///
    /// When the set has another class, a module or a component of that
    /// name already.
    #[track_caller]
    pub fn class<C: Class>(&mut self) -> &mut Self {
        self.class_def(C::DEF)
    }

    /// Registers the classes in `refs` (a signature names them), unless
    /// the set has them.
    #[track_caller]
    fn classes_named(&mut self, refs: Vec<&'static ClassDef>) -> &mut Self {
        for def in refs {
            self.class_def(def);
        }
        self
    }

    #[track_caller]
    fn class_def(&mut self, def: &'static ClassDef) -> &mut Self {
        let type_name = (def.type_name)();
        let set = self.ui_set.or(self.namespace);
        if let Some(old) = self.classes.get(&(set, def.name)) {
            if (old.spec.def.type_id)() == (def.type_id)() {
                return self;
            }
            panic!("{}", same_class(def.name, old.type_name, type_name, set));
        }
        if self.modules.contains_key(def.name)
            || self.types.values().any(|t| t.spec.export == def.name)
            || self.interfaces.contains_key(def.name)
            || self.aliases.contains_key(def.name)
        {
            panic!(
                "class `{type_name}` is named `{}` like a module, a component, an interface or a type alias{}: rename one",
                def.name,
                of_set(set)
            );
        }
        Arc::make_mut(&mut self.classes).insert(
            (set, def.name),
            ClassType {
                spec: Arc::new(ClassSpec::of(def)),
                type_name,
            },
        );
        // What its members name, in turn (it is registered: a cycle ends).
        let mut refs = Vec::new();
        for m in def.members {
            (m.refs)(&mut refs);
        }
        self.classes_named(refs)
    }

    /// Registers class extension `E` (`#[class_extension(C)]`: instance
    /// methods this crate adds to class `C` of another crate or set; the
    /// rules are [`ClassExtension`]'s). Wherever the host serving this set
    /// makes `C`'s objects, they have `E`'s methods, from whichever set JS
    /// imports `C`. `C` itself is not registered, so the set does not
    /// export it (it adds to the class wherever the class is exported);
    /// the other classes and named types its methods' signatures name are
    /// registered, as a class's members' are. The set's declarations list
    /// the methods in `C`'s when the set registers `C` too. Registering it
    /// again does nothing.
    ///
    /// # Panics
    ///
    /// When one of its methods has the JS name of an instance member of
    /// `C` (a method, getter or setter; a static is no clash), or of a
    /// method another extension of `C` in this set adds. Two sets adding
    /// one name to one class are refused where a host takes its sets
    /// ([`Registry::class_extensions_in`]).
    #[track_caller]
    pub fn class_extension<E: ClassExtension>(&mut self) -> &mut Self {
        self.class_extension_def(E::DEF)
    }

    /// [`Self::class_extension`] past what depends on the extension's type.
    #[track_caller]
    #[inline(never)]
    fn class_extension_def(&mut self, def: &'static ClassExtensionDef) -> &mut Self {
        let id = (def.type_id)();
        if self.extensions.iter().any(|e| (e.type_id)() == id) {
            return self;
        }
        let class = def.class;
        let set = self.ui_set.or(self.namespace);
        for method in def.members {
            let own = class.members.iter().any(|m| {
                matches!(
                    m.kind,
                    MemberKind::Method | MemberKind::Getter | MemberKind::Setter
                ) && m.name == method.name
            });
            if own {
                panic!(
                    "class extension `{}` adds `{}` to class `{}` (`{}`), which has a member of that name: rename one",
                    (def.type_name)(),
                    method.name,
                    class.name,
                    (class.type_name)(),
                );
            }
            if let Some(other) = self
                .extensions
                .iter()
                .find(|e| e.extends(class) && e.members.iter().any(|m| m.name == method.name))
            {
                panic!(
                    "class extensions `{}` and `{}` both add `{}` to class `{}` (`{}`){}: rename one",
                    (other.type_name)(),
                    (def.type_name)(),
                    method.name,
                    class.name,
                    (class.type_name)(),
                    of_set(set),
                );
            }
        }
        Arc::make_mut(&mut self.extensions).push(def);
        // What its methods name, but not its class: the extension adds to
        // the class, it does not export it.
        let mut refs = Vec::new();
        for m in def.members {
            (m.refs)(&mut refs);
        }
        refs.retain(|r| (r.type_id)() != (class.type_id)());
        self.classes_named(refs)
    }

    /// The class extensions this set registers
    /// ([`Registry::class_extension`]), by their class's name, then their
    /// type's; a [`Registry::merged`] registry's are every merged set's.
    pub fn class_extensions(&self) -> Vec<&'static ClassExtensionDef> {
        let mut defs: Vec<_> = self.extensions.iter().copied().collect();
        sort_extensions(&mut defs);
        defs
    }

    /// The class extensions of `sets`, their submodules and the sets they
    /// serve ([`Registry::served_sets`]), each once (one registered by two
    /// sets is one extension), by their class's name, then their type's:
    /// the methods a host serving those sets puts on its classes.
    ///
    /// # Panics
    ///
    /// When two different extensions in those sets add one name to one
    /// class, or two different sets have one specifier: a mistake in the
    /// code, caught when the host starts.
    #[track_caller]
    pub fn class_extensions_in(sets: &[Registry]) -> Vec<&'static ClassExtensionDef> {
        let mut out: Vec<(&'static ClassExtensionDef, Option<&'static str>)> = Vec::new();
        for set in Self::served_sets(sets) {
            for &def in set.extensions.iter() {
                if out.iter().any(|(e, _)| (e.type_id)() == (def.type_id)()) {
                    continue;
                }
                for method in def.members {
                    if let Some((other, other_set)) = out.iter().find(|(e, _)| {
                        e.extends(def.class) && e.members.iter().any(|m| m.name == method.name)
                    }) {
                        panic!(
                            "class extensions `{}` (in {}) and `{}` (in {}) both add `{}` to class `{}` (`{}`), by two UI sets: rename one",
                            (other.type_name)(),
                            set_name(*other_set),
                            (def.type_name)(),
                            set_name(set.specifier()),
                            method.name,
                            def.class.name,
                            (def.class.type_name)(),
                        );
                    }
                }
                out.push((def, set.specifier()));
            }
        }
        let mut defs: Vec<_> = out.into_iter().map(|(def, _)| def).collect();
        sort_extensions(&mut defs);
        defs
    }

    /// Registers interface `I` (`#[interface]` on a trait; `I` is its
    /// stand-in, `Js<Trait>`): the set's declarations get a TS `export
    /// interface` of the trait's name, which TS objects implement for Rust
    /// to call, and the classes its members name are registered with it. A
    /// signature naming the stand-in declares the interface too; this
    /// declares one no signature names (a trait TS services implement).
    /// Registering it again does nothing.
    ///
    /// # Panics
    ///
    /// When the set has a class, module, component or another interface of
    /// that name.
    #[track_caller]
    pub fn interface<I: super::Interface>(&mut self) -> &mut Self {
        let def = I::DEF;
        let set = self.ui_set.or(self.namespace);
        if let Some(old) = self.interfaces.get(def.name) {
            if (old.type_id)() == (def.type_id)() {
                return self;
            }
            panic!(
                "interfaces `{}` and `{}` are both named `{}`{}: rename one",
                (old.type_name)(),
                (def.type_name)(),
                def.name,
                of_set(set)
            );
        }
        if self.modules.contains_key(def.name)
            || self.types.values().any(|t| t.spec.export == def.name)
            || self.classes.keys().any(|(_, name)| *name == def.name)
            || self.aliases.contains_key(def.name)
        {
            panic!(
                "interface `{}` is named `{}` like a class, a module, a component or a type alias{}: rename one",
                (def.type_name)(),
                def.name,
                of_set(set)
            );
        }
        Arc::make_mut(&mut self.interfaces).insert(def.name, def);
        let mut refs = Vec::new();
        <I as super::JsValue>::classes(&mut refs);
        self.classes_named(refs)
    }

    /// The registered interfaces ([`Registry::interface`]), by name.
    pub fn interface_defs(&self) -> Vec<&'static super::InterfaceDef> {
        let mut defs: Vec<_> = self.interfaces.values().copied().collect();
        defs.sort_by_key(|d| d.name);
        defs
    }

    /// Exports TypeScript type alias `alias` in the set's declarations
    /// (`export type ServiceKey<T> = …;`), for the signatures it writes in
    /// TypeScript (`#[ts("…")]`) to share. A declaration only: JS has no
    /// value of it, and the classes its text names are the set's to
    /// register. Registering it again does nothing.
    ///
    /// # Panics
    ///
    /// When its name is no identifier (with its type parameters between
    /// `<` and `>` after it) or it names no type, or the set has another
    /// type alias, a class, an interface, a module or a component of that
    /// name.
    #[track_caller]
    pub fn ts_alias(&mut self, alias: super::TsAlias) -> &mut Self {
        if let Some(mistake) = alias.mistake() {
            panic!("{mistake}");
        }
        let name = alias.base_name();
        let set = self.ui_set.or(self.namespace);
        if let Some(old) = self.aliases.get(name) {
            if *old == alias {
                return self;
            }
            panic!(
                "type aliases `{}` and `{}` are both named `{name}`{}: rename one",
                old.name,
                alias.name,
                of_set(set)
            );
        }
        if self.modules.contains_key(name)
            || self.types.values().any(|t| t.spec.export == name)
            || self.classes.keys().any(|(_, class)| *class == name)
            || self.interfaces.contains_key(name)
        {
            panic!(
                "type alias `{}` is named `{name}` like a class, an interface, a module or a component{}: rename one",
                alias.name,
                of_set(set)
            );
        }
        Arc::make_mut(&mut self.aliases).insert(name, alias);
        self
    }

    /// The registered type aliases ([`Registry::ts_alias`]), by name.
    pub fn ts_aliases(&self) -> Vec<super::TsAlias> {
        let mut aliases: Vec<_> = self.aliases.values().copied().collect();
        aliases.sort_by_key(|a| a.base_name());
        aliases
    }

    /// Exports `values` and `types` of the runtime as they are (accessors
    /// such as `safeArea`, hooks, types such as `Insets`).
    pub fn reexport(&mut self, values: &[&'static str], types: &[&'static str]) -> &mut Self {
        let r = Arc::make_mut(&mut self.reexports);
        r.0.extend_from_slice(values);
        r.1.extend_from_slice(types);
        self
    }

    /// The components of all of `sets` and the sets they serve
    /// ([`Registry::served_sets`]), for making them by name (the core's
    /// view of the sets a host serves). Not a module of its own.
    /// Components of different sets may share an export (JS imports them
    /// from different modules), not a name. Modules and classes are not
    /// in it: their calls carry what they run. Its
    /// [`class_extensions`](Registry::class_extensions) are every set's
    /// ([`Registry::class_extensions_in`]).
    ///
    /// # Panics
    ///
    /// When two sets register the same component, two types of the same
    /// name or two fakes of the same name ([`Registry::fake`]) (naming
    /// both sets), two sets' class extensions add one name to one class,
    /// or two different sets have one specifier.
    #[track_caller]
    pub fn merged(sets: &[Registry]) -> Self {
        let mut out = Self {
            extensions: Arc::new(Self::class_extensions_in(sets)),
            ..Self::default()
        };
        // The set that registered each component, for the message of a
        // second one.
        let mut owners: FxHashMap<&'static str, Option<&'static str>> = FxHashMap::default();
        for set in Self::served_sets(sets) {
            // Each set's documents, in set order, never merged: the core
            // activates the first ([`super::theme`](super::theme)).
            for (all, theirs) in [
                (&mut out.theme_documents, &set.theme_documents),
                (&mut out.fallback_documents, &set.fallback_documents),
            ] {
                for document in theirs.iter() {
                    if !all.iter().any(|d| d.text == document.text) {
                        Arc::make_mut(all).push(document.clone());
                    }
                }
            }
            for (role, owner) in set.roles.iter().zip(set.role_sets.iter()) {
                out.add_role(role.clone(), *owner);
            }
            out.scrollbars = out.scrollbars.or(set.scrollbars);
            Arc::make_mut(&mut out.setups).extend(set.setups.iter().cloned());
            let fakes = Arc::make_mut(&mut out.fakes);
            for (name, fake) in set.fakes.iter() {
                assert!(
                    fakes.iter().all(|(n, _)| n != name),
                    "two fakes are named `{name}`, by two UI sets"
                );
                fakes.push((name, fake.clone()));
            }
            let types = Arc::make_mut(&mut out.types);
            for (name, ty) in set.types.iter() {
                if let Some(old) = types.insert(name, ty.clone()) {
                    let first = owners.get(name).copied().flatten();
                    panic!(
                        "{}",
                        in_two_sets(name, old.id == ty.id, first, set.specifier())
                    );
                }
                owners.insert(name, set.specifier());
            }
        }
        out
    }

    /// The registry of a UI set apps import as `specifier`
    /// (`app/native`). An app's crate makes its own with
    /// `#[native_module("…")]`.
    pub fn ui_set(specifier: &'static str) -> Self {
        Self {
            ui_set: Some(specifier),
            ..Self::default()
        }
    }

    /// The JS module the registry is imported as: a UI set's, or the
    /// runtime module's (`None`: a Rust app's components).
    pub fn specifier(&self) -> Option<&'static str> {
        self.ui_set.or(self.namespace)
    }

    /// The runtime values and types exported as they are
    /// ([`Registry::reexport`]).
    pub fn reexports(&self) -> (&[&'static str], &[&'static str]) {
        (&self.reexports.0, &self.reexports.1)
    }

    /// Serves UI set `set` wherever this one is served, as a module of
    /// its own under a path of this set's specifier (`nsh/base/plot` of
    /// `nsh/base`, as `gpui-base`'s `plot` is a module of the crate): a
    /// family of components whose names (`Line`, `Grid`) only make sense
    /// together, apart from the set's other exports. Hosts, the toolchain
    /// and the declarations take the set's submodules with it
    /// ([`Registry::sets`]), so a list of UI sets names only the set.
    ///
    /// # Panics
    ///
    /// When `set`'s specifier is not a path under this set's.
    #[track_caller]
    pub fn submodule(&mut self, set: Registry) -> &mut Self {
        let under = match (self.ui_set, set.ui_set) {
            (Some(parent), Some(child)) => child
                .strip_prefix(parent)
                .is_some_and(|rest| rest.len() > 1 && rest.starts_with('/')),
            _ => false,
        };
        assert!(
            under,
            "a submodule is a UI set under a path of its set's specifier: {:?} under {:?}",
            set.ui_set, self.ui_set
        );
        Arc::make_mut(&mut self.submodules).push(set);
        self
    }

    /// Serves UI set `set` wherever this one is served, as a module of its
    /// own under its own specifier (`acme/ui`), not a path of this set's
    /// (that is [`Registry::submodule`]): a library's set. The library's
    /// crate makes it in a plain function (`pub fn set() -> Registry`,
    /// [`Registry::ui_set`] and its registrations), and the crate of each
    /// app using the library serves it from its `#[native_module]` entry
    /// (`registry.serve(acme_ui::set())`): every app imports the library's
    /// components by the library's specifier, and so do the library's own
    /// files, with nothing more to list. Hosts, the toolchain and the
    /// declarations take the set with this one ([`Registry::sets`]); two
    /// sets serving one library serve it once ([`Registry::served_sets`]).
    ///
    /// # Panics
    ///
    /// When `set` has no specifier (it is no UI set), or its specifier is
    /// this set's or a path under it.
    #[track_caller]
    pub fn serve(&mut self, set: Registry) -> &mut Self {
        let Some(served) = set.ui_set else {
            panic!("a served set is a UI set, with a specifier of its own (`Registry::ui_set`)");
        };
        if let Some(own) = self.specifier() {
            let under = served
                .strip_prefix(own)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'));
            assert!(
                !under,
                "a served set has a specifier of its own, not this set's or a path under it (a submodule's): {served:?} served by {own:?}"
            );
        }
        Arc::make_mut(&mut self.served).push(set);
        self
    }

    /// This set, its submodules ([`Registry::submodule`]), then the sets
    /// it serves ([`Registry::serve`]), each with theirs: every module apps
    /// import of it, what a host serves and the toolchain declares for it.
    /// A set served twice (by this set and one of its submodules) is listed
    /// twice: a list of sets is flattened with [`Registry::served_sets`].
    pub fn sets(&self) -> Vec<&Registry> {
        let mut out = vec![self];
        for set in self.submodules.iter().chain(self.served.iter()) {
            out.extend(set.sets());
        }
        out
    }

    /// The UI sets `sets` serve ([`Registry::sets`] of each), flattened, in
    /// order and each specifier once: the modules a host serving `sets`
    /// serves and the toolchain describes. A set whose specifier is listed
    /// already is left out when it is the same set: two crates of one app
    /// built on one library both serve its set, each made by the library's
    /// function. Two sets are the same when they export the same: the same
    /// components, modules, classes, interfaces, class extensions and type
    /// aliases, by name and Rust type. Registries without a specifier (a
    /// Rust app's components) are all listed.
    ///
    /// # Panics
    ///
    /// When another set has a specifier listed already, naming the sets
    /// serving both: a specifier is one module.
    #[track_caller]
    pub fn served_sets(sets: &[Registry]) -> Vec<&Registry> {
        Self::served_sets_by_entry(sets)
            .into_iter()
            .map(|(_, set)| set)
            .collect()
    }

    /// [`Registry::served_sets`], each with the index in `sets` of the set
    /// whose [`Registry::sets`] listed it first (`SetDescription::entry`).
    #[track_caller]
    pub(crate) fn served_sets_by_entry(sets: &[Registry]) -> Vec<(usize, &Registry)> {
        let mut out: Vec<(usize, &Registry)> = Vec::new();
        for (entry, root) in sets.iter().enumerate() {
            for set in root.sets() {
                let listed = set
                    .specifier()
                    .and_then(|s| out.iter().find(|(_, o)| o.specifier() == Some(s)));
                match listed {
                    None => out.push((entry, set)),
                    Some((_, listed)) if listed.same_exports(set) => {}
                    Some((first, _)) => {
                        let specifier = set.specifier().unwrap_or_default();
                        let by = |root: &Registry| match root.specifier() {
                            Some(s) if s == specifier => "listed".to_owned(),
                            other => format!("served by {}", set_name(other)),
                        };
                        panic!(
                            "two different UI sets are imported as `{specifier}`, one {}, the other {}: a specifier is one module (two versions of one library?)",
                            by(&sets[*first]),
                            by(root),
                        );
                    }
                }
            }
        }
        out
    }

    /// Whether `other` exports what this set does: the same components,
    /// modules, classes, interfaces, class extensions and type aliases, by
    /// name and Rust type ([`Registry::served_sets`]).
    fn same_exports(&self, other: &Registry) -> bool {
        fn same<K: Eq + std::hash::Hash, V>(
            a: &FxHashMap<K, V>,
            b: &FxHashMap<K, V>,
            eq: impl Fn(&V, &V) -> bool,
        ) -> bool {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| eq(v, w)))
        }
        let extension = |e: &&'static ClassExtensionDef| (e.type_id)();
        same(&*self.types, &*other.types, |a, b| a.id == b.id)
            && same(&*self.modules, &*other.modules, |a, b| {
                a.type_name == b.type_name
            })
            && same(&*self.classes, &*other.classes, |a, b| {
                (a.spec.def.type_id)() == (b.spec.def.type_id)()
            })
            && same(&*self.interfaces, &*other.interfaces, |a, b| {
                (a.type_id)() == (b.type_id)()
            })
            && same(&*self.aliases, &*other.aliases, |a, b| a == b)
            && self.extensions.len() == other.extensions.len()
            && self.extensions.iter().all(|e| {
                other
                    .extensions
                    .iter()
                    .any(|o| extension(o) == extension(e))
            })
    }

    /// Exports `values` `nsh-js` makes for the set (`NavStack`, `NavPage`),
    /// declared by hand rather than generated from a spec.
    pub fn native_values(&mut self, values: &[&'static str]) -> &mut Self {
        Arc::make_mut(&mut self.native_values).extend_from_slice(values);
        self
    }

    /// Ships `files` with the apps that use the set (`assets!()`: a crate's
    /// `assets/` directory): `nsh run` serves them, `nsh package` copies
    /// them into the app, and the engine registers the fonts among them
    /// before the first render. `asset!` refers to one. Every target gets
    /// them; [`Registry::assets_for`] limits them to some.
    pub fn assets(&mut self, files: &'static [super::AssetFile]) -> &mut Self {
        self.assets_for(&[], files)
    }

    /// Like [`Registry::assets`], for the packages of `targets` only: the
    /// others neither copy the files nor register their fonts, so a file
    /// only the web downloads stays out of the desktop and mobile
    /// packages. No `targets`: every target. A file the app reads on a
    /// target outside the list is not there to read.
    pub fn assets_for(
        &mut self,
        targets: &'static [crate::platform::Os],
        files: &'static [super::AssetFile],
    ) -> &mut Self {
        Arc::make_mut(&mut self.assets).push((targets, files));
        self
    }

    /// Makes the built-in bar of the scroll views and lists that show one
    /// ([`set_builtin_scrollbars`](super::set_builtin_scrollbars), or a
    /// theme as their `scrollbar`) with `source`. Of the sets a host
    /// serves, the first that has one makes them.
    pub fn scrollbars_from(&mut self, source: ScrollbarSource) -> &mut Self {
        self.scrollbars = Some(source);
        self
    }

    /// How the set makes the views' built-in scroll bars
    /// ([`Registry::scrollbars_from`]).
    pub(crate) fn scrollbar_source(&self) -> Option<ScrollbarSource> {
        self.scrollbars
    }

    /// Runs `f` once on each engine that serves the set, in its reactive
    /// runtime, before the app's first op: what the set's crates keep per
    /// engine is set up there (the languages an `Editor` knows,
    /// `nsh_base::editor::register_language`; a value of
    /// [`global`](super::prelude::global)), as a Rust app's root sets it up
    /// before its children. An app's crate calls it from its
    /// `#[native_module]` function.
    pub fn setup(&mut self, f: impl Fn() + Send + Sync + 'static) -> &mut Self {
        Arc::make_mut(&mut self.setups).push(Arc::new(f));
        self
    }

    /// Gives headless runs `fake` in place of what a module of the set asks
    /// of the platform, under `name` (`"nsh-extra/speech"`: the set's
    /// specifier and the module), which tests call it by
    /// ([`crate::fake`]). Apps never use it; the headless driver installs
    /// it before the app starts. An app's crate calls it from its
    /// `#[native_module]` function.
    ///
    /// # Panics
    ///
    /// When the set has a fake of that name already. Names are unique
    /// across the sets an app is served, too ([`Registry::merged`]).
    #[track_caller]
    pub fn fake(&mut self, name: &'static str, fake: Arc<dyn crate::fake::Fake>) -> &mut Self {
        assert!(
            self.fakes.iter().all(|(n, _)| *n != name),
            "two fakes are named `{name}`{}",
            of_set(self.ui_set.or(self.namespace))
        );
        Arc::make_mut(&mut self.fakes).push((name, fake));
        self
    }

    /// The fakes [`Registry::fake`] gave the set and the sets served under
    /// it, by name, in order: what the headless driver installs.
    #[doc(hidden)]
    pub fn fakes(&self) -> Vec<(&'static str, Arc<dyn crate::fake::Fake>)> {
        self.sets()
            .into_iter()
            .flat_map(|set| set.fakes.iter().cloned())
            .collect()
    }

    /// Runs the functions [`Registry::setup`] gave the set (a merged set's:
    /// every set's, in order). The core runs them when it takes its sets.
    pub(crate) fn run_setups(&self) {
        for setup in self.setups.iter() {
            setup();
        }
    }

    /// Runs the setup functions of this set that `old` does not have
    /// (`Core::extend_components`).
    pub(crate) fn run_setups_not_in(&self, old: &Registry) {
        for setup in self.setups.iter() {
            let known = old
                .setups
                .iter()
                .any(|o| std::ptr::addr_eq(Arc::as_ptr(o), Arc::as_ptr(setup)));
            if !known {
                setup();
            }
        }
    }

    /// Registers the set's theme document `json` (`include_str!` of it), in
    /// the theme format (`docs/dev/theme-format.md`): the app's tokens,
    /// modifiers and roles when the app has no theme of its own and this
    /// set is the first, in the host's set order, to register one
    /// ([`super::theme`](super::theme)). Documents are never merged.
    ///
    /// It never panics: its diagnostics are logged (target `nsh::theme`). A
    /// text that is no document (`not-a-document`) or of the old format
    /// (`legacy-format`) is skipped, so the next document by
    /// precedence is active; a document with errors keeps its good entries.
    pub fn theme_document(&mut self, json: &'static str) -> &mut Self {
        let owner = self.ui_set.unwrap_or("the app's set");
        register_theme(&mut self.theme_documents, owner, json);
        self
    }

    /// Registers a fallback theme document `json`: the active document when
    /// neither the app nor any set has one (`nsh-base`'s default theme). As
    /// [`Registry::theme_document`], it never panics.
    pub fn fallback_theme_document(&mut self, json: &'static str) -> &mut Self {
        let owner = self.ui_set.unwrap_or("the app's set");
        register_theme(&mut self.fallback_documents, owner, json);
        self
    }

    /// The theme documents registered ([`Registry::theme_document`]), in
    /// set order: each one's owner and text.
    pub fn theme_documents(&self) -> Vec<(&'static str, &'static str)> {
        self.theme_documents
            .iter()
            .map(|d| (d.owner, d.text))
            .collect()
    }

    /// The fallback documents registered
    /// ([`Registry::fallback_theme_document`]): each one's owner and text.
    pub fn fallback_theme_documents(&self) -> Vec<(&'static str, &'static str)> {
        self.fallback_documents
            .iter()
            .map(|d| (d.owner, d.text))
            .collect()
    }

    /// The document the sets make active when the app has none of its own:
    /// the first registered by a set, else the first fallback. With the
    /// warnings `document-unused` of each other set's document.
    pub fn theme_document_of_sets(
        &self,
    ) -> (
        Option<Arc<nsh_theme::ThemeDocument>>,
        Vec<nsh_theme::Diagnostic>,
    ) {
        let mut documents = self.theme_documents.iter();
        let active = documents.next();
        let unused = documents
            .map(|d| {
                nsh_theme::Diagnostic::warning(
                    nsh_theme::code::DOCUMENT_UNUSED,
                    "",
                    format!(
                        "The theme document of {} is unused: {} registers the active one.",
                        d.owner,
                        active.map_or("another set", |a| a.owner)
                    ),
                )
            })
            .collect();
        let document = active
            .or_else(|| self.fallback_documents.first())
            .map(|d| d.document.clone());
        (document, unused)
    }

    /// Registers the roles the set's components draw with, mapped by a
    /// theme or not (`nsh_base::role::all()`, or a design system's own made
    /// with [`crate::roles!`]): their declarations ([`super::RoleSpec`]),
    /// which the toolchain types TS's `role()` from and the core checks a
    /// theme's mappings against. A role is its name, so one two sets list
    /// with the same value type is listed once. Listed again with another
    /// value type, it logs the warning `role-conflict` (naming the role,
    /// both types and both sets) and the first is kept. Never panics.
    pub fn roles(&mut self, roles: impl IntoIterator<Item = super::RoleSpec>) -> &mut Self {
        let set = self.specifier();
        for role in roles {
            self.add_role(role, set);
        }
        self
    }

    fn add_role(&mut self, spec: super::RoleSpec, set: Option<&'static str>) {
        let roles = Arc::make_mut(&mut self.roles);
        let sets = Arc::make_mut(&mut self.role_sets);
        if let Some(conflict) = super::tokens::merge_role(roles, sets, spec, set) {
            super::theme::report(&[conflict]);
        }
    }

    /// The roles listed ([`Registry::roles`]), as data.
    pub fn role_specs(&self) -> &[super::RoleSpec] {
        &self.roles
    }

    /// The set that listed each of [`Registry::role_specs`], in order
    /// (`None`: the app's components).
    pub(crate) fn role_sets(&self) -> &[Option<&'static str>] {
        &self.role_sets
    }

    /// The files [`Registry::assets`] and [`Registry::assets_for`] ship, for
    /// every target.
    pub fn asset_files(&self) -> impl Iterator<Item = &'static super::AssetFile> + '_ {
        self.assets.iter().flat_map(|(_, files)| files.iter())
    }

    /// The files of [`Registry::asset_files`] with the targets each is
    /// for (none: every target).
    pub fn scoped_asset_files(
        &self,
    ) -> impl Iterator<Item = (&'static [crate::platform::Os], &'static super::AssetFile)> + '_
    {
        self.assets
            .iter()
            .flat_map(|&(targets, files)| files.iter().map(move |file| (targets, file)))
    }

    /// The values of [`Registry::native_values`].
    pub fn native_value_names(&self) -> &[&'static str] {
        &self.native_values
    }

    /// A registry of the runtime's module `namespace` (`nsh/android`).
    pub(crate) fn runtime(namespace: &'static str) -> Self {
        Self {
            namespace: Some(namespace),
            ..Self::default()
        }
    }

    /// The runtime module the registry's functions belong to (`None`: a UI
    /// set's).
    pub fn namespace(&self) -> Option<&'static str> {
        self.namespace
    }

    /// The registered classes, by name.
    pub fn class_specs(&self) -> Vec<Arc<ClassSpec>> {
        let mut specs: Vec<_> = self.classes.values().map(|c| c.spec.clone()).collect();
        specs.sort_by_key(|s| s.name);
        specs
    }

    /// The registered modules, by name.
    pub fn module_specs(&self) -> Vec<Arc<ModuleSpec>> {
        let mut specs: Vec<_> = self.modules.values().map(|m| m.spec.clone()).collect();
        specs.sort_by_key(|s| s.name);
        specs
    }

    /// The module whose functions JS finds on the function `function` of
    /// a flat module of the set: a module named like it
    /// (`#[module(name = "test")]`: `test.skip`), as a module named like a
    /// component is on the component.
    pub fn function_members(&self, function: &str) -> Option<Arc<ModuleSpec>> {
        let exported = self.modules.values().any(|m| {
            m.spec.flat
                && m.spec
                    .functions
                    .iter()
                    .any(|f| !f.getter && f.name == function)
        });
        exported
            .then(|| {
                self.modules
                    .values()
                    .find(|m| !m.spec.flat && m.spec.name == function)
                    .map(|m| m.spec.clone())
            })
            .flatten()
    }

    /// The registered components, by export.
    pub fn specs(&self) -> Vec<Arc<ComponentSpec>> {
        let mut specs: Vec<_> = self.types.values().map(|t| t.spec.clone()).collect();
        specs.sort_by(|a, b| a.export.cmp(&b.export).then(a.name.cmp(b.name)));
        specs
    }

    /// The exports of the set that TS sees without documentation, by
    /// their TS names (`Gauge`, `Gauge.max`, `Gauge.onChange`,
    /// `Ledger.copy`, `new Account`, `Account.balance`): what to give a
    /// doc comment. A component is documented by its `#[component]`
    /// function, a module or class by its struct, the rest by their own
    /// (a setter needs none beside its getter's; a module merged into a
    /// component or a function of its name, or exported flat, by that
    /// component or function, or its functions). A method the set's class
    /// extensions add is listed under its class (`Builder.addI18n`).
    pub fn undocumented(&self) -> Vec<String> {
        let mut out = Vec::new();
        let specs = self.specs();
        for c in &specs {
            if c.doc.is_empty() {
                out.push(c.export.clone());
            }
            for p in c.props.iter().filter(|p| p.doc.is_empty()) {
                out.push(format!("{}.{}", c.export, p.name));
            }
            for e in c.events.iter().filter(|e| e.doc.is_empty()) {
                out.push(format!("{}.{}", c.export, handler_name(e.name)));
            }
            for m in c.methods.iter().filter(|m| m.doc.is_empty()) {
                out.push(format!("{}.{}", c.export, m.name));
            }
        }
        for m in self.module_specs() {
            let merged = specs.iter().any(|c| c.export == m.name)
                || (!m.flat && self.function_members(m.name).is_some());
            if m.doc.is_empty() && !merged && !m.flat {
                out.push(m.name.to_owned());
            }
            for f in m.functions.iter().filter(|f| f.doc.is_empty()) {
                out.push(if m.flat {
                    f.name.to_owned()
                } else {
                    format!("{}.{}", m.name, f.name)
                });
            }
        }
        for c in self.class_specs() {
            if c.doc.is_empty() {
                out.push(c.name.to_owned());
            }
            let documented_getter = |name: &str| {
                c.members
                    .iter()
                    .any(|m| m.kind == MemberKind::Getter && m.name == name && !m.doc.is_empty())
            };
            for m in c.members.iter().filter(|m| m.doc.is_empty()) {
                match m.kind {
                    MemberKind::Constructor => out.push(format!("new {}", c.name)),
                    MemberKind::Setter if documented_getter(m.name) => {}
                    _ => out.push(format!("{}.{}", c.name, m.name)),
                }
            }
        }
        for e in self.class_extensions() {
            for m in e.members.iter().filter(|m| m.doc.is_empty()) {
                out.push(format!("{}.{}", e.class.name, m.name));
            }
        }
        for i in self.interface_defs() {
            if i.doc.is_empty() {
                out.push(i.name.to_owned());
            }
            for m in i.members.iter().filter(|m| m.doc.is_empty()) {
                out.push(format!("{}.{}", i.name, m.name));
            }
        }
        for a in self.ts_aliases().iter().filter(|a| a.doc.is_empty()) {
            out.push(a.base_name().to_owned());
        }
        out
    }

    /// A new instance of the component of type `id`, and its registered
    /// name (a component element made by name).
    pub(crate) fn make_type(
        &self,
        id: TypeId,
    ) -> Option<(Arc<ComponentSpec>, Box<dyn AnyInstance>)> {
        self.types
            .values()
            .find(|t| t.id == id)
            .map(|t| (t.spec.clone(), (t.make)()))
    }

    /// A new instance of component `name`.
    pub(crate) fn make(&self, name: &str) -> Option<(Arc<ComponentSpec>, Box<dyn AnyInstance>)> {
        self.types.get(name).map(|t| (t.spec.clone(), (t.make)()))
    }
}

/// The message of two registrations under component name `name`.
fn same_name(name: &str, old: TypeId, new: TypeId, set: Option<&str>) -> String {
    if old == new {
        format!("component `{name}` is registered twice{}", of_set(set))
    } else {
        format!(
            "two different types named `{name}` are registered{} (two versions of one crate?)",
            of_set(set)
        )
    }
}

/// The message of component `name` registered by two UI sets, `first` and
/// `second` (`same`: one type; else two types of that name): what
/// [`Registry::merged`] panics with and the toolchain refuses before an
/// engine starts ([`super::description::component_clash`]).
pub(crate) fn in_two_sets(
    name: &str,
    same: bool,
    first: Option<&str>,
    second: Option<&str>,
) -> String {
    let (first, second) = (set_name(first), set_name(second));
    if same {
        format!(
            "component `{name}` is registered by both {first} and {second}: register it in one set; a library's set is served with `Registry::serve`"
        )
    } else {
        format!(
            "two different types named `{name}` are registered, by {first} and {second} (two versions of one crate?)"
        )
    }
}

/// The message of two modules named `name`.
fn same_module(name: &str, old: &str, new: &str, set: Option<&str>) -> String {
    if old == new {
        format!("module `{old}` is registered twice{}", of_set(set))
    } else {
        format!(
            "modules `{old}` and `{new}` are both named `{name}`{}: rename one",
            of_set(set)
        )
    }
}

/// The message of two classes named `name`.
fn same_class(name: &str, old: &str, new: &str, set: Option<&str>) -> String {
    if old == new {
        format!("class `{old}` is registered twice{}", of_set(set))
    } else {
        format!(
            "classes `{old}` and `{new}` are both named `{name}`{}: rename one",
            of_set(set)
        )
    }
}

/// ` in `app/native`` for a registry with a specifier.
fn of_set(set: Option<&str>) -> String {
    set.map(|s| format!(" in `{s}`")).unwrap_or_default()
}

/// `` `app/native` `` for a registry with a specifier, `the app's set`
/// for one without (a Rust app's components).
fn set_name(set: Option<&str>) -> String {
    set.map_or_else(|| "the app's set".to_owned(), |s| format!("`{s}`"))
}

/// Class extensions by their class's name, then their type's.
fn sort_extensions(defs: &mut [&'static ClassExtensionDef]) {
    defs.sort_by(|a, b| {
        a.class
            .name
            .cmp(b.class.name)
            .then_with(|| (a.type_name)().cmp((b.type_name)()))
    });
}

/// The declarations of what `registry`'s specs export: one component per
/// spec (by its [`ComponentSpec::export`] name), one object per module
/// (its type `<Name>Module`; merged into a component of the same name; a
/// flat module's functions one by one), one class per class, and the
/// named types they use.
/// Exports named in `omit` are left out (their types stay): `nsh-js`
/// declares them, building on the module (`BackHandlerModule & ...`), as
/// it declares the re-exports and native values it makes.
pub fn declarations(registry: &Registry, omit: &[&str]) -> String {
    let specs = registry.specs();
    let modules = registry.module_specs();
    let classes = registry.class_specs();
    use std::fmt::Write as _;
    let mut out = String::new();
    let doc = |out: &mut String, indent: &str, doc: &str| {
        if !doc.is_empty() {
            let _ = writeln!(out, "{indent}/** {doc} */");
        }
    };
    // The named types, once each: the set's type aliases first.
    let mut types = TsTypes::default();
    for alias in registry.ts_aliases() {
        types.declare(alias.base_name(), |_| alias.declaration());
    }
    for spec in &specs {
        types.merge(&spec.types);
    }
    for m in &modules {
        types.merge(&m.types);
    }
    for c in &classes {
        types.merge(&c.types);
    }
    // The methods the set adds to classes, with the named types theirs.
    let extensions: Vec<(&'static ClassExtensionDef, Vec<MemberSpec>)> = registry
        .class_extensions()
        .into_iter()
        .map(|e| (e, e.member_specs(&mut types)))
        .collect();
    // The interfaces TS implements: types only, with the named types theirs.
    for i in registry.interface_defs() {
        super::__private::interface_ts(i, &mut types);
    }
    for declaration in types.declarations() {
        let _ = write!(out, "{declaration}\n\n");
    }
    let module_of = |name: &str| modules.iter().find(|m| m.name == name);
    // A function's signatures: its own, or its overloads (type
    // parameters, parameters, result).
    let signatures = |f: &FunctionSpec| {
        let one = |generics: &'static str,
                   params: &[(&str, String, bool)],
                   returns: &Option<String>| {
            let params = params
                .iter()
                .map(|(n, ts, optional)| format!("{n}{}: {ts}", if *optional { "?" } else { "" }))
                .collect::<Vec<_>>()
                .join(", ");
            let value = returns.clone().unwrap_or_else(|| "void".to_owned());
            let returns = if f.promises() {
                format!("Promise<{value}>")
            } else {
                value
            };
            (generics, params, returns)
        };
        if f.overloads.is_empty() {
            vec![one("", &f.params, &f.returns)]
        } else {
            f.overloads
                .iter()
                .map(|o| one(o.generics, &o.params, &o.returns))
                .collect()
        }
    };
    // A module's object type.
    let module_object = |out: &mut String, m: &ModuleSpec| {
        out.push_str("{\n");
        for f in &m.functions {
            for (generics, params, returns) in signatures(f) {
                doc(out, "  ", f.doc);
                if f.getter {
                    let _ = writeln!(out, "  readonly {}: {returns};", f.name);
                    continue;
                }
                let _ = writeln!(out, "  {}{generics}({params}): {returns};", f.name);
            }
        }
        for constant in &m.constants {
            doc(out, "  ", constant.doc);
            let _ = writeln!(
                out,
                "  readonly {}: {};",
                constant.name,
                constant.value.ts_type()
            );
        }
        out.push('}');
    };
    for spec in &specs {
        // A component with rows is generic over their type, like `List`.
        // Props generic over their rows' type default it to `any`, so a mark
        // without rows of its own takes any field name.
        let (generic, params, extends) = match spec.items() {
            Some(_) => ("<T>", "<T>", ", ListItemsProps<T>"),
            None if spec.generic => ("<T>", "<T = any>", ""),
            None => ("", "", ""),
        };
        // A component with methods: its ref is an object of them. Host
        // props it declares itself are its own.
        let mut omit: Vec<&str> = HOST_PROPS
            .iter()
            .copied()
            .filter(|&h| !spec.host_props().any(|p| p == h))
            .collect();
        if !spec.methods.is_empty() {
            omit.insert(0, "ref");
        }
        let host = if omit.is_empty() {
            "ComponentHostProps".to_owned()
        } else {
            let names = omit.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>();
            format!("Omit<ComponentHostProps, {}>", names.join(" | "))
        };
        if !spec.methods.is_empty() {
            let _ = write!(
                out,
                "/** The ref of `{0}`: its methods. */\nexport interface {0}Ref {{\n  /** The component's host node. */\n  readonly node: ViewRef;\n",
                spec.export
            );
            for m in &spec.methods {
                doc(&mut out, "  ", m.doc);
                let params = m
                    .params
                    .iter()
                    .map(|(n, ts, optional)| {
                        format!("{n}{}: {ts}", if *optional { "?" } else { "" })
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let returns = match &m.returns {
                    Some(ts) => format!("Promise<{ts}>"),
                    None => "void".to_owned(),
                };
                let _ = writeln!(out, "  {}({params}): {returns};", m.name);
            }
            out.push_str("}\n\n");
        }
        let _ = writeln!(
            out,
            "export interface {}Props{params} extends {host}{extends} {{",
            spec.export
        );
        if !spec.methods.is_empty() {
            let _ = writeln!(
                out,
                "  ref?: {0}Ref | ((ref: {0}Ref) => void);",
                spec.export
            );
        }
        for p in spec.props.iter().filter(|p| p.kind != PropKind::Items) {
            doc(&mut out, "  ", p.doc);
            let optional = if p.optional { "?" } else { "" };
            let _ = writeln!(out, "  {}{optional}: {};", p.name, p.ts);
        }
        for e in &spec.events {
            doc(&mut out, "  ", e.doc);
            let handler = handler_name(e.name);
            let arg = if e.data == "void" {
                String::new()
            } else {
                format!("{}: {}", e.arg, e.data)
            };
            let _ = writeln!(out, "  {handler}?: ({arg}) => void;");
        }
        out.push_str("}\n\n");
        if omit.contains(&spec.export.as_str()) {
            continue;
        }
        doc(&mut out, "", spec.doc);
        // A module of the same name: its functions on the component.
        let mut with = String::new();
        if let Some(m) = module_of(&spec.export) {
            with.push_str(" & ");
            module_object(&mut with, m);
        }
        let _ = write!(
            out,
            "export declare const {}: ({params}(props: {}Props{generic}) => JSX.Element){with};\n\n",
            spec.export, spec.export,
        );
    }
    for m in &modules {
        if specs.iter().any(|s| s.export == m.name)
            || (!m.flat && registry.function_members(m.name).is_some())
        {
            continue;
        }
        if m.flat {
            for f in m
                .functions
                .iter()
                .filter(|f| !f.getter && !omit.contains(&f.name))
            {
                for (generics, params, returns) in signatures(f) {
                    doc(&mut out, "", f.doc);
                    let _ = writeln!(
                        out,
                        "export declare function {}{generics}({params}): {returns};",
                        f.name
                    );
                }
                // A module of its name: its functions on the function.
                if let Some(members) = registry.function_members(f.name) {
                    let _ = writeln!(out, "export declare namespace {} {{", f.name);
                    for member in &members.functions {
                        for (generics, params, returns) in signatures(member) {
                            doc(&mut out, "  ", member.doc);
                            let _ = if member.getter {
                                writeln!(out, "  const {}: {returns};", member.name)
                            } else {
                                writeln!(
                                    out,
                                    "  function {}{generics}({params}): {returns};",
                                    member.name
                                )
                            };
                        }
                    }
                    for constant in &members.constants {
                        doc(&mut out, "  ", constant.doc);
                        let _ = writeln!(
                            out,
                            "  const {}: {};",
                            constant.name,
                            constant.value.ts_type()
                        );
                    }
                    out.push_str("}\n");
                }
                out.push('\n');
            }
            for constant in &m.constants {
                if !omit.contains(&constant.name) {
                    doc(&mut out, "", constant.doc);
                    let _ = writeln!(
                        out,
                        "export declare const {}: {};\n",
                        constant.name,
                        constant.value.ts_type()
                    );
                }
            }
            if m.default && !omit.contains(&"default") {
                let _ = write!(out, "declare const defaultExport: ");
                module_object(&mut out, m);
                // The registry's values `nsh-js` makes are on the object too.
                for native in registry.native_value_names() {
                    let _ = write!(out, " & {{ {native}: typeof {native} }}");
                }
                out.push_str(";\nexport default defaultExport;\n\n");
            }
            continue;
        }
        doc(&mut out, "", m.doc);
        let _ = write!(out, "export interface {}Module ", m.name);
        module_object(&mut out, m);
        out.push_str("\n\n");
        if !omit.contains(&m.name) {
            doc(&mut out, "", m.doc);
            let _ = write!(out, "export declare const {0}: {0}Module;\n\n", m.name);
        }
        if m.default && !omit.contains(&"default") {
            let _ = write!(
                out,
                "declare const defaultExport: {}Module;\nexport default defaultExport;\n\n",
                m.name
            );
        }
    }
    for c in classes.iter().filter(|c| !omit.contains(&c.name)) {
        // The methods the set's extensions of it add.
        let added: Vec<&MemberSpec> = extensions
            .iter()
            .filter(|(e, _)| e.extends(c.def))
            .flat_map(|(_, members)| members)
            .collect();
        class_declaration(&mut out, c, &added);
    }
    out
}

/// `export declare class Name { ... }`, with the methods its set's
/// extensions add (`added`) after its own members.
fn class_declaration(out: &mut String, c: &ClassSpec, added: &[&MemberSpec]) {
    use std::fmt::Write as _;
    let doc = |out: &mut String, indent: &str, doc: &str| {
        if !doc.is_empty() {
            let _ = writeln!(out, "{indent}/** {doc} */");
        }
    };
    let signatures = member_signatures;
    let generics = class_generics(c.def);
    doc(out, "", c.doc);
    if c.def.interface {
        // No value: what its objects have.
        let _ = writeln!(out, "export interface {}{generics} {{", c.name);
        let has_setter = |name| {
            c.members
                .iter()
                .any(|s| s.kind == MemberKind::Setter && s.name == name)
        };
        for m in c.members.iter().chain(added.iter().copied()) {
            for (generics, params, returns) in signatures(m) {
                let line = match m.kind {
                    MemberKind::Method => format!("  {}{generics}({params}): {returns};", m.name),
                    MemberKind::Getter if has_setter(m.name) => {
                        format!("  {}: {returns};", m.name)
                    }
                    MemberKind::Getter => format!("  readonly {}: {returns};", m.name),
                    // A setter's property is its getter's; constructors and
                    // statics belong to a class value it has none of.
                    _ => continue,
                };
                doc(out, "  ", m.doc);
                let _ = writeln!(out, "{line}");
            }
        }
        out.push_str("}\n\n");
        return;
    }
    let _ = writeln!(out, "export declare class {}{generics} {{", c.name);
    if !c.def.generics.is_empty() {
        // An ES private member nothing reads: without it, TypeScript would
        // take a `ServiceToken<A>` for a `ServiceToken<B>` whenever no member
        // names the type parameters.
        let names: Vec<&str> = c.def.generics.iter().map(|(name, _)| *name).collect();
        let _ = writeln!(
            out,
            "  /** Its type parameters, kept in its type (no value at run time). */\n  #types: [{}];",
            names.join(", ")
        );
    }
    if !c.members.iter().any(|m| m.kind == MemberKind::Constructor) {
        out.push_str("  /** Made by the class's members only. */\n  private constructor();\n");
    }
    for m in c.members.iter().chain(added.iter().copied()) {
        for (generics, params, returns) in signatures(m) {
            doc(out, "  ", m.doc);
            let _ = match m.kind {
                MemberKind::Constructor => writeln!(out, "  constructor({params});"),
                MemberKind::Method => {
                    writeln!(out, "  {}{generics}({params}): {returns};", m.name)
                }
                MemberKind::Getter => writeln!(out, "  get {}(): {returns};", m.name),
                MemberKind::Setter => writeln!(out, "  set {}({params});", m.name),
                MemberKind::Static => {
                    writeln!(out, "  static {}{generics}({params}): {returns};", m.name)
                }
            };
        }
    }
    out.push_str(
        "  /** Frees the Rust value now (else when the object is collected); the object throws after. */\n  dispose(): void;\n  [Symbol.dispose](): void;\n}\n\n",
    );
}

/// A member's signatures: its own, or those declared instead
/// (`#[overload]`, `#[ts]`): type parameters, parameters and result (a
/// promise of it for an `async` member).
fn member_signatures(m: &MemberSpec) -> Vec<(&'static str, String, String)> {
    let one =
        |generics: &'static str, params: &[(&str, String, bool)], returns: &Option<String>| {
            let params = params
                .iter()
                .map(|(n, ts, optional)| format!("{n}{}: {ts}", if *optional { "?" } else { "" }))
                .collect::<Vec<_>>()
                .join(", ");
            let value = returns.clone().unwrap_or_else(|| "void".to_owned());
            let returns = if m.is_async {
                format!("Promise<{value}>")
            } else {
                value
            };
            (generics, params, returns)
        };
    if m.overloads.is_empty() {
        vec![one("", &m.params, &m.returns)]
    } else {
        m.overloads
            .iter()
            .map(|o| one(o.generics, &o.params, &o.returns))
            .collect()
    }
}

/// `<T, U = never>` of a generic class, as its declaration writes it
/// (empty for most classes).
pub(crate) fn class_generics(def: &ClassDef) -> String {
    if def.generics.is_empty() {
        String::new()
    } else {
        let params: Vec<&str> = def.generics.iter().map(|(_, text)| *text).collect();
        format!("<{}>", params.join(", "))
    }
}

/// The lines of the methods class extension `def` adds, as its class's
/// body has them (each with its doc comment): what `.nsh/types` adds to
/// the class's other declarations
/// ([`super::description::ClassExtensionDescription`]).
pub(crate) fn extension_lines(def: &ClassExtensionDef) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    // Their named types are the extending set's ([`declarations`]).
    let mut types = TsTypes::default();
    for m in def.member_specs(&mut types) {
        for (generics, params, returns) in member_signatures(&m) {
            if !m.doc.is_empty() {
                let _ = writeln!(out, "  /** {} */", m.doc);
            }
            let _ = writeln!(out, "  {}{generics}({params}): {returns};", m.name);
        }
    }
    out
}

/// `press` -> `onPress`.
fn handler_name(event: &str) -> String {
    let mut chars = event.chars();
    match chars.next() {
        Some(first) => format!("on{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => "on".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn by_name_marker_names_the_stand_in_make() {
        let name = std::any::type_name_of_val(&missing_instance);
        assert!(
            name.ends_with(&format!("::{BY_NAME_MARKER}")),
            "{name} is not {BY_NAME_MARKER}: update nsh-cli's by-name detection"
        );
    }

    #[component]
    fn ByName() -> impl IntoElement {
        text("by name")
    }

    #[test]
    fn a_component_made_by_name_has_the_marked_make() {
        let mut element = crate::element::ComponentElement::<ByName>::named(());
        let crate::element::ElKind::Component(c) = &element.node().kind else {
            panic!("not a component element");
        };
        assert!(std::ptr::fn_addr_eq(
            c.make,
            missing_instance as fn() -> Box<dyn AnyInstance>
        ));
    }

    #[derive(Default)]
    struct Stepper;

    /// Picks a number.
    #[derive(Clone, PartialEq, Props)]
    struct StepperProps {
        /// The value.
        value: f64,
        #[prop(default)]
        size: StepperSize,
        max_value: Option<f64>,
        /// Above the control.
        label: Option<Slot>,
        children: Children,
    }

    #[derive(Clone, Copy, PartialEq, Default, PropEnum)]
    enum StepperSize {
        #[default]
        Regular,
        ExtraLarge,
    }

    #[derive(Event)]
    enum StepperEvent {
        /// The value changed.
        Change {
            new_value: f64,
        },
        #[args(limit)]
        Limit(u32),
        Reset,
    }

    struct StepperApi;

    #[methods]
    impl StepperApi {
        /// Jumps to `value`.
        pub(crate) fn jump_to(&self, value: f64) {
            let _ = value;
        }

        pub(crate) fn value(&self) -> f64 {
            0.0
        }

        /// A helper without `pub`: not a method of the ref.
        #[allow(dead_code)]
        fn clamp(&self, value: f64) -> f64 {
            value
        }
    }

    impl Component for Stepper {
        type Props = StepperProps;
        type Event = StepperEvent;
        const METHODS: Methods = Methods::of::<StepperApi>();
        const DOC: &'static str = "Picks a number.";

        fn view(_: StepperProps, _: super::super::Scope<StepperEvent>) -> impl IntoElement {
            view()
        }
    }

    #[test]
    fn the_declarations_come_from_the_types() {
        let mut registry = Registry::new();
        registry.register::<Stepper>();
        let ts = declarations(&registry, &[]);
        for line in [
            "  /** The value. */\n  value: number;",
            "export type StepperSize = \"regular\" | \"extraLarge\";",
            "  size?: StepperSize;",
            "  maxValue?: number | null;",
            "  /** Above the control. */\n  label?: JSX.Element;",
            "  children?: JSX.Element;",
            "  /** The value changed. */\n  onChange?: (event: { newValue: number }) => void;",
            "  onLimit?: (event: { limit: number }) => void;",
            "  onReset?: () => void;",
            "/** Picks a number. */",
            "export interface StepperRef {",
            "  /** Jumps to `value`. */\n  jumpTo(value: number): void;",
            "  value(): Promise<number>;",
            "export interface StepperProps extends Omit<ComponentHostProps, \"ref\"> {\n  ref?: StepperRef | ((ref: StepperRef) => void);",
            "export declare const Stepper: ((props: StepperProps) => JSX.Element);",
        ] {
            assert!(ts.contains(line), "missing {line:?} in\n{ts}");
        }
        assert!(!ts.contains("clamp"), "a helper is not a method:\n{ts}");
        assert_eq!(StepperApi::METHODS.len(), 2);
        let mut props = StepperProps::initial();
        let size = |json| {
            let specs = StepperProps::spec(&mut TsTypes::default());
            let decode = specs
                .iter()
                .find(|p| p.name == "size")
                .unwrap()
                .decode
                .unwrap();
            super::super::json_host::decode(decode, json)
        };
        assert!(
            props
                .set("size", Some(size(serde_json::json!("extraLarge")).unwrap()))
                .unwrap()
        );
        assert!(props.size == StepperSize::ExtraLarge);
        // A value of another type: the conversion fails (JS warns, the prop
        // keeps its value), and so does a decoded value of another prop's
        // type.
        assert!(size(serde_json::json!("huge")).is_err());
        assert!(props.set("size", Some(AnyValue::new(1.0f64))).is_err());
        assert!(props.set("size", None).unwrap());
        assert!(props.size == StepperSize::Regular);
        let data = |e: &StepperEvent| super::super::json_host::event_data(e);
        assert_eq!(
            data(&StepperEvent::Change { new_value: 2.5 }),
            Some(serde_json::json!({ "newValue": 2.5 }))
        );
        assert_eq!(
            data(&StepperEvent::Limit(3)),
            Some(serde_json::json!({ "limit": 3 }))
        );
        assert_eq!(data(&StepperEvent::Reset), None);
        assert_eq!(StepperEvent::Reset.name(), "reset");
    }

    #[test]
    fn exports_without_docs_are_listed() {
        let mut registry = Registry::new();
        registry.register::<Stepper>();
        // `size`, `maxValue`, `children`, `onLimit`, `onReset` and the
        // ref's `value` have no doc comment.
        assert_eq!(
            registry.undocumented(),
            [
                "Stepper.size",
                "Stepper.maxValue",
                "Stepper.children",
                "Stepper.onLimit",
                "Stepper.onReset",
                "Stepper.value",
            ]
        );
        let mut r = Registry::ui_set("test/maker");
        r.module::<Maker>();
        assert_eq!(r.undocumented(), ["Maker.make", "new Lonely"]);
    }

    /// A class no set registers.
    #[crate::element::class]
    struct Lonely;

    #[crate::element::class]
    impl Lonely {
        pub(crate) fn new() -> Self {
            Lonely
        }
    }

    /// Makes a `Lonely`.
    #[crate::element::module]
    struct Maker;

    #[crate::element::module]
    impl Maker {
        pub(crate) fn make() -> Instance<Lonely> {
            Instance::new(Lonely)
        }
    }

    #[test]
    fn the_classes_a_signature_names_are_registered_with_it() {
        let mut r = Registry::ui_set("test/maker");
        r.module::<Maker>();
        // The module's result names `Lonely`: it is the set's, declared and
        // exported.
        let names: Vec<_> = r.class_specs().iter().map(|c| c.name).collect();
        assert_eq!(names, ["Lonely"]);
        let ts = declarations(&r, &[]);
        assert!(ts.contains("export declare class Lonely"), "{ts}");
        assert!(ts.contains("make(): Lonely;"), "{ts}");
        // Registering it again does nothing.
        r.class::<Lonely>();
        assert_eq!(r.class_specs().len(), 1);
        let _ = Registry::merged(&[r]);
    }

    /// Checks.
    #[crate::element::module]
    struct Checks;

    #[crate::element::module(flat)]
    impl Checks {
        /// Declares a check.
        pub(crate) fn check(name: String) {
            let _ = name;
        }
    }

    /// Unused: a module's doc when its functions are on `check`.
    #[crate::element::module]
    struct CheckVariants;

    #[crate::element::module(name = "check")]
    impl CheckVariants {
        /// How many variants there are.
        pub(crate) const COUNT: u32 = 2;

        /// Declares a check that does not run.
        pub(crate) fn skip(name: String) {
            let _ = name;
        }

        pub(crate) fn todo(name: String) {
            let _ = name;
        }
    }

    /// Named like no function of the set: an object of its own.
    #[crate::element::module]
    struct Loose;

    #[crate::element::module(name = "loose")]
    impl Loose {
        /// Nothing.
        pub(crate) fn nothing() {}
    }

    #[test]
    fn a_module_named_like_a_function_puts_its_functions_on_it() {
        let mut r = Registry::ui_set("test/checks");
        r.module::<Checks>()
            .module::<CheckVariants>()
            .module::<Loose>();
        assert_eq!(r.function_members("check").unwrap().name, "check");
        assert!(r.function_members("loose").is_none());
        let ts = declarations(&r, &[]);
        assert!(
            ts.contains(
                "/** Declares a check. */\n\
                 export declare function check(name: string): void;\n\
                 export declare namespace check {\n  \
                 /** Declares a check that does not run. */\n  \
                 function skip(name: string): void;\n  \
                 function todo(name: string): void;\n  \
                 /** How many variants there are. */\n  \
                 const COUNT: number;\n}\n"
            ),
            "{ts}"
        );
        assert!(
            !ts.contains("CheckVariants") && !ts.contains("checkModule"),
            "{ts}"
        );
        assert!(
            ts.contains("export declare const loose: looseModule;"),
            "{ts}"
        );
        // The members are documented as the function's.
        assert_eq!(r.undocumented(), ["check.todo"]);
        let described = super::super::description::SetDescription::of(&r).unwrap();
        let exports: Vec<_> = described
            .exports
            .iter()
            .map(|e| (e.name.as_str(), e.kind))
            .collect();
        assert_eq!(
            exports,
            [
                ("check", super::super::description::ExportKind::Function),
                ("loose", super::super::description::ExportKind::Module)
            ]
        );
    }

    mod clash {
        /// Another class named `Lonely`.
        #[crate::element::class]
        pub(super) struct Lonely;

        #[crate::element::class]
        impl Lonely {
            pub(super) fn new() -> Self {
                Lonely
            }
        }
    }

    #[test]
    fn a_clash_through_a_signature_names_both_types() {
        let mut r = Registry::ui_set("test/maker");
        r.class::<clash::Lonely>();
        // `Maker.make` names the other `Lonely`.
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            r.module::<Maker>();
        }))
        .unwrap_err();
        let message = failed.downcast_ref::<String>().unwrap();
        assert!(
            message.contains("`nsh_core::element::component::tests::clash::Lonely`")
                && message.contains("`nsh_core::element::component::tests::Lonely`"),
            "{message}"
        );
    }

    #[test]
    fn components_are_named_after_their_types() {
        let mut registry = Registry::ui_set("test/ui");
        registry.register::<Stepper>();
        let specs = registry.specs();
        assert_eq!(
            specs[0].name,
            "nsh_core::element::component::tests::Stepper"
        );
        assert_eq!(specs[0].export, "Stepper");
        assert_eq!(export_name("app::picker::Picker<app::Fruit>"), "Picker");
        assert_eq!(export_name("Bare"), "Bare");
    }

    mod other {
        use crate::prelude::*;

        /// Another component exported as `Stepper`.
        #[derive(Default)]
        pub(super) struct Stepper;

        impl Component for Stepper {
            type Props = ();
            type Event = NoEvent;

            fn view(_: (), _: crate::element::Scope<NoEvent>) -> impl IntoElement {
                view()
            }
        }
    }

    #[test]
    #[should_panic(
        expected = "components `nsh_core::element::component::tests::Stepper` and `nsh_core::element::component::tests::other::Stepper` are both exported as `Stepper` in `test/ui`"
    )]
    fn two_components_of_one_set_cannot_share_an_export() {
        Registry::ui_set("test/ui")
            .register::<Stepper>()
            .register::<other::Stepper>();
    }

    #[test]
    #[should_panic(
        expected = "component `nsh_core::element::component::tests::Stepper` is registered twice"
    )]
    fn a_component_is_registered_once() {
        Registry::new().register::<Stepper>().register::<Stepper>();
    }

    /// A set's submodules come with it, under paths of its specifier, and
    /// their components join the core's.
    #[test]
    fn a_sets_submodules_come_with_it() {
        let mut sub = Registry::ui_set("test/set/sub");
        sub.register::<Stepper>();
        let mut set = Registry::ui_set("test/set");
        set.submodule(sub);
        let specifiers: Vec<_> = set.sets().iter().map(|s| s.specifier()).collect();
        assert_eq!(specifiers, [Some("test/set"), Some("test/set/sub")]);
        assert_eq!(Registry::merged(&[set]).specs().len(), 1);
        // Only a path under the set's own specifier.
        let elsewhere = std::panic::catch_unwind(|| {
            Registry::ui_set("test/set").submodule(Registry::ui_set("test/settings"));
        });
        assert!(elsewhere.is_err());
    }

    #[test]
    fn sets_may_share_exports_but_not_components() {
        let mut a = Registry::ui_set("test/a");
        a.register::<Stepper>();
        let mut b = Registry::ui_set("test/b");
        b.register::<other::Stepper>();
        let merged = Registry::merged(&[a.clone(), b]);
        assert_eq!(merged.specs().len(), 2);
    }

    /// The panic of a set served twice with different exports, or of two
    /// sets registering one component.
    fn panic_of(f: impl FnOnce()) -> String {
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_err();
        match failed.downcast::<String>() {
            Ok(message) => *message,
            Err(failed) => (*failed.downcast::<&str>().unwrap()).to_owned(),
        }
    }

    /// A library's set served by an app's (`Registry::serve`): under its
    /// own specifier, with the set serving it, once however many serve it.
    mod serve {
        use super::*;

        /// A library's set, as its crate's `set()` makes it: anew for each
        /// set serving it.
        fn library() -> Registry {
            let mut plot = Registry::ui_set("acme/ui/plot");
            plot.class::<Lonely>();
            let mut set = Registry::ui_set("acme/ui");
            set.register::<Stepper>().module::<Maker>().submodule(plot);
            set
        }

        fn app(specifier: &'static str) -> Registry {
            let mut app = Registry::ui_set(specifier);
            app.serve(library());
            app
        }

        fn specifiers(sets: &[&Registry]) -> Vec<Option<&'static str>> {
            sets.iter().map(|s| s.specifier()).collect()
        }

        #[test]
        fn a_served_set_comes_after_the_sets_submodules() {
            let mut app = Registry::ui_set("app/native");
            app.submodule(Registry::ui_set("app/native/charts"))
                .serve(library());
            assert_eq!(
                specifiers(&app.sets()),
                [
                    Some("app/native"),
                    Some("app/native/charts"),
                    Some("acme/ui"),
                    Some("acme/ui/plot")
                ]
            );
            // The library's components join the core's.
            let merged = Registry::merged(std::slice::from_ref(&app));
            assert_eq!(merged.specs()[0].export, "Stepper");
        }

        #[test]
        fn a_served_set_has_a_specifier_of_its_own() {
            let refused = |set: Registry| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Registry::ui_set("app/native").serve(set);
                }))
                .is_err()
            };
            assert!(refused(Registry::new()));
            assert!(refused(Registry::ui_set("app/native")));
            // A path under it is a submodule's.
            assert!(refused(Registry::ui_set("app/native/charts")));
            assert!(!refused(Registry::ui_set("app/natives")));
            // A set without a specifier (a Rust app's) may serve any.
            Registry::new().serve(library());
        }

        #[test]
        fn a_library_served_by_two_sets_is_served_once() {
            let sets = [app("app/a"), app("app/b")];
            let served = Registry::served_sets(&sets);
            assert_eq!(
                specifiers(&served),
                [
                    Some("app/a"),
                    Some("acme/ui"),
                    Some("acme/ui/plot"),
                    Some("app/b")
                ]
            );
            // Its component once, so no clash; its class once.
            assert_eq!(Registry::merged(&sets).specs().len(), 1);
            let entries: Vec<_> = Registry::served_sets_by_entry(&sets)
                .iter()
                .map(|(entry, set)| (*entry, set.specifier()))
                .collect();
            assert_eq!(
                entries,
                [
                    (0, Some("app/a")),
                    (0, Some("acme/ui")),
                    (0, Some("acme/ui/plot")),
                    (1, Some("app/b"))
                ]
            );
            // A set listed twice is one set.
            let a = app("app/a");
            assert_eq!(Registry::served_sets(&[a.clone(), a]).len(), 3);
        }

        #[test]
        fn two_different_sets_of_one_specifier_are_refused() {
            let mut other = Registry::ui_set("acme/ui");
            other.register::<other::Stepper>();
            let mut b = Registry::ui_set("app/b");
            b.serve(other.clone());
            let message = panic_of(|| {
                Registry::served_sets(&[app("app/a"), b]);
            });
            assert_eq!(
                message,
                "two different UI sets are imported as `acme/ui`, one served by `app/a`, the other served by `app/b`: a specifier is one module (two versions of one library?)"
            );
            // Listed as it is, too.
            let message = panic_of(|| {
                Registry::merged(&[app("app/a"), other]);
            });
            assert!(
                message.contains("one served by `app/a`, the other listed"),
                "{message}"
            );
            // A set exporting more is another set.
            let mut more = library();
            more.module::<Checks>();
            let message = panic_of(|| {
                Registry::served_sets(&[library(), more]);
            });
            assert!(message.contains("imported as `acme/ui`"), "{message}");
        }
    }

    /// Two sets registering one component: refused naming both, by the core
    /// as an engine starts.
    mod duplicate_components {
        use super::*;

        #[test]
        fn two_sets_registering_one_component_are_named() {
            let mut a = Registry::ui_set("test/a");
            a.register::<Stepper>();
            let mut c = Registry::ui_set("acme/ui");
            c.register::<Stepper>();
            let message = panic_of(|| {
                Registry::merged(&[a.clone(), c]);
            });
            assert_eq!(
                message,
                "component `nsh_core::element::component::tests::Stepper` is registered by both `test/a` and `acme/ui`: register it in one set; a library's set is served with `Registry::serve`"
            );
            // A Rust app's components are the app's set.
            let mut root = Registry::new();
            root.register::<Stepper>();
            let message = panic_of(|| {
                Registry::merged(&[root, a]);
            });
            assert!(
                message.contains("registered by both the app's set and `test/a`"),
                "{message}"
            );
        }
    }

    struct NoFake;

    impl crate::fake::Fake for NoFake {
        fn install(&self) {}

        fn call(&self, _: &str, _: &[crate::JsonValue]) -> Result<crate::JsonValue, String> {
            Ok(crate::JsonValue::Null)
        }
    }

    /// A test calls a fake by its name alone: two sets cannot give one name.
    #[test]
    fn fake_names_are_unique_across_sets() {
        let mut a = Registry::ui_set("test/a");
        a.fake("test/mic", Arc::new(NoFake));
        let mut b = Registry::ui_set("test/b");
        b.fake("test/camera", Arc::new(NoFake));
        let names: Vec<_> = Registry::merged(&[a.clone(), b])
            .fakes()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["test/mic", "test/camera"]);
        let mut c = Registry::ui_set("test/c");
        c.fake("test/mic", Arc::new(NoFake));
        let twice =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Registry::merged(&[a, c])));
        let message = *twice.unwrap_err().downcast::<String>().unwrap();
        assert_eq!(message, "two fakes are named `test/mic`, by two UI sets");
    }
}

#[cfg(test)]
mod root_tests {
    use super::Root;
    use crate::{access::Capabilities, prelude::*};

    #[component]
    fn Empty() -> impl IntoElement {
        view()
    }

    #[test]
    fn a_root_reads_its_declared_capabilities_as_packages_are_read() {
        assert_eq!(Root::of::<Empty>().capabilities_ref(), None);
        // What this runtime does not know is not granted.
        let root = Root::of::<Empty>().capabilities_json(r#"{"clipboard":true,"bluetooth":true}"#);
        let expected = Capabilities {
            clipboard: true,
            ..Capabilities::default()
        };
        assert_eq!(root.capabilities_ref(), Some(&expected));
        // A declaration it cannot read grants nothing.
        let root = Root::of::<Empty>().capabilities_json("not json");
        assert_eq!(root.capabilities_ref(), Some(&Capabilities::default()));
    }
}

/// A theme document a set registers: who, its text, and the document
/// read from it.
#[derive(Clone)]
struct RegisteredTheme {
    owner: &'static str,
    text: &'static str,
    document: Arc<nsh_theme::ThemeDocument>,
}

/// Reads `json`, logs its diagnostics, and adds it to `documents` unless it
/// is refused whole or there already.
fn register_theme(
    documents: &mut Arc<Vec<RegisteredTheme>>,
    owner: &'static str,
    json: &'static str,
) {
    if documents.iter().any(|d| d.text == json) {
        return;
    }
    let (document, _) = super::theme::read_document(json);
    if let Some(document) = document {
        Arc::make_mut(documents).push(RegisteredTheme {
            owner,
            text: json,
            document: Arc::new(document),
        });
    }
}
