//! `nsh_base::Theme` over the theme format (BASE-ROLES, BASE-DEFAULT and
//! CORE-SIGNALS' `theme()` clause): nsh-base's roles are declarations with
//! value types; its fields read single values through them in the active
//! document alone (a theme of other names maps them in `roles`, one that
//! maps none draws transparent with no radius); `theme()` is a memo over
//! its role reads; and nsh-base's fallback document is the old default
//! converted, light by default and dark in `colorMode`'s `dark` context.

use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

use nsh_base::{DEFAULT_THEME, Theme, role, text_view, theme};
use nsh_core::{
    Color, Hsla, Size,
    element::{
        Patch, PatchValues, Root, Tokens, ValueType, effect, set_modifier,
        theme_convert::{self, OLD_DEFAULT},
    },
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

use crate::app_theme::{Add, default_with};

fn hex(c: Hsla) -> String {
    format!("#{:08x}", Color::from(c).0)
}

/// The packed color of transparent.
fn transparent() -> Color {
    Color::from(nsh_core::color::transparent_black())
}

/// `C` drawn as a desktop app serving nsh-base, with its own theme document
/// `theme` (`None`: none, nsh-base's fallback is active).
fn start<C: Component<Props = ()>>(theme: Option<String>) -> Headless {
    let mut root = Root::of::<C>();
    if let Some(theme) = theme {
        root = root.theme(theme);
    }
    let mut config = EngineConfig {
        viewport: Size::new(px(800.0), px(600.0)),
        ui_sets: vec![nsh_base::registry()],
        ..EngineConfig::new(root)
    };
    config.environment.os = Os::Linux;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).expect("the engine starts");
    app.wait_for("text:Ready").unwrap();
    app
}

/// Each of `set_color` and `set_number` changes its token and keeps every
/// edit made before it (an embedding app sets them one by one). A patch never
/// adds a name: `set_font` of a font the document doesn't declare changes
/// nothing, and keeps the earlier edits too.
#[test]
fn successive_theme_edits_keep_earlier_ones() {
    let mut theme = Theme::default();
    theme.set_color("primary", AdaptiveColor::rgb(0x123456, 0x123456).light);
    theme.set_color("background", AdaptiveColor::rgb(0x654321, 0x654321).light);
    theme.set_number("radius", 7.0);
    theme.set_font("font-sans", "Inter");
    theme.set_font("font-mono", "Iosevka");

    assert_eq!(hex(theme.primary), "#123456ff");
    assert_eq!(hex(theme.background), "#654321ff");
    assert_eq!(theme.radius.0, 7.0);
    assert_eq!(theme.font_sans, None);
    assert_eq!(theme.font_mono, None);
    assert_eq!(hex(theme.color("primary").unwrap()), "#123456ff");
    assert_eq!(theme.number("radius"), Some(7.0));
}

/// shadcn's colors, every one: the theme's color fields, each the role of
/// its name.
const SHADCN: &[&str] = &[
    "background",
    "foreground",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    "primary",
    "primary-foreground",
    "secondary",
    "secondary-foreground",
    "muted",
    "muted-foreground",
    "accent",
    "accent-foreground",
    "destructive",
    "border",
    "input",
    "ring",
    "chart-1",
    "chart-2",
    "chart-3",
    "chart-4",
    "chart-5",
    "sidebar",
    "sidebar-foreground",
    "sidebar-primary",
    "sidebar-primary-foreground",
    "sidebar-accent",
    "sidebar-accent-foreground",
    "sidebar-border",
    "sidebar-ring",
];

/// nsh-base's 48 roles are declarations with their value types and docs,
/// none required: the colors `color`, `radius` a `number`, the two fonts
/// `font`; and its registry registers them.
#[test]
fn nsh_bases_roles_are_declared_with_their_value_types() {
    let all = role::all();
    assert_eq!(all.len(), 48);
    for declaration in &all {
        let expected = match declaration.name.as_str() {
            "radius" => ValueType::Number,
            "font-sans" | "font-mono" => ValueType::Font,
            _ => ValueType::Color,
        };
        assert_eq!(declaration.value_type, expected, "{}", declaration.name);
        assert!(!declaration.required, "{}", declaration.name);
        assert!(declaration.doc.is_some(), "{}", declaration.name);
    }
    for name in SHADCN {
        assert!(
            all.iter()
                .any(|d| d.name == *name && d.value_type == ValueType::Color),
            "{name}"
        );
    }
    // The typed constants read their declared types.
    assert_eq!(role::RADIUS.name(), "radius");
    assert_eq!(
        nsh_core::element::Role::<f32>::VALUE_TYPE,
        ValueType::Number
    );
    let registry = nsh_base::registry();
    let registered = registry.role_specs();
    for declaration in &all {
        assert!(registered.contains(declaration), "{}", declaration.name);
    }
}

/// A paragraph with a link in a `TextView`, and what `theme()` gives.
#[component]
fn Shown() -> impl IntoElement {
    view()
        .child(
            view()
                .test_id("tv")
                .child(text_view().markdown("plain words [linked](https://example.com)")),
        )
        .child(text(move || {
            let t = theme();
            format!(
                "theme primary {} popover {} radius {}",
                hex(t.primary),
                hex(t.popover),
                t.radius.0
            )
        }))
        .child(text("Ready"))
}

/// The text view's text and link colors.
fn drawn(screen: &Screen) -> [Option<Color>; 2] {
    let color = |selector: &str| screen.span_style(selector).ok().map(|s| s.color);
    [color("id:tv >> text:plain"), color("id:tv >> text:linked")]
}

/// A theme whose tokens have names of its own: nsh-base's roles mapped
/// onto them make its components draw those tokens.
const OWN_NAMES: &str = r##"{
    "tokens": {
        "ink": { "kind": "color", "value": "#112233" },
        "brand": { "kind": "color", "value": "#445566" },
        "surface-raised": { "kind": "color", "value": "#778899" },
        "corner": { "kind": "number", "value": 6 }
    },
    "roles": { "foreground": "ink", "primary": "brand", "popover": "surface-raised", "radius": "corner" }
}"##;

#[test]
fn a_component_draws_the_tokens_a_theme_of_other_names_maps_its_roles_onto() {
    let mut app = start::<Shown>(Some(OWN_NAMES.to_owned()));
    app.wait_for("text:theme primary #445566ff popover #778899ff radius 6")
        .unwrap();
    assert_eq!(
        drawn(app.screen()),
        [Some(Color(0x112233ff)), Some(Color(0x445566ff))]
    );
}

/// The same tokens with no `roles`: the active document maps none of
/// nsh-base's roles and has no token of their names, so its components draw
/// transparent, with no radius; nothing comes from nsh-base's default.
#[test]
fn under_a_document_that_maps_none_of_its_roles_colors_are_transparent_and_the_radius_0() {
    let unmapped = OWN_NAMES.replace(
        r#""roles": { "foreground": "ink", "primary": "brand", "popover": "surface-raised", "radius": "corner" }"#,
        r#""roles": {}"#,
    );
    let mut app = start::<Shown>(Some(unmapped));
    app.wait_for("text:theme primary #00000000 popover #00000000 radius 0")
        .unwrap();
    assert_eq!(
        drawn(app.screen()),
        [Some(transparent()), Some(transparent())]
    );
}

/// Counts the runs of an effect reading `theme()`; each press of `count`
/// adds the count so far to the text.
#[component]
fn Reruns() -> impl IntoElement {
    let runs = Rc::new(Cell::new(0_usize));
    let counted = runs.clone();
    effect(move || {
        let _ = theme();
        counted.set(counted.get() + 1);
    });
    let counts = signal(Vec::<usize>::new());
    let press =
        |id: &'static str, f: Box<dyn Fn()>| view().test_id(id).w(px(20.0)).h(px(20.0)).on_press(f);
    view()
        // A token no role of nsh-base reads.
        .child(press(
            "other",
            Box::new(|| {
                update_tokens(
                    Patch::new().value("note", Hsla::from(nsh_core::color::rgb(0x00ff00))),
                );
            }),
        ))
        // The token of one of its roles.
        .child(press(
            "primary",
            Box::new(|| {
                update_tokens(
                    Patch::new().value("primary", Hsla::from(nsh_core::color::rgb(0x123456))),
                );
            }),
        ))
        // One of its roles mapped onto another token.
        .child(press(
            "map",
            Box::new(|| update_tokens(Patch::new().role(role::POPOVER.name(), "note"))),
        ))
        .child(press(
            "count",
            Box::new(move || counts.update(|c| c.push(runs.get()))),
        ))
        .child(text(move || {
            let counts = counts.get();
            let listed: Vec<String> = counts.iter().map(usize::to_string).collect();
            format!("counts {} {}", counts.len(), listed.join(" "))
        }))
        .child(text("Ready"))
}

/// The counts once `n` are shown.
fn counts(app: &mut Headless, n: usize) -> Vec<usize> {
    let prefix = format!("counts {n} ");
    let found = |s: &Screen| s.texts().iter().any(|t| t.starts_with(&prefix));
    app.wait_until("the counts", Duration::from_secs(5), found)
        .unwrap();
    let screen = app.screen();
    let line = screen
        .texts()
        .into_iter()
        .find(|t| t.starts_with(&prefix))
        .unwrap()
        .to_owned();
    line[prefix.len()..]
        .split(' ')
        .map(|n| n.parse().unwrap())
        .collect()
}

/// A `theme()` reader runs again when the value or the mapping of one of
/// nsh-base's roles changes, and not when a token no role of it reads does.
#[test]
fn a_theme_reader_reruns_only_when_one_of_its_roles_changes() {
    let theme = default_with(
        &[("note", Add::Color(AdaptiveColor::rgb(0x0000ff, 0x0000ff)))],
        &[],
    );
    let mut app = start::<Reruns>(Some(theme));
    app.press("id:count").unwrap();
    app.press("id:other").unwrap();
    app.press("id:count").unwrap();
    app.press("id:primary").unwrap();
    app.press("id:count").unwrap();
    app.press("id:map").unwrap();
    app.press("id:count").unwrap();
    let seen = counts(&mut app, 4);
    let start = seen[0];
    assert!(start >= 1, "{seen:?}");
    assert_eq!(seen, [start, start, start + 1, start + 2]);
}

/// The fallback document is the converter's output for its fixtures' copy
/// of the old default (CONVERT-RULES), with a `colorMode` modifier, light by
/// default: shadcn's 31 colors and the radius, no font. nsh-base registers
/// it as its fallback, and no document of its own.
#[test]
fn the_fallback_document_is_the_old_default_converted() {
    let (converted, said) = theme_convert::convert(OLD_DEFAULT, None);
    assert!(said.iter().all(|d| !d.is_error()), "{said:#?}");
    let read = nsh_theme::read(DEFAULT_THEME);
    assert!(read.diagnostics.is_empty(), "{:#?}", read.diagnostics);
    let document = read.document;
    assert_eq!(document, nsh_theme::read(&converted).document);

    let color_mode = document.modifiers.get("colorMode").expect("colorMode");
    assert_eq!(color_mode.default, "light");
    let contexts: Vec<&str> = color_mode.contexts.keys().collect();
    assert_eq!(contexts, ["light", "dark"]);
    let mut colors: Vec<&str> = document
        .tokens
        .iter()
        .filter(|(_, t)| t.value_type == ValueType::Color)
        .map(|(name, _)| name)
        .collect();
    colors.sort_unstable();
    let mut expected = SHADCN.to_vec();
    expected.sort_unstable();
    assert_eq!(colors, expected);
    let others: Vec<(&str, ValueType)> = document
        .tokens
        .iter()
        .filter(|(_, t)| t.value_type != ValueType::Color)
        .map(|(name, t)| (name, t.value_type))
        .collect();
    assert_eq!(others, [("radius", ValueType::Number)]);
    assert!(DEFAULT_THEME.find("cssVars").is_none()); // converted by nsh theme convert

    let registry = nsh_base::registry();
    assert_eq!(
        registry.fallback_theme_documents(),
        [(nsh_base::SPECIFIER, DEFAULT_THEME)]
    );
    assert!(registry.theme_documents().is_empty());
}

/// The theme's color fields, by their roles' names.
type Field = (&'static str, fn(&Theme) -> Hsla);

const FIELDS: &[Field] = &[
    ("background", |t| t.background),
    ("foreground", |t| t.foreground),
    ("card", |t| t.card),
    ("card-foreground", |t| t.card_foreground),
    ("popover", |t| t.popover),
    ("popover-foreground", |t| t.popover_foreground),
    ("primary", |t| t.primary),
    ("primary-foreground", |t| t.primary_foreground),
    ("secondary", |t| t.secondary),
    ("secondary-foreground", |t| t.secondary_foreground),
    ("muted", |t| t.muted),
    ("muted-foreground", |t| t.muted_foreground),
    ("accent", |t| t.accent),
    ("accent-foreground", |t| t.accent_foreground),
    ("destructive", |t| t.destructive),
    ("border", |t| t.border),
    ("input", |t| t.input),
    ("ring", |t| t.ring),
    ("chart-1", |t| t.chart_1),
    ("chart-2", |t| t.chart_2),
    ("chart-3", |t| t.chart_3),
    ("chart-4", |t| t.chart_4),
    ("chart-5", |t| t.chart_5),
    ("sidebar", |t| t.sidebar),
    ("sidebar-foreground", |t| t.sidebar_foreground),
    ("sidebar-primary", |t| t.sidebar_primary),
    ("sidebar-primary-foreground", |t| {
        t.sidebar_primary_foreground
    }),
    ("sidebar-accent", |t| t.sidebar_accent),
    ("sidebar-accent-foreground", |t| t.sidebar_accent_foreground),
    ("sidebar-border", |t| t.sidebar_border),
    ("sidebar-ring", |t| t.sidebar_ring),
];

/// Every field of `theme`, as the app shows it.
fn fields(theme: &Theme) -> String {
    let mut out: Vec<String> = FIELDS
        .iter()
        .map(|(name, field)| format!("{name}={}", hex(field(theme))))
        .collect();
    out.push(format!("radius={}", theme.radius.0));
    out.push(format!("font-sans={:?}", theme.font_sans.as_deref()));
    out.push(format!("font-mono={:?}", theme.font_mono.as_deref()));
    out.join(" ")
}

/// What the fields read in context `context` of `colorMode`, by the
/// converter's output for the old default resolved there: what the old
/// reader read in that appearance (the converter's equivalence test,
/// `equivalence_default_theme`).
fn expected(context: &str) -> String {
    let (converted, _) = theme_convert::convert(OLD_DEFAULT, None);
    let document = Arc::new(nsh_theme::read(&converted).document);
    let selection = nsh_theme::Selection::new().with("colorMode", context);
    let (tokens, said) = Tokens::resolve(document, selection, Vec::new());
    assert!(said.is_empty(), "{said:#?}");
    let mut out: Vec<String> = FIELDS
        .iter()
        .map(|(name, _)| format!("{name}={}", hex(tokens.color(name).expect(name))))
        .collect();
    out.push(format!("radius={}", tokens.number("radius").unwrap()));
    out.push(format!("font-sans={:?}", None::<&str>));
    out.push(format!("font-mono={:?}", None::<&str>));
    out.join(" ")
}

/// What `theme()` gives, and a press that picks `colorMode`'s `dark`.
#[component]
fn Appearances() -> impl IntoElement {
    view()
        .child(
            view()
                .test_id("dark")
                .w(px(20.0))
                .h(px(20.0))
                .on_press(|| set_modifier("colorMode", "dark")),
        )
        .child(text(move || fields(&theme())))
        .child(text("Ready"))
}

/// An app with no theme, whose sets register no other document, draws
/// nsh-base's fallback: light by default, dark once `colorMode`'s `dark` is
/// picked, each equal to the old default's values in that appearance.
#[test]
fn an_app_without_a_theme_draws_the_default_in_light_then_in_dark() {
    let (light, dark) = (expected("light"), expected("dark"));
    // The old reader's values the old default's test pinned: oklch(0.205 0
    // 0) and oklch(0.922 0 0) for `primary`, oklch(1 0 0 / 10%) for the
    // dark `border`, 0.625rem for the radius.
    assert!(light.contains("primary=#171717ff "), "{light}");
    assert!(dark.contains("primary=#e5e5e5ff "), "{dark}");
    assert!(dark.contains("border=#ffffff1a "), "{dark}");
    assert!(light.contains("radius=10 "), "{light}");
    assert_ne!(light, dark);

    let mut app = start::<Appearances>(None);
    let shows = |expected: String| move |s: &Screen| s.shows(&expected);
    app.wait_until(
        "the light default",
        Duration::from_secs(5),
        shows(light.clone()),
    )
    .unwrap_or_else(|e| panic!("{e}: {:?}", app.screen().texts()));
    assert_eq!(fields(&Theme::default()), light);
    app.press("id:dark").unwrap();
    app.wait_until(
        "the dark default",
        Duration::from_secs(5),
        shows(dark.clone()),
    )
    .unwrap_or_else(|e| panic!("{e}: {:?}", app.screen().texts()));
}
