//! The syntax roles (`syntax-comment` … `syntax-type`): what highlighted
//! code is colored with in a `TextView`'s code blocks, an `Editor` and a
//! `DiffView`. Each kind keeps the highlighter's own color while the app's
//! theme document has no token for its role, takes the token of the role's
//! name or the one the theme maps the role onto, and the spans, which are
//! made once per change of the code, are made again when the theme changes.

use std::time::Duration;

use nsh_base::{diff_view, editor::editor, role, text_view};
use nsh_core::{
    Color, Size,
    element::{Patch as ThemePatch, PatchValues},
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

use crate::app_theme::{Add, default_with};

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

/// The harness drawn with the app's theme document `theme` (`None`:
/// nsh-base's default theme) and its overrides `overrides`.
fn start<C: Component<Props = ()>>(
    theme: Option<String>,
    overrides: Option<ThemePatch>,
) -> Headless {
    let mut root = nsh_testing::windowed_root::<C>();
    if let Some(theme) = theme {
        root = root.theme(theme);
    }
    if let Some(overrides) = overrides {
        root = root.theme_overrides(nsh_theme::write_patch(&overrides));
    }
    let mut config = EngineConfig {
        viewport: Size::new(px(900.0), px(700.0)),
        ui_sets: vec![nsh_base::registry()],
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
    let mut app = start::<Harness>(None, None);
    see(&mut app, colors(KEYWORD, NUMBER, COMMENT));
}

#[test]
fn a_theme_with_the_roles_tokens_colors_the_code() {
    let theme = default_with(
        &[
            ("syntax-keyword", Add::Color(VIOLET)),
            ("syntax-number", Add::Color(ORANGE)),
            ("syntax-comment", Add::Color(SLATE)),
        ],
        &[],
    );
    let mut app = start::<Harness>(Some(theme), None);
    see(&mut app, colors(VIOLET, ORANGE, SLATE));
}

#[test]
fn a_role_mapped_onto_a_token_of_the_apps_draws_that_token() {
    let theme = default_with(
        &[
            ("code-keyword", Add::Color(VIOLET)),
            // The token of the role's own name is not read while it is
            // mapped.
            ("syntax-keyword", Add::Color(ORANGE)),
        ],
        &[],
    );
    let overrides = ThemePatch::new().role(role::SYNTAX_KEYWORD.name(), "code-keyword");
    let mut app = start::<Harness>(Some(theme), Some(overrides));
    see(&mut app, colors(VIOLET, NUMBER, COMMENT));
}

/// The harness, which gives the roles' tokens other values once the code
/// is shown.
#[component]
fn Changes() -> impl IntoElement {
    set_timeout(Duration::from_millis(300), || {
        let tokens = ThemePatch::new();
        let tokens = tokens.value("syntax-keyword", VIOLET.light);
        let tokens = tokens.value("syntax-number", ORANGE.light);
        let tokens = tokens.value("syntax-comment", SLATE.light);
        update_tokens(tokens);
    });
    content()
}

#[test]
fn code_shown_already_is_colored_again_when_the_theme_changes() {
    // The highlighter's own colors at first, declared by the app's theme.
    let theme = default_with(
        &[
            ("syntax-keyword", Add::Color(KEYWORD)),
            ("syntax-number", Add::Color(NUMBER)),
            ("syntax-comment", Add::Color(COMMENT)),
        ],
        &[],
    );
    let mut app = start::<Changes>(Some(theme), None);
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
    let mut app = start::<Patch>(None, None);
    app.wait_until("the block's own red", Duration::from_secs(10), move |s| {
        removed(s) == own
    })
    .unwrap();
    let theme = default_with(&[("diff-deleted", Add::Color(ORANGE))], &[]);
    let mut app = start::<Patch>(Some(theme), None);
    app.wait_until("the theme's color", Duration::from_secs(10), move |s| {
        removed(s) == Some(light(ORANGE))
    })
    .unwrap();
}
