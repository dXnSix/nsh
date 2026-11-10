//! [`ThemeScope`]: a view that shows itself and the views in it in an
//! appearance of its own, whatever the surface shows (a dark sidebar beside
//! a light page).
//!
//! The core resolves the appearance by node (`core/classes.rs`,
//! `Core::dark_at`): `dark:` variants, theme tokens' classes and the
//! adaptive styles of Rust components pick the scope's side with nothing
//! running. What reads the appearance in code reads the nearest scope's:
//! [`super::dark`] and [`super::color`] through the context a scope
//! provides to what it renders, TS's `colorScheme()` and `token()` through
//! `nsh-js`'s (`components.rs`).

use crate::{
    element::{Memo, NoEvent, computed, provide_context, reactive::with_data, use_context},
    platform::{Appearance, ColorScheme},
    prelude::*,
};

/// The props of a [`ThemeScope`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ThemeScopeProps {
    /// The appearance the scope and what is in it are shown in, whatever
    /// the app's or the system's: `"light"` or `"dark"`. Left out, the
    /// scope is shown as the view around it.
    #[prop(default)]
    pub appearance: Prop<Option<ColorScheme>>,
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

/// A view that shows itself and everything in it in an appearance of its
/// own, whatever the app's and the system's: a dark sidebar beside a light
/// page, a light sheet of paper in a dark app. It is a view: its `class`
/// and `style` lay it out and draw it.
///
/// In it, everything that follows the appearance follows the scope's:
/// `dark:` classes, the classes of theme tokens (`bg-background`), the
/// theme colors of components, `colorScheme()` and `token()`, and the
/// platform's own controls (a text input's keyboard and menus). Scopes
/// nest: the nearest one decides. Without `appearance` a scope is shown
/// as the view around it.
///
/// As in CSS, what a view inherited stays what it was: a scope's root sets
/// its own background and text color (`bg-background text-foreground`).
/// Menus, alerts and sheets the platform draws are the surface's, also
/// when a view in a scope opens them.
///
/// # Rust
///
/// Read the appearance where it is used (a binding, a style's closure):
/// [`dark`] and [`color`] answer for the scope around what reads them,
/// and an element built before the scope places it is built outside it.
#[component]
pub fn ThemeScope(props: ThemeScopeProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let outer = use_context::<ScopeAppearance>();
    let appearance = props.appearance;
    let dark = computed(move || match appearance.get() {
        Some(ColorScheme::Dark) => true,
        Some(ColorScheme::Light) => false,
        None => outer.map_or_else(super::ambient::surface_dark, |scope| scope.0.get()),
    });
    with_data(|d| d.ambient.set_scoped());
    provide_context(ScopeAppearance(dark));
    let mut scope = view();
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
