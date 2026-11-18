//! [`ThemeScope`]: a view that shows itself and the views in it in a color
//! scheme of its own, with modifier contexts and overrides of its own, or
//! both, whatever the surface shows (a dark sidebar beside a light page, a
//! card with its own `primary`).
//!
//! The core resolves both by node (`core/classes.rs`): the color scheme
//! (`Core::dark_at`: `dark:` variants and the adaptive styles of Rust
//! components pick the scope's side with nothing running) and the tokens
//! (`Core::classes_at`: a class list that reads a token the scope changes is
//! parsed with the scope's values, once). What reads them in code reads the
//! nearest scope's: [`super::dark`], [`super::color`], [`super::tokens`],
//! [`super::token`] and typed keys ([`super::TokenKey::get`]) through the
//! context a scope provides to what it renders, TS's `colorScheme()` and
//! `token()` through `nsh-js`'s (`components.rs`).
//!
//! A scope's tokens are the same stack as the surface's
//! (`nsh_theme::resolve`): its `modifiers` picks over the selection around
//! it, its `theme` patch appended after every patch around it (the app's,
//! the host's, outer scopes'), references resolved once for the subtree.
//! They are made here and given to the core whole (`Prop::ScopeTokens`):
//! when the tokens around the scope change they are made again.

use std::sync::Arc;

use crate::{
    JsonValue,
    element::{
        JsHost, JsIn, JsOwned, JsValue, Memo, NoEvent, Patch, PropError, Tokens, TsTypes, computed,
        provide_context, reactive::with_data, tokens::ThemeLayers, use_context,
    },
    platform::{Appearance, ColorScheme},
    prelude::*,
};

/// The tokens a scope with picks `picks` and patch `patch` draws with,
/// around `around`: the same stack, its picks over the selection around it
/// and its patch after every patch around it, references resolved once.
/// A pick of a modifier or context the document lacks, and a patch entry it
/// refuses, are left out and returned as diagnostics.
pub(crate) fn scoped(
    around: &Arc<Tokens>,
    picks: Option<&ScopeModifiers>,
    patch: Option<Patch>,
) -> (Arc<Tokens>, Vec<nsh_theme::Diagnostic>) {
    let Some(layers) = around.layers() else {
        return (around.clone(), Vec::new());
    };
    let document = layers.document.clone();
    let mut selection = layers.selection.clone();
    let mut said = Vec::new();
    for (name, context) in picks.iter().flat_map(|picks| picks.0.iter()) {
        match super::theme::unknown_pick(&document, name, context) {
            Some(diagnostic) => said.push(diagnostic),
            None => {
                selection.set(&**name, &**context);
            }
        }
    }
    let mut patches = layers.patches.clone();
    let patched = patch.is_some();
    if let Some(patch) = patch {
        said.extend(patch.check(&document));
        patches.push(Arc::new(patch));
    }
    let (mut tokens, resolved) = Tokens::resolve_layers(Arc::new(ThemeLayers {
        document,
        selection,
        patches,
        roles: layers.roles.clone(),
    }));
    said.extend(
        resolved
            .diagnostics
            .iter()
            .filter(|d| d.code != nsh_theme::code::REFERENCE_MISSING)
            .cloned(),
    );
    // What the role checks find of the scope's patch: what they find of
    // the stack around it was said where that was installed (checked with
    // the scope's values, which a patch changes and never removes).
    if patched {
        let around_said = around.check_roles(&resolved);
        said.extend(
            tokens
                .check_roles(&resolved)
                .into_iter()
                .filter(|d| !around_said.contains(d)),
        );
    }
    tokens.set_themed(around.is_themed());
    (Arc::new(tokens), said)
}

impl Tokens {
    /// The tokens a [`ThemeScope`] with picks `picks` and patch `patch`
    /// draws with inside these, as the core makes them: the same stack,
    /// its picks over the selection and its patch after every patch,
    /// references resolved once (TS's `ThemeScope` in `nsh-js`). What it
    /// leaves out is returned as diagnostics. Tokens made of no document
    /// are the scope's as they are.
    pub fn scoped(
        self: &Arc<Self>,
        picks: Option<&ScopeModifiers>,
        patch: Option<Patch>,
    ) -> (Arc<Tokens>, Vec<nsh_theme::Diagnostic>) {
        scoped(self, picks, patch)
    }
}

/// The contexts a [`ThemeScope`] picks of the theme's modifiers: each
/// modifier's name and the context's (TS: `{ brand: "forest" }`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScopeModifiers(pub Arc<[(Box<str>, Box<str>)]>);

impl ScopeModifiers {
    /// `picks`, each a modifier's name and a context's.
    pub fn new<N: Into<Box<str>>, C: Into<Box<str>>>(
        picks: impl IntoIterator<Item = (N, C)>,
    ) -> Self {
        Self(
            picks
                .into_iter()
                .map(|(n, c)| (n.into(), c.into()))
                .collect(),
        )
    }

    /// These picks and context `context` of another modifier (one of the
    /// enums `theme_keys!` makes): `Brand::Forest.into()` then
    /// `.and(Density::Compact)`. A context of a modifier these pick already
    /// takes its place.
    #[must_use]
    pub fn and<C: ModifierContext>(self, context: C) -> Self {
        let mut picks: Vec<(Box<str>, Box<str>)> = self
            .0
            .iter()
            .filter(|(name, _)| **name != *C::MODIFIER)
            .cloned()
            .collect();
        picks.push((C::MODIFIER.into(), context.context().into()));
        Self(picks.into())
    }
}

/// A context of a theme modifier as a typed value: the enums `theme_keys!`
/// makes per modifier implement it (`modifiers::Brand::Forest`).
pub trait ModifierContext: Copy {
    /// The modifier's name (`brand`).
    const MODIFIER: &'static str;
    /// The context's name (`forest`).
    fn context(self) -> &'static str;
}

/// A scope that picks context `context`.
impl<C: ModifierContext> From<C> for ScopeModifiers {
    fn from(context: C) -> Self {
        Self::new([(C::MODIFIER, context.context())])
    }
}

/// In TS an object of modifiers' names and contexts' (`{ brand: "forest" }`).
impl JsValue for ScopeModifiers {
    fn ts_type(_types: &mut TsTypes) -> String {
        "{ [M in keyof ThemeModifiers]?: ThemeModifiers[M] }".into()
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        let json = JsonValue::from_js(value)?;
        let Some(object) = json.as_object() else {
            return Err(PropError::expected("modifiers' contexts", &json));
        };
        let mut picks = Vec::new();
        for (name, context) in object {
            let Some(context) = context.as_str() else {
                return Err(PropError::expected("a context's name", context));
            };
            picks.push((name.as_str(), context));
        }
        Ok(Self::new(picks))
    }

    fn to_js<'a>(&self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        let object: serde_json::Map<String, JsonValue> = self
            .0
            .iter()
            .map(|(n, c)| (n.to_string(), JsonValue::from(&**c)))
            .collect();
        JsonValue::Object(object).to_js(host)
    }
}

/// In TS a theme patch's object (`ThemePatch`, as `updateTheme` takes
/// one): `{ values?, roles? }`. What it cannot read is logged and left out.
impl JsValue for Patch {
    fn ts_type(_types: &mut TsTypes) -> String {
        "ThemePatch".into()
    }

    fn from_js(value: JsIn<'_>) -> Result<Self, PropError> {
        let json = JsonValue::from_js(value)?;
        if !json.is_object() {
            return Err(PropError::expected("a theme patch", &json));
        }
        Ok(super::theme::read_patch(&json.to_string()).0)
    }

    fn to_js<'a>(&self, host: &'a dyn JsHost) -> Result<JsOwned<'a>, PropError> {
        let json: JsonValue = serde_json::from_str(&nsh_theme::write_patch(self))
            .map_err(|e| PropError(format!("a theme patch: {e}")))?;
        json.to_js(host)
    }
}

/// The props of a [`ThemeScope`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ThemeScopeProps {
    /// The color scheme the scope and what is in it are shown in, whatever
    /// the app's or the system's: `"light"` or `"dark"`. It drives `dark:`
    /// variants, `colorScheme()`, adaptive style values and platform
    /// controls, never token values: a scope shows the theme's dark values
    /// by also picking the theme's light/dark modifier in `modifiers`. Left
    /// out, the scope is shown as the view around it.
    #[prop(default)]
    pub appearance: Prop<Option<ColorScheme>>,
    /// Overrides of the scope (`{ values: { primary: "#7c3aed" } }`, a
    /// patch as `updateTheme` takes one): a layer above every override
    /// around it (the app's, the host's, outer scopes'). It sets tokens the
    /// active document declares: an entry it refuses is left out with a
    /// warning. Left out, the scope is drawn with the overrides around it.
    #[prop(default)]
    pub theme: Prop<Option<Patch>>,
    /// The contexts the scope picks of the theme's modifiers
    /// (`{ brand: "forest" }`), over the picks around it. A modifier or
    /// context the theme has not is left out with a warning.
    #[prop(default)]
    pub modifiers: Prop<Option<ScopeModifiers>>,
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
/// In it, everything that follows the color scheme follows the scope's
/// `appearance`: `dark:` classes, adaptive style values, `colorScheme()`
/// and the platform's own controls (a text input's keyboard and menus).
/// Token values follow its `modifiers` and `theme`: the classes of theme
/// tokens (`bg-background`), the components' theme and `token()` take the
/// scope's values, resolved once for the subtree. A scope shows the
/// theme's dark values by picking the theme's light/dark modifier. Scopes
/// nest: an inner one starts from the outer one's. Without `appearance`,
/// `modifiers` and `theme` a scope is shown as the view around it.
///
/// As in CSS, what a view inherited stays what it was: a scope's root sets
/// its own background and text color (`bg-background text-foreground`).
/// Menus, alerts and sheets the platform draws are the surface's, also
/// when a view in a scope opens them.
///
/// # Rust
///
/// Read the appearance and the tokens where they are used (a binding, a
/// style's closure): [`dark`], [`color`], [`tokens`], [`super::token`] and
/// typed keys answer for the scope around what reads them, and an
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
    // Its tokens: the stack around it with its picks and its patch (made
    // again when either changes), else the ones around it.
    let around = use_context::<ScopeTokens>();
    let theme = props.theme;
    let modifiers = props.modifiers;
    // What it said, logged once each while it lives.
    let logged = std::rc::Rc::new(std::cell::RefCell::new(rustc_hash::FxHashSet::default()));
    let own = computed(move || {
        let (theme, modifiers) = (theme.get(), modifiers.get());
        if theme.is_none() && modifiers.is_none() {
            return None;
        }
        let around = around.map_or_else(super::ambient::surface_tokens, |scope| scope.0.get());
        let (tokens, said) = scoped(&around, modifiers.as_ref(), theme);
        let fresh: Vec<_> = said
            .into_iter()
            .filter(|d| {
                logged
                    .borrow_mut()
                    .insert(format!("{} {}: {}", d.code, d.pointer, d.message))
            })
            .collect();
        super::theme::report(&fresh);
        Some(tokens)
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
        |attrs, tokens| attrs.rare_mut().scope_tokens = tokens,
    );
    scope.node().bind_attr(
        move || appearance.get(),
        |attrs, appearance| {
            attrs.rare_mut().appearance = match appearance {
                Some(ColorScheme::Dark) => Appearance::Dark,
                Some(ColorScheme::Light) => Appearance::Light,
                None => Appearance::System,
            }
        },
    );
    scope.child(props.children)
}
