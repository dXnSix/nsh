//! [`ThemeScope`]: a view that shows itself and the views in it in an
//! appearance of its own, with design tokens of its own, or both, whatever
//! the surface shows (a dark sidebar beside a light page, a card with its
//! own `primary`).
//!
//! The core resolves both by node (`core/classes.rs`): the appearance
//! (`Core::dark_at`: `dark:` variants, theme tokens' classes and the
//! adaptive styles of Rust components pick the scope's side with nothing
//! running) and the tokens (`Core::classes_at`: a class list that reads a
//! token the scope changes is parsed with the scope's values, once). What
//! reads them in code reads the nearest scope's: [`super::dark`],
//! [`super::color`], [`super::tokens`] and [`super::theme_of`] through the
//! context a scope provides to what it renders, TS's `colorScheme()` and
//! `token()` through `nsh-js`'s (`components.rs`).
//!
//! A scope's tokens are its `theme`'s values over the tokens around it,
//! made here ([`ScopeTheme::within`]) and given to the core whole
//! (`Prop::ScopeTokens`): when the tokens around it change they are made
//! again.

use std::sync::Arc;

use crate::{
    JsonValue,
    element::{
        JsHost, JsIn, JsOwned, JsValue, Memo, NoEvent, PropError, Tokens, TsTypes, computed,
        provide_context, reactive::with_data, use_context,
    },
    platform::{Appearance, ColorScheme},
    prelude::*,
};

/// The design tokens a [`ThemeScope`] sets: a theme file (TS's `theme`,
/// shadcn's `cssVars`), or tokens made in Rust. A scope sets values of
/// tokens the theme around it has; it adds no names.
#[derive(Debug, Clone, PartialEq)]
pub enum ScopeTheme {
    /// A theme file, as `updateTheme` takes one: read by the app's UI
    /// sets, a `var()` it does not set naming the tokens around the scope.
    File(Arc<str>),
    /// Tokens.
    Tokens(Arc<Tokens>),
}

impl ScopeTheme {
    /// The theme file `json`.
    pub fn file(json: impl Into<Arc<str>>) -> Self {
        Self::File(json.into())
    }

    /// The tokens of a theme type's value (the app's own, or `nsh-base`'s
    /// `Theme`).
    pub fn of<T: super::AppTheme>(theme: &T) -> Self {
        Self::Tokens(Arc::new(super::theme::theme_tokens(theme)))
    }

    /// The tokens a scope with this theme draws with: its values over
    /// `around`, the tokens around the scope. What it cannot set (a value
    /// that does not read, a name `around` lacks) is left out with a
    /// warning.
    fn within(&self, around: &Arc<Tokens>) -> Arc<Tokens> {
        let (tokens, left_out) = match self {
            ScopeTheme::Tokens(tokens) => {
                let (tokens, unknown) = (**tokens).clone().within(around);
                let said = unknown
                    .iter()
                    .map(|name| format!("`{name}`: the theme has no token of this name"))
                    .collect();
                (tokens, said)
            }
            ScopeTheme::File(json) => {
                let read = with_data(|d| d.ambient.theme_tokens())
                    .and_then(|theme| theme.scoped(around, json));
                match read {
                    Some(Ok(read)) => read,
                    Some(Err(e)) => {
                        log::warn!("ThemeScope: {e}");
                        return around.clone();
                    }
                    None => {
                        log::warn!("ThemeScope: no UI set reads theme files");
                        return around.clone();
                    }
                }
            }
        };
        for what in left_out {
            log::warn!("ThemeScope: {what}, ignored");
        }
        Arc::new(tokens)
    }
}

impl From<Tokens> for ScopeTheme {
    fn from(tokens: Tokens) -> Self {
        Self::Tokens(Arc::new(tokens))
    }
}

impl From<Arc<Tokens>> for ScopeTheme {
    fn from(tokens: Arc<Tokens>) -> Self {
        Self::Tokens(tokens)
    }
}

/// In TS a theme file's object (`Theme`, as `setTheme` takes it).
impl JsValue for ScopeTheme {
    fn ts_type(_types: &mut TsTypes) -> String {
        "Theme".into()
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        let json = JsonValue::from_js(value)?;
        if !json.is_object() {
            return Err(PropError::expected("a theme", &json));
        }
        Ok(Self::File(json.to_string().into()))
    }

    fn to_js<'a>(&self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        let json = match self {
            ScopeTheme::File(json) => json.to_string(),
            ScopeTheme::Tokens(tokens) => super::theme_file::write(tokens),
        };
        let json: JsonValue =
            serde_json::from_str(&json).map_err(|e| PropError(format!("a theme: {e}")))?;
        json.to_js(host)
    }
}

/// The props of a [`ThemeScope`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ThemeScopeProps {
    /// The appearance the scope and what is in it are shown in, whatever
    /// the app's or the system's: `"light"` or `"dark"`. Left out, the
    /// scope is shown as the view around it.
    #[prop(default)]
    pub appearance: Prop<Option<ColorScheme>>,
    /// The design tokens the scope and what is in it are drawn with: a
    /// theme file (`{ cssVars: { light: { primary: "#7c3aed" } } }`, as
    /// `updateTheme` takes one) whose values are set over the tokens around
    /// the scope. It sets tokens the app's theme has: a name it lacks is
    /// left out with a warning. Left out, the scope is drawn with the
    /// tokens around it.
    #[prop(default)]
    pub theme: Prop<Option<ScopeTheme>>,
    /// What the scope holds.
    pub children: Prop<Children>,
}

/// Whether the component registered as `name` ([`super::ComponentSpec`]'s)
/// is [`ThemeScope`]: its JS face provides the scope to what it renders
/// (`nsh-js`).
#[doc(hidden)]
pub fn is_theme_scope(name: &str) -> bool {
    name == super::component::name_of::<ThemeScope>()
}

/// Dark or light as the nearest [`ThemeScope`] shows, for what reads the
/// appearance in code ([`super::dark`]).
#[derive(Clone, Copy)]
pub(crate) struct ScopeAppearance(Memo<bool>);

impl ScopeAppearance {
    /// The nearest scope's appearance, if a scope is around the current
    /// owner. Reading it subscribes.
    pub(crate) fn dark() -> Option<bool> {
        // No scope was ever made on this surface: nothing to look for.
        if !with_data(|d| d.ambient.scoped()) {
            return None;
        }
        use_context::<ScopeAppearance>().map(|scope| scope.0.get())
    }
}

/// The design tokens the nearest [`ThemeScope`] draws with, for what reads
/// them in code ([`super::tokens`]).
#[derive(Clone, Copy)]
pub(crate) struct ScopeTokens(Memo<Arc<Tokens>>);

impl ScopeTokens {
    /// The nearest scope's tokens, if a scope is around the current owner.
    /// Reading them subscribes.
    pub(crate) fn nearest() -> Option<Arc<Tokens>> {
        if !with_data(|d| d.ambient.scoped()) {
            return None;
        }
        use_context::<ScopeTokens>().map(|scope| scope.0.get())
    }
}

/// A view that shows itself and everything in it in an appearance of its
/// own, with design tokens of its own, or both, whatever the app's and the
/// system's: a dark sidebar beside a light page, a card with its own
/// `primary`, a theme's preview beside the app. It is a view: its `class`
/// and `style` lay it out and draw it.
///
/// In it, everything that follows the appearance follows the scope's:
/// `dark:` classes, the classes of theme tokens (`bg-background`), the
/// theme colors of components, `colorScheme()` and `token()`, and the
/// platform's own controls (a text input's keyboard and menus). With a
/// `theme`, the classes of theme tokens, the components' theme and
/// `token()` take the scope's values for the tokens it sets, and the app's
/// for the others. Scopes nest: an inner one starts from the outer one's.
/// Without `appearance` and `theme` a scope is shown as the view around
/// it.
///
/// As in CSS, what a view inherited stays what it was: a scope's root sets
/// its own background and text color (`bg-background text-foreground`).
/// Menus, alerts and sheets the platform draws are the surface's, also
/// when a view in a scope opens them.
///
/// # Rust
///
/// Read the appearance and the tokens where they are used (a binding, a
/// style's closure): [`dark`], [`color`], [`tokens`] and
/// [`super::theme_of`] answer for the scope around what reads them, and an
/// element built before the scope places it is built outside it.
#[component]
pub fn ThemeScope(props: ThemeScopeProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let outer = use_context::<ScopeAppearance>();
    let appearance = props.appearance;
    let dark = computed(move || match appearance.get() {
        Some(ColorScheme::Dark) => true,
        Some(ColorScheme::Light) => false,
        None => outer.map_or_else(super::ambient::surface_dark, |scope| scope.0.get()),
    });
    // Its tokens: its theme's values over those around it (made again
    // when either changes), else the ones around it.
    let around = use_context::<ScopeTokens>();
    let theme = props.theme;
    let own = computed(move || {
        let theme = theme.get()?;
        let around = around.map_or_else(super::ambient::surface_tokens, |scope| scope.0.get());
        Some(theme.within(&around))
    });
    let tokens = computed(move || {
        own.get().unwrap_or_else(|| {
            around.map_or_else(super::ambient::surface_tokens, |scope| scope.0.get())
        })
    });
    with_data(|d| d.ambient.set_scoped());
    provide_context(ScopeAppearance(dark));
    provide_context(ScopeTokens(tokens));
    let mut scope = view();
    scope.node().bind_attr(
        move || own.get(),
        |attrs, tokens| attrs.scope_tokens = tokens,
    );
    scope.node().bind_attr(
        move || appearance.get(),
        |attrs, appearance| {
            attrs.appearance = match appearance {
                Some(ColorScheme::Dark) => Appearance::Dark,
                Some(ColorScheme::Light) => Appearance::Light,
                None => Appearance::System,
            }
        },
    );
    scope.child(props.children)
}
