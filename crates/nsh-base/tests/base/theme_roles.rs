//! `theme()` reads each field through its role: the token a theme maps the
//! role onto, else the token of the role's own name, so an app whose theme
//! names its popover surface `surface-overlay` draws `nsh/base`'s popovers
//! in it. Without a mapping it is the theme it always was.

use nsh_base::{Theme, role, theme};
use nsh_core::{
    Color, Size,
    element::{Root, Tokens},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

fn start<C: Component<Props = ()>>(tokens: Tokens) -> Headless {
    let config = EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ..EngineConfig::new(Root::of::<C>().tokens(tokens))
    };
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

fn hex(c: nsh_core::element::AdaptiveColor) -> String {
    format!("#{:08x}", Color::from(c.light).0)
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

#[test]
fn theme_reads_the_tokens_roles_map_onto() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("surface-overlay", AdaptiveColor::rgb(0x123456, 0x654321));
    tokens.set_color("action", AdaptiveColor::rgb(0xabcdef, 0xfedcba));
    tokens.set_color("highlight", AdaptiveColor::rgb(0x00ff00, 0x00ff00));
    tokens.set_number("radius-control", 6.0);
    tokens.set_font("font-body", "Inter");
    let tokens = tokens
        .with_role(role::POPOVER, "surface-overlay")
        .with_role(role::PRIMARY, "action")
        .with_role(role::SELECTION, "highlight")
        .with_role(role::RADIUS, "radius-control")
        .with_role(role::FONT_SANS, "font-body");
    let mut app = start::<Fields>(tokens);
    app.wait_for(
        "text:popover #123456ff primary #abcdefff selection #00ff00ff radius 6 font Inter",
    )
    .unwrap();
}

#[test]
fn without_a_mapping_the_theme_is_the_same() {
    let default = Theme::default();
    let mut app = start::<Fields>(default.clone().into_tokens());
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
/// its own name, then the default theme's.
#[test]
fn a_role_mapped_onto_a_missing_token_falls_back() {
    let tokens = Theme::default()
        .into_tokens()
        .with_role(role::POPOVER, "nowhere");
    let default = Theme::default();
    let mut app = start::<Fields>(tokens);
    app.wait_for(&format!(
        "text:popover {} primary {} selection {} radius {} font system",
        hex(default.popover),
        hex(default.primary),
        hex(default.selection()),
        default.radius.0,
    ))
    .unwrap();
}
