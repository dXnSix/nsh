//! The one-map token store and the active document (CORE-STORE,
//! CORE-ACTIVE, CORE-REPORT): every value type in its Rust type, reads by
//! name and by key, names deciding nothing, the active document's
//! precedence and switches, `Styled::typography`, and a log line for each
//! former panic.

use std::{cell::RefCell, sync::Arc};

use nsh_theme::{Selection, code};

use crate::{
    Color, Core,
    core::tests::{FakeEngine, text_item},
    element::{
        BlurKind, CubicBezier, Patch, PatchValues, Registry, Root, Token, TokenError, TokenKey,
        Tokens, Typography, ValueType, theme::LOG_TARGET,
    },
    geometry::{Points, size_pt},
    prelude::*,
};

thread_local! {
    /// The theme lines logged on this thread while [`logged`] runs.
    static LOGGED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Keeps the lines of the target `nsh::theme`, per thread.
struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target() == LOG_TARGET
    }

    fn log(&self, record: &log::Record) {
        if record.target() == LOG_TARGET {
            LOGGED.with(|lines| {
                if let Some(lines) = lines.borrow_mut().as_mut() {
                    lines.push(record.args().to_string());
                }
            });
        }
    }

    fn flush(&self) {}
}

static CAPTURE: Capture = Capture;

/// `f`'s result and the theme lines it logged.
pub(super) fn logged<R>(f: impl FnOnce() -> R) -> (R, Vec<String>) {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        if log::set_logger(&CAPTURE).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
    });
    LOGGED.with(|lines| *lines.borrow_mut() = Some(Vec::new()));
    let out = f();
    let lines = LOGGED.with(|lines| lines.borrow_mut().take().unwrap_or_default());
    (out, lines)
}

/// Whether one of `lines` starts with diagnostic code `code`.
pub(super) fn said(lines: &[String], code: &str) -> bool {
    lines.iter().any(|l| l.starts_with(&format!("{code} ")))
}

/// The tokens of document `text`, which reads with no diagnostic.
pub(super) fn tokens_of(text: &str) -> Tokens {
    let read = nsh_theme::read(text);
    assert!(read.diagnostics.is_empty(), "{:?}", read.diagnostics);
    Tokens::resolve(Arc::new(read.document), Selection::new(), Vec::new()).0
}

/// A core with no theme.
pub(super) fn bare_core() -> Core<FakeEngine> {
    Core::new(FakeEngine::default(), size_pt(320.0, 480.0))
}

/// The packed color of token `name`.
pub(super) fn color(tokens: &Tokens, name: &str) -> Option<Color> {
    tokens.color(name).map(Color::from)
}

/// Every value type, one token each.
const EVERY_TYPE: &str = r##"{
  "tokens": {
    "ink": { "kind": "color", "value": "#112233", "path": "colors.ink", "doc": "Text." },
    "radius-card": { "kind": "number", "value": 12, "path": "radius.card" },
    "font-ui": { "kind": "font", "value": "Inter, system-ui, sans-serif" },
    "label": { "kind": "string", "value": "Hello" },
    "compact": { "kind": "boolean", "value": true },
    "fast": { "kind": "duration", "value": "150ms" },
    "snappy": { "kind": "easing", "value": [0.2, 0, 0, 1] },
    "text-title": { "kind": "typography", "value": {
      "fontFamily": "Inter", "fontSize": 20, "fontWeight": 600,
      "lineHeight": "28px", "letterSpacing": -0.5 } },
    "text-body": { "kind": "typography", "value": { "fontSize": 16, "lineHeight": 1.5 } },
    "raised": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 2 }] },
    "hero": { "kind": "gradient", "value": { "type": "linear", "angle": 90,
      "stops": [{ "color": "#7c3aed", "position": 0 }, { "color": "#db2777", "position": 1 }] } },
    "frost": { "kind": "blur", "value": { "type": "background", "radius": 8 } },
    "outline": { "kind": "border", "value": { "color": "#112233", "width": 1, "style": "dashed" } },
    "fade": { "kind": "transition", "value": { "duration": "200ms", "delay": "50ms", "easing": "ease-out" } }
  }
}"##;

#[test]
fn every_value_type_is_stored_in_its_rust_type() {
    let t = tokens_of(EVERY_TYPE);
    assert_eq!(color(&t, "ink"), Some(Color(0x1122_33ff)));
    assert_eq!(t.number("radius-card"), Some(12.0));
    assert_eq!(
        t.font("font-ui").map(|f| &**f),
        Some("Inter, system-ui, sans-serif")
    );
    assert_eq!(t.string("label"), Some("Hello"));
    assert_eq!(t.boolean("compact"), Some(true));
    assert_eq!(
        t.duration("fast"),
        Some(std::time::Duration::from_millis(150))
    );
    assert_eq!(t.easing("snappy"), Some(CubicBezier([0.2, 0.0, 0.0, 1.0])));
    let raised = t.shadow("raised").unwrap();
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].offset_y, Points(1.0));
    assert_eq!(raised[0].blur, Points(2.0));
    assert_eq!(raised[0].color, Color(0x0000_001a));
    let hero = t.gradient("hero").unwrap();
    assert_eq!(hero.stops.len(), 2);
    assert_eq!(hero.stops[1].color, Color(0xdb27_77ff));
    assert!(matches!(hero.kind, crate::GradientKind::Linear { .. }));
    let frost = t.blur("frost").unwrap();
    assert_eq!(
        (frost.kind, frost.radius),
        (BlurKind::Background, Points(8.0))
    );
    let outline = t.border("outline").unwrap();
    assert_eq!(Color::from(outline.color), Color(0x1122_33ff));
    assert_eq!(outline.width, Points(1.0));
    assert_eq!(outline.style, crate::style::BorderStyle::Dashed);
    let fade = t.transition("fade").unwrap();
    assert_eq!(fade.duration, std::time::Duration::from_millis(200));
    assert_eq!(fade.delay, std::time::Duration::from_millis(50));
    assert_eq!(fade.easing, CubicBezier::EASE_OUT);
    // One value per name, of the type its declaration gives.
    for (name, ty) in [
        ("ink", ValueType::Color),
        ("radius-card", ValueType::Number),
        ("font-ui", ValueType::Font),
        ("label", ValueType::String),
        ("compact", ValueType::Boolean),
        ("fast", ValueType::Duration),
        ("snappy", ValueType::Easing),
        ("text-title", ValueType::Typography),
        ("raised", ValueType::Shadow),
        ("hero", ValueType::Gradient),
        ("frost", ValueType::Blur),
        ("outline", ValueType::Border),
        ("fade", ValueType::Transition),
    ] {
        assert_eq!(t.get(name).map(|v| v.value_type()), Some(ty), "{name}");
        assert_eq!(t.kind_of(name), Some(ty), "{name}");
    }
}

#[test]
fn a_typography_token_carries_its_family_and_a_line_height_of_points_or_a_multiple() {
    let t = tokens_of(EVERY_TYPE);
    let title = t.typography("text-title").unwrap();
    assert_eq!(title.font_family.as_deref(), Some("Inter"));
    assert_eq!(title.size, Points(20.0));
    assert_eq!(title.font_weight, Some(600.0));
    assert_eq!(
        title.line_height,
        Some(crate::LineHeight::Points(Points(28.0)))
    );
    assert_eq!(title.letter_spacing, Some(Points(-0.5)));
    let body = t.typography("text-body").unwrap();
    assert_eq!(body.line_height, Some(crate::LineHeight::Multiple(1.5)));
    assert_eq!(body.font_family, None);
    // No typography part is a token of its own.
    assert_eq!(t.get("text-title--line-height"), None);
}

#[test]
fn kind_path_doc_and_names_come_from_the_active_document() {
    let t = tokens_of(EVERY_TYPE);
    assert_eq!(t.path_of("ink"), Some("colors.ink"));
    assert_eq!(t.doc_of("ink"), Some("Text."));
    assert_eq!(t.name_at("radius.card"), Some("radius-card"));
    assert_eq!(t.path_of("label"), None);
    let names: Vec<&str> = t.names().collect();
    assert_eq!(names.first(), Some(&"ink"));
    assert_eq!(names.len(), 14);
    assert!(t.kind_of("nope").is_none());
    let info = t.infos();
    assert_eq!(info.len(), 14);
    assert_eq!(
        info.iter().find(|i| &*i.name == "ink").unwrap().kind,
        "color"
    );
}

#[test]
fn token_keys_and_get_as_read_every_value_type() {
    let t = tokens_of(EVERY_TYPE);
    const INK: TokenKey<crate::Hsla> = TokenKey::new("ink");
    const CARD: TokenKey<f32> = TokenKey::new("radius-card");
    const CARD_POINTS: TokenKey<Points> = TokenKey::new("radius-card");
    const FONT: TokenKey<Arc<str>> = TokenKey::new("font-ui");
    const LABEL: TokenKey<String> = TokenKey::new("label");
    const COMPACT: TokenKey<bool> = TokenKey::new("compact");
    const FAST: TokenKey<std::time::Duration> = TokenKey::new("fast");
    const SNAPPY: TokenKey<CubicBezier> = TokenKey::new("snappy");
    const TITLE: TokenKey<Typography> = TokenKey::new("text-title");
    const RAISED: TokenKey<Arc<[crate::BoxShadow]>> = TokenKey::new("raised");
    const HERO: TokenKey<crate::Gradient> = TokenKey::new("hero");
    const FROST: TokenKey<crate::element::Blur> = TokenKey::new("frost");
    const OUTLINE: TokenKey<crate::element::Border> = TokenKey::new("outline");
    const FADE: TokenKey<crate::element::TransitionToken> = TokenKey::new("fade");
    assert_eq!(t.key(INK).map(Color::from), Some(Color(0x1122_33ff)));
    assert_eq!(t.key(CARD), Some(12.0));
    assert_eq!(t.key(CARD_POINTS), Some(Points(12.0)));
    assert!(t.key(FONT).is_some());
    assert_eq!(t.key(LABEL).as_deref(), Some("Hello"));
    assert_eq!(t.key(COMPACT), Some(true));
    assert!(t.key(FAST).is_some() && t.key(SNAPPY).is_some());
    assert_eq!(t.key(TITLE).map(|t| t.size), Some(Points(20.0)));
    assert!(t.key(RAISED).is_some() && t.key(HERO).is_some());
    assert!(t.key(FROST).is_some() && t.key(OUTLINE).is_some() && t.key(FADE).is_some());
    assert_eq!(t.get_as::<f32>("radius-card"), Ok(12.0));
    assert_eq!(
        t.get_as::<bool>("label"),
        Err(TokenError::WrongKind {
            expected: "boolean",
            found: "string",
        })
    );
    assert_eq!(t.get_as::<f32>("nope"), Err(TokenError::Missing));
}

/// B4: a name decides nothing about a type.
#[test]
fn a_name_decides_nothing_about_a_type() {
    let t = tokens_of(
        r##"{ "tokens": {
            "radius-tint": { "kind": "color", "value": "#ff0000" },
            "font-scale": { "kind": "number", "value": 1.2 },
            "shadow-strength": { "kind": "number", "value": 0.4 },
            "stack": { "kind": "font", "value": "system-ui, sans-serif" }
        } }"##,
    );
    assert_eq!(color(&t, "radius-tint"), Some(Color(0xff00_00ff)));
    assert_eq!(t.number("font-scale"), Some(1.2));
    assert_eq!(t.number("shadow-strength"), Some(0.4));
    // A font is read whole.
    assert_eq!(t.font("stack").map(|f| &**f), Some("system-ui, sans-serif"));
    // A number named like a duration takes no duration.
    let read = nsh_theme::read(
        r#"{ "tokens": { "duration-x": { "kind": "number", "value": "150ms" } } }"#,
    );
    assert!(read.diagnostics.iter().any(|d| d.code == code::BAD_VALUE));
    let t = Tokens::resolve(Arc::new(read.document), Selection::new(), Vec::new()).0;
    assert_eq!(t.get("duration-x"), None);
}

#[test]
fn a_patch_built_from_typed_values_sets_each_type() {
    let read = nsh_theme::read(EVERY_TYPE);
    let patch = Patch::new()
        .value("ink", crate::Hsla::from(crate::color::rgb(0x445566)))
        .value("radius-card", 14.0)
        .value("label", String::from("Bye"))
        .value("compact", false)
        .value("fast", std::time::Duration::from_millis(90))
        .value(
            "text-title",
            Typography::new(Points(24.0)).with_line_height(crate::LineHeight::Multiple(1.25)),
        )
        .value(
            "frost",
            crate::element::Blur::new(BlurKind::Layer, Points(2.0)),
        );
    assert!(
        patch.check(&read.document).is_empty(),
        "{:?}",
        patch.check(&read.document)
    );
    let t = Tokens::resolve(
        Arc::new(read.document),
        Selection::new(),
        vec![Arc::new(patch)],
    )
    .0;
    assert_eq!(color(&t, "ink"), Some(Color(0x4455_66ff)));
    assert_eq!(t.number("radius-card"), Some(14.0));
    assert_eq!(t.string("label"), Some("Bye"));
    assert_eq!(t.boolean("compact"), Some(false));
    assert_eq!(
        t.duration("fast"),
        Some(std::time::Duration::from_millis(90))
    );
    let title = t.typography("text-title").unwrap();
    assert_eq!(title.size, Points(24.0));
    assert_eq!(title.line_height, Some(crate::LineHeight::Multiple(1.25)));
    assert_eq!(t.blur("frost").map(|b| b.kind), Some(BlurKind::Layer));
}

#[test]
fn roles_read_the_token_they_are_mapped_onto_else_their_own() {
    let t = tokens_of(
        r##"{ "tokens": {
            "surface-overlay": { "kind": "color", "value": "#123456" },
            "primary": { "kind": "color", "value": "#000000" },
            "font-body": { "kind": "font", "value": "Inter" }
        }, "roles": { "popover": "surface-overlay", "font-sans": "font-body" } }"##,
    );
    use crate::element::role;
    assert_eq!(
        t.role(role::POPOVER).map(Color::from),
        Some(Color(0x1234_56ff))
    );
    assert_eq!(t.role(role::FONT_SANS).as_deref(), Some("Inter"));
    assert_eq!(
        t.role(role::PRIMARY).map(Color::from),
        Some(Color(0x0000_00ff))
    );
    assert_eq!(t.role_name(role::POPOVER), "surface-overlay");
    assert_eq!(t.role(role::ACCENT), None);
    // A role is no token.
    assert_eq!(t.get("popover"), None);
}

const SET_A: &str = r##"{ "tokens": { "a-only": { "kind": "color", "value": "#aaaaaa" },
    "shared": { "kind": "number", "value": 1 } } }"##;
const SET_B: &str = r##"{ "tokens": { "b-only": { "kind": "color", "value": "#bbbbbb" },
    "shared": { "kind": "number", "value": 2 } } }"##;
const FALLBACK: &str = r##"{ "tokens": { "fallback-only": { "kind": "number", "value": 3 },
    "shared": { "kind": "number", "value": 3 } } }"##;
const APP: &str = r##"{ "tokens": { "app-only": { "kind": "number", "value": 4 },
    "shared": { "kind": "number", "value": 4 } } }"##;

fn set(specifier: &'static str, document: &'static str) -> Registry {
    let mut registry = Registry::ui_set(specifier);
    registry.theme_document(document);
    registry
}

fn fallback() -> Registry {
    let mut registry = Registry::ui_set("base/native");
    registry.fallback_theme_document(FALLBACK);
    registry
}

#[test]
fn the_first_sets_document_is_active_and_each_other_is_unused() {
    let mut c = bare_core();
    let ((), lines) = logged(|| {
        c.set_components(Registry::merged(&[
            set("a/native", SET_A),
            set("b/native", SET_B),
        ]))
    });
    assert_eq!(c.tokens().number("shared"), Some(1.0));
    assert!(c.tokens().get("a-only").is_some());
    // Never merged: the unused document's tokens are not readable.
    assert_eq!(c.tokens().get("b-only"), None);
    assert!(said(&lines, code::DOCUMENT_UNUSED), "{lines:?}");
}

#[test]
fn a_sets_document_wins_over_a_fallback_and_a_fallback_over_none() {
    let mut c = bare_core();
    c.set_components(Registry::merged(&[fallback(), set("a/native", SET_A)]));
    assert_eq!(c.tokens().number("shared"), Some(1.0));
    assert_eq!(c.tokens().get("fallback-only"), None);
    let mut c = bare_core();
    c.set_components(Registry::merged(&[fallback()]));
    assert_eq!(c.tokens().number("shared"), Some(3.0));
    // A fallback alone is no theme of the app's.
    assert!(!c.tokens().is_themed());
}

#[test]
fn the_apps_theme_wins_over_the_sets_and_the_fallback() {
    let mut c = bare_core();
    c.set_components(Registry::merged(&[fallback(), set("a/native", SET_A)]));
    assert!(c.set_app_theme(Some(APP), None).is_empty());
    c.commit();
    assert_eq!(c.tokens().number("shared"), Some(4.0));
    assert_eq!(c.tokens().get("a-only"), None);
    assert!(c.tokens().is_themed());
}

#[test]
fn set_theme_none_returns_to_the_apps_built_in_document() {
    let mut c = bare_core();
    c.set_components(Registry::merged(&[set("a/native", SET_A)]));
    c.set_app_theme(Some(APP), None);
    assert!(c.set_theme(Some(SET_B)).is_empty());
    c.commit();
    assert_eq!(c.tokens().number("shared"), Some(2.0));
    c.set_theme(None);
    c.commit();
    assert_eq!(c.tokens().number("shared"), Some(4.0));
}

#[test]
fn set_theme_none_without_a_built_in_document_returns_to_the_sets() {
    let mut c = bare_core();
    c.set_components(Registry::merged(&[set("a/native", SET_A)]));
    c.set_theme(Some(SET_B));
    c.commit();
    assert_eq!(c.tokens().number("shared"), Some(2.0));
    c.set_theme(None);
    c.commit();
    assert_eq!(c.tokens().number("shared"), Some(1.0));
}

const PICKS_A: &str = r##"{
  "tokens": { "radius-card": { "kind": "number", "value": 8 },
              "brand-only": { "kind": "number", "value": 1 } },
  "modifiers": {
    "density": { "default": "regular", "contexts": { "regular": {}, "compact": { "radius-card": 4 } } },
    "brand": { "default": "ocean", "contexts": { "ocean": {}, "forest": { "brand-only": 2 } } }
  },
  "resolutionOrder": ["density", "brand"]
}"##;
const PICKS_B: &str = r##"{
  "tokens": { "radius-card": { "kind": "number", "value": 10 } },
  "modifiers": {
    "density": { "default": "regular", "contexts": { "regular": {}, "compact": { "radius-card": 5 } } }
  },
  "resolutionOrder": ["density"]
}"##;

#[test]
fn picks_are_kept_across_a_switch_for_what_the_new_document_has() {
    let mut c = bare_core();
    c.set_theme(Some(PICKS_A));
    c.install_theme();
    c.set_modifier("density", "compact");
    c.set_modifier("brand", "forest");
    c.install_theme();
    assert_eq!(c.tokens().number("radius-card"), Some(4.0));
    c.set_theme(Some(PICKS_B));
    c.install_theme();
    // B has `density` and its `compact`: kept.
    assert_eq!(c.tokens().modifier("density"), Some("compact"));
    assert_eq!(c.tokens().number("radius-card"), Some(5.0));
    // B lacks `brand`: back to its default when A is active again.
    c.set_theme(Some(PICKS_A));
    c.install_theme();
    assert_eq!(c.tokens().modifier("density"), Some("compact"));
    assert_eq!(c.tokens().modifier("brand"), Some("ocean"));
    assert_eq!(c.tokens().number("brand-only"), Some(1.0));
}

/// The app's patch is kept as given: an override of a name B lacks is
/// skipped while B is active and applies again with A.
#[test]
fn an_override_of_a_name_b_lacks_comes_back_with_a() {
    let mut c = bare_core();
    c.set_theme(Some(PICKS_A));
    c.install_theme();
    c.update_tokens(Patch::new().value("brand-only", 9.0));
    c.install_theme();
    assert_eq!(c.tokens().number("brand-only"), Some(9.0));
    let ((), lines) = logged(|| {
        c.set_theme(Some(PICKS_B));
        c.install_theme();
    });
    assert_eq!(c.tokens().get("brand-only"), None);
    // Reported, and kept.
    assert!(said(&lines, code::UNDECLARED_TOKEN), "{lines:?}");
    c.set_theme(Some(PICKS_A));
    c.install_theme();
    assert_eq!(c.tokens().number("brand-only"), Some(9.0));
    assert_eq!(c.app_patch().values.len(), 1);
}

const OLD: &str = r##"{ "cssVars": { "theme": { "radius": "12px" } } }"##;

#[test]
fn a_registration_of_an_old_cssvars_theme_is_skipped() {
    let (old, lines) = logged(|| set("old/native", OLD));
    assert!(said(&lines, code::LEGACY_FORMAT), "{lines:?}");
    assert!(old.theme_documents().is_empty());
    let mut c = bare_core();
    let ((), lines) = logged(|| c.set_components(Registry::merged(&[old, set("a/native", SET_A)])));
    // The next document by precedence is active, and nothing is unused.
    assert_eq!(c.tokens().number("shared"), Some(1.0));
    assert!(!said(&lines, code::DOCUMENT_UNUSED), "{lines:?}");
}

#[test]
fn a_rust_activation_of_an_old_theme_is_refused_and_keeps_the_document() {
    let mut c = bare_core();
    c.set_theme(Some(SET_A));
    c.install_theme();
    let (diagnostics, lines) = logged(|| c.set_theme(Some(OLD)));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, code::LEGACY_FORMAT);
    assert!(said(&lines, code::LEGACY_FORMAT));
    c.install_theme();
    assert_eq!(c.tokens().number("shared"), Some(1.0));
    let not_json = c.set_theme(Some("{ nope"));
    assert_eq!(not_json[0].code, code::NOT_A_DOCUMENT);
    c.install_theme();
    assert_eq!(c.tokens().number("shared"), Some(1.0));
}

/// An editor adds a token and removes another by activating its edited
/// document, through Rust's `set_theme` in a component's runtime.
#[test]
fn a_rust_activation_adds_and_removes_tokens_at_run_time() {
    let mut c = bare_core();
    c.set_components(Registry::merged(&[set("a/native", SET_A)]));
    c.commit();
    {
        let _entered = c.test_runtime().enter();
        let said = crate::element::set_theme(Some(
            r##"{ "tokens": { "shared": { "kind": "number", "value": 1 },
                "added": { "kind": "color", "value": "#00ff00" } } }"##,
        ));
        assert!(said.is_empty(), "{said:?}");
    }
    c.commit();
    let names: Vec<&str> = c.tokens().names().collect();
    assert_eq!(names, ["shared", "added"]);
    assert_eq!(color(c.tokens(), "added"), Some(Color(0x00ff_00ff)));
    assert_eq!(c.tokens().get("a-only"), None);
    // The change set names both.
    let changed = &c.theme_changes().tokens;
    assert!(changed.iter().any(|n| n == "added") && changed.iter().any(|n| n == "a-only"));
}

const TITLE_DOCUMENT: &str = r##"{ "tokens": { "text-title": { "kind": "typography", "value": {
    "fontFamily": "Inter", "fontSize": 20, "fontWeight": 600,
    "lineHeight": "28px", "letterSpacing": -0.5 } } } }"##;

#[component]
fn Title() -> impl IntoElement {
    const TITLE: TokenKey<Typography> = TokenKey::new("text-title");
    text("Title").typography(move || TITLE.get().unwrap_or_default())
}

#[test]
fn a_typography_token_styles_a_text_whole() {
    let mut c = bare_core();
    c.set_app_theme(Some(TITLE_DOCUMENT), None);
    c.mount_root(&Root::of::<Title>());
    let text = text_item(&c.commit().unwrap()).unwrap();
    let style = text.spans[0].style;
    assert_eq!(style.font_size, Points(20.0));
    assert_eq!(style.line_height, Points(28.0));
    assert_eq!(style.letter_spacing, Points(-0.5));
    assert_eq!(style.font_weight, 600);
    assert_eq!(
        style.font_family,
        Some(crate::style::FontFamily::new("Inter"))
    );
}

#[test]
fn a_registration_that_is_not_json_logs_and_the_registry_goes_on() {
    let (registry, lines) = logged(|| {
        let mut registry = Registry::ui_set("x/native");
        registry.theme_document("{ not json");
        registry.theme_document(SET_A);
        registry
    });
    assert!(said(&lines, code::NOT_A_DOCUMENT), "{lines:?}");
    assert_eq!(registry.theme_documents().len(), 1);
    let mut c = bare_core();
    c.set_components(registry);
    assert_eq!(c.tokens().number("shared"), Some(1.0));
}

#[test]
fn a_name_declared_twice_logs_and_the_document_loads() {
    let (registry, lines) = logged(|| {
        set(
            "x/native",
            r##"{ "tokens": {
                "twice": { "kind": "number", "value": 1 },
                "twice": { "kind": "number", "value": 2 },
                "good": { "kind": "number", "value": 3 } } }"##,
        )
    });
    assert!(said(&lines, code::DUPLICATE_MEMBER), "{lines:?}");
    let mut c = bare_core();
    c.set_components(registry);
    assert_eq!(c.tokens().number("good"), Some(3.0));
}

#[test]
fn a_role_mapped_twice_logs_and_the_document_loads() {
    let (registry, lines) = logged(|| {
        set(
            "x/native",
            r##"{ "tokens": { "a": { "kind": "color", "value": "#111111" },
                              "b": { "kind": "color", "value": "#222222" } },
                  "roles": { "popover": "a", "popover": "b" } }"##,
        )
    });
    assert!(said(&lines, code::DUPLICATE_MEMBER), "{lines:?}");
    let mut c = bare_core();
    c.set_components(registry);
    assert!(c.tokens().color("a").is_some() && c.tokens().color("b").is_some());
}

#[test]
fn a_role_listed_reading_two_kinds_logs_a_conflict_and_keeps_the_first() {
    use crate::element::{NumberRole, Role};
    let (registry, lines) = logged(|| {
        let mut registry = Registry::ui_set("x/native");
        registry.roles([Role::<crate::Hsla>::new("banner").declaration("", false)]);
        registry.roles([NumberRole::new("banner").declaration("", false)]);
        registry
    });
    assert!(said(&lines, code::ROLE_CONFLICT), "{lines:?}");
    assert_eq!(registry.role_specs().len(), 1);
    assert_eq!(registry.role_specs()[0].value_type, ValueType::Color);
}

#[test]
fn a_diagnostic_is_logged_once_as_its_code_its_pointer_and_its_message() {
    let mut c = bare_core();
    let ((), lines) = logged(|| {
        c.set_theme(Some(
            r##"{ "tokens": { "a": { "kind": "color", "value": "{nope}" },
                              "b": { "kind": "number", "value": 0 } } }"##,
        ));
        c.install_theme();
        // Resolved again, with the same document: not logged again.
        c.update_tokens(Patch::new().value("b", 1.0));
        c.install_theme();
    });
    let missing: Vec<&String> = lines
        .iter()
        .filter(|l| l.starts_with(code::REFERENCE_MISSING))
        .collect();
    assert_eq!(missing.len(), 1, "{lines:?}");
    assert!(
        missing[0].starts_with("reference-missing /tokens/a: "),
        "{lines:?}"
    );
}

#[test]
fn module_functions_activate_documents_in_order_with_the_other_actions() {
    use crate::element::{RustCall, RustOp, ToJs};
    let mut c = bare_core();
    c.set_theme(Some(PICKS_A));
    c.install_theme();
    let call = |run: fn(&mut crate::element::ModuleCx)| {
        crate::Op::Rust(RustOp {
            call: 0,
            run: RustCall::Op(Box::new(move |cx| {
                run(cx);
                Ok(Box::new(()) as Box<dyn ToJs>)
            })),
        })
    };
    c.apply(call(|cx| {
        assert!(cx.set_theme(Some(PICKS_B)).is_empty());
        cx.set_modifier("density", "compact");
        let refused = cx.set_theme(Some(OLD));
        assert_eq!(refused[0].code, code::LEGACY_FORMAT);
    }))
    .unwrap();
    c.commit();
    assert_eq!(c.tokens().number("radius-card"), Some(5.0));
    c.apply(call(|cx| {
        cx.set_theme(None);
    }))
    .unwrap();
    c.commit();
    // No built-in document and no set's: none.
    assert!(c.tokens().is_empty());
}

#[test]
fn the_store_holds_one_value_per_name() {
    let t = tokens_of(EVERY_TYPE);
    assert_eq!(t.iter().count(), 14);
    assert!(matches!(t.get("radius-card"), Some(Token::Number(n)) if n == 12.0));
    assert!(t.color("radius-card").is_none());
    assert!(t.number("ink").is_none());
}
