//! `set_theme` of `nsh_base::Theme` and `radius`'s dark value: `radius` is
//! the token's light value, so a theme read, changed in a color and set
//! keeps the dark radius, and a radius it changes is both appearances'.

use std::time::Duration;

use nsh_base::{Theme, theme};
use nsh_core::{
    Color, Size,
    element::{Root, Tokens},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

fn start<C: Component<Props = ()>>() -> Headless {
    let config = EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ..EngineConfig::new(Root::of::<C>())
    };
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

/// Gives `radius` 10 points, and 6 in the dark appearance.
fn dark_radius() {
    let mut tokens = Tokens::default();
    tokens.set_number("radius", 10.0);
    tokens.set_dark_number("radius", 6.0);
    update_tokens(tokens);
}

/// `radius` in the light and the dark appearance, and the primary.
fn shown() -> String {
    let tokens = tokens();
    format!(
        "radius {:?} {:?} primary #{:08x}",
        tokens.number_in("radius", false),
        tokens.number_in("radius", true),
        Color::from(theme().primary.light).0
    )
}

/// Reads the theme, changes its primary and sets it.
#[component]
fn SetsAColor() -> impl IntoElement {
    dark_radius();
    set_timeout(Duration::from_millis(50), || {
        let mut theme: Theme = (*theme()).clone();
        theme.primary = AdaptiveColor::rgb(0x123456, 0x123456);
        set_theme(theme);
    });
    text(shown)
}

#[test]
fn a_theme_read_and_set_with_another_color_keeps_the_dark_radius() {
    let mut app = start::<SetsAColor>();
    app.wait_for("text:radius Some(10.0) Some(6.0) primary #123456ff")
        .unwrap();
}

/// Reads the theme, changes its radius and sets it.
#[component]
fn SetsTheRadius() -> impl IntoElement {
    dark_radius();
    set_timeout(Duration::from_millis(50), || {
        let mut theme: Theme = (*theme()).clone();
        theme.radius = px(16.0);
        theme.primary = AdaptiveColor::rgb(0x123456, 0x123456);
        set_theme(theme);
    });
    text(shown)
}

#[test]
fn a_radius_the_theme_changes_is_both_appearances() {
    let mut app = start::<SetsTheRadius>();
    app.wait_for("text:radius Some(16.0) Some(16.0) primary #123456ff")
        .unwrap();
}
