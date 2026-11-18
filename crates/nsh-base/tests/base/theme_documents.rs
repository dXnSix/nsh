//! `nsh_base::Theme` reads each field through its role: without a document
//! the token of the role's name (shadcn's), under an app's registered theme
//! document the token the document maps the role onto, and shadcn's tokens
//! no document has the default theme's. `theme()` makes one theme per change
//! of the tokens.

use std::time::Duration;

use nsh_base::{Theme, theme};
use nsh_core::{
    Color, Size,
    element::{Patch, PatchValues, Registry, Root},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

fn hex(c: AdaptiveColor) -> String {
    format!("#{:08x}", Color::from(c.light).0)
}

fn start<C: Component<Props = ()>>(ui_sets: Vec<Registry>) -> Headless {
    let mut config = EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ..EngineConfig::new(Root::of::<C>())
    };
    if !ui_sets.is_empty() {
        config.ui_sets = ui_sets;
    }
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

/// Shows `theme()`'s primary, and whether two reads are one theme; then
/// changes the primary token.
#[component]
fn Updates() -> impl IntoElement {
    set_timeout(Duration::from_millis(50), || {
        let change = Patch::new();
        let change = change.value("primary", AdaptiveColor::rgb(0x123456, 0x123456).light);
        update_tokens(change);
    });
    text(move || {
        let (a, b) = (theme(), theme());
        format!(
            "primary {} same {}",
            hex(a.primary),
            std::sync::Arc::ptr_eq(&a, &b)
        )
    })
}

#[test]
fn theme_is_made_once_per_change_and_follows_the_tokens() {
    let mut app = start::<Updates>(Vec::new());
    app.wait_for("text:primary #123456ff same true").unwrap();
}

/// A design system's document mapping `popover` onto its `surface-overlay`.
const DS: &str = r##"{
    "cssVars": { "theme": { "surface-overlay": "#0a0b0c" } },
    "tokens": { "surface-overlay": { "kind": "color" } },
    "roles": { "popover": "surface-overlay" }
}"##;

#[component]
fn Fields() -> impl IntoElement {
    text(move || {
        let t = theme();
        format!("popover {} primary {}", hex(t.popover), hex(t.primary))
    })
}

/// Under a registered document, `theme()` reads the tokens its roles are
/// mapped onto, and shadcn's tokens no document has are the default
/// theme's.
#[test]
fn under_a_document_theme_reads_its_roles() {
    let mut ds = Registry::new();
    ds.theme_document(DS);
    let mut app = start::<Fields>(vec![nsh_base::registry(), ds]);
    app.wait_for(&format!(
        "text:popover #0a0b0cff primary {}",
        hex(Theme::default().primary)
    ))
    .unwrap();
}

/// A radius with a dark value of its own: `theme().radius` is its value in
/// the appearance shown, and follows the appearance.
const RADII: &str = r##"{
    "cssVars": { "light": { "corner": "8px" }, "dark": { "corner": "12px" } },
    "tokens": { "corner": { "kind": "number" } },
    "roles": { "radius": "corner" }
}"##;

#[component]
fn Radius() -> impl IntoElement {
    set_timeout(Duration::from_millis(50), || {
        set_color_scheme(nsh_core::platform::Appearance::Dark)
    });
    text(move || format!("radius {}", theme().radius.0))
}

#[test]
fn theme_reads_the_radius_in_the_appearance_shown() {
    let mut ds = Registry::new();
    ds.theme_document(RADII);
    let mut app = start::<Radius>(vec![nsh_base::registry(), ds]);
    app.wait_for("text:radius 12").unwrap();
}
