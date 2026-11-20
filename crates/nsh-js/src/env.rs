//! The surface's environment: one signal per part
//! (window, safe area, keyboard, color scheme, font scale, screen reader,
//! reduced motion, pixel ratio, locale, locale settings, lifecycle, the
//! wake count, the
//! desktop window's state and the displays)
//! written when the platform reports a
//! change, read through read-only accessors; `nsh/device` (`platform`) and
//! `nsh/accessibility`; the layout direction, which follows the user's
//! language; the insets
//! context screens and modals provide (`safeAreaInsets`); the design
//! tokens the app is drawn with (`token()` of `nsh`).

use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_int},
    ptr,
    sync::Arc,
};

use nsh_core::{
    desktop::Display,
    element::{Diagnostic, ScopeModifiers, ThemeDocument, Tokens},
    platform::{Appearance, ColorScheme, Environment, Idiom, Os},
};
use rquickjs::qjs;
use rustc_hash::FxHashMap;

use crate::{
    modules::{Export, Item, Ts},
    reactive::{self, arg},
};

type JsValue = qjs::JSValue;
type JsContext = qjs::JSContext;

const UNDEFINED: JsValue = qjs::JS_UNDEFINED;

/// The parts of the environment, each a signal.
#[derive(Clone, Copy)]
enum Part {
    Window,
    SafeArea,
    Keyboard,
    ColorScheme,
    FontScale,
    ScreenReader,
    ReduceMotion,
    PixelRatio,
    TitleBar,
    Locale,
    Lifecycle,
    LocaleSettings,
    WindowState,
    Displays,
    Fps,
    PerfStats,
    Shaders,
    WakeCount,
    HorizontalSizeClass,
    VerticalSizeClass,
}

const PARTS: usize = 20;

/// The environment's part of a context's state.
pub(crate) struct Env {
    /// What the platform reported last (`None`: nothing yet).
    current: RefCell<Option<Environment>>,
    /// One signal per part (owned).
    signals: [Cell<JsValue>; PARTS],
    /// Read-only accessors of the signals (owned memos).
    accessors: [Cell<JsValue>; PARTS],
    /// What names a `ThemeScope`'s appearance in the context of what it
    /// renders ([`provide_scope`]): an object of its own, made with the
    /// first scope.
    scope_key: Cell<JsValue>,
    /// What names a `ThemeScope`'s design tokens in the context of what it
    /// renders ([`provide_scope_tokens`]), as [`Env::scope_key`] names its
    /// appearance.
    scope_tokens_key: Cell<JsValue>,
    /// The tokens each live `ThemeScope` draws with, by its slot
    /// ([`new_scope_slot`]); `None`: a free slot, or not made yet.
    scope_tokens: RefCell<Vec<Option<Arc<Tokens>>>>,
    /// Set on every change of the surface's tokens, for the scopes: a
    /// scope's tokens are made over all of them, whichever `token()` reads
    /// by name. Made with the first scope.
    scopes_signal: Cell<JsValue>,
    /// The slots of scopes that are gone, to use again.
    scope_free: RefCell<Vec<usize>>,
    /// Counts the times a scope's tokens were made: its memo's value is new
    /// each time, so what read `token()` in it runs again.
    scope_versions: Cell<u32>,
    /// The app's appearance (`nsh.json`, `setColorScheme`, Rust's
    /// `set_color_scheme`).
    appearance: Cell<Appearance>,
    /// An appearance Rust set inside the synchronous call from JS running
    /// now (`set_color_scheme`, whose op went with the call's turn), shown
    /// when the call returns ([`show_appearance_set_in_call`]).
    appearance_in_call: Cell<Option<Appearance>>,
    /// The insets context (owned): screens and modals provide theirs.
    insets: Cell<JsValue>,
    /// The design tokens the app is drawn with, as the core resolved them
    /// ([`set_tokens`]).
    tokens: RefCell<Arc<Tokens>>,
    /// A signal of their version (owned), which every `token()` reads:
    /// set when most of the tokens read changed (a new theme).
    tokens_signal: Cell<JsValue>,
    /// A signal of the tokens' shape (owned): set when their names, their
    /// kinds or the modifiers' contexts change, which `tokens.names`,
    /// `entries`, `kind` and `modifier()` read.
    shape_signal: Cell<JsValue>,
    /// A signal (owned) that what reads a name past [`TOKEN_SIGNALS`] reads
    /// in place of a signal of its own (a name no theme has, of the many an
    /// app made up): set on every change of the tokens.
    overflow_signal: Cell<JsValue>,
    /// Whether a name was read past [`TOKEN_SIGNALS`] ([`Env::overflow_signal`]).
    overflowed: Cell<bool>,
    tokens_version: Cell<u32>,
    /// A signal per token `token()` read (owned), set when that token's
    /// value changes and few do: what read a token follows that token,
    /// and a change of another one runs nothing of it.
    token_signals: RefCell<FxHashMap<Box<str>, JsValue>>,
    /// The document `setTheme(null)` gives back: the app's built-in one
    /// (`nsh.json`'s `"theme"`), else the UI sets' the core resolved with
    /// while no `setTheme` document was active (`None`: none known yet).
    base_document: RefCell<Option<Arc<ThemeDocument>>>,
    /// Whether a `setTheme` document is active (until `setTheme(null)`).
    runtime_theme: Cell<bool>,
    /// The document a write of this turn activated (the bundle's setup,
    /// `setTheme`), until the core hands tokens resolved with it: what
    /// `setModifier` and `updateTheme` are checked against meanwhile.
    turn_document: RefCell<Option<Arc<ThemeDocument>>>,
    /// The displays the platform reported last.
    displays: RefCell<Vec<Display>>,
    /// What `appInfo()` reports: the build's ([`set_app_info`]).
    app: RefCell<AppInfo>,
}

/// The app as it was built (`appInfo()`), from the JSON the bundler writes.
#[derive(Default, serde::Deserialize)]
#[serde(default)]
struct AppInfo {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    build: Option<f64>,
    development: bool,
}

impl Default for Env {
    fn default() -> Self {
        Env {
            current: RefCell::new(None),
            signals: std::array::from_fn(|_| Cell::new(UNDEFINED)),
            accessors: std::array::from_fn(|_| Cell::new(UNDEFINED)),
            scope_key: Cell::new(UNDEFINED),
            scope_tokens_key: Cell::new(UNDEFINED),
            scope_tokens: RefCell::default(),
            scopes_signal: Cell::new(UNDEFINED),
            scope_free: RefCell::default(),
            scope_versions: Cell::new(0),
            appearance: Cell::new(Appearance::System),
            appearance_in_call: Cell::new(None),
            insets: Cell::new(UNDEFINED),
            tokens: RefCell::new(Arc::default()),
            tokens_signal: Cell::new(UNDEFINED),
            shape_signal: Cell::new(UNDEFINED),
            overflow_signal: Cell::new(UNDEFINED),
            overflowed: Cell::new(false),
            tokens_version: Cell::new(0),
            token_signals: RefCell::default(),
            base_document: RefCell::default(),
            runtime_theme: Cell::new(false),
            turn_document: RefCell::default(),
            displays: RefCell::default(),
            app: RefCell::default(),
        }
    }
}

/// Takes in the app's information as the build wrote it (`setup`'s fifth
/// argument); what this runtime cannot read is left out.
pub(crate) fn set_app_info(cx: *mut JsContext, json: &str) {
    match serde_json::from_str(json) {
        Ok(info) => *env(cx).app.borrow_mut() = info,
        Err(e) => log::warn!("appInfo: {e}, ignored"),
    }
}

/// `appInfo()`: the app as it was built.
unsafe extern "C" fn app_info(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
) -> JsValue {
    let app = env(cx).app.borrow();
    let text = |value: &Option<String>| match value {
        Some(value) => string(cx, value),
        None => qjs::JS_NULL,
    };
    object(
        cx,
        &[
            (c"id", text(&app.id)),
            (c"name", text(&app.name)),
            (c"version", text(&app.version)),
            (c"build", app.build.map_or(qjs::JS_NULL, qjs::JS_NewFloat64)),
            (c"development", boolean(app.development)),
        ],
    )
}

/// `appInfo` of `nsh`.
pub(crate) const APP_INFO_EXPORT: crate::modules::Item = (
    "appInfo",
    Export::Function(app_info, 0),
    Ts::function(
        "The app as it was built: its id, name, version and build number\n(`nsh.json`'s; a package's as `nsh package` and `nsh publish` name and\nversion it, `--id`, `--name`, `--version` and `--build` included, with\nversion `1.0.0` and build `1` where nothing gives them), and whether it\nis a development build (`nsh run`, `nsh dev`, `nsh test`, `nsh build`\nwithout `--release`). `nsh/updater`'s `shellInfo()` reports the\ninstalled app's.",
        &["(): AppInfo"],
    ),
);

fn env<'a>(cx: *mut JsContext) -> &'a Env {
    // SAFETY: a projection of the state's field, reached by shared
    // reference only.
    unsafe { &(*crate::state::state(cx)).env }
}

impl Env {
    /// Frees the values the environment holds (the state is unregistered).
    pub(crate) fn free_all(&mut self, raw: *mut JsContext) {
        // SAFETY: the environment's own values; no JS runs from here on.
        unsafe {
            for c in self.signals.iter().chain(&self.accessors) {
                qjs::JS_FreeValue(raw, c.replace(UNDEFINED));
            }
            qjs::JS_FreeValue(raw, self.insets.replace(UNDEFINED));
            qjs::JS_FreeValue(raw, self.scope_key.replace(UNDEFINED));
            qjs::JS_FreeValue(raw, self.scope_tokens_key.replace(UNDEFINED));
            qjs::JS_FreeValue(raw, self.scopes_signal.replace(UNDEFINED));
            qjs::JS_FreeValue(raw, self.tokens_signal.replace(UNDEFINED));
            qjs::JS_FreeValue(raw, self.shape_signal.replace(UNDEFINED));
            qjs::JS_FreeValue(raw, self.overflow_signal.replace(UNDEFINED));
            for (_, signal) in self.token_signals.get_mut().drain() {
                qjs::JS_FreeValue(raw, signal);
            }
        }
    }
}

/// Makes the signals and accessors (after the graph and the renderer).
pub(crate) fn init(cx: *mut JsContext) {
    let e = env(cx);
    for i in 0..PARTS {
        // The displays are an empty list until the platform reports any
        // (off desktops, for good).
        let initial = if i == Part::Displays as usize {
            // SAFETY: a new array, borrowed by the signal and freed after.
            unsafe { qjs::JS_NewArray(cx) }
        } else if i == Part::Fps as usize {
            // No performance monitor until the platform installs one.
            boolean(false)
        } else if i == Part::PerfStats as usize {
            // No stats until the monitor measured a second.
            qjs::JS_NULL
        } else {
            UNDEFINED
        };
        let signal = reactive::new_signal(cx, initial);
        // SAFETY: the signal holds its own reference.
        unsafe { qjs::JS_FreeValue(cx, initial) };
        let mut data = [signal];
        // SAFETY: the function keeps a dup of the signal.
        let func = unsafe { qjs::JS_NewCFunctionData(cx, Some(read), 0, 0, 1, data.as_mut_ptr()) };
        e.signals[i].set(signal);
        e.accessors[i].set(reactive::memo(cx, func, false));
    }
    // The insets of the nearest screen or modal: the surface's safe area
    // outside of them.
    let safe_area = e.accessors[Part::SafeArea as usize].get();
    let mut args = [safe_area];
    // SAFETY: a context whose default is the accessor (borrowed).
    let insets = unsafe { crate::dom::context(cx, UNDEFINED, 1, args.as_mut_ptr()) };
    e.insets.set(insets);
    e.tokens_signal.set(reactive::new_signal(cx, number(0.0)));
    e.shape_signal.set(reactive::new_signal(cx, number(0.0)));
    e.overflow_signal.set(reactive::new_signal(cx, number(0.0)));
}

/// How many names no theme has get a signal of their own
/// ([`Env::token_signals`]): a theme's names always do (a design tool's
/// collection has thousands); past this many made-up names (an app's
/// without end), what reads another one follows every change of the tokens
/// ([`Env::overflow_signal`]).
const TOKEN_SIGNALS: usize = 1024;

/// Takes in the design tokens the core draws the app with (after a theme
/// change, the host's included): what read a token whose value changed
/// runs again ([`install`]). While no `setTheme` document is active, their
/// document is the one `setTheme(null)` gives back.
pub(crate) fn set_tokens(cx: *mut JsContext, tokens: Arc<Tokens>) {
    let e = env(cx);
    e.turn_document.replace(None);
    if !e.runtime_theme.get()
        && let Some(document) = tokens.active_document()
    {
        e.base_document.replace(Some(document.clone()));
    }
    install(cx, tokens);
}

/// Makes `tokens` what `token()` reads: what read a token whose value
/// changed runs again, once however many of the tokens it read changed.
///
/// A change of few of the tokens read (`updateTheme` of a color) sets
/// their own signals. A change of most of them (a new theme) sets the one
/// signal of all tokens in their place: nearly every reader runs either
/// way, and one signal runs them in the order they were made, the tree's,
/// where the names' signals would run them name by name.
fn install(cx: *mut JsContext, tokens: Arc<Tokens>) {
    let e = env(cx);
    let old = e.tokens.replace(tokens.clone());
    let values = *old != *tokens;
    // The names, their kinds or the modifiers (their contexts, the ones
    // shown) changed: what lists them reads them again.
    let shape = !old.same_names(&tokens) || old.modifiers() != tokens.modifiers();
    if !values && !shape {
        return;
    }
    let version = e.tokens_version.get().wrapping_add(1);
    e.tokens_version.set(version);
    // What `token()` reads of a name: its value. A name that came or went
    // differs too.
    let differs = |name: &str| old.value(name) != tokens.value(name);
    // Collected first: a binding running again may read a new name.
    let (changed, names): (Vec<JsValue>, usize) = if values {
        let signals = e.token_signals.borrow();
        let changed = signals
            .iter()
            .filter(|(name, _)| differs(name))
            .map(|(_, signal)| *signal)
            .collect();
        (changed, signals.len())
    } else {
        (Vec::new(), 0)
    };
    // The scopes' tokens are made over these, whatever changed.
    let scopes = e.scopes_signal.get();
    // SAFETY: a value the environment owns.
    let scoped = !unsafe { qjs::JS_IsUndefined(scopes) };
    // Roles mapped onto other tokens: what read a role reads another one.
    let whole = values && (2 * changed.len() > names || !old.same_roles(&tokens));
    if whole && !scoped && !shape {
        return reactive::set_signal(cx, e.tokens_signal.get(), number(version as f32));
    }
    reactive::batch_with(cx, || {
        if shape {
            reactive::set_signal(cx, e.shape_signal.get(), number(version as f32));
        }
        if whole {
            reactive::set_signal(cx, e.tokens_signal.get(), number(version as f32));
        } else if values {
            for signal in changed {
                reactive::set_signal(cx, signal, number(version as f32));
            }
            if e.overflowed.get() {
                reactive::set_signal(cx, e.overflow_signal.get(), number(version as f32));
            }
        }
        if scoped {
            reactive::set_signal(cx, scopes, number(version as f32));
        }
    });
}

/// The signal of token `name` (borrowed), made the first time the name is
/// read: a theme's name always has one, a name no theme has none past
/// [`TOKEN_SIGNALS`] names, which reads [`Env::overflow_signal`] then.
fn token_signal(cx: *mut JsContext, e: &Env, name: &str) -> JsValue {
    if let Some(signal) = e.token_signals.borrow().get(name) {
        return *signal;
    }
    // A theme's name: the tokens', or the document a write of this turn
    // activated (the app's own, as the entry starts).
    let named = e.tokens.borrow().kind_name(name).is_some()
        || e.turn_document
            .borrow()
            .as_ref()
            .is_some_and(|document| document.tokens.get(name).is_some());
    if e.token_signals.borrow().len() >= TOKEN_SIGNALS && !named {
        e.overflowed.set(true);
        return e.overflow_signal.get();
    }
    let signal = reactive::new_signal(cx, number(e.tokens_version.get() as f32));
    e.token_signals.borrow_mut().insert(name.into(), signal);
    signal
}

/// Takes in the app's theme as the bundle ships it (`nsh:rt`'s `setup`,
/// which the entry's first import runs before any module of the app:
/// `{ document, overrides }`, `nsh.json`'s `"theme"` and
/// `"themeOverrides"`): the core takes them with this turn's ops, and
/// `token()` reads them from here on, as the core installs them
/// (`Tokens::with_app_theme`): its own install after the turn gives the
/// same tokens. The built-in document is the one `setTheme(null)` gives
/// back.
pub(crate) fn take_app_theme(cx: *mut JsContext, json: &str) -> Result<(), JsValue> {
    let e = env(cx);
    let (document, overrides, _) = nsh_core::element::theme::read_app_theme(json);
    if document.is_none() && overrides.is_none() {
        return Ok(());
    }
    let document = document.map(Arc::new);
    let overrides = overrides.map(Arc::new);
    if let Some(document) = &document {
        e.base_document.replace(Some(document.clone()));
        e.turn_document.replace(Some(document.clone()));
    }
    // What the entry reads from here on: the core's install of them over
    // the tokens it handed at start (`Tokens::with_app_theme`), which its
    // own install, after this turn, gives again, so nothing reads twice.
    if !e.runtime_theme.get() {
        let current = e.tokens.borrow().clone();
        install(
            cx,
            current.with_app_theme(document.as_ref(), overrides.as_ref()),
        );
    }
    let prop = nsh_core::Prop::UiAppTheme(document, overrides);
    write_theme(cx, prop)
}

/// Hands the core theme write `prop`, on the root, with this turn's ops:
/// it resolves the theme once for all of them, and `token()` reads the
/// result when the core hands the tokens on ([`set_tokens`]).
fn write_theme(cx: *mut JsContext, prop: nsh_core::Prop) -> Result<(), JsValue> {
    crate::dom::push_op(
        cx,
        nsh_core::Op::SetProp {
            id: nsh_core::ROOT_ID,
            prop,
        },
    )
}

/// The document this turn's writes are checked against: the one a write
/// of this turn activated, else the one the tokens were resolved with.
fn active_document(e: &Env) -> Arc<ThemeDocument> {
    let turn = e.turn_document.borrow().clone();
    turn.or_else(|| e.tokens.borrow().active_document().cloned())
        .unwrap_or_default()
}

/// `diagnostics` as `setTheme` and `updateTheme` return them (owned): an
/// array of `ThemeDiagnostic`s.
fn diagnostics(cx: *mut JsContext, diagnostics: &[Diagnostic]) -> JsValue {
    crate::host::json_to_js(cx, &nsh_core::element::theme::diagnostics_json(diagnostics))
}

/// `setTheme(theme)`: theme document `theme` is the app's from now on, in
/// place of its built-in one (`nsh.json`'s `"theme"`), else the UI sets';
/// `null` gives that one back. The core activates it with this turn's ops,
/// and `token()` reads its tokens once the core hands them on. Returns the
/// read's diagnostics and the role checks of its mappings; a text that is
/// no document, or one in the old format (`nsh theme convert` converts
/// it), keeps the active document.
unsafe extern "C" fn set_theme(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let e = env(cx);
    let v = arg(argc, argv, 0);
    // SAFETY: inline tag checks.
    let nullish = unsafe { qjs::JS_IsNull(v) || qjs::JS_IsUndefined(v) };
    if nullish {
        e.runtime_theme.set(false);
        let base = e.base_document.borrow().clone();
        e.turn_document.replace(base);
        if let Err(err) = write_theme(cx, nsh_core::Prop::UiThemeDocument(None)) {
            return err;
        }
        return reactive::finish_value(cx, diagnostics(cx, &[]));
    }
    // SAFETY: an inline tag check.
    if !unsafe { qjs::JS_IsObject(v) } {
        return type_error(
            cx,
            "setTheme: expected a theme document (an object) or null",
        );
    }
    let json = match to_json_text(cx, v) {
        Ok(json) => json,
        Err(exception) => return exception,
    };
    let (document, mut said) = nsh_core::element::theme::read_document(&json);
    if let Some(document) = document {
        let document = Arc::new(document);
        said.extend(e.tokens.borrow().check_document(&document));
        e.runtime_theme.set(true);
        e.turn_document.replace(Some(document.clone()));
        if let Err(err) = write_theme(cx, nsh_core::Prop::UiThemeDocument(Some(document))) {
            return err;
        }
    }
    reactive::finish_value(cx, diagnostics(cx, &said))
}

/// `v` as JSON (a theme, a list of names), walked in Rust; `Err`: the
/// exception thrown.
fn to_json(cx: *mut JsContext, v: JsValue) -> Result<serde_json::Value, JsValue> {
    // SAFETY: a context reference and a dup for the converter.
    let value = unsafe {
        let rcx = rquickjs::Ctx::from_raw(ptr::NonNull::new_unchecked(cx));
        rquickjs::Value::from_raw(rcx, qjs::JS_DupValue(cx, v))
    };
    crate::protocol::to_json(&value).map_err(|_| qjs::JS_EXCEPTION)
}

/// `v` as JSON text (a theme document or patch), walked in Rust with each
/// object's members in the order JS gives them, which a document keeps
/// (a modifier's contexts); `Err`: the exception thrown.
fn to_json_text(cx: *mut JsContext, v: JsValue) -> Result<String, JsValue> {
    // SAFETY: a context reference and a dup for the converter.
    let value = unsafe {
        let rcx = rquickjs::Ctx::from_raw(ptr::NonNull::new_unchecked(cx));
        rquickjs::Value::from_raw(rcx, qjs::JS_DupValue(cx, v))
    };
    crate::protocol::to_json_text(&value).map_err(|_| qjs::JS_EXCEPTION)
}

/// `updateTheme(patch)`: theme patch `patch` (`{ values, roles }`) is
/// merged into the app's one patch, a later entry replacing an earlier
/// one, with this turn's ops; `token()` reads the result once the core
/// hands it on. Returns its diagnostics: what reading it and the active
/// document refuse, and the role checks of its mappings.
unsafe extern "C" fn update_theme(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let v = arg(argc, argv, 0);
    // SAFETY: an inline tag check.
    if !unsafe { qjs::JS_IsObject(v) } {
        return type_error(cx, "updateTheme: expected a theme patch (an object)");
    }
    let json = match to_json_text(cx, v) {
        Ok(json) => json,
        Err(exception) => return exception,
    };
    let e = env(cx);
    let (patch, mut said) = nsh_core::element::theme::read_patch(&json);
    said.extend(e.tokens.borrow().check_patch(&active_document(e), &patch));
    if let Err(err) = write_theme(cx, nsh_core::Prop::UiThemeUpdate(Arc::new(patch))) {
        return err;
    }
    reactive::finish_value(cx, diagnostics(cx, &said))
}

/// `resetTheme(names?)`: the app's overrides of the tokens and roles
/// `names` (none given: every one) go, with this turn's ops, so they read
/// the layer below.
unsafe extern "C" fn reset_theme(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    const EXPECTED: &str = "resetTheme: expected token names (string[]) or nothing";
    let v = arg(argc, argv, 0);
    // SAFETY: inline tag checks.
    let names = if unsafe { qjs::JS_IsNull(v) || qjs::JS_IsUndefined(v) } {
        None
    } else {
        let json = match to_json(cx, v) {
            Ok(json) => json,
            Err(exception) => return exception,
        };
        let Some(list) = json.as_array() else {
            return type_error(cx, EXPECTED);
        };
        let names: Option<Vec<Box<str>>> =
            list.iter().map(|n| n.as_str().map(Into::into)).collect();
        match names {
            Some(names) => Some(names),
            None => return type_error(cx, EXPECTED),
        }
    };
    if let Err(err) = write_theme(cx, nsh_core::Prop::UiThemeReset(names)) {
        return err;
    }
    reactive::finish_value(cx, UNDEFINED)
}

/// `setModifier(name, context)`: the active document's modifier `name`
/// shows context `context`, with this turn's ops; `token()` reads its
/// values once the core hands them on. The document is the one active in
/// this turn, a `setTheme` of it included: a modifier or context it lacks
/// throws a `TypeError` naming it, and nothing changes.
unsafe extern "C" fn set_modifier(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    const EXPECTED: &str = "setModifier: expected a modifier's name and a context's (strings)";
    let name = crate::dom::string_arg(cx, arg(argc, argv, 0));
    let context = crate::dom::string_arg(cx, arg(argc, argv, 1));
    let (Some(name), Some(context)) = (name, context) else {
        return type_error(cx, EXPECTED);
    };
    let document = active_document(env(cx));
    match nsh_core::element::theme::unknown_pick(&document, &name, &context) {
        Some(said) if said.code == nsh_theme::code::UNKNOWN_MODIFIER => {
            return type_error(
                cx,
                &format!("setModifier: the theme has no modifier `{name}`"),
            );
        }
        Some(_) => {
            return type_error(
                cx,
                &format!("setModifier: the modifier `{name}` has no context `{context}`"),
            );
        }
        None => {}
    }
    let prop = nsh_core::Prop::UiModifier(name.into(), context.into());
    if let Err(err) = write_theme(cx, prop) {
        return err;
    }
    reactive::finish_value(cx, UNDEFINED)
}

/// The tokens of the scope around what reads them, else the surface's,
/// their shape read tracked (the names, kinds and modifiers' contexts).
fn tracked_tokens(cx: *mut JsContext) -> Result<Arc<Tokens>, JsValue> {
    let e = env(cx);
    let scoped = scope_tokens(cx)?;
    if scoped.is_none() {
        reactive::track(cx, e.shape_signal.get());
    }
    Ok(scoped.unwrap_or_else(|| e.tokens.borrow().clone()))
}

/// `modifier(name)`: the context the theme's modifier `name` has now.
unsafe extern "C" fn modifier(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let name = crate::dom::string_arg(cx, arg(argc, argv, 0));
    let tokens = match tracked_tokens(cx) {
        Ok(tokens) => tokens,
        Err(exception) => return exception,
    };
    let selected = name.and_then(|name| tokens.modifier(&name).map(str::to_owned));
    match selected {
        Some(context) => string(cx, &context),
        None => UNDEFINED,
    }
}

/// `modifiers()`: the active document's modifiers in `resolutionOrder`,
/// each with its contexts, its default and the context it shows.
unsafe extern "C" fn modifiers(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
) -> JsValue {
    let tokens = match tracked_tokens(cx) {
        Ok(tokens) => tokens,
        Err(exception) => return exception,
    };
    let list: Vec<serde_json::Value> = tokens
        .modifiers()
        .into_iter()
        .map(|axis| {
            serde_json::json!({
                "name": &*axis.name,
                "contexts": axis.contexts.iter().map(|c| &**c).collect::<Vec<_>>(),
                "default": &*axis.default,
                "selected": &*axis.selected,
            })
        })
        .collect();
    crate::host::json_to_js(cx, &serde_json::Value::Array(list))
}

/// `tokens.get(name)`: token `name` as `{ kind, value }`, its value in the
/// canonical form (`Tokens::value_json`), tracked as `token()` tracks it.
unsafe extern "C" fn tokens_get(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let name = crate::dom::string_arg(cx, arg(argc, argv, 0));
    let e = env(cx);
    let scoped = match scope_tokens(cx) {
        Ok(scoped) => scoped,
        Err(exception) => return exception,
    };
    if scoped.is_none() {
        reactive::track(cx, e.tokens_signal.get());
        if let Some(name) = name.as_deref() {
            reactive::track(cx, token_signal(cx, e, name));
        }
    }
    let tokens = scoped.unwrap_or_else(|| e.tokens.borrow().clone());
    match name.and_then(|name| tokens.value_json(&name)) {
        Some(value) => crate::host::json_to_js(cx, &value),
        None => UNDEFINED,
    }
}

/// `tokens.kind(name)`: the kind of token `name` (`Tokens::kind_name`).
unsafe extern "C" fn tokens_kind(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let name = crate::dom::string_arg(cx, arg(argc, argv, 0));
    let tokens = match tracked_tokens(cx) {
        Ok(tokens) => tokens,
        Err(exception) => return exception,
    };
    match name.and_then(|name| tokens.kind_name(&name)) {
        Some(kind) => string(cx, kind),
        None => UNDEFINED,
    }
}

/// `tokens.names(filter?)`: every token's name, sorted, of the kind and with
/// the prefix `filter` gives.
unsafe extern "C" fn tokens_names(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let filter = match to_json(cx, arg(argc, argv, 0)) {
        Ok(filter) => filter,
        Err(exception) => return exception,
    };
    let text = |key: &str| {
        filter
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let (kind, prefix) = (text("kind"), text("prefix"));
    let tokens = match tracked_tokens(cx) {
        Ok(tokens) => tokens,
        Err(exception) => return exception,
    };
    let names: Vec<serde_json::Value> = tokens
        .infos()
        .into_iter()
        .filter(|info| kind.as_deref().is_none_or(|k| info.kind == k))
        .filter(|info| prefix.as_deref().is_none_or(|p| info.name.starts_with(p)))
        .map(|info| (*info.name).into())
        .collect();
    crate::host::json_to_js(cx, &serde_json::Value::Array(names))
}

/// `tokens.entries()`: every token, sorted by name, with its kind and what
/// declares it (`Tokens::infos`).
unsafe extern "C" fn tokens_entries(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
) -> JsValue {
    let tokens = match tracked_tokens(cx) {
        Ok(tokens) => tokens,
        Err(exception) => return exception,
    };
    let list = tokens.infos().iter().map(|info| info.to_json()).collect();
    crate::host::json_to_js(cx, &serde_json::Value::Array(list))
}

/// The `tokens` object of `nsh`: the token dictionary by name.
fn make_tokens(cx: *mut JsContext) -> JsValue {
    use crate::web::js;
    let made = (|| -> js::R<js::V> {
        let obj = js::new_object(cx)?;
        for (name, key, f, length) in [
            (c"get", "get", tokens_get as js::Native, 1),
            (c"kind", "kind", tokens_kind as js::Native, 1),
            (c"names", "names", tokens_names as js::Native, 1),
            (c"entries", "entries", tokens_entries as js::Native, 0),
        ] {
            let function = js::function(cx, name, f, length)?;
            js::define(cx, obj.raw(), key, function, true)?;
        }
        Ok(obj)
    })();
    made.map_or(qjs::JS_EXCEPTION, js::V::take)
}

/// `tokens` of `nsh`.
pub(crate) const TOKENS_EXPORT: crate::modules::Item = (
    "tokens",
    Export::Value(make_tokens),
    Ts::value(
        "The app's design tokens by name, reactive, for code that does not know\ntheir names ahead (an editor, a tool, an agent): `get` a token as\n`{ kind, value }` (its value as `token()` gives it), `kind` of a token,\n`names` of every token (of a kind, with a prefix), `entries` with each\none's kind and what the document declares of it.",
        "TokenDictionary",
    ),
);

/// `setModifier` of `nsh`.
pub(crate) const SET_MODIFIER_EXPORT: crate::modules::Item = (
    "setModifier",
    Export::Function(set_modifier, 2),
    Ts::function(
        "Shows the context `context` (`\"forest\"`) of the active theme document's\nmodifier `name` (`\"brand\"`): the tokens its contexts set take that\ncontext's values with the turn's other theme writes: `token()`, classes\nnaming tokens, `nsh/base` and Rust components read them in the frame\nafter the turn, a read in the same turn the values before it. Overrides\n(`updateTheme`) still win over a context. A modifier or context the active\ndocument lacks (a `setTheme` of this turn counts) throws a `TypeError`\nnaming it, and nothing changes.",
        &["<M extends keyof ThemeModifiers>(name: M, context: ThemeModifiers[M]): void"],
    ),
);

/// `modifier` of `nsh`.
pub(crate) const MODIFIER_EXPORT: crate::modules::Item = (
    "modifier",
    Export::Function(modifier, 1),
    Ts::function(
        "The context the active theme document's modifier `name` shows, reactive:\nthe one picked (`setModifier`), else its default; `undefined` when the\ndocument has no such modifier.",
        &["<M extends keyof ThemeModifiers>(name: M): ThemeModifiers[M] | undefined"],
    ),
);

/// `modifiers` of `nsh`.
pub(crate) const MODIFIERS_EXPORT: crate::modules::Item = (
    "modifiers",
    Export::Function(modifiers, 0),
    Ts::function(
        "The active theme document's modifiers in its `resolutionOrder`, reactive:\neach one's name, its contexts, the context it shows when none is picked\n(`default`) and the one it shows (`selected`), typed by `ThemeModifiers`:
an entry's name and contexts go to `setModifier` as they are.",
        &["(): ModifierAxes[]"],
    ),
);

/// Throws a `TypeError` of `message`.
fn type_error(cx: *mut JsContext, message: &str) -> JsValue {
    let message = std::ffi::CString::new(message.replace('\0', "")).unwrap_or_default();
    // SAFETY: throws on a live context; `%s` reads the C string.
    unsafe { qjs::JS_ThrowTypeError(cx, c"%s".as_ptr(), message.as_ptr()) }
}

/// `setTheme` of `nsh`.
pub(crate) const SET_THEME_EXPORT: crate::modules::Item = (
    "setTheme",
    Export::Function(set_theme, 1),
    Ts::function(
        "Makes theme document `theme` (its tokens with their value types, its\nmodifiers and its roles) the app's while it runs, in place of its built-in\none (`nsh.json`'s `\"theme\"`), else the UI sets', with the turn's other\ntheme writes: `token()`, classes naming tokens, `nsh/base` and Rust\ncomponents read it in the frame after the turn. `null` gives the built-in\none back. The modifier contexts picked stay picked where the new document\nhas them, and the app's overrides (`updateTheme`) stay, those\nfor names it lacks left out while it is active. A bad entry is left out;\nwhat it returns names each one, with the role checks. A text that is no\ndocument, or one of the old `cssVars` format (`nsh theme convert`\nconverts it), keeps the active document and returns its diagnostic.\nAnything but an object or `null` throws a `TypeError`.",
        &["(theme: Theme | null): ThemeDiagnostic[]"],
    ),
);

/// `updateTheme` of `nsh`.
pub(crate) const UPDATE_THEME_EXPORT: crate::modules::Item = (
    "updateTheme",
    Export::Function(update_theme, 1),
    Ts::function(
        "Merges theme patch `patch` into the app's overrides while it runs, a\nlater entry replacing an earlier one for the same token or role\n(a value refused still replaces an earlier one, as a patch is kept as\ngiven), with the turn's other theme writes: `token()`, classes naming\ntokens, `nsh/base` and Rust components read the result in the frame after\nthe turn. Overrides win over the\nmodifier contexts picked, and an embedding app's still win over them. A\nvalue for a name the active document doesn't declare, or one that doesn't\nfit its type, is left out (and applies once a document that declares it is\nactive); a role mapped onto a token of a type it doesn't read is left out,\nand the role reads the layer below. What it returns names each. Anything\nbut an object throws a `TypeError`.",
        &["(patch: ThemePatch): ThemeDiagnostic[]"],
    ),
);

/// `resetTheme` of `nsh`.
pub(crate) const RESET_THEME_EXPORT: crate::modules::Item = (
    "resetTheme",
    Export::Function(reset_theme, 1),
    Ts::function(
        "Removes the tokens and roles `names` from the app's overrides\n(`nsh.json`'s `\"themeOverrides\"` and what `updateTheme` merged), so they\nread the layer below: the picked context's value, else the active\ndocument's; without names, every override. A reset never copies values.",
        &["(names?: (keyof Tokens | keyof Roles)[]): void"],
    ),
);

/// `token(name)`: the design token `name` in the appearance shown (a
/// color as `#rrggbbaa`, a number, a font family), `undefined` when there
/// is none. Reading it tracks that token ([`token_signal`]), the signal
/// of all tokens and the appearance.
unsafe extern "C" fn token(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    read_token(cx, crate::dom::string_arg(cx, arg(argc, argv, 0)), false)
}

/// `role(name)`: what the theme role `name` draws with, as `token()` gives
/// a token: the token a theme maps the role onto, else the token of the
/// role's name (`Tokens::role`, by name). Reading it tracks both tokens.
unsafe extern "C" fn role(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    read_token(cx, crate::dom::string_arg(cx, arg(argc, argv, 0)), true)
}

/// The token `name` (`role`: the one the role `name` reads) in the
/// appearance shown, tracked.
fn read_token(cx: *mut JsContext, name: Option<String>, role: bool) -> JsValue {
    let e = env(cx);
    // The tokens of the scope around what reads them (tracked as one: a
    // change of any runs it again), else the surface's, by name.
    let scoped = match scope_tokens(cx) {
        Ok(scoped) => scoped,
        Err(exception) => return exception,
    };
    let tracked = scoped.is_none();
    let tokens = scoped.unwrap_or_else(|| e.tokens.borrow().clone());
    // A role reads the token it resolves to (`Tokens::role_json`).
    let mapped = name
        .as_deref()
        .filter(|_| role)
        .and_then(|name| tokens.mapped(name))
        .map(str::to_owned);
    if tracked {
        reactive::track(cx, e.tokens_signal.get());
        for name in [name.as_deref(), mapped.as_deref()].into_iter().flatten() {
            reactive::track(cx, token_signal(cx, e, name));
        }
    }
    let Some(name) = name else {
        return UNDEFINED;
    };
    let value = if role {
        tokens.role_json(&name)
    } else {
        tokens.json(&name)
    };
    match value {
        Some(json) => token_value(cx, json),
        None => UNDEFINED,
    }
}

/// A token's canonical JSON form (`Tokens::json`) as JS (owned), a scalar
/// made at once.
fn token_value(cx: *mut JsContext, json: serde_json::Value) -> JsValue {
    match json {
        serde_json::Value::String(s) => string(cx, &s),
        serde_json::Value::Number(n) => qjs::JS_NewFloat64(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::Bool(b) => boolean(b),
        json => crate::host::json_to_js(cx, &json),
    }
}

/// `role` of `nsh`.
pub(crate) const ROLE_EXPORT: crate::modules::Item = (
    "role",
    Export::Function(role, 1),
    Ts::function(
        "What the theme role `name` draws with, reactive, as `token()` gives a\ntoken: the token the app's overrides or its active theme document map the\nrole onto (`\"roles\"`), else the token of the role's name, else\n`undefined`. A component written against a role is themed by any design\nsystem: `role(\"popover\")` is `surface-overlay`'s color where the theme\nmaps it there. The names are the roles the app's UI sets list.",
        &["<K extends keyof Roles>(name: K): Roles[K] | undefined"],
    ),
);

/// `token` of `nsh`.
pub(crate) const TOKEN_EXPORT: crate::modules::Item = (
    "token",
    Export::Function(token, 1),
    Ts::function(
        "The design token `name` of the app's active theme document, reactive, in\nthe theme format's canonical form with references resolved: a color as\n`#rrggbb` (`#rrggbbaa` below full opacity), a number, a font family, a\nstring, a boolean, a duration as `<n>ms`, an easing as it was read (a\nkeyword, `\"ease-in\"`, or its four control points), a text style, shadow,\ngradient, blur, border or transition as an object of its members;\n`undefined` when the document has none. Classes name the same tokens\n(`bg-primary`, `rounded-(--radius-card)`).",
        &["<K extends keyof Tokens>(name: K): Tokens[K]"],
    ),
);

/// An accessor's run: reads its signal (tracked).
unsafe extern "C" fn read(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
    _magic: c_int,
    data: *mut JsValue,
) -> JsValue {
    // SAFETY: one data value: a signal object.
    unsafe { qjs::JS_Call(cx, *data, UNDEFINED, 0, ptr::null_mut()) }
}

fn object(cx: *mut JsContext, fields: &[(&CStr, JsValue)]) -> JsValue {
    // SAFETY: a new object taking the field values.
    unsafe {
        let o = qjs::JS_NewObject(cx);
        for (k, v) in fields {
            qjs::JS_SetPropertyStr(cx, o, k.as_ptr(), *v);
        }
        o
    }
}

/// A number as JS shows it: the f32's shortest decimal (`51.809525`, not
/// the f64 widening `51.80952453613281`).
fn number(v: f32) -> JsValue {
    let shortest = v.to_string().parse::<f64>().unwrap_or(v as f64);
    qjs::JS_NewFloat64(shortest)
}

fn boolean(v: bool) -> JsValue {
    if v { qjs::JS_TRUE } else { qjs::JS_FALSE }
}

fn string(cx: *mut JsContext, s: &str) -> JsValue {
    // SAFETY: a new string.
    unsafe { qjs::JS_NewStringLen(cx, s.as_ptr().cast(), s.len() as _) }
}

/// The accessor of the appearance the nearest `ThemeScope` around the
/// current owner shows (owned), if there is one.
pub(crate) fn scope_scheme(cx: *mut JsContext) -> Option<JsValue> {
    let key = env(cx).scope_key.get();
    // No scope was made yet: nothing to look for.
    // SAFETY: a value the environment owns.
    if unsafe { qjs::JS_IsUndefined(key) } {
        return None;
    }
    reactive::lookup(cx, key)
}

/// What the current owner makes from now on is in a `ThemeScope` showing
/// what `scheme` (borrowed: an accessor of `"light"` or `"dark"`) says.
pub(crate) fn provide_scope(cx: *mut JsContext, scheme: JsValue) {
    let e = env(cx);
    // SAFETY: a new object the environment keeps, as the scopes' key.
    unsafe {
        if qjs::JS_IsUndefined(e.scope_key.get()) {
            e.scope_key.set(qjs::JS_NewObject(cx));
        }
    }
    reactive::provide(cx, e.scope_key.get(), scheme);
}

/// A scope's slot is the low part of its memo's value; the rest counts its
/// changes ([`Env::scope_versions`]).
const SCOPE_SLOTS: f64 = 1_048_576.0;

/// The accessor of the design tokens the nearest `ThemeScope` around the
/// current owner draws with (owned), if there is one with tokens.
pub(crate) fn scope_tokens_accessor(cx: *mut JsContext) -> Option<JsValue> {
    let key = env(cx).scope_tokens_key.get();
    // SAFETY: a value the environment owns.
    if unsafe { qjs::JS_IsUndefined(key) } {
        return None;
    }
    reactive::lookup(cx, key)
}

/// What the current owner makes from now on is in a `ThemeScope` drawn
/// with the tokens `accessor` (borrowed: [`make_scope_tokens`]'s memo)
/// names.
pub(crate) fn provide_scope_tokens(cx: *mut JsContext, accessor: JsValue) {
    let e = env(cx);
    // SAFETY: a new object the environment keeps, as the scopes' key.
    unsafe {
        if qjs::JS_IsUndefined(e.scope_tokens_key.get()) {
            e.scope_tokens_key.set(qjs::JS_NewObject(cx));
        }
    }
    reactive::provide(cx, e.scope_tokens_key.get(), accessor);
}

/// A slot for a new `ThemeScope`'s tokens ([`free_scope_slot`] gives it
/// back).
pub(crate) fn new_scope_slot(cx: *mut JsContext) -> usize {
    let e = env(cx);
    // SAFETY: a value the environment owns; the signal borrows the number.
    if unsafe { qjs::JS_IsUndefined(e.scopes_signal.get()) } {
        e.scopes_signal.set(reactive::new_signal(
            cx,
            number(e.tokens_version.get() as f32),
        ));
    }
    if let Some(slot) = e.scope_free.borrow_mut().pop() {
        return slot;
    }
    let mut slots = e.scope_tokens.borrow_mut();
    slots.push(None);
    slots.len() - 1
}

/// The scope of `slot` is gone (its memo with it): the slot is free for a
/// scope made later.
pub(crate) fn free_scope_slot(cx: *mut JsContext, slot: usize) {
    let e = env(cx);
    if let Some(tokens) = e.scope_tokens.borrow_mut().get_mut(slot) {
        *tokens = None;
        e.scope_free.borrow_mut().push(slot);
    }
}

/// The tokens the memo value `named` (borrowed: a scope's accessor's)
/// names.
fn scope_tokens_named(cx: *mut JsContext, named: JsValue) -> Option<Arc<Tokens>> {
    let mut value = 0f64;
    // SAFETY: a number conversion of a live value.
    if unsafe { qjs::JS_ToFloat64(cx, &mut value, named) } != 0 || !value.is_finite() {
        return None;
    }
    let slot = (value % SCOPE_SLOTS) as usize;
    env(cx).scope_tokens.borrow().get(slot).cloned().flatten()
}

/// The tokens the nearest `ThemeScope` around the current owner draws
/// with, tracked; `None`: no scope with tokens around. The exception when
/// reading them throws.
fn scope_tokens(cx: *mut JsContext) -> Result<Option<Arc<Tokens>>, JsValue> {
    let Some(accessor) = scope_tokens_accessor(cx) else {
        return Ok(None);
    };
    // SAFETY: calls the scope's accessor, ours until freed.
    unsafe {
        let named = qjs::JS_Call(cx, accessor, UNDEFINED, 0, ptr::null_mut());
        qjs::JS_FreeValue(cx, accessor);
        if qjs::JS_IsException(named) {
            return Err(named);
        }
        let tokens = scope_tokens_named(cx, named);
        qjs::JS_FreeValue(cx, named);
        Ok(tokens)
    }
}

/// Makes the tokens of the `ThemeScope` of `slot`: the values `theme`
/// (borrowed: its `theme` prop, a theme file's object) sets over the
/// tokens around it, those `outer` (borrowed: the accessor of the scope
/// around it, else `undefined`) names or the surface's. Without a theme
/// they are the ones around it. Returns the scope's memo value (a number),
/// new when the tokens are; an exception when reading throws.
pub(crate) fn make_scope_tokens(
    cx: *mut JsContext,
    slot: usize,
    theme: JsValue,
    modifiers: JsValue,
    outer: JsValue,
) -> JsValue {
    let e = env(cx);
    // SAFETY: an inline tag check.
    let has_outer = unsafe { qjs::JS_IsFunction(cx, outer) };
    // SAFETY: inline tag checks.
    let (has_theme, has_modifiers) =
        unsafe { (qjs::JS_IsObject(theme), qjs::JS_IsObject(modifiers)) };
    if !has_theme && !has_modifiers {
        // No tokens of its own: the ones around it, by their own value
        // (`undefined`: the surface's, which `token()` follows by name).
        if let Some(held) = e.scope_tokens.borrow_mut().get_mut(slot) {
            *held = None;
        }
        return if has_outer {
            // SAFETY: calls the outer scope's accessor.
            unsafe { qjs::JS_Call(cx, outer, UNDEFINED, 0, ptr::null_mut()) }
        } else {
            UNDEFINED
        };
    }
    let around = if has_outer {
        // SAFETY: calls the outer scope's accessor; its value is ours.
        unsafe {
            let named = qjs::JS_Call(cx, outer, UNDEFINED, 0, ptr::null_mut());
            if qjs::JS_IsException(named) {
                return named;
            }
            let tokens = scope_tokens_named(cx, named);
            qjs::JS_FreeValue(cx, named);
            tokens
        }
    } else {
        None
    };
    let around = around.unwrap_or_else(|| {
        reactive::track(cx, e.scopes_signal.get());
        e.tokens.borrow().clone()
    });
    // Its modifiers' contexts first, its theme patch over them; what it
    // leaves out is said once, by the component in the core.
    let picks = if has_modifiers {
        let picks = match to_json(cx, modifiers) {
            Ok(json) => json,
            Err(exception) => return exception,
        };
        let picks: Vec<(String, String)> = picks
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(n, c)| Some((n.clone(), c.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        Some(ScopeModifiers::new(picks))
    } else {
        None
    };
    let patch = if has_theme {
        let json = match to_json_text(cx, theme) {
            Ok(json) => json,
            Err(exception) => return exception,
        };
        Some(nsh_theme::read_patch(&json).0)
    } else {
        None
    };
    let tokens = around.scoped(picks.as_ref(), patch).0;
    let mut slots = e.scope_tokens.borrow_mut();
    let Some(held) = slots.get_mut(slot) else {
        return UNDEFINED;
    };
    // The same tokens as before: the memo keeps its value.
    let same = held.as_ref().is_some_and(|held| **held == *tokens);
    if !same {
        *held = Some(tokens);
        e.scope_versions.set(e.scope_versions.get().wrapping_add(1));
    }
    qjs::JS_NewFloat64(f64::from(e.scope_versions.get()) * SCOPE_SLOTS + slot as f64)
}

/// The appearance the surface shows (owned: `"light"` or `"dark"`),
/// whatever scope is around. Reading it tracks it.
pub(crate) fn surface_scheme(cx: *mut JsContext) -> JsValue {
    let accessor = env(cx).accessors[Part::ColorScheme as usize].get();
    // SAFETY: calls an accessor the environment owns.
    unsafe { qjs::JS_Call(cx, accessor, UNDEFINED, 0, ptr::null_mut()) }
}

/// The appearance shown where it is read (owned: `"light"` or `"dark"`):
/// the nearest `ThemeScope`'s, else the surface's. Reading it tracks it.
fn shown_scheme(cx: *mut JsContext) -> JsValue {
    match scope_scheme(cx) {
        // SAFETY: calls the scope's accessor, ours until freed.
        Some(scope) => unsafe {
            let shown = qjs::JS_Call(cx, scope, UNDEFINED, 0, ptr::null_mut());
            qjs::JS_FreeValue(cx, scope);
            shown
        },
        None => surface_scheme(cx),
    }
}

/// `colorScheme()`.
unsafe extern "C" fn color_scheme(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
) -> JsValue {
    shown_scheme(cx)
}

/// `colorScheme` itself (owned): a function, called as the accessors are.
fn color_scheme_export(cx: *mut JsContext) -> JsValue {
    // SAFETY: a new function.
    unsafe {
        qjs::JS_NewCFunction2(
            cx,
            Some(color_scheme),
            c"colorScheme".as_ptr(),
            0,
            qjs::JSCFunctionEnum_JS_CFUNC_generic,
            0,
        )
    }
}

/// The color scheme shown: the app's, else the system's.
fn scheme(e: &Env, env: &Environment) -> &'static str {
    match e.appearance.get().resolve(env.color_scheme) {
        ColorScheme::Light => "light",
        ColorScheme::Dark => "dark",
    }
}

/// Shows the surface in `appearance` (`nsh.json` `"appearance"`,
/// `setColorScheme`): `colorScheme()` follows it at once, the core
/// (`dark:` classes, the components' theme) and the platform's controls
/// with the root's `appearance` prop.
pub(crate) fn set_appearance(cx: *mut JsContext, appearance: Appearance) -> JsValue {
    let e = env(cx);
    e.appearance.set(appearance);
    let op = nsh_core::Op::SetProp {
        id: nsh_core::ROOT_ID,
        prop: nsh_core::Prop::Appearance(appearance),
    };
    if let Err(err) = crate::dom::push_op(cx, op) {
        return err;
    }
    let value = string(cx, scheme(e, &current(cx)));
    reactive::set_signal(cx, e.signals[Part::ColorScheme as usize].get(), value);
    reactive::finish_value(cx, UNDEFINED)
}

/// Shows the surface in `appearance` the core already has (Rust's
/// `set_color_scheme`): `colorScheme()` follows it as it follows
/// `setColorScheme`, and no op goes back. Whether JS's appearance changed.
pub(crate) fn show_appearance(cx: *mut JsContext, appearance: Appearance) -> bool {
    let e = env(cx);
    if e.appearance.replace(appearance) == appearance {
        return false;
    }
    let value = string(cx, scheme(e, &current(cx)));
    reactive::set_signal(cx, e.signals[Part::ColorScheme as usize].get(), value);
    true
}

/// Rust set `appearance` inside the synchronous call from JS running now
/// (`set_color_scheme`): its op goes with the call's turn, and
/// `colorScheme()` follows when the call returns
/// ([`show_appearance_set_in_call`]).
pub(crate) fn appearance_set_in_call(cx: *mut JsContext, appearance: Appearance) {
    env(cx).appearance_in_call.set(Some(appearance));
}

/// As a synchronous call from JS returns `value` (owned, or the
/// exception): an appearance Rust set in it shows in `colorScheme()` now,
/// as JS's `setColorScheme` shows its own, and what read it runs (an error
/// of theirs is the call's).
pub(crate) fn show_appearance_set_in_call(cx: *mut JsContext, value: JsValue) -> JsValue {
    let Some(appearance) = env(cx).appearance_in_call.take() else {
        return value;
    };
    show_appearance(cx, appearance);
    reactive::finish_value(cx, value)
}

/// `"light"` / `"dark"` / `"system"`.
pub(crate) fn parse_appearance(name: &str) -> Option<Appearance> {
    match name {
        "system" => Some(Appearance::System),
        "light" => Some(Appearance::Light),
        "dark" => Some(Appearance::Dark),
        _ => None,
    }
}

/// `setColorScheme(scheme)`.
unsafe extern "C" fn set_color_scheme(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let name = crate::dom::string_arg(cx, arg(argc, argv, 0)).unwrap_or_default();
    match parse_appearance(&name) {
        Some(appearance) => set_appearance(cx, appearance),
        // SAFETY: throws on a live context.
        None => unsafe {
            qjs::JS_ThrowTypeError(
                cx,
                c"setColorScheme: expected \"light\", \"dark\" or \"system\"".as_ptr(),
            )
        },
    }
}

/// Takes in what the platform reports: writes the parts that changed.
pub(crate) fn set_environment(cx: *mut JsContext, next: &Environment) -> Result<(), ()> {
    let e = env(cx);
    let previous = e.current.replace(Some(*next));
    let changed = |f: &dyn Fn(&Environment) -> bool| match &previous {
        Some(p) => !f(p),
        None => true,
    };
    let insets = |i: &nsh_core::Edges<nsh_core::Points>| {
        [
            (c"top", number(i.top.0)),
            (c"right", number(i.right.0)),
            (c"bottom", number(i.bottom.0)),
            (c"left", number(i.left.0)),
        ]
    };
    let mut writes: Vec<(Part, JsValue)> = Vec::new();
    if changed(&|p| p.window == next.window) {
        let (w, h) = (next.window.width.0, next.window.height.0);
        writes.push((
            Part::Window,
            object(cx, &[(c"width", number(w)), (c"height", number(h))]),
        ));
    }
    if changed(&|p| p.horizontal_size_class() == next.horizontal_size_class()) {
        writes.push((
            Part::HorizontalSizeClass,
            string(cx, next.horizontal_size_class().name()),
        ));
    }
    if changed(&|p| p.vertical_size_class() == next.vertical_size_class()) {
        writes.push((
            Part::VerticalSizeClass,
            string(cx, next.vertical_size_class().name()),
        ));
    }
    if changed(&|p| p.safe_area == next.safe_area) {
        writes.push((Part::SafeArea, object(cx, &insets(&next.safe_area))));
    }
    if changed(&|p| p.title_bar == next.title_bar) {
        let t = next.title_bar;
        writes.push((
            Part::TitleBar,
            object(
                cx,
                &[
                    (c"height", number(t.top.0)),
                    (c"left", number(t.left.0)),
                    (c"right", number(t.right.0)),
                ],
            ),
        ));
    }
    if changed(&|p| {
        p.keyboard_height == next.keyboard_height && p.keyboard_duration == next.keyboard_duration
    }) {
        writes.push((
            Part::Keyboard,
            object(
                cx,
                &[
                    (c"height", number(next.keyboard_height.0)),
                    (c"duration", number(next.keyboard_duration)),
                ],
            ),
        ));
    }
    if changed(&|p| p.color_scheme == next.color_scheme) {
        writes.push((Part::ColorScheme, string(cx, scheme(e, next))));
    }
    if changed(&|p| p.font_scale == next.font_scale) {
        writes.push((Part::FontScale, number(next.font_scale)));
    }
    if changed(&|p| p.locale == next.locale) {
        writes.push((Part::Locale, string(cx, next.locale.as_str())));
    }
    if changed(&|p| p.locale_settings == next.locale_settings) {
        let settings = &next.locale_settings;
        let mut tags: Vec<JsValue> = settings
            .languages
            .iter()
            .map(|l| string(cx, l.as_str()))
            .collect();
        // SAFETY: the array takes the strings.
        let languages = unsafe { qjs::JS_NewArrayFrom(cx, tags.len() as c_int, tags.as_mut_ptr()) };
        let first_day_of_week = f32::from(settings.first_day_of_week.number());
        writes.push((
            Part::LocaleSettings,
            object(
                cx,
                &[
                    (c"languages", languages),
                    (c"timeZone", string(cx, settings.time_zone.as_str())),
                    (c"hourCycle", string(cx, settings.hour_cycle.name())),
                    (c"firstDayOfWeek", number(first_day_of_week)),
                    (
                        c"measurementSystem",
                        string(cx, settings.measurement_system.name()),
                    ),
                    (
                        c"temperatureUnit",
                        string(cx, settings.temperature_unit.name()),
                    ),
                ],
            ),
        ));
    }
    if changed(&|p| p.screen_reader == next.screen_reader) {
        writes.push((Part::ScreenReader, boolean(next.screen_reader)));
    }
    if changed(&|p| p.reduce_motion == next.reduce_motion) {
        writes.push((Part::ReduceMotion, boolean(next.reduce_motion)));
    }
    if changed(&|p| p.pixel_ratio == next.pixel_ratio) {
        writes.push((Part::PixelRatio, number(next.pixel_ratio)));
    }
    if changed(&|p| p.lifecycle == next.lifecycle) {
        writes.push((Part::Lifecycle, string(cx, next.lifecycle.name())));
    }
    if changed(&|p| p.wake_count == next.wake_count) {
        writes.push((Part::WakeCount, number(next.wake_count as f32)));
    }
    if changed(&|p| p.shaders == next.shaders) {
        writes.push((Part::Shaders, boolean(next.shaders)));
    }
    if changed(&|p| p.window_state == next.window_state) {
        writes.push((
            Part::WindowState,
            crate::window::state_value(cx, next.window_state.as_ref()),
        ));
    }
    // The layout direction follows the user's language.
    // SAFETY: the state's renderer, by shared reference.
    unsafe { &(*crate::state::state(cx)).dom }.set_direction(next.rtl);
    reactive::batch_with(cx, || {
        for (part, value) in writes {
            reactive::set_signal(cx, e.signals[part as usize].get(), value);
        }
    });
    let failed = reactive::finish_value(cx, UNDEFINED);
    // SAFETY: an inline tag check.
    if unsafe { qjs::JS_IsException(failed) } {
        Err(())
    } else {
        Ok(())
    }
}

/// Takes in the displays the platform reports.
pub(crate) fn set_displays(cx: *mut JsContext, displays: &[Display]) -> Result<(), ()> {
    *env(cx).displays.borrow_mut() = displays.to_vec();
    let value = to_js(cx, &displays.to_vec());
    let state = current(cx).window_state;
    reactive::batch_with(cx, || {
        reactive::set_signal(cx, env(cx).signals[Part::Displays as usize].get(), value);
        // The windows' states name their displays: those objects change too.
        let main = crate::window::state_value(cx, state.as_ref());
        reactive::set_signal(cx, env(cx).signals[Part::WindowState as usize].get(), main);
        crate::window::displays_changed(cx);
    });
    let failed = reactive::finish_value(cx, UNDEFINED);
    // SAFETY: an inline tag check.
    if unsafe { qjs::JS_IsException(failed) } {
        Err(())
    } else {
        Ok(())
    }
}

/// Takes in whether the platform's performance monitor measures
/// (`fpsEnabled()`).
pub(crate) fn set_fps_enabled(cx: *mut JsContext, enabled: bool) -> Result<(), ()> {
    reactive::set_signal(
        cx,
        env(cx).signals[Part::Fps as usize].get(),
        boolean(enabled),
    );
    let failed = reactive::finish_value(cx, UNDEFINED);
    // SAFETY: an inline tag check.
    if unsafe { qjs::JS_IsException(failed) } {
        Err(())
    } else {
        Ok(())
    }
}

/// Takes in what the performance monitor measured last (`perfStats()`):
/// `None` while it does not measure.
pub(crate) fn set_perf_stats(
    cx: *mut JsContext,
    stats: Option<&nsh_core::platform::PerfStats>,
) -> Result<(), ()> {
    let value = stats.map_or(qjs::JS_NULL, |stats| to_js(cx, stats));
    reactive::set_signal(cx, env(cx).signals[Part::PerfStats as usize].get(), value);
    let failed = reactive::finish_value(cx, UNDEFINED);
    // SAFETY: an inline tag check.
    if unsafe { qjs::JS_IsException(failed) } {
        Err(())
    } else {
        Ok(())
    }
}

/// `value` as JS (owned): what Rust's conversion makes of it, `undefined`
/// when it fails (it does not for the environment's plain objects).
pub(crate) fn to_js<T: nsh_core::element::JsValue>(cx: *mut JsContext, value: &T) -> JsValue {
    let host = crate::classes::host_outside_calls(cx);
    let converted = value
        .to_js(&host)
        .map(|v| crate::classes::from_raw(v.into_raw()));
    host.clear_pending();
    converted.unwrap_or(UNDEFINED)
}

fn accessor(cx: *mut JsContext, part: Part) -> JsValue {
    // SAFETY: a dup of an accessor the environment owns.
    unsafe { qjs::JS_DupValue(cx, env(cx).accessors[part as usize].get()) }
}

/// The displays the platform reported last.
pub(crate) fn display_list(cx: *mut JsContext) -> Vec<Display> {
    env(cx).displays.borrow().clone()
}

/// `displays()` (`nsh/desktop`): the displays (owned).
pub(crate) fn displays(cx: *mut JsContext) -> JsValue {
    accessor(cx, Part::Displays)
}

pub(crate) fn current(cx: *mut JsContext) -> Environment {
    (*env(cx).current.borrow()).unwrap_or_default()
}

/// The user's locale and preferred languages, read so that what reads them
/// (`nsh/i18n`'s `t`) runs again when they change.
pub(crate) fn tracked_languages(
    cx: *mut JsContext,
) -> (nsh_core::platform::Locale, nsh_core::platform::Languages) {
    for part in [Part::Locale, Part::LocaleSettings] {
        let accessor = env(cx).accessors[part as usize].get();
        // SAFETY: calling the accessor reads (and tracks) the signal; its
        // value is freed at once.
        unsafe {
            let value = qjs::JS_Call(cx, accessor, UNDEFINED, 0, ptr::null_mut());
            qjs::JS_FreeValue(cx, value);
        }
    }
    let current = current(cx);
    (current.locale, current.locale_settings.languages)
}

/// Shows the app in `locale` at once (`setLocale`): `locale()`, `t` and
/// `Intl` read it from now on; the engine keeps it in the environment it
/// reports after.
pub(crate) fn show_locale(cx: *mut JsContext, locale: nsh_core::platform::Locale) {
    let e = env(cx);
    let mut current = e.current.borrow_mut();
    let mut next = current.unwrap_or_default();
    if next.locale == locale {
        return;
    }
    // `localeSettings()` follows when the engine reports it.
    next.locale = locale;
    next.locale_settings.languages = next.locale_settings.languages.preferring(locale);
    *current = Some(next);
    drop(current);
    reactive::set_signal(
        cx,
        e.signals[Part::Locale as usize].get(),
        string(cx, locale.as_str()),
    );
}

/// The user's locale (`Intl`'s default), read where it is used: a change
/// applies to what is formatted next.
pub(crate) fn locale(cx: *mut JsContext) -> nsh_core::platform::Locale {
    current(cx).locale
}

/// The device's time zone (`Intl`'s default, `localeSettings().timeZone`),
/// read where it is used as the locale is; `None` until the platform
/// reports one.
pub(crate) fn time_zone(cx: *mut JsContext) -> Option<nsh_core::platform::TimeZone> {
    env(cx)
        .current
        .borrow()
        .as_ref()
        .map(|e| e.locale_settings.time_zone)
}

/// `safeAreaInsets()`: the insets of the nearest screen or modal (an
/// accessor).
unsafe extern "C" fn safe_area_insets(
    cx: *mut JsContext,
    _this: JsValue,
    _argc: c_int,
    _argv: *mut JsValue,
) -> JsValue {
    let insets = env(cx).insets.get();
    // SAFETY: calling the context reads its nearest value.
    unsafe { qjs::JS_Call(cx, insets, UNDEFINED, 0, ptr::null_mut()) }
}

/// The insets context (borrowed), which screens and modals provide.
pub(crate) fn insets_context(cx: *mut JsContext) -> JsValue {
    env(cx).insets.get()
}

/// The values registries re-export as they are (`Registry::reexport`):
/// the environment's accessors and hooks the UI sets export.
pub(crate) const REEXPORTS: &[Item] = &[
    (
        "safeArea",
        Export::Value(|cx| accessor(cx, Part::SafeArea)),
        Ts::value(
            "Insets of the surface not covered by system UI (notch, home indicator,\ntitle bar), in points (reactive).",
            "Accessor<Edges>",
        ),
    ),
    (
        "keyboardFrame",
        Export::Value(|cx| accessor(cx, Part::Keyboard)),
        Ts::value(
            "Height of the software keyboard over the surface (0 when hidden) and the\nduration of its animation in milliseconds (reactive).",
            "Accessor<{ height: number; duration: number }>",
        ),
    ),
    (
        "safeAreaInsets",
        Export::Function(safe_area_insets, 0),
        Ts::function(
            "Insets not covered by system UI or headers where this component renders:\nthe surface's safe area, a dialog's, or a page's (below its header).",
            &["(): Accessor<Edges>"],
        ),
    ),
    (
        "nextTick",
        Export::Function(crate::standard::process::next_tick, 1),
        Ts::function(
            "Calls `callback` with `args` as a JS job, after the current one and before timers.",
            &["<A extends unknown[]>(callback: (...args: A) => void, ...args: A): void"],
        ),
    ),
];

/// The host elements a UI set exports (`Registry::native_values`):
/// `nsh/base`'s `Input`, a base component the platforms make natively.
pub(crate) const ELEMENTS: &[Item] = &[(
    "Input",
    Export::Value(|cx| crate::dom::host_component(cx, "input", true)),
    Ts::function(
        "A native text input (UITextField / UITextView, EditText); `ref` has\n`focus()`, `blur()`, `setSelection(start, end)` and `insertText(text)`.",
        &["(props: InputProps): JSX.Element"],
    ),
)];

/// Value `name` a registry re-exports or names as native
/// (`Registry::native_values`): [`REEXPORTS`], [`ELEMENTS`] and
/// navigation's.
pub(crate) fn reexport(cx: *mut JsContext, name: &str) -> JsValue {
    let item = REEXPORTS
        .iter()
        .chain(ELEMENTS)
        .chain(crate::navigation::EXPORTS)
        .find(|(n, ..)| *n == name);
    match item {
        Some((n, Export::Function(f, length), _)) => {
            let cname = std::ffi::CString::new(*n).unwrap_or_default();
            reactive::new_function(cx, &cname, *f, *length)
        }
        Some((_, Export::Value(make), _)) => make(cx),
        None => {
            let message =
                std::ffi::CString::new(format!("nsh: the runtime has no native `{name}`"))
                    .unwrap_or_default();
            // SAFETY: throws on a live context.
            unsafe { qjs::JS_ThrowReferenceError(cx, c"%s".as_ptr(), message.as_ptr()) }
        }
    }
}

// --- nsh/device ---------------------------------------------------------------------

/// The OS version as a string: `"17.5"` (iOS, desktops), the API level on
/// Android (`"35"`), `"0.0"` on the web.
fn os_version(env: &Environment) -> String {
    let (major, minor) = env.os_version;
    if env.os == Os::Android {
        major.to_string()
    } else {
        format!("{major}.{minor}")
    }
}

/// `platform.idiom`: `"terminal"` in a terminal, else `"desktop"` exactly
/// where desktop behavior applies
/// ([`nsh_core::platform::Idiom::reported_on`]).
fn idiom(env: &Environment) -> &'static str {
    match env.reported_idiom() {
        Idiom::Phone => "phone",
        Idiom::Pad => "pad",
        Idiom::Mac => "mac",
        Idiom::Tv => "tv",
        Idiom::Desktop => "desktop",
        Idiom::Terminal => "terminal",
    }
}

/// `platform`: the OS and form factor, constant while the app runs.
fn platform(cx: *mut JsContext) -> JsValue {
    let e = current(cx);
    let select = reactive::new_function(cx, c"select", platform_select, 1);
    object(
        cx,
        &[
            (c"name", string(cx, e.platform_name())),
            (c"os", string(cx, e.os.name())),
            (c"version", string(cx, &os_version(&e))),
            (c"idiom", string(cx, idiom(&e))),
            (c"desktop", boolean(e.desktop())),
            (c"tv", boolean(e.tv())),
            (c"catalyst", boolean(e.os.is_catalyst())),
            (c"select", select),
        ],
    )
}

/// `platform.select(spec)`: this platform's name (when built outside NSH),
/// else its OS, else `tv` (on a TV), else `mobile` / `desktop`, else
/// `native` (not on the web), else `default`.
unsafe extern "C" fn platform_select(
    cx: *mut JsContext,
    _this: JsValue,
    argc: c_int,
    argv: *mut JsValue,
) -> JsValue {
    let spec = arg(argc, argv, 0);
    // SAFETY: an inline check, then property reads of a live object.
    if !unsafe { qjs::JS_IsObject(spec) } {
        return UNDEFINED;
    }
    let env = current(cx);
    let kind = if env.desktop() { "desktop" } else { "mobile" };
    let external = env.platform.filter(|&name| name != env.os.name());
    let rest = [env.os.name(), "tv", kind, "native", "default"];
    for key in external
        .into_iter()
        .chain(rest)
        .filter(|k| *k != "native" || env.os != Os::Web)
        .filter(|k| *k != "tv" || env.tv())
    {
        let Ok(key) = std::ffi::CString::new(key) else {
            continue;
        };
        let v = unsafe { qjs::JS_GetPropertyStr(cx, spec, key.as_ptr()) };
        if !unsafe { qjs::JS_IsUndefined(v) } {
            return v;
        }
    }
    UNDEFINED
}

/// The exports of `nsh/device`.
pub(crate) const DEVICE_EXPORTS: &[Item] = &[
    (
        "horizontalSizeClass",
        Export::Value(|cx| accessor(cx, Part::HorizontalSizeClass)),
        Ts::value(
            "Horizontal space (reactive): UIKit's trait on iOS; elsewhere compact below 600pt of window width, regular at 600pt and above.",
            "Accessor<SizeClass>",
        ),
    ),
    (
        "verticalSizeClass",
        Export::Value(|cx| accessor(cx, Part::VerticalSizeClass)),
        Ts::value(
            "Vertical space (reactive): UIKit's trait on iOS; elsewhere compact below 600pt of window height, regular at 600pt and above.",
            "Accessor<SizeClass>",
        ),
    ),
    (
        "windowSize",
        Export::Value(|cx| accessor(cx, Part::Window)),
        Ts::value(
            "Size of the surface in points (reactive).",
            "Accessor<Size>",
        ),
    ),
    (
        "colorScheme",
        Export::Value(color_scheme_export),
        Ts::value(
            "The appearance shown (reactive): the system's, unless the app set its\nown (`nsh.json` `\"appearance\"`, `setColorScheme`). In a `ThemeScope`\nwith an appearance, the scope's.",
            "Accessor<\"light\" | \"dark\">",
        ),
    ),
    (
        "setColorScheme",
        Export::Function(set_color_scheme, 1),
        Ts::function(
            "Shows the app in `scheme` whatever the system's appearance, at once:\n`colorScheme()`, `dark:` classes, the components' theme and the\nplatform's controls (alerts, the status bar's default style) follow it;\n`\"system\"` follows the system again. `nsh.json` `\"appearance\"` sets it\nfrom the start.",
            &["(scheme: \"light\" | \"dark\" | \"system\"): void"],
        ),
    ),
    (
        "fpsEnabled",
        Export::Value(|cx| accessor(cx, Part::Fps)),
        Ts::value(
            "Whether the performance monitor measures (reactive): on with `--fps`,\n`NSH_FPS=1` or `setFpsEnabled(true)` where the platform has one.",
            "Accessor<boolean>",
        ),
    ),
    (
        "perfStats",
        Export::Value(|cx| accessor(cx, Part::PerfStats)),
        Ts::value(
            "What the performance monitor measured over the last second (reactive):\nits frame rate and more, read once a second while it measures\n(`fpsEnabled()`), `null` while it does not and until its first second.",
            "Accessor<PerfStats | null>",
        ),
    ),
    (
        "fontScale",
        Export::Value(|cx| accessor(cx, Part::FontScale)),
        Ts::value(
            "The user's preferred text size relative to the default (reactive):\nthe system's, unless the embedding app set its own.",
            "Accessor<number>",
        ),
    ),
    (
        "pixelRatio",
        Export::Value(|cx| accessor(cx, Part::PixelRatio)),
        Ts::value(
            "Device pixels per point (reactive): it changes when the window moves to\na screen of another density. A length on whole device pixels is\n`Math.round(x * pixelRatio()) / pixelRatio()`.",
            "Accessor<number>",
        ),
    ),
    (
        "locale",
        Export::Value(|cx| accessor(cx, Part::Locale)),
        Ts::value(
            "The user's locale as a BCP 47 tag (`\"en-US\"`, `\"zh-Hans-CN\"`), which\n`Intl` formats for by default (reactive): the system's, unless the\nembedding app set its own.",
            "Accessor<string>",
        ),
    ),
    (
        "localeSettings",
        Export::Value(|cx| accessor(cx, Part::LocaleSettings)),
        Ts::value(
            "The user's locale settings beside the locale (reactive): the preferred\nlanguages, the device's time zone and the regional preferences, the\nsystem's settings, which change together. Their names and values are\n`Intl`'s, so they pass into its options as they are\n(`new Intl.DateTimeFormat(locale(), { timeZone, hourCycle })`). Read the\none a view shows in a `computed` to follow only it. In headless tests they\nare the locale's (it alone as the languages, its region's preferences)\nin UTC, unless `open` sets `localeSettings`.",
            "Accessor<LocaleSettings>",
        ),
    ),
    (
        "lifecycle",
        Export::Value(|cx| accessor(cx, Part::Lifecycle)),
        Ts::value(
            "Whether the app is in use (reactive): `\"active\"` in the foreground,\n`\"inactive\"` while visible without the focus (another app or window has\nit, or the system shows something over it), `\"background\"` while not\nvisible (in the background, minimized or hidden). An embedded app\nfollows the embedding app's.",
            "Accessor<\"active\" | \"inactive\" | \"background\">",
        ),
    ),
    (
        "wakeCount",
        Export::Value(|cx| accessor(cx, Part::WakeCount)),
        Ts::value(
            "How many times the system woke from sleep while the app ran, 0 until\nthe first wake (reactive): an effect that reads it runs again after each\nwake. A sleep may leave `lifecycle()` as it was (a laptop's lid closed\nand opened with the app in front), while network connections and timers\ndid not live through it: check or open them again there. macOS, Windows\nand Linux count wakes; on iOS and Android a sleeping device takes the\napp to the background, which `lifecycle()` reports.",
            "Accessor<number>",
        ),
    ),
    (
        "platform",
        Export::Value(platform),
        Ts::value(
            "The platform the app runs on, constant while it runs.",
            "PlatformInfo",
        ),
    ),
    (
        "shadersAvailable",
        Export::Value(|cx| accessor(cx, Part::Shaders)),
        Ts::value(
            "Whether the platform draws shader views (`Shader`, `ShaderEffect`,\n`ShaderTransition`, `ScreenEffect`, `ScreenTransition`) (reactive).\n`false` where the platform was built without a renderer or draws in a\nterminal's cells; elsewhere `true` until the first shader mounts on a\nmachine without a GPU (WebGL 2 in a browser), when it turns `false` for\ngood. Show a shader while it holds and the app's own drawing (a `Canvas`,\nan image) otherwise.",
            "Accessor<boolean>",
        ),
    ),
];

/// `nsh/device`'s types.
pub(crate) const DEVICE_TYPES: &str = r#"/** Space available along one axis of the surface. */
export type SizeClass = "compact" | "regular";

export interface PlatformSelectSpec<T> {
  ios?: T;
  android?: T;
  macos?: T;
  windows?: T;
  linux?: T;
  web?: T;
  /** A native Tizen app (`platform.os` is `"tizen"`, a TV); a Tizen web
   * app (`.wgt`) looks for `web`. */
  tizen?: T;
  /** Where `platform.tv` is true: the TV idiom, on any OS (Android TV, a
   * TV's browser); looked up after the OS's own key, before `mobile`. */
  tv?: T;
  /** Where `platform.desktop` is false: iOS (Mac Catalyst included),
   * Android, browsers in the phone or tablet idiom, and TVs (Tizen
   * included). */
  mobile?: T;
  /** Where `platform.desktop` is true: macOS, Windows, Linux, browsers in
   * the desktop idiom, and terminals. */
  desktop?: T;
  /** Every platform but the web. */
  native?: T;
  default?: T;
  /** A platform built outside NSH, by its `platform.name` (`"miniprogram"`, ...). */
  [platform: string]: T | undefined;
}

/** What the performance monitor measured over the last second
 * (`perfStats()`). */
export interface PerfStats {
  /** Frames shown per second: display refreshes in which the app changed
   * something. An idle app reads low, which is not slowness. */
  readonly fps: number;
  /** The rate the display refreshes at (Hz), once the monitor saw it
   * refresh. */
  readonly refreshRate: number | null;
  /** The share of recent frames that took longer than one refresh, from 0
   * to 1 (`0.05` is 5%), as Rust's `dropped` (a `Percentage`) reads it. */
  readonly dropped: number;
}

/** The user's locale settings beside the locale (`localeSettings()`). */
export interface LocaleSettings {
  /** The preferred languages as BCP 47 tags, most preferred first: the
   * system's list (`["de-DE", "en-US"]`), with the embedding app's own
   * language first when it sets one. Show the app's text in the first it
   * has, the others are its fallbacks. */
  readonly languages: readonly string[];
  /** The device's time zone as the system names it: an IANA name
   * (`"Europe/Berlin"`, `"America/New_York"`), `"UTC"`, or an offset zone
   * where the system has one (`"GMT+01:00"` on Android). It changes with the
   * system's setting, as the user travels; `Intl` formats local time in it. */
  readonly timeZone: string;
  /** 12- or 24-hour time, as `Intl` names its hour cycles: `"h12"`
   * (`12:30 PM`), `"h23"` (`00:30`), and the rarer `"h11"` (`0:30 PM`) and
   * `"h24"` (`24:30`). The system's setting, else the region's; pass it as
   * `Intl.DateTimeFormat`'s `hourCycle` to write times as the user reads
   * them. `TimeField`'s and `DatePicker`'s `hourCycle` is `"h12"` for
   * `"h11"` and `"h12"`, `"h23"` for the others. */
  readonly hourCycle: "h11" | "h12" | "h23" | "h24";
  /** The day weeks start on, as ISO 8601 numbers days (and `Intl.Locale`'s
   * `firstDay`): 1 for Monday to 7 for Sunday. A calendar's first column.
   * The system's setting, else the region's (7 in the US, 1 in Germany).
   * `Calendar`'s and `DatePicker`'s `firstDayOfWeek` count from 0 for
   * Sunday: pass `firstDayOfWeek % 7`. */
  readonly firstDayOfWeek: 1 | 2 | 3 | 4 | 5 | 6 | 7;
  /** The units of lengths, weights and volumes: `"metric"`, `"us"` (feet,
   * miles, pounds, gallons) or `"uk"` (metric, with miles and pints). The
   * system's setting, else the region's. */
  readonly measurementSystem: "metric" | "us" | "uk";
  /** The unit of temperatures: `"celsius"` or `"fahrenheit"`. The system's
   * setting, else the region's. */
  readonly temperatureUnit: "celsius" | "fahrenheit";
}

export interface PlatformInfo {
  /** The platform the app runs on: the OS's name on NSH's own platforms
   * (`"ios"`, `"android"`, `"macos"`, `"windows"`, `"linux"`, `"web"`,
   * `"tizen"`), or
   * the name of a platform built outside NSH (`"miniprogram"`), whose OS is
   * `os`: handle a name you do not know. */
  readonly name: OS | (string & {});
  /** The operating system (`"ios"`, `"android"`, `"macos"`, `"windows"`,
   * `"linux"`, `"web"`, `"tizen"`). A native Tizen app is `"tizen"`, in
   * the TV idiom; a Tizen web app (`.wgt`) is `"web"`, a TV by what its
   * page tells the player. */
  readonly os: OS;
  /** iOS and desktops: the OS version (`"17.5"`); Android: the API level
   * (`"35"`); the web: `"0.0"`. */
  readonly version: string;
  /** The kind of device. `"terminal"` in a terminal: drawn on a grid of
   * character cells, with desktop behavior (`desktop` true; `os` is still
   * the machine's). Elsewhere `"desktop"` exactly where `desktop` is true,
   * else the idiom the system reports: `"phone"`, `"pad"`, `"tv"` (on any
   * OS: `tv` is true), and `"mac"`, UIKit's Mac idiom under Mac Catalyst (a
   * mobile idiom). */
  readonly idiom: "phone" | "pad" | "mac" | "tv" | "desktop" | "terminal";
  /** Desktop behavior applies, NSH's one definition of desktop: macOS,
   * Windows, Linux, browsers in the desktop idiom (a mouse and a wide
   * viewport), and terminals; never a TV. `desktop={{…}}` props and
   * `select({ desktop })` follow it. Mac Catalyst runs the iOS backend and
   * is mobile. `nsh/desktop`'s `isAvailable` is narrower: its window and
   * menu APIs work on the desktop OSes, not in browsers. */
  readonly desktop: boolean;
  /** The app runs on a TV: the TV idiom (`idiom` is `"tv"`), on any OS
   * (Android TV, Tizen, a TV's browser, whose page says so). `tv={{…}}` props and
   * `select({ tv })` follow it. A TV is not a desktop (`desktop` is false),
   * so the rest behaves as on mobile. */
  readonly tv: boolean;
  /** The app runs under Mac Catalyst, in the Mac or the iPad idiom
   * (`os` is `"ios"`, `desktop` false). */
  readonly catalyst: boolean;
  /** The value for this platform: its `name` (`"miniprogram"`, ...),
   * else its OS (`ios`, `macos`, `web`, `tizen`, ...), else `tv` on a TV
   * (`platform.tv`), else `desktop` / `mobile` (by `platform.desktop`),
   * else `native` (not on the web), else `default`. */
  select<T>(spec: PlatformSelectSpec<T>): T | undefined;
}
"#;

// --- nsh/accessibility --------------------------------------------------------------

/// The exports of `nsh/accessibility` (with the runtime registry's).
pub(crate) const ACCESSIBILITY_EXPORTS: &[Item] = &[
    (
        "reduceMotion",
        Export::Value(|cx| accessor(cx, Part::ReduceMotion)),
        Ts::value(
            "Whether the user asked for less motion (reactive; also `motion-reduce:`).",
            "Accessor<boolean>",
        ),
    ),
    (
        "screenReaderEnabled",
        Export::Value(|cx| accessor(cx, Part::ScreenReader)),
        Ts::value(
            "Whether a screen reader (VoiceOver, TalkBack) runs (reactive).",
            "Accessor<boolean>",
        ),
    ),
];

#[cfg(test)]
mod tests;
