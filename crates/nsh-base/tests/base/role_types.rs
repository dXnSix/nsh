//! The status, diff and syntax roles under an app's own theme document: it
//! maps them onto its tokens in its `"roles"`, one token playing two of
//! them, and nsh-base's components draw with those tokens and follow a
//! change of them.

use std::time::Duration;

use nsh_base::{DiffIndicators, DiffLayout, diff_view, text_view};
use nsh_core::{
    Color, Size,
    element::{Registry, Root, TokenKey, Tokens},
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

const AMBER: AdaptiveColor = AdaptiveColor::rgb(0xb45309, 0xfbbf24);
const GREEN: AdaptiveColor = AdaptiveColor::rgb(0x15803d, 0x4ade80);
const VIOLET: AdaptiveColor = AdaptiveColor::rgb(0x7c3aed, 0xc4b5fd);
const TEAL: AdaptiveColor = AdaptiveColor::rgb(0x0f766e, 0x2dd4bf);

/// A design system's document: its caution color is nsh-base's warning,
/// one green is what succeeded and a diff's added lines, and its keyword
/// color is the highlighter's.
const ACME: &str = r##"{
    "cssVars": {
        "light": { "status-caution": "#b45309", "status-positive": "#15803d", "code-keyword": "#7c3aed" },
        "dark": { "status-caution": "#fbbf24", "status-positive": "#4ade80", "code-keyword": "#c4b5fd" }
    },
    "tokens": {
        "status-caution": { "kind": "color" },
        "status-positive": { "kind": "color" },
        "code-keyword": { "kind": "color" }
    },
    "roles": { "warning": "status-caution", "success": "status-positive",
               "diff-added": "status-positive", "syntax-keyword": "code-keyword" }
}"##;

const POSITIVE: TokenKey<AdaptiveColor> = TokenKey::new("status-positive");

fn light(color: AdaptiveColor) -> Color {
    Color::from(color.light)
}

/// A warning and a tip, a diff of one changed line, and a block of Rust.
fn content() -> impl IntoElement {
    view()
        .child(
            view()
                .test_id("warning")
                .child(text_view().markdown("> [!WARNING]\n> w")),
        )
        .child(
            view()
                .test_id("tip")
                .child(text_view().markdown("> [!TIP]\n> t")),
        )
        .child(
            view().test_id("diff").w(px(800.0)).h(px(200.0)).child(
                diff_view()
                    .old_value("keep\nold line\nkeep")
                    .new_value("keep\nnew line\nkeep")
                    .layout(DiffLayout::Split)
                    .indicators(DiffIndicators::Classic)
                    .size_full(),
            ),
        )
        .child(
            view()
                .test_id("code")
                .child(text_view().markdown("```rust\nlet alpha = 1;\n```")),
        )
        .child(text("Ready"))
}

#[component]
fn Shows() -> impl IntoElement {
    content()
}

fn start<C: Component<Props = ()>>() -> Headless {
    let mut acme = Registry::new();
    acme.theme_document(ACME);
    let mut config = EngineConfig {
        viewport: Size::new(px(900.0), px(700.0)),
        ui_sets: vec![nsh_base::registry(), acme],
        ..EngineConfig::new(Root::of::<C>())
    };
    config.environment.os = Os::Linux;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).expect("the engine starts");
    app.wait_for("text:Ready").unwrap();
    app
}

/// The warning's and the tip's icons, the added line's sign and the code's
/// keyword.
fn drawn(screen: &Screen) -> [Option<Color>; 4] {
    let color = |selector: &str| screen.span_style(selector).ok().map(|s| s.color);
    [
        color("id:warning >> text:!"),
        color("id:tip >> text:✓"),
        color("id:diff >> text:+"),
        color("id:code >> text:let"),
    ]
}

fn see(app: &mut Headless, of: [AdaptiveColor; 4]) {
    let expected = of.map(|c| Some(light(c)));
    let seen = app.wait_until("the roles' colors", Duration::from_secs(10), move |s| {
        drawn(s) == expected
    });
    assert!(
        seen.is_ok(),
        "{seen:?}: drawn {:?}, expected {expected:?}",
        drawn(app.screen())
    );
}

#[test]
fn a_documents_tokens_color_what_their_roles_draw() {
    let mut app = start::<Shows>();
    // The tip and the added line are the one green.
    see(&mut app, [AMBER, GREEN, GREEN, VIOLET]);
}

/// Gives the green token another green once the content is shown.
#[component]
fn Sets() -> impl IntoElement {
    set_timeout(Duration::from_millis(300), || {
        update_tokens(Tokens::default().with(POSITIVE, TEAL));
    });
    content()
}

#[test]
fn both_roles_of_one_token_follow_a_change_of_it() {
    let mut app = start::<Sets>();
    see(&mut app, [AMBER, TEAL, TEAL, VIOLET]);
}
