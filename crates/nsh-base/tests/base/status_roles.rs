//! The status roles (`success`, `warning`, `info`, `highlight`): what an
//! `Editor`'s warning and info diagnostics and its search matches, and a
//! `TextView`'s alerts and marked text draw with. Each keeps its own color
//! while the theme has no token for its role, takes the token of the
//! role's name or the one the theme maps the role onto, and follows a
//! change of it.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use nsh_base::{
    Theme,
    editor::{
        EditorDiagnostic, EditorDiagnosticSeverity, EditorPosition, EditorRange, EditorSearch,
        editor,
    },
    role, text_view,
};
use nsh_core::{
    Color, MountItem, Size,
    element::{Root, Tokens},
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

/// The colors the components draw on their own, light and dark.
const WARNING: AdaptiveColor = AdaptiveColor::rgb(0xd97706, 0xfbbf24);
const INFO: AdaptiveColor = AdaptiveColor::rgb(0x2563eb, 0x60a5fa);
const MATCH: AdaptiveColor = AdaptiveColor::rgb(0xfbbf24, 0xd97706);
const NOTE: AdaptiveColor = AdaptiveColor::rgb(0x0969da, 0x4493f8);
const TIP: AdaptiveColor = AdaptiveColor::rgb(0x1a7f37, 0x3fb950);
const IMPORTANT: AdaptiveColor = AdaptiveColor::rgb(0x8250df, 0xab7df8);
const ALERT_WARNING: AdaptiveColor = AdaptiveColor::rgb(0x9a6700, 0xd29922);
const MARK: AdaptiveColor = AdaptiveColor::rgb(0xfde047, 0xca8a04);

/// A theme's colors for the roles.
const GREEN: AdaptiveColor = AdaptiveColor::rgb(0x15803d, 0x4ade80);
const AMBER: AdaptiveColor = AdaptiveColor::rgb(0xb45309, 0xfcd34d);
const BLUE: AdaptiveColor = AdaptiveColor::rgb(0x1d4ed8, 0x93c5fd);
const PINK: AdaptiveColor = AdaptiveColor::rgb(0xdb2777, 0xf472b6);

fn light(color: AdaptiveColor) -> Color {
    Color::from(color.light)
}

fn diagnostic(from: u32, to: u32, severity: EditorDiagnosticSeverity) -> EditorDiagnostic {
    let mut d = EditorDiagnostic::default();
    d.range = EditorRange::new(EditorPosition::new(0, from), EditorPosition::new(0, to));
    d.severity = severity;
    d.message = "m".into();
    d
}

fn alert(id: &'static str, markdown: &'static str) -> impl IntoElement {
    view().test_id(id).child(text_view().markdown(markdown))
}

/// An editor with a warning under `warn` and an information under `note`
/// (and a view that searches it for `find`), a text view per alert and one
/// with marked text.
#[component]
fn Harness() -> impl IntoElement {
    let search = signal(None::<EditorSearch>);
    view()
        .child(
            view().test_id("editor").w(px(600.0)).h(px(120.0)).child(
                editor()
                    .default_value("warn note find")
                    .diagnostics(vec![
                        diagnostic(0, 4, EditorDiagnosticSeverity::Warning),
                        diagnostic(5, 9, EditorDiagnosticSeverity::Info),
                    ])
                    .search(move || search.get())
                    .size_full(),
            ),
        )
        .child(
            view()
                .test_id("search")
                .w(px(20.0))
                .h(px(20.0))
                .on_press(move || search.set(Some(EditorSearch::new("find")))),
        )
        .child(alert("a-note", "> [!NOTE]\n> n"))
        .child(alert("a-tip", "> [!TIP]\n> t"))
        .child(alert("a-important", "> [!IMPORTANT]\n> m"))
        .child(alert("a-warning", "> [!WARNING]\n> w"))
        .child(alert("a-mark", "Some <mark>marked</mark> text"))
        .child(text("Ready"))
}

/// The harness drawn with `tokens` (`None`: nsh-base's default theme).
fn start<C: Component<Props = ()>>(tokens: Option<Tokens>) -> Headless {
    let root = match tokens {
        Some(tokens) => Root::of::<C>().tokens(tokens),
        None => Root::of::<C>(),
    };
    let mut config = EngineConfig {
        viewport: Size::new(px(800.0), px(900.0)),
        ..EngineConfig::new(root)
    };
    config.environment.os = Os::Linux;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).expect("the engine starts");
    app.wait_for("text:Ready").unwrap();
    app
}

/// What each part draws with now: the warning's and the information's
/// underlines, the note's, the tip's, the important one's and the
/// warning's icons, and the mark's background.
fn drawn(screen: &Screen) -> [Option<Color>; 7] {
    let span = |selector: &str| screen.span_style(selector).ok();
    [
        span("id:editor >> text:warn").and_then(|s| s.decoration_color),
        span("id:editor >> text:note").and_then(|s| s.decoration_color),
        span("id:a-note >> text:i").map(|s| s.color),
        span("id:a-tip >> text:✓").map(|s| s.color),
        span("id:a-important >> text:i").map(|s| s.color),
        span("id:a-warning >> text:!").map(|s| s.color),
        span("id:a-mark >> text:marked").and_then(|s| s.background.map(|b| b.color)),
    ]
}

fn colors(of: [AdaptiveColor; 7]) -> [Option<Color>; 7] {
    let mut out = of.map(|c| Some(light(c)));
    // A mark is drawn at 40 %.
    out[6] = Some(light(of[6].opacity(0.4)));
    out
}

/// The backgrounds of the views a search's matches are, as they are
/// mounted from now on.
fn backgrounds(app: &mut Headless) -> Arc<Mutex<Vec<Color>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let into = seen.clone();
    app.observe(move |_, batch| {
        for item in &batch.items {
            if let MountItem::ViewStyle { style, .. } = item {
                into.lock().unwrap().push(style.background_color);
            }
        }
    });
    seen
}

/// Searches the editor and waits for a view of `color`.
fn search_draws(app: &mut Headless, color: Color) {
    let seen = backgrounds(app);
    app.press("id:search").unwrap();
    let found = app.wait_until("a search match", Duration::from_secs(5), move |_| {
        seen.lock().unwrap().contains(&color)
    });
    assert!(found.is_ok(), "no match drawn in {color:?}: {found:?}");
}

#[test]
fn without_a_token_each_part_draws_its_own_color() {
    let mut app = start::<Harness>(None);
    assert_eq!(
        drawn(app.screen()),
        colors([WARNING, INFO, NOTE, TIP, IMPORTANT, ALERT_WARNING, MARK])
    );
    search_draws(&mut app, light(MATCH.opacity(0.25)));
}

#[test]
fn a_theme_with_the_roles_tokens_colors_each_part() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("success", GREEN);
    tokens.set_color("warning", AMBER);
    tokens.set_color("info", BLUE);
    tokens.set_color("highlight", PINK);
    let mut app = start::<Harness>(Some(tokens));
    // The editor's and the text view's warnings are the one color now, as
    // are their marks; an important alert has no role.
    assert_eq!(
        drawn(app.screen()),
        colors([AMBER, BLUE, BLUE, GREEN, IMPORTANT, AMBER, PINK])
    );
    search_draws(&mut app, light(PINK.opacity(0.25)));
}

#[test]
fn a_role_mapped_onto_a_token_of_the_apps_draws_that_token() {
    let mut tokens = Theme::default().into_tokens();
    tokens.set_color("status-caution", AMBER);
    tokens.set_color("status-positive", GREEN);
    // The token of the role's own name is not read while it is mapped.
    tokens.set_color("warning", PINK);
    let tokens = tokens
        .with_role(role::WARNING, "status-caution")
        .with_role(role::SUCCESS, "status-positive");
    let app = start::<Harness>(Some(tokens));
    assert_eq!(
        drawn(app.screen()),
        colors([AMBER, INFO, NOTE, GREEN, IMPORTANT, AMBER, MARK])
    );
}

/// The harness, which gives the theme a `warning` and a `highlight` after
/// its first frames.
#[component]
fn Changes() -> impl IntoElement {
    set_timeout(Duration::from_millis(100), || {
        let mut tokens = Tokens::default();
        tokens.set_color("warning", AMBER);
        tokens.set_color("highlight", PINK);
        update_tokens(tokens);
    });
    harness()
}

#[test]
fn the_parts_follow_a_change_of_the_tokens() {
    let mut app = start::<Changes>(None);
    let now = colors([AMBER, INFO, NOTE, TIP, IMPORTANT, AMBER, PINK]);
    app.wait_until("the theme's colors", Duration::from_secs(5), move |s| {
        drawn(s) == now
    })
    .unwrap();
}
