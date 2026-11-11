//! The diff roles (`diff-added`, `diff-deleted`): what a `DiffView` draws
//! added and deleted lines with. Its signs and changed words keep the
//! view's own green and red while the theme has no token for the roles,
//! take the token of a role's name or the one the theme maps the role
//! onto, and follow a change of it.

use std::time::Duration;

use nsh_base::{DiffIndicators, DiffLayout, Theme, diff_view, role};
use nsh_core::{
    Color, Size,
    element::{Root, Tokens},
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

/// The colors the view draws on its own, light and dark.
const ADDED: AdaptiveColor = AdaptiveColor::rgb(0x16a34a, 0x4ade80);
const DELETED: AdaptiveColor = AdaptiveColor::rgb(0xdc2626, 0xf87171);

/// A theme's colors for the roles.
const TEAL: AdaptiveColor = AdaptiveColor::rgb(0x0f766e, 0x5eead4);
const ROSE: AdaptiveColor = AdaptiveColor::rgb(0xbe123c, 0xfda4af);

fn light(color: AdaptiveColor) -> Color {
    Color::from(color.light)
}

/// A diff of one changed line, `old word here` to `new word here`, split,
/// with `+` and `−` before the text.
fn content() -> impl IntoElement {
    view()
        .child(
            view().test_id("diff").w(px(800.0)).h(px(300.0)).child(
                diff_view()
                    .old_value("keep\nold word here\nkeep")
                    .new_value("keep\nnew word here\nkeep")
                    .layout(DiffLayout::Split)
                    .indicators(DiffIndicators::Classic)
                    .size_full(),
            ),
        )
        .child(text("Ready"))
}

#[component]
fn Harness() -> impl IntoElement {
    content()
}

fn start<C: Component<Props = ()>>(tokens: Option<Tokens>) -> Headless {
    let root = match tokens {
        Some(tokens) => Root::of::<C>().tokens(tokens),
        None => Root::of::<C>(),
    };
    let mut config = EngineConfig {
        viewport: Size::new(px(900.0), px(600.0)),
        ..EngineConfig::new(root)
    };
    config.environment.os = Os::Linux;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).expect("the engine starts");
    app.wait_for("text:Ready").unwrap();
    app.wait_for("id:diff >> text:+").unwrap();
    app
}

/// What the view draws with now: the added line's sign and its changed
/// word's background, then the deleted line's.
fn drawn(screen: &Screen) -> [Option<Color>; 4] {
    let span = |selector: &str| screen.span_style(selector).ok();
    let word = |selector: &str| span(selector).and_then(|s| s.background.map(|b| b.color));
    [
        span("id:diff >> text:+").map(|s| s.color),
        word("id:diff >> text:new"),
        span("id:diff >> text:−").map(|s| s.color),
        word("id:diff >> text:old"),
    ]
}

/// The signs in `added` and `deleted`, the changed words on them at 30 %.
fn colors(added: AdaptiveColor, deleted: AdaptiveColor) -> [Option<Color>; 4] {
    [
        Some(light(added)),
        Some(light(added.opacity(0.3))),
        Some(light(deleted)),
        Some(light(deleted.opacity(0.3))),
    ]
}

#[test]
fn without_a_token_the_view_draws_its_own_green_and_red() {
    let app = start::<Harness>(None);
    assert_eq!(drawn(app.screen()), colors(ADDED, DELETED));
}

#[test]
fn a_theme_with_the_roles_tokens_colors_the_lines() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("diff-added", TEAL);
    tokens.set_color("diff-deleted", ROSE);
    let app = start::<Harness>(Some(tokens));
    assert_eq!(drawn(app.screen()), colors(TEAL, ROSE));
}

#[test]
fn a_role_mapped_onto_a_token_of_the_apps_draws_that_token() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("status-positive", TEAL);
    // The token of the role's own name is not read while it is mapped.
    tokens.set_color("diff-added", ROSE);
    let tokens = tokens.with_role(role::DIFF_ADDED, "status-positive");
    let app = start::<Harness>(Some(tokens));
    assert_eq!(drawn(app.screen()), colors(TEAL, DELETED));
}

/// The harness, which gives the theme the roles' tokens after its first
/// frames.
#[component]
fn Changes() -> impl IntoElement {
    set_timeout(Duration::from_millis(100), || {
        let mut tokens = Tokens::default();
        tokens.set_color("diff-added", TEAL);
        tokens.set_color("diff-deleted", ROSE);
        update_tokens(tokens);
    });
    content()
}

#[test]
fn the_lines_follow_a_change_of_the_tokens() {
    let mut app = start::<Changes>(None);
    let now = colors(TEAL, ROSE);
    app.wait_until("the theme's colors", Duration::from_secs(5), move |s| {
        drawn(s) == now
    })
    .unwrap();
}
