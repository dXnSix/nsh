//! The converter's rules, their refusals, and the equivalence of the old
//! reader's values with the converted document's resolved ones.

use nsh_theme::{
    Color, Diagnostic, Easing, EasingKeyword, LineHeight, Member, Resolved, Selection,
    ThemeDocument, Value, ValueType, read, resolve,
};

use super::{
    code, convert,
    reader::{self, Old, font_family},
    store::{Store, TYPOGRAPHY_PARTS},
    writer,
};
use crate::color::{Hsla, Rgba};

/// nsh-base's old default theme.
const DEFAULT: &str = include_str!("fixtures/default-theme.json");
/// The gallery's theme, which extended the default.
const GALLERY: &str = include_str!("fixtures/gallery-theme.json");
/// The compat `base` case's theme (its `nsh.json`'s `"theme"`), which
/// extended the default.
const COMPAT_BASE: &str = include_str!("fixtures/compat-base-theme.json");
/// post-editor's theme before the conversion, which extended the default.
const POST_EDITOR: &str = include_str!("fixtures/post-editor-theme.json");
/// gallery-rs' theme before the conversion, which extended the default.
const GALLERY_RS: &str = include_str!("fixtures/gallery-rs-theme.json");
/// A theme with modifiers whose contexts differ by appearance.
const APPEARANCE: &str = include_str!("fixtures/appearance-modifier.json");
/// A theme document with paths, docs, every old kind, roles and a modifier
/// whose default is not its first context.
const DECLARED: &str = include_str!("fixtures/declared-theme.json");

/// `old` converted (over `over`): no error reported, and the output reads
/// with no diagnostic.
fn converted(old: &str, over: Option<&str>) -> (ThemeDocument, Vec<Diagnostic>) {
    let (text, diagnostics) = convert(old, over);
    assert!(
        diagnostics.iter().all(|d| !d.is_error()),
        "{diagnostics:#?}"
    );
    let read = read(&text);
    assert!(
        read.diagnostics.is_empty(),
        "{:#?}\n{text}",
        read.diagnostics
    );
    (read.document, diagnostics)
}

/// `document` resolved in the appearance `dark` (or light) with `picks`.
fn resolved(document: &ThemeDocument, dark: bool, picks: &[(&str, &str)]) -> Resolved {
    let mut selection = Selection::new();
    if document.modifiers.contains_key("colorMode") {
        selection = selection.with("colorMode", if dark { "dark" } else { "light" });
    }
    for (modifier, context) in picks {
        selection = selection.with(*modifier, *context);
    }
    let resolved = resolve(document, &selection, &[]);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:#?}",
        resolved.diagnostics
    );
    resolved
}

fn hex(text: &str) -> Value {
    Value::Color(Color::from_hex(text).unwrap())
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code).collect()
}

fn mentions(diagnostics: &[Diagnostic], code: &str, text: &str) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code == code && d.message.contains(text))
}

// ---------------------------------------------------------------------------
// The rules

#[test]
fn rule_1_every_name_becomes_a_token_of_its_old_kind() {
    let (document, _) = converted(DECLARED, None);
    let kind = |name: &str| document.token(name).map(|t| t.value_type);
    for (name, ty) in [
        ("surface", ValueType::Color),
        ("card", ValueType::Color),
        ("radius-card", ValueType::Number),
        ("font-body", ValueType::Font),
        ("text-title", ValueType::Typography),
        ("duration-fast", ValueType::Duration),
        ("ease-snappy", ValueType::Easing),
        ("label-ok", ValueType::String),
        ("motion", ValueType::Boolean),
        ("shadow-raised", ValueType::Shadow),
        ("background-image-hero", ValueType::Gradient),
    ] {
        assert_eq!(kind(name), Some(ty), "{name}");
    }
    // A typography's parts are members of it, no tokens of their own.
    for suffix in TYPOGRAPHY_PARTS {
        assert_eq!(kind(&format!("text-title{suffix}")), None);
    }
    let Value::Typography(title) = &document.token("text-title").unwrap().value else {
        panic!("text-title is no typography");
    };
    assert_eq!(title.font_size, Member::Value(20.0));
    assert_eq!(
        title.line_height,
        Some(Member::Value(LineHeight::Multiple(1.25)))
    );
    assert_eq!(title.font_weight, Some(Member::Value(650.0)));
    assert_eq!(title.letter_spacing, Some(Member::Value(-0.2)));

    // A name with no declaration takes the kind the old reader guessed.
    let (document, diagnostics) = converted(
        r##"{ "cssVars": { "theme": {
            "radius": "8px", "font-sans": "Inter, sans-serif", "font-weight-strong": "650",
            "duration-fast": "150ms", "ease-quick": "ease-out", "shadow-sm": "0 1px 2px #0000001a",
            "background-image-hero": "linear-gradient(#7c3aed, #db2777)", "brand": "#e11d48"
        } } }"##,
        None,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let kind = |name: &str| document.token(name).map(|t| t.value_type);
    for (name, ty) in [
        ("radius", ValueType::Number),
        ("font-sans", ValueType::Font),
        ("font-weight-strong", ValueType::Number),
        ("duration-fast", ValueType::Duration),
        ("ease-quick", ValueType::Easing),
        ("shadow-sm", ValueType::Shadow),
        ("background-image-hero", ValueType::Gradient),
        ("brand", ValueType::Color),
    ] {
        assert_eq!(kind(name), Some(ty), "{name}");
    }
    // In the order the file writes them.
    let names: Vec<&str> = document.tokens.keys().collect();
    assert_eq!(names[0], "radius");
    assert_eq!(names[names.len() - 1], "brand");
}

#[test]
fn rule_1_paths_and_docs_are_copied() {
    let (document, _) = converted(DECLARED, None);
    let token = |name: &str| document.token(name).unwrap();
    assert_eq!(token("surface").path.as_deref(), Some("colors.surface"));
    assert_eq!(token("surface").doc.as_deref(), Some("Cards and popovers."));
    assert_eq!(token("card").path.as_deref(), Some("colors.card"));
    assert_eq!(token("card").doc, None);
    assert_eq!(token("text-title").path.as_deref(), Some("type.title"));
    assert_eq!(token("text-title").doc.as_deref(), Some("Titles."));
    assert_eq!(token("label-ok").path, None);
    assert_eq!(
        token("label-ok").doc.as_deref(),
        Some("The confirm button's label.")
    );
}

#[test]
fn rule_1_refusal_a_declaration_with_no_value_or_kind_is_reported() {
    let (document, diagnostics) = converted(
        r##"{
            "cssVars": { "light": { "surface": "#ffffff" } },
            "tokens": {
                "surface": { "kind": "color" },
                "ink": { "kind": "color" },
                "gap": { "kind": "dimension" }
            }
        }"##,
        None,
    );
    assert!(document.token("surface").is_some());
    assert!(document.token("ink").is_none());
    assert!(document.token("gap").is_none());
    assert!(mentions(
        &diagnostics,
        code::INEXPRESSIBLE,
        "`ink`: declared"
    ));
    assert!(mentions(
        &diagnostics,
        code::LEFT_OUT,
        "no kind `dimension`"
    ));
    let ink = diagnostics
        .iter()
        .find(|d| d.message.contains("`ink`"))
        .unwrap();
    assert_eq!(ink.pointer, "/tokens/ink");
}

#[test]
fn rule_2_values_take_the_formats_forms() {
    let (document, _) = converted(DEFAULT, None);
    let value = |name: &str| &document.token(name).unwrap().value;
    // A length in points, an `oklch()` color as hex.
    assert_eq!(value("radius"), &Value::Number(10.0));
    assert_eq!(value("background"), &hex("#ffffff"));
    // Below full opacity, `#rrggbbaa` (`oklch(1 0 0 / 10%)`: 25.5, 26).
    let dark = &document
        .modifiers
        .get("colorMode")
        .unwrap()
        .contexts
        .get("dark")
        .unwrap()
        .values;
    assert_eq!(dark.get("border"), Some(&hex("#ffffff1a")));
    let (text, _) = convert(DEFAULT, None);
    assert!(text.contains("\"#ffffff1a\""), "{text}");

    // Shadows and gradients as structured values.
    let (document, _) = converted(DECLARED, None);
    let Value::Shadow(shadow) = &document.token("shadow-raised").unwrap().value else {
        panic!("shadow-raised is no shadow");
    };
    assert_eq!(shadow.0.len(), 2);
    assert_eq!(shadow.0[0].color, Member::Value(Color::rgba(0, 0, 0, 0x1a)));
    assert_eq!(shadow.0[0].y, Member::Value(1.0));
    assert_eq!(shadow.0[0].blur, Member::Value(2.0));
    assert_eq!(shadow.0[1].blur, Member::Value(3.0));
    let Value::Gradient(gradient) = &document.token("background-image-hero").unwrap().value else {
        panic!("background-image-hero is no gradient");
    };
    assert_eq!(
        gradient.kind,
        nsh_theme::GradientKind::Linear {
            angle: Member::Value(90.0)
        }
    );
    assert_eq!(gradient.stops.len(), 2);
    assert_eq!(
        gradient.stops[0].color,
        Member::Value(Color::rgb(0x7c, 0x3a, 0xed))
    );
    assert_eq!(
        document.token("duration-fast").unwrap().value,
        Value::Duration(nsh_theme::Duration::from_millis(150.0))
    );
    assert_eq!(
        document.token("ease-exit").unwrap().value,
        Value::Easing(Easing::Keyword(EasingKeyword::EaseIn))
    );
    assert_eq!(
        document.token("ease-snappy").unwrap().value,
        Value::Easing(Easing::Bezier([0.2, 0.0, 0.0, 1.0]))
    );
    assert_eq!(
        document.token("font-body").unwrap().value,
        Value::Font(nsh_theme::Font::new("Inter, system-ui, sans-serif"))
    );
}

#[test]
fn rule_2_channels_are_rounded_not_cut() {
    let half = Rgba {
        r: 0.5,
        g: 0.0,
        b: 1.0,
        a: 0.5,
    };
    assert_eq!(writer::rgba(half), Color::rgba(128, 0, 255, 128));
    assert_eq!(
        writer::color(Hsla::from(half)),
        Color::rgba(128, 0, 255, 128)
    );
}

#[test]
fn rule_2_refusal_a_value_the_format_cannot_hold_is_reported() {
    // A stop past the gradient's end (150 %): the format's positions are
    // between 0 and 1.
    let (document, diagnostics) = converted(
        r##"{ "cssVars": { "theme": {
            "background-image-past": "linear-gradient(red, blue 150%)", "brand": "#e11d48"
        } } }"##,
        None,
    );
    assert!(document.token("background-image-past").is_none());
    assert!(document.token("brand").is_some());
    let past = diagnostics
        .iter()
        .find(|d| d.message.starts_with("`background-image-past`"))
        .unwrap_or_else(|| panic!("{diagnostics:#?}"));
    assert_eq!(past.code, code::INEXPRESSIBLE);
    assert!(past.message.contains("between 0 and 1"), "{}", past.message);
}

#[test]
fn rule_3_values_that_differ_by_appearance_add_color_mode_first() {
    let (document, _) = converted(APPEARANCE, None);
    assert_eq!(document.resolution_order, ["colorMode", "brand", "density"]);
    let color_mode = document.modifiers.get("colorMode").unwrap();
    assert_eq!(color_mode.default, "light");
    let contexts: Vec<&str> = color_mode.contexts.keys().collect();
    assert_eq!(contexts, ["light", "dark"]);
    assert!(color_mode.contexts.get("light").unwrap().values.is_empty());
    let dark = &color_mode.contexts.get("dark").unwrap().values;
    assert_eq!(dark.get("surface"), Some(&hex("#0a0a0a")));
    assert_eq!(dark.get("gap"), Some(&Value::Number(14.0)));
    // A value the same in both appearances has no dark entry.
    assert_eq!(dark.get("radius"), None);
    assert_eq!(document.token("gap").unwrap().value, Value::Number(12.0));

    // Nothing differs: no `colorMode`.
    let (document, _) = converted(
        r##"{ "cssVars": { "theme": { "radius": "8px" }, "light": { "ink": "#111111" } } }"##,
        None,
    );
    assert!(document.modifiers.is_empty());
    assert!(document.resolution_order.is_empty());
}

#[test]
fn rule_3_refusal_a_theme_with_a_color_mode_modifier_is_not_converted() {
    let with_color_mode = r##"{
        "cssVars": { "light": { "ink": "#111111" } },
        "modifiers": { "colorMode": { "default": "a", "contexts": { "a": {}, "b": {} } } }
    }"##;
    let (text, diagnostics) = convert(with_color_mode, None);
    assert_eq!(text, "");
    assert_eq!(codes(&diagnostics), [code::COLOR_MODE_TAKEN]);
    assert!(diagnostics[0].is_error());
    assert_eq!(diagnostics[0].pointer, "/modifiers/colorMode");
    // Over a base that has one, too.
    let (text, diagnostics) = convert(COMPAT_BASE, Some(with_color_mode));
    assert_eq!(text, "");
    assert_eq!(codes(&diagnostics), [code::COLOR_MODE_TAKEN]);
    assert!(diagnostics[0].message.contains("the base theme"));
}

#[test]
fn rule_4_old_modifiers_keep_their_names_contexts_and_defaults() {
    let (document, _) = converted(DECLARED, None);
    assert_eq!(document.resolution_order, ["colorMode", "contrast"]);
    let contrast = document.modifiers.get("contrast").unwrap();
    // A default that is not the first context.
    assert_eq!(contrast.default, "high");
    let contexts: Vec<&str> = contrast.contexts.keys().collect();
    assert_eq!(contexts, ["normal", "high"]);
    assert!(contrast.contexts.get("normal").unwrap().values.is_empty());

    let (document, _) = converted(APPEARANCE, None);
    let brand = document.modifiers.get("brand").unwrap();
    assert_eq!(brand.default, "ocean");
    let contexts: Vec<&str> = brand.contexts.keys().collect();
    assert_eq!(contexts, ["ocean", "forest", "mono"]);
    // A context's value the same in both appearances is the entry as it is.
    let mono = &brand.contexts.get("mono").unwrap().values;
    assert_eq!(mono.get("accent"), Some(&hex("#525252")));
    let compact = &document
        .modifiers
        .get("density")
        .unwrap()
        .contexts
        .get("compact")
        .unwrap()
        .values;
    assert_eq!(compact.get("radius"), Some(&Value::Number(4.0)));
}

#[test]
fn rule_4_a_context_value_that_differs_by_appearance_goes_through_a_helper() {
    let (document, _) = converted(APPEARANCE, None);
    let forest = &document
        .modifiers
        .get("brand")
        .unwrap()
        .contexts
        .get("forest")
        .unwrap()
        .values;
    assert_eq!(
        forest.get("accent"),
        Some(&Value::reference("accent.brand.forest"))
    );
    let helper = document.token("accent.brand.forest").unwrap();
    assert_eq!(helper.value_type, ValueType::Color);
    assert_eq!(helper.value, hex("#16a34a"));
    let dark = &document
        .modifiers
        .get("colorMode")
        .unwrap()
        .contexts
        .get("dark")
        .unwrap()
        .values;
    assert_eq!(dark.get("accent.brand.forest"), Some(&hex("#4ade80")));
    assert_eq!(dark.get("gap.density.compact"), Some(&Value::Number(10.0)));
    let dark = resolved(&document, true, &[("brand", "forest")]);
    assert_eq!(dark.get("accent"), Some(&hex("#4ade80")));
    let light = resolved(&document, false, &[("brand", "forest")]);
    assert_eq!(light.get("accent"), Some(&hex("#16a34a")));
}

#[test]
fn rule_4_refusal_a_modifier_the_old_reader_left_out_is_reported() {
    let (document, diagnostics) = converted(
        r##"{
            "cssVars": { "light": { "ink": "#111111" } },
            "modifiers": {
                "single": { "default": "a", "contexts": { "a": {} } },
                "nameless": { "default": "z", "contexts": { "a": {}, "b": {} } }
            }
        }"##,
        None,
    );
    assert!(document.modifiers.is_empty());
    assert!(mentions(
        &diagnostics,
        code::LEFT_OUT,
        "modifiers.single: expected `contexts`, an object of two or more contexts"
    ));
    assert!(mentions(
        &diagnostics,
        code::LEFT_OUT,
        "modifiers.nameless: expected `default`"
    ));
}

#[test]
fn rule_5_a_whole_var_becomes_a_reference() {
    let (document, _) = converted(GALLERY, Some(DEFAULT));
    let value = |name: &str| &document.token(name).unwrap().value;
    assert_eq!(value("card-foreground"), &Value::reference("foreground"));
    assert_eq!(value("secondary"), &Value::reference("raised"));
    assert_eq!(value("down"), &Value::reference("destructive"));
    // Any other `var()` is copied, as the old reader copied it.
    assert!(matches!(value("scrollbar-thumb"), Value::Color(_)));
    assert!(matches!(value("hover"), Value::Color(_)));
    // The reference follows the dark appearance through its target.
    let dark = resolved(&document, true, &[]);
    assert_eq!(dark.get("secondary"), dark.get("raised"));
    assert_eq!(dark.get("secondary"), Some(&hex("#33353b")));

    let (document, _) = converted(APPEARANCE, None);
    assert_eq!(
        document.token("on-accent").unwrap().value,
        Value::reference("surface")
    );
    let (document, _) = converted(DECLARED, None);
    assert_eq!(
        document.token("card").unwrap().value,
        Value::reference("surface")
    );
}

#[test]
fn rule_5_a_var_naming_a_role_refers_to_its_token() {
    // Over a document that maps `popover` onto `surface`, `var(--popover)`
    // names `surface`.
    let (document, _) = converted(
        r##"{ "cssVars": { "light": { "card": "var(--popover)", "ink": "#222222" } } }"##,
        Some(DECLARED),
    );
    assert_eq!(
        document.token("card").unwrap().value,
        Value::reference("surface")
    );
    assert_eq!(document.token("ink").unwrap().value, hex("#222222"));
    assert_eq!(
        document.roles.get("popover").map(String::as_str),
        Some("surface")
    );
}

#[test]
fn rule_5_a_font_or_boolean_var_refers_to_its_token() {
    // The old reader copied no font (`var()` copied colors and numbers):
    // it left `font-heading` out, and now it refers to `font-sans`.
    let (document, diagnostics) = converted(
        r##"{ "cssVars": { "theme": {
            "font-sans": "Inter, system-ui, sans-serif", "font-heading": "var(--font-sans)"
        } } }"##,
        None,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let heading = document.token("font-heading").unwrap();
    assert_eq!(heading.value_type, ValueType::Font);
    assert_eq!(heading.value, Value::reference("font-sans"));
    let light = resolved(&document, false, &[]);
    assert_eq!(
        light.get("font-heading"),
        Some(&Value::Font(nsh_theme::Font::new(
            "Inter, system-ui, sans-serif"
        )))
    );

    // A declared boolean, and a string, whose old value is its own text.
    let (document, diagnostics) = converted(
        r##"{
            "cssVars": { "theme": {
                "motion": true, "motion-menus": "var(--motion)",
                "label-ok": "OK", "label-confirm": "var(--label-ok)"
            } },
            "tokens": {
                "motion": { "kind": "boolean" }, "motion-menus": { "kind": "boolean" },
                "label-ok": { "kind": "string" }, "label-confirm": { "kind": "string" }
            }
        }"##,
        None,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        document.token("motion-menus").unwrap().value,
        Value::reference("motion")
    );
    // The old reader read the string `var(--label-ok)`: `{label-ok}` would
    // not resolve to it, so it stays.
    assert_eq!(
        document.token("label-confirm").unwrap().value,
        Value::String("var(--label-ok)".to_owned())
    );
}

#[test]
fn rule_5_refusal_a_font_var_a_context_changes_is_no_reference() {
    // A context of `type` sets `font-sans`: `{font-sans}` would follow it,
    // where the old reader read no value for `font-heading` in any context.
    // No reference is made, and the old reader's left-out stays reported.
    let (document, diagnostics) = converted(
        r##"{
            "cssVars": { "theme": {
                "font-sans": "Inter, sans-serif", "font-heading": "var(--font-sans)"
            } },
            "modifiers": { "type": { "default": "plain", "contexts": {
                "plain": {}, "serif": { "theme": { "font-sans": "Georgia, serif" } }
            } } }
        }"##,
        None,
    );
    assert!(document.token("font-heading").is_none());
    let left_out = diagnostics
        .iter()
        .find(|d| d.message.starts_with("`font-heading`"))
        .unwrap_or_else(|| panic!("{diagnostics:#?}"));
    assert_eq!(left_out.code, code::LEFT_OUT);
    assert_eq!(left_out.pointer, "/cssVars/theme/font-heading");
}

#[test]
fn rule_5_refusal_a_var_whose_target_a_context_changes_stays_its_value() {
    let (document, _) = converted(APPEARANCE, None);
    // `ring: var(--accent)` and `radius-card: var(--radius)` copied the
    // values once; `brand` and `density` change the targets.
    assert_eq!(document.token("ring").unwrap().value, hex("#2563eb"));
    assert_eq!(
        document.token("radius-card").unwrap().value,
        Value::Number(8.0)
    );
    let forest = resolved(&document, false, &[("brand", "forest")]);
    assert_eq!(forest.get("ring"), Some(&hex("#2563eb")));
    let compact = resolved(&document, false, &[("density", "compact")]);
    assert_eq!(compact.get("radius-card"), Some(&Value::Number(8.0)));
}

#[test]
fn rule_6_roles_are_copied() {
    let (document, _) = converted(DECLARED, None);
    assert_eq!(
        document.roles.get("popover").map(String::as_str),
        Some("surface")
    );
    let (document, _) = converted(
        r##"{ "cssVars": { "light": { "surface": "#ffffff" } }, "roles": { "card": "surface" } }"##,
        None,
    );
    assert_eq!(
        document.roles.get("card").map(String::as_str),
        Some("surface")
    );
}

#[test]
fn rule_6_refusal_a_role_onto_no_token_is_reported() {
    let (document, diagnostics) = converted(
        r##"{ "cssVars": { "light": { "surface": "#ffffff" } }, "roles": { "card": "nothing" } }"##,
        None,
    );
    assert!(document.roles.is_empty());
    assert!(mentions(
        &diagnostics,
        code::LEFT_OUT,
        "the theme has no token `nothing`"
    ));
}

#[test]
fn rule_7_over_a_base_makes_one_complete_document() {
    let (base, _) = converted(DEFAULT, None);
    let (document, _) = converted(COMPAT_BASE, Some(DEFAULT));
    for name in base.tokens.keys() {
        assert!(document.token(name).is_some(), "{name}");
    }
    // The base's first, in its order, then the app's own.
    let names: Vec<&str> = document.tokens.keys().collect();
    assert_eq!(
        names[..base.tokens.len()],
        base.tokens.keys().collect::<Vec<_>>()[..]
    );
    assert_eq!(
        document.token("radius-card").unwrap().value,
        Value::Number(14.0)
    );
    assert_eq!(document.token("up").unwrap().value, hex("#138a5a"));
    assert_eq!(document.token("radius").unwrap().value, Value::Number(10.0));
    // The app's wins by name, and its name is the document's.
    let (document, _) = converted(GALLERY, Some(DEFAULT));
    assert_eq!(document.token("primary").unwrap().value, hex("#c6ef6a"));
    assert_eq!(document.name.as_deref(), Some("NSH Gallery UI"));
}

#[test]
fn rule_7_a_theme_that_declares_tokens_replaces_a_base_that_declares_none() {
    // The old runtime read a theme that declares tokens over a UI set's own
    // defaults with no declarations (`Declarations::default()`): it replaced
    // them. Only its own names resolve.
    let (alone, _) = converted(DECLARED, None);
    let (document, diagnostics) = converted(DECLARED, Some(DEFAULT));
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let names: Vec<&str> = document.tokens.keys().collect();
    assert_eq!(names, alone.tokens.keys().collect::<Vec<_>>());
    assert!(document.token("background").is_none());
    assert!(document.token("radius").is_none());
    assert_eq!(document.name.as_deref(), Some("Declared"));
    assert_eq!(document, alone);
}

#[test]
fn rule_7_refusal_a_base_that_is_no_json_is_not_converted() {
    let (text, diagnostics) = convert(COMPAT_BASE, Some("{ \"cssVars\": "));
    assert_eq!(text, "");
    assert_eq!(codes(&diagnostics), [nsh_theme::code::NOT_A_DOCUMENT]);
    assert!(diagnostics[0].message.contains("the base theme"));
}

#[test]
fn rule_8_what_the_format_cannot_express_is_reported_and_left_out() {
    let (document, diagnostics) = converted(
        r##"{ "cssVars": { "theme": {
            "shadow-flat": "none",
            "background-image-layers": "linear-gradient(red, blue), linear-gradient(green, white)",
            "background-image-corner": "linear-gradient(to top right, red, blue)",
            "background-image-bands": "repeating-linear-gradient(red, blue 10%)",
            "brand": "#e11d48"
        } } }"##,
        None,
    );
    let names: Vec<&str> = document.tokens.keys().collect();
    assert_eq!(names, ["brand"]);
    for name in [
        "shadow-flat",
        "background-image-layers",
        "background-image-corner",
        "background-image-bands",
    ] {
        let diagnostic = diagnostics
            .iter()
            .find(|d| d.message.starts_with(&format!("`{name}`")))
            .unwrap_or_else(|| panic!("{name}: {diagnostics:#?}"));
        assert_eq!(diagnostic.code, code::INEXPRESSIBLE);
        assert!(!diagnostic.is_error());
        assert_eq!(diagnostic.pointer, format!("/cssVars/theme/{name}"));
    }
}

#[test]
fn refusal_text_that_is_no_json_object_is_not_converted() {
    for text in ["{", "[1]", "\"cssVars\""] {
        let (out, diagnostics) = convert(text, None);
        assert_eq!(out, "");
        assert_eq!(codes(&diagnostics), [nsh_theme::code::NOT_A_DOCUMENT]);
        assert!(diagnostics[0].is_error());
    }
}

#[test]
fn what_the_old_reader_left_out_is_reported() {
    let (document, diagnostics) = converted(
        r##"{ "cssVars": { "light": { "edge": "#zzz", "ink": "#111111" } } }"##,
        None,
    );
    assert!(document.token("edge").is_none());
    let edge = diagnostics
        .iter()
        .find(|d| d.message.contains("`edge`"))
        .unwrap();
    assert_eq!(edge.code, code::LEFT_OUT);
    assert_eq!(edge.pointer, "/cssVars/light/edge");
}

#[test]
fn the_outputs_read_with_no_error() {
    for (old, over) in [
        (DEFAULT, None),
        (GALLERY, Some(DEFAULT)),
        (COMPAT_BASE, Some(DEFAULT)),
        (APPEARANCE, None),
        (DECLARED, None),
    ] {
        let (_, diagnostics) = converted(old, over);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        // Writing is stable: the output read and written again is the same.
        let (text, _) = convert(old, over);
        assert_eq!(nsh_theme::write(&read(&text).document), text);
    }
}

// ---------------------------------------------------------------------------
// Equivalence: for every appearance and every choice of one context per old
// modifier, the old reader's values equal the resolved output's.

/// `old` (over `over`) converted, and compared with the old reader's values
/// in every combination.
fn assert_equivalent(old_text: &str, over: Option<&str>) {
    let (document, _) = converted(old_text, over);
    let old = reader::read_old(old_text, over).unwrap_or_else(|(_, e)| panic!("{e}"));
    let axes: Vec<(String, Vec<String>)> = old
        .modifiers
        .iter()
        .flat_map(|m| &m.axes)
        .map(|axis| {
            let contexts = axis.contexts.iter().map(|(c, _)| c.to_string()).collect();
            (axis.name.to_string(), contexts)
        })
        .collect();
    let mut combinations: Vec<Vec<(String, String)>> = vec![Vec::new()];
    for (axis, contexts) in &axes {
        combinations = combinations
            .into_iter()
            .flat_map(|picked| {
                contexts.iter().map(move |context| {
                    let mut picked = picked.clone();
                    picked.push((axis.clone(), context.clone()));
                    picked
                })
            })
            .collect();
    }
    let mut compared = 0;
    for combination in &combinations {
        let picks: Vec<(&str, &str)> = combination
            .iter()
            .map(|(m, c)| (m.as_str(), c.as_str()))
            .collect();
        let values = old.values(&picks);
        for dark in [false, true] {
            let new = resolved(&document, dark, &picks);
            for name in values.names() {
                if is_part(&old, name) {
                    continue;
                }
                let at = format!(
                    "`{name}` ({}, {picks:?})",
                    if dark { "dark" } else { "light" }
                );
                let token = document
                    .token(name)
                    .unwrap_or_else(|| panic!("{at}: not in the output"));
                let value = new
                    .get(name)
                    .unwrap_or_else(|| panic!("{at}: no resolved value"));
                compare(&values, name, token.value_type, value, dark, &at);
                compared += 1;
            }
        }
    }
    assert!(compared > 0);
}

/// Whether `name` is a typography's part, compared with its typography.
fn is_part(old: &Old, name: &str) -> bool {
    old.set
        .as_ref()
        .and_then(|s| s.get(name))
        .is_some_and(|d| d.part_of.is_some())
}

/// The old color rounded as rule 2 rounds it: each channel half away from
/// zero.
fn rounded(c: Hsla) -> Color {
    let c = Rgba::from(c);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgba(byte(c.r), byte(c.g), byte(c.b), byte(c.a))
}

fn packed(c: crate::Color) -> Color {
    let v = c.0;
    Color::rgba((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)
}

#[track_caller]
fn close(old: f32, new: f64, at: &str) {
    assert!(
        (f64::from(old) - new).abs() <= 1e-4,
        "{at}: old {old}, new {new}"
    );
}

#[track_caller]
fn member<T: Copy>(member: &Member<T>, at: &str) -> T {
    *member
        .value()
        .unwrap_or_else(|| panic!("{at}: a resolved member is a reference"))
}

/// The old value of `name` in `old` equals `new` by CONVERT-RULES' rules:
/// colors rounded then exact, numbers within 1e-4 points, fonts, strings
/// and booleans exactly, compounds member by member.
fn compare(old: &Store, name: &str, ty: ValueType, new: &Value, dark: bool, at: &str) {
    match (ty, new) {
        (ValueType::Color, Value::Color(c)) => {
            let o = old
                .color(name)
                .unwrap_or_else(|| panic!("{at}: no old color"));
            assert_eq!(rounded(*o.get(dark)), *c, "{at}");
        }
        (ValueType::Number, Value::Number(n)) => {
            close(old.number_in(name, dark).unwrap(), *n, at);
        }
        (ValueType::Duration, Value::Duration(d)) => {
            close(old.number_in(name, dark).unwrap(), d.millis, at);
        }
        (ValueType::Font, Value::Font(f)) => {
            // The old store kept the family drawn with: its first.
            let o = old.font(name).unwrap();
            assert_eq!(font_family(o), font_family(&f.0), "{at}");
        }
        (ValueType::String, Value::String(s)) => {
            assert_eq!(old.string(name), Some(s.as_str()), "{at}");
        }
        (ValueType::Boolean, Value::Boolean(b)) => {
            assert_eq!(old.boolean(name), Some(*b), "{at}");
        }
        (ValueType::Easing, Value::Easing(e)) => {
            let o = old.easing(name).unwrap();
            for (o, n) in o.iter().zip(e.to_bezier()) {
                close(*o, n, at);
            }
        }
        (ValueType::Shadow, Value::Shadow(s)) => {
            let o = old.shadow(name).unwrap().get(dark);
            assert_eq!(o.len(), s.0.len(), "{at}: layers");
            for (o, n) in o.iter().zip(&s.0) {
                assert_eq!(packed(o.color), member(&n.color, at), "{at}");
                close(o.offset_x.0, member(&n.x, at), at);
                close(o.offset_y.0, member(&n.y, at), at);
                close(o.blur.0, member(&n.blur, at), at);
                close(o.spread.0, member(&n.spread, at), at);
                assert_eq!(o.inset, n.inset, "{at}");
            }
        }
        (ValueType::Gradient, Value::Gradient(g)) => {
            let o = old.gradient(name).unwrap().get(dark);
            let [o] = o.as_slice() else {
                panic!("{at}: {} old gradients", o.len());
            };
            match (&o.kind, &g.kind) {
                (
                    crate::effects::GradientKind::Linear { angle },
                    nsh_theme::GradientKind::Linear { angle: new },
                ) => close(angle.0.to_degrees(), member(new, at), at),
                (
                    crate::effects::GradientKind::Conic { from, .. },
                    nsh_theme::GradientKind::Conic { from: new },
                ) => close(from.0.to_degrees(), member(new, at), at),
                (crate::effects::GradientKind::Radial { .. }, nsh_theme::GradientKind::Radial) => {}
                (o, n) => panic!("{at}: {o:?} became {n:?}"),
            }
            assert_eq!(o.stops.len(), g.stops.len(), "{at}: stops");
            for (o, n) in o.stops.iter().zip(&g.stops) {
                assert_eq!(packed(o.color), member(&n.color, at), "{at}");
                match (o.position, &n.position) {
                    (None, None) => {}
                    (Some(crate::geometry::DefiniteLength::Fraction(p)), Some(n)) => {
                        close(p.0, member(n, at), at);
                    }
                    (o, n) => panic!("{at}: stop at {o:?} became {n:?}"),
                }
            }
        }
        (ValueType::Typography, Value::Typography(t)) => {
            let part = |i: usize| old.number_in(&format!("{name}{}", TYPOGRAPHY_PARTS[i]), dark);
            close(
                old.number_in(name, dark).unwrap(),
                member(&t.font_size, at),
                at,
            );
            match (part(0), &t.line_height) {
                (None, None) => {}
                (Some(o), Some(n)) => match member(n, at) {
                    LineHeight::Multiple(n) => close(o, n, at),
                    other => panic!("{at}: line height {other:?}"),
                },
                (o, n) => panic!("{at}: line height {o:?} became {n:?}"),
            }
            for (o, n) in [(part(1), &t.letter_spacing), (part(2), &t.font_weight)] {
                match (o, n) {
                    (None, None) => {}
                    (Some(o), Some(n)) => close(o, member(n, at), at),
                    (o, n) => panic!("{at}: {o:?} became {n:?}"),
                }
            }
        }
        (ty, new) => panic!("{at}: a {ty} token resolved to {new:?}"),
    }
}

#[test]
fn equivalence_default_theme() {
    assert_equivalent(DEFAULT, None);
}

#[test]
fn equivalence_gallery_theme_over_the_default() {
    assert_equivalent(GALLERY, Some(DEFAULT));
}

#[test]
fn equivalence_compat_base_theme_over_the_default() {
    assert_equivalent(COMPAT_BASE, Some(DEFAULT));
}

#[test]
fn equivalence_contexts_that_differ_by_appearance() {
    assert_equivalent(APPEARANCE, None);
}

#[test]
fn equivalence_declared_tokens_and_a_late_default() {
    assert_equivalent(DECLARED, None);
}

#[test]
fn equivalence_post_editor_theme_over_the_default() {
    assert_equivalent(POST_EDITOR, Some(DEFAULT));
}

#[test]
fn equivalence_gallery_rs_theme_over_the_default() {
    assert_equivalent(GALLERY_RS, Some(DEFAULT));
}

#[test]
fn equivalence_declared_theme_over_the_default() {
    assert_equivalent(DECLARED, Some(DEFAULT));
}

#[test]
fn equivalence_font_and_boolean_vars() {
    assert_equivalent(
        r##"{
            "cssVars": { "theme": {
                "font-sans": "Inter, sans-serif", "font-heading": "var(--font-sans)",
                "motion": true, "motion-menus": "var(--motion)"
            } },
            "tokens": {
                "font-sans": { "kind": "font" }, "font-heading": { "kind": "font" },
                "motion": { "kind": "boolean" }, "motion-menus": { "kind": "boolean" }
            }
        }"##,
        None,
    );
}

#[test]
fn equivalence_a_role_var_over_a_declared_base() {
    assert_equivalent(
        r##"{ "cssVars": { "light": { "card": "var(--popover)", "ink": "#222222" } } }"##,
        Some(DECLARED),
    );
}

// ---------------------------------------------------------------------------
// The committed conversions: each app theme that extended nsh-base's default
// is in the repository as the converter writes it over that default.

/// The JSON of `text`, failing with `what`'s name.
fn json(text: &str, what: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{what}: {e}"))
}

/// `committed` is `old` converted over the default, as JSON.
fn assert_committed(old: &str, committed: &serde_json::Value, what: &str) {
    let (text, diagnostics) = convert(old, Some(DEFAULT));
    assert!(
        diagnostics.iter().all(|d| !d.is_error()),
        "{what}: {diagnostics:#?}"
    );
    assert_eq!(
        &json(&text, what),
        committed,
        "{what} is not the converter's output over the default"
    );
}

#[test]
fn committed_nsh_gallery_ui_theme_is_its_conversion_over_the_default() {
    let committed = include_str!("../../../../../examples/nsh-gallery-ui/theme.json");
    assert_committed(
        GALLERY,
        &json(committed, "nsh-gallery-ui"),
        "examples/nsh-gallery-ui/theme.json",
    );
}

#[test]
fn committed_post_editor_theme_is_its_conversion_over_the_default() {
    let committed = include_str!("../../../../../examples/post-editor/theme.json");
    assert_committed(
        POST_EDITOR,
        &json(committed, "post-editor"),
        "examples/post-editor/theme.json",
    );
}

#[test]
fn committed_gallery_rs_theme_is_its_conversion_over_the_default() {
    let committed = include_str!("../../../../../examples/gallery-rs/theme.json");
    assert_committed(
        GALLERY_RS,
        &json(committed, "gallery-rs"),
        "examples/gallery-rs/theme.json",
    );
}

#[test]
fn committed_compat_base_theme_is_its_conversion_over_the_default() {
    let config = include_str!("../../../../../tests/compat/cases/base/nsh.json");
    let theme = json(config, "tests/compat/cases/base/nsh.json")["theme"].clone();
    assert_committed(
        COMPAT_BASE,
        &theme,
        "tests/compat/cases/base/nsh.json's \"theme\"",
    );
}
