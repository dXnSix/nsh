//! The syntax roles (`syntax-comment` … `syntax-type`): what highlighted
//! code is colored with in a `TextView`'s code blocks, an `Editor` and a
//! `DiffView`. Each kind keeps the highlighter's own color while the theme
//! has no token for its role, takes the token of the role's name or the one
//! the theme maps the role onto, and the spans, which are made once per
//! change of the code, are made again when the theme changes.

use std::time::Duration;

use nsh_base::{Theme, diff_view, editor::editor, role, text_view};
use nsh_core::{
    Color, Size,
    element::{Root, Tokens},
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

/// The highlighter's own colors, light and dark.
const KEYWORD: AdaptiveColor = AdaptiveColor::rgb(0x8250df, 0xc4a7ff);
const NUMBER: AdaptiveColor = AdaptiveColor::rgb(0x953800, 0xf5b27a);
const COMMENT: AdaptiveColor = AdaptiveColor::rgb(0x6e7781, 0x7d7a72);

/// A theme's colors for the roles.
const VIOLET: AdaptiveColor = AdaptiveColor::rgb(0x7c3aed, 0xc4b5fd);
const ORANGE: AdaptiveColor = AdaptiveColor::rgb(0xc2410c, 0xfdba74);
const SLATE: AdaptiveColor = AdaptiveColor::rgb(0x475569, 0x94a3b8);

fn light(color: AdaptiveColor) -> Color {
    Color::from(color.light)
}

/// A text view with a block of Rust, an editor and a diff view of Rust:
/// each has a `let`, the text view a number and a comment too.
fn content() -> impl IntoElement {
    view()
        .child(
            view()
                .test_id("tv")
                .child(text_view().markdown("```rust\nlet alpha = 71; // one\n```")),
        )
        .child(
            view().test_id("editor").w(px(600.0)).h(px(100.0)).child(
                editor()
                    .default_value("let beta = 2;")
                    .language("rust")
                    .size_full(),
            ),
        )
        .child(
            view().test_id("diff").w(px(800.0)).h(px(200.0)).child(
                diff_view()
                    .old_value("let gamma = 3;")
                    .new_value("let gamma = 3;\nlet delta = 4;")
                    .language("rust")
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
        viewport: Size::new(px(900.0), px(700.0)),
        ..EngineConfig::new(root)
    };
    config.environment.os = Os::Linux;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).expect("the engine starts");
    app.wait_for("text:Ready").unwrap();
    app
}

/// What the code is colored with now: the text view's keyword, number and
/// comment, then the editor's and the diff view's keywords.
fn drawn(screen: &Screen) -> [Option<Color>; 5] {
    let color = |selector: &str| screen.span_style(selector).ok().map(|s| s.color);
    [
        color("id:tv >> text:let"),
        color("id:tv >> text:71"),
        // The comment, by a word of it.
        color("id:tv >> text:one"),
        color("id:editor >> text:let"),
        // The added line's `let`.
        color("id:diff >> text:let delta"),
    ]
}

fn colors(
    keyword: AdaptiveColor,
    number: AdaptiveColor,
    comment: AdaptiveColor,
) -> [Option<Color>; 5] {
    [keyword, number, comment, keyword, keyword].map(|c| Some(light(c)))
}

/// Waits for the code to be colored as `expected` (a block is highlighted
/// off the engine's thread when it takes long).
fn see(app: &mut Headless, expected: [Option<Color>; 5]) {
    let seen = app.wait_until("the code's colors", Duration::from_secs(10), move |s| {
        drawn(s) == expected
    });
    assert!(
        seen.is_ok(),
        "{seen:?}: drawn {:?}, expected {expected:?}",
        drawn(app.screen())
    );
}

#[test]
fn without_a_token_code_keeps_the_highlighters_colors() {
    let mut app = start::<Harness>(None);
    see(&mut app, colors(KEYWORD, NUMBER, COMMENT));
}

#[test]
fn a_theme_with_the_roles_tokens_colors_the_code() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("syntax-keyword", VIOLET);
    tokens.set_color("syntax-number", ORANGE);
    tokens.set_color("syntax-comment", SLATE);
    let mut app = start::<Harness>(Some(tokens));
    see(&mut app, colors(VIOLET, ORANGE, SLATE));
}

#[test]
fn a_role_mapped_onto_a_token_of_the_apps_draws_that_token() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("code-keyword", VIOLET);
    // The token of the role's own name is not read while it is mapped.
    tokens.set_color("syntax-keyword", ORANGE);
    let tokens = tokens.with_role(role::SYNTAX_KEYWORD, "code-keyword");
    let mut app = start::<Harness>(Some(tokens));
    see(&mut app, colors(VIOLET, NUMBER, COMMENT));
}

/// The harness, which gives the theme the roles' tokens once the code is
/// shown.
#[component]
fn Changes() -> impl IntoElement {
    set_timeout(Duration::from_millis(300), || {
        let mut tokens = Tokens::default();
        tokens.set_color("syntax-keyword", VIOLET);
        tokens.set_color("syntax-number", ORANGE);
        tokens.set_color("syntax-comment", SLATE);
        update_tokens(tokens);
    });
    content()
}

#[test]
fn code_shown_already_is_colored_again_when_the_theme_changes() {
    let mut app = start::<Changes>(None);
    see(&mut app, colors(VIOLET, ORANGE, SLATE));
}

/// A block of a patch: its removed line is the highlighter's `Deletion`.
#[component]
fn Patch() -> impl IntoElement {
    view()
        .child(
            view()
                .test_id("patch")
                .child(text_view().markdown("```diff\n-gone line\n+kept line\n```")),
        )
        .child(text("Ready"))
}

/// A patch's removed lines in a code block are colored with `diff-deleted`,
/// as a `DiffView`'s deleted lines are.
#[test]
fn a_patchs_removed_lines_take_the_diff_deleted_role() {
    let removed = |s: &Screen| s.span_style("id:patch >> text:gone").ok().map(|s| s.color);
    let own = Some(light(AdaptiveColor::rgb(0xcf222e, 0xf87171)));
    let mut app = start::<Patch>(None);
    app.wait_until("the block's own red", Duration::from_secs(10), move |s| {
        removed(s) == own
    })
    .unwrap();
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("diff-deleted", ORANGE);
    let mut app = start::<Patch>(Some(tokens));
    app.wait_until("the theme's color", Duration::from_secs(10), move |s| {
        removed(s) == Some(light(ORANGE))
    })
    .unwrap();
}
