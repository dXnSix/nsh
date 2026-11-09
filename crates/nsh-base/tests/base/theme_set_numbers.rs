//! `set_theme` of `nsh_base::Theme` and `radius`'s dark value: `radius` is
//! the token's light value, so a theme read, changed in a color and set
//! keeps the dark radius, and a radius it changes is both appearances'. A
//! derived type's number fields keep theirs the same way.

use std::time::Duration;

use nsh_base::{Theme, theme};
use nsh_core::{
    Color, Size,
    element::{AppTheme, Root, Tokens},
    geometry::Points,
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

/// A design system's theme, derived: a color and the cards' radius.
#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
struct Ds {
    #[token(color)]
    ink: AdaptiveColor,
    #[token(radius)]
    card: Points,
}

/// Gives `radius-card` a dark value, then reads the derived theme, changes
/// its color and sets it.
#[component]
fn SetsADerivedTheme() -> impl IntoElement {
    let mut card = Tokens::default();
    card.set_number("radius-card", 10.0);
    card.set_dark_number("radius-card", 6.0);
    update_tokens(card);
    set_timeout(Duration::from_millis(50), || {
        let mut ds: Ds = (*theme_of::<Ds>()).clone();
        assert_eq!(ds.card, px(10.0));
        ds.ink = AdaptiveColor::rgb(0x123456, 0x123456);
        set_theme(ds);
    });
    text(|| {
        let tokens = tokens();
        format!(
            "card {:?} {:?} ink #{:08x}",
            tokens.number_in("radius-card", false),
            tokens.number_in("radius-card", true),
            Color::from(theme_of::<Ds>().ink.light).0
        )
    })
}

#[test]
fn a_derived_theme_read_and_set_with_another_color_keeps_a_numbers_dark_value() {
    let mut app = start::<SetsADerivedTheme>();
    app.wait_for("text:card Some(10.0) Some(6.0) ink #123456ff")
        .unwrap();
}
