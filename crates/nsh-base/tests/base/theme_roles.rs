//! `theme()` reads each field through its role: the token a theme maps the
//! role onto, else the token of the role's own name, so an app whose theme
//! names its popover surface `surface-overlay` draws `nsh/base`'s popovers
//! in it. Without a mapping it is the default theme.

use nsh_base::{Theme, role, theme};
use nsh_core::{
    Color, Size,
    element::{Patch, PatchValues, Root},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

use crate::app_theme::{Add, default_with};

/// The app drawn with its theme document `theme` (`None`: `nsh/base`'s
/// default) and its overrides `overrides`.
fn start<C: Component<Props = ()>>(theme: Option<String>, overrides: Patch) -> Headless {
    let mut root = Root::of::<C>().theme_overrides(nsh_theme::write_patch(&overrides));
    if let Some(theme) = theme {
        root = root.theme(theme);
    }
    let config = EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ui_sets: vec![nsh_base::registry()],
        ..EngineConfig::new(root)
    };
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

fn hex(c: nsh_core::Hsla) -> String {
    format!("#{:08x}", Color::from(c).0)
}

/// What `theme()` gives for the fields roles reach.
#[component]
fn Fields() -> impl IntoElement {
    text(move || {
        let t = theme();
        format!(
            "popover {} primary {} selection {} radius {} font {}",
            hex(t.popover),
            hex(t.primary),
            hex(t.selection()),
            t.radius.0,
            t.font_sans.as_deref().unwrap_or("system"),
        )
    })
}

/// The app's theme declares tokens of its own names, and its overrides map
/// the roles onto them.
#[test]
fn theme_reads_the_tokens_roles_map_onto() {
    let theme = default_with(
        &[
            (
                "surface-overlay",
                Add::Color(AdaptiveColor::rgb(0x123456, 0x654321)),
            ),
            ("action", Add::Color(AdaptiveColor::rgb(0xabcdef, 0xfedcba))),
            (
                "highlight",
                Add::Color(AdaptiveColor::rgb(0x00ff00, 0x00ff00)),
            ),
            ("radius-control", Add::Number(6.0)),
            ("font-body", Add::Font("Inter")),
        ],
        &[],
    );
    let overrides = Patch::new()
        .role(role::POPOVER.name(), "surface-overlay")
        .role(role::PRIMARY.name(), "action")
        .role(role::SELECTION.name(), "highlight")
        .role(role::RADIUS.name(), "radius-control")
        .role(role::FONT_SANS.name(), "font-body");
    let mut app = start::<Fields>(Some(theme), overrides);
    app.wait_for(
        "text:popover #123456ff primary #abcdefff selection #00ff00ff radius 6 font Inter",
    )
    .unwrap();
}

#[test]
fn without_a_mapping_the_theme_is_the_same() {
    let default = Theme::default();
    let mut app = start::<Fields>(None, Patch::new());
    app.wait_for(&format!(
        "text:popover {} primary {} selection {} radius {} font system",
        hex(default.popover),
        hex(default.primary),
        hex(default.selection()),
        default.radius.0,
    ))
    .unwrap();
}

/// A role mapped onto a token the theme does not have reads the token of
/// its own name.
#[test]
fn a_role_mapped_onto_a_missing_token_falls_back() {
    let tokens = Patch::new().role(role::POPOVER.name(), "nowhere");
    let default = Theme::default();
    let mut app = start::<Fields>(None, tokens);
    app.wait_for(&format!(
        "text:popover {} primary {} selection {} radius {} font system",
        hex(default.popover),
        hex(default.primary),
        hex(default.selection()),
        default.radius.0,
    ))
    .unwrap();
}
