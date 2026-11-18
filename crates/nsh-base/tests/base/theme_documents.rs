//! `nsh_base::Theme` reads each field through its role in the active
//! document alone: nsh-base's default document when nothing else is
//! active, under a design system's registered document the token the
//! document maps the role onto, and nothing (transparent) for a role that
//! document neither maps nor has a token of. `theme()` makes one theme per
//! change of its roles.

use std::time::Duration;

use nsh_base::theme;
use nsh_core::{
    Color, Size,
    element::{Patch, PatchValues, Registry, Root, set_modifier},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

fn hex(c: nsh_core::Hsla) -> String {
    format!("#{:08x}", Color::from(c).0)
}

fn start<C: Component<Props = ()>>(ui_sets: Vec<Registry>) -> Headless {
    let config = EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ui_sets,
        ..EngineConfig::new(Root::of::<C>())
    };
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

/// Under nsh-base's default document (its fallback, active as no other
/// document is), an edit of `primary` reaches `theme()`, which is one theme
/// per change.
#[test]
fn theme_is_made_once_per_change_and_follows_the_tokens() {
    let mut app = start::<Updates>(vec![nsh_base::registry()]);
    app.wait_for("text:primary #123456ff same true").unwrap();
}

/// A design system's document mapping `popover` onto its `surface-overlay`.
const DS: &str = r##"{
    "tokens": { "surface-overlay": { "kind": "color", "value": "#0a0b0c" } },
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
/// mapped onto, and a role the document neither maps nor has a token of
/// reads nothing: transparent, not the default theme's (nsh-base's roles
/// resolve in the active document alone).
#[test]
fn under_a_document_theme_reads_its_roles() {
    let mut ds = Registry::new();
    ds.theme_document(DS);
    let mut app = start::<Fields>(vec![nsh_base::registry(), ds]);
    app.wait_for("text:popover #0a0b0cff primary #00000000")
        .unwrap();
}

/// A radius with a dark value of its own, in the document's `colorMode`
/// modifier: `theme().radius` is its value in the context picked, and
/// follows a pick.
const RADII: &str = r##"{
    "tokens": { "corner": { "kind": "number", "value": 8 } },
    "modifiers": {
        "colorMode": {
            "default": "light",
            "contexts": { "light": {}, "dark": { "corner": 12 } }
        }
    },
    "resolutionOrder": ["colorMode"],
    "roles": { "radius": "corner" }
}"##;

#[component]
fn Radius() -> impl IntoElement {
    set_timeout(Duration::from_millis(50), || {
        set_modifier("colorMode", "dark")
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
