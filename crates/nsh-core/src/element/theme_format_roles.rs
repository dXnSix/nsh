//! Roles as data (ROLE-REGISTER, ROLE-READ, ROLE-CHECK): `roles!` with
//! every value type and `required`, `Registry::roles` merging without
//! panics, typed and by-name reads with the own-name fallback, and the role
//! checks on each of the core's channels (the document, an edited document,
//! the overrides file, `update_tokens`, a scope's patch, the host's patch),
//! B5 included.

use nsh_theme::code;

use super::theme_format_store::{bare_core, color, logged, said, tokens_of};
use crate::{
    Color, Core, Hsla,
    core::tests::FakeEngine,
    element::{
        CubicBezier, FontRole, NumberRole, Patch, PatchValues, Registry, Role, Token, Typography,
        ValueType, role, theme_scope::scoped,
    },
};

/// A design system's roles, one of each value type.
#[allow(unreachable_pub, reason = "as a UI set's crate writes its roles")]
mod kit {
    use std::{sync::Arc, time::Duration};

    use crate::{
        BoxShadow,
        effects::Gradient,
        element::{
            Blur, Border, CubicBezier, FontRole, NumberRole, Role, TransitionToken, Typography,
        },
    };

    crate::roles! {
        /// A banner's surface,
        /// whatever the app calls it.
        ///
        /// Name `banner`.
        pub BANNER: Role = "banner" required,
        /// How far apart a card's parts are.
        pub CARD_GAP: NumberRole = "card-gap",
        /// The headings' font.
        pub HEADING_FONT: FontRole = "heading-font",
        /// The greeting.
        pub GREETING: Role<String> = "greeting",
        /// Whether lists are dense.
        pub DENSE: Role<bool> = "dense",
        /// How long a press shows.
        pub PRESS: Role<Duration> = "press",
        /// How a sheet moves.
        pub SHEET_EASING: Role<CubicBezier> = "sheet-easing",
        /// The headings' text style.
        pub HEADING: Role<Typography> = "heading" required,
        /// A raised card's shadow.
        pub RAISED: Role<Arc<[BoxShadow]>> = "raised",
        /// The hero's background.
        pub HERO: Role<Gradient> = "hero",
        /// A sheet's backdrop.
        pub FROST: Role<Blur> = "frost",
        /// A field's outline.
        pub OUTLINE: Role<Border> = "outline",
        pub FADE: Role<TransitionToken> = "fade",
    }
}

/// A theme with names of its own, of every type, mapping the kit's roles.
const KIT_THEME: &str = r##"{
  "tokens": {
    "surface-banner": { "kind": "color", "value": "#112233" },
    "space-3": { "kind": "number", "value": 12 },
    "font-display": { "kind": "font", "value": "Fraunces, serif" },
    "hello": { "kind": "string", "value": "Hi" },
    "compact": { "kind": "boolean", "value": true },
    "fast": { "kind": "duration", "value": "150ms" },
    "snappy": { "kind": "easing", "value": [0.2, 0, 0, 1] },
    "title": { "kind": "typography", "value": { "fontSize": 20, "fontWeight": 600 } },
    "lift": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 2 }] },
    "dawn": { "kind": "gradient", "value": { "type": "linear", "angle": 90,
      "stops": [{ "color": "#7c3aed", "position": 0 }, { "color": "#db2777", "position": 1 }] } },
    "glass": { "kind": "blur", "value": { "type": "background", "radius": 8 } },
    "hairline": { "kind": "border", "value": { "color": "#112233", "width": 1, "style": "solid" } },
    "ease": { "kind": "transition", "value": { "duration": "200ms", "delay": "50ms", "easing": "ease-out" } }
  },
  "roles": {
    "banner": "surface-banner", "card-gap": "space-3", "heading-font": "font-display",
    "greeting": "hello", "dense": "compact", "press": "fast", "sheet-easing": "snappy",
    "heading": "title", "raised": "lift", "hero": "dawn", "frost": "glass",
    "outline": "hairline", "fade": "ease"
  }
}"##;

/// The document of the channel tests: `popover` mapped onto `surface`, a
/// token `popover` of its own, and a number.
const DOCUMENT: &str = r##"{
  "tokens": {
    "surface": { "kind": "color", "value": "#ffffff" },
    "overlay": { "kind": "color", "value": "#eeeeee" },
    "popover": { "kind": "color", "value": "#dddddd" },
    "gap": { "kind": "number", "value": 4 }
  },
  "roles": { "popover": "surface" }
}"##;

/// A core drawing with `document`, which reads with no diagnostic.
fn core_with(document: &str) -> Core<FakeEngine> {
    let mut c = bare_core();
    assert!(c.set_theme(Some(document)).is_empty());
    c.install_theme();
    c
}

/// The packed color `role` reads in `c`.
fn popover(c: &Core<FakeEngine>) -> Option<Color> {
    c.tokens().role(role::POPOVER).map(Color::from)
}

/// The lines of `lines` with diagnostic code `code`.
fn with_code<'a>(lines: &'a [String], code: &str) -> Vec<&'a String> {
    lines
        .iter()
        .filter(|l| l.starts_with(&format!("{code} ")))
        .collect()
}

#[test]
fn roles_declares_every_value_type_with_its_doc_and_whether_it_is_required() {
    let all = kit::all();
    let listed: Vec<(&str, ValueType, bool)> = all
        .iter()
        .map(|d| (d.name.as_str(), d.value_type, d.required))
        .collect();
    assert_eq!(
        listed,
        [
            ("banner", ValueType::Color, true),
            ("card-gap", ValueType::Number, false),
            ("heading-font", ValueType::Font, false),
            ("greeting", ValueType::String, false),
            ("dense", ValueType::Boolean, false),
            ("press", ValueType::Duration, false),
            ("sheet-easing", ValueType::Easing, false),
            ("heading", ValueType::Typography, true),
            ("raised", ValueType::Shadow, false),
            ("hero", ValueType::Gradient, false),
            ("frost", ValueType::Blur, false),
            ("outline", ValueType::Border, false),
            ("fade", ValueType::Transition, false),
        ]
    );
    // The doc's lines joined, without the paragraph naming it; none
    // without a doc.
    assert_eq!(
        all[0].doc.as_deref(),
        Some("A banner's surface, whatever the app calls it.")
    );
    assert_eq!(all[12].doc, None);
    // The constants are typed, and a role is its name.
    assert_eq!(kit::BANNER, Role::new("banner"));
    assert_eq!(kit::CARD_GAP, NumberRole::new("card-gap"));
    assert_eq!(Role::<Typography>::VALUE_TYPE, ValueType::Typography);
}

#[test]
fn the_cores_roles_are_declarations_none_required() {
    let all = role::all();
    let listed: Vec<(&str, ValueType)> = all
        .iter()
        .map(|d| (d.name.as_str(), d.value_type))
        .collect();
    assert_eq!(
        listed,
        [
            ("popover", ValueType::Color),
            ("popover-foreground", ValueType::Color),
            ("primary", ValueType::Color),
            ("accent", ValueType::Color),
            ("accent-foreground", ValueType::Color),
            ("muted-foreground", ValueType::Color),
            ("border", ValueType::Color),
            ("destructive", ValueType::Color),
            ("font-sans", ValueType::Font),
        ]
    );
    assert!(all.iter().all(|d| !d.required && d.doc.is_some()));
    let _: FontRole = role::FONT_SANS;
}

#[test]
fn a_role_two_sets_register_with_one_type_is_listed_once() {
    let mut a = Registry::ui_set("test/a");
    a.roles(kit::all());
    let mut b = Registry::ui_set("test/b");
    b.roles([
        kit::BANNER.declaration("Another set's words for it.", false),
        role::POPOVER.declaration("", false),
    ]);
    let (merged, lines) = logged(|| Registry::merged(&[a, b]));
    assert!(lines.is_empty(), "{lines:?}");
    let names: Vec<&str> = merged
        .role_specs()
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(names.iter().filter(|n| **n == "banner").count(), 1);
    assert_eq!(names.len(), kit::all().len() + 1);
    // The first keeps its doc and `required`.
    let banner = &merged.role_specs()[0];
    assert!(banner.required);
    assert_eq!(
        banner.doc.as_deref(),
        Some("A banner's surface, whatever the app calls it.")
    );
}

#[test]
fn a_role_two_sets_register_with_two_types_warns_role_conflict_and_keeps_the_first() {
    let mut a = Registry::ui_set("test/a");
    a.roles([kit::BANNER.declaration("", false)]);
    let mut b = Registry::ui_set("test/b");
    b.roles([NumberRole::new("banner").declaration("", false)]);
    let (merged, lines) = logged(|| Registry::merged(&[a, b]));
    let conflicts = with_code(&lines, code::ROLE_CONFLICT);
    assert_eq!(conflicts.len(), 1, "{lines:?}");
    // An empty pointer, the role, both types and both sets.
    let line = conflicts[0];
    assert!(line.starts_with("role-conflict : "), "{line}");
    for part in ["`banner`", "color", "number", "`test/a`", "`test/b`"] {
        assert!(line.contains(part), "{part}: {line}");
    }
    assert_eq!(merged.role_specs().len(), 1);
    assert_eq!(merged.role_specs()[0].value_type, ValueType::Color);
    // Against the core's own: the core's is kept, and nothing panics.
    let mut c = bare_core();
    let mut odd = Registry::ui_set("test/odd");
    odd.roles([NumberRole::new("popover").declaration("", false)]);
    let ((), lines) = logged(|| c.set_components(odd));
    assert!(said(&lines, code::ROLE_CONFLICT), "{lines:?}");
    c.set_theme(Some(DOCUMENT));
    c.install_theme();
    assert_eq!(popover(&c), Some(Color(0xffff_ffff)));
}

#[test]
fn a_set_registers_a_typography_role_and_a_required_role() {
    let mut set = Registry::ui_set("test/kit");
    set.roles(kit::all());
    let mut c = bare_core();
    c.set_components(set);
    let ((), lines) = logged(|| {
        assert!(c.set_theme(Some(KIT_THEME)).is_empty());
        c.install_theme();
    });
    assert!(lines.is_empty(), "{lines:?}");
    let heading = c.tokens().role(kit::HEADING).unwrap();
    assert_eq!(heading.size.0, 20.0);
    assert_eq!(heading.font_weight, Some(600.0));
    // A theme that maps neither required role: a warning for each, at load.
    let ((), lines) = logged(|| {
        c.set_theme(Some(
            r##"{ "tokens": { "title": { "kind": "typography", "value": { "fontSize": 16 } } } }"##,
        ));
        c.install_theme();
    });
    let unmapped = with_code(&lines, code::ROLE_UNMAPPED);
    assert_eq!(unmapped.len(), 2, "{lines:?}");
    assert!(unmapped.iter().any(|l| l.contains("/roles/heading")));
    assert!(unmapped.iter().any(|l| l.contains("/roles/banner")));
    assert!(c.tokens().role(kit::HEADING).is_none());
}

#[test]
fn every_value_type_reads_through_its_role() {
    let t = tokens_of(KIT_THEME);
    assert_eq!(
        t.role(kit::BANNER).map(Color::from),
        Some(Color(0x1122_33ff))
    );
    assert_eq!(t.role(kit::CARD_GAP), Some(12.0));
    assert_eq!(
        t.role(kit::HEADING_FONT).as_deref(),
        Some("Fraunces, serif")
    );
    assert_eq!(t.role(kit::GREETING).as_deref(), Some("Hi"));
    assert_eq!(t.role(kit::DENSE), Some(true));
    assert_eq!(
        t.role(kit::PRESS),
        Some(std::time::Duration::from_millis(150))
    );
    assert_eq!(
        t.role(kit::SHEET_EASING),
        Some(CubicBezier([0.2, 0.0, 0.0, 1.0]))
    );
    assert_eq!(t.role(kit::HEADING).map(|h| h.size.0), Some(20.0));
    assert_eq!(t.role(kit::RAISED).map(|s| s.len()), Some(1));
    assert!(t.role(kit::HERO).is_some());
    assert!(t.role(kit::FROST).is_some());
    assert_eq!(t.role(kit::OUTLINE).map(|b| b.width.0), Some(1.0));
    assert_eq!(
        t.role(kit::FADE).map(|f| f.delay),
        Some(std::time::Duration::from_millis(50))
    );
    // A role is no token.
    assert_eq!(t.get("banner"), None);
}

#[test]
fn role_by_name_reads_the_resolved_token_else_its_own() {
    let t = tokens_of(DOCUMENT);
    // Through its mapping.
    assert!(
        matches!(t.role_by_name("popover"), Some(Token::Color(c)) if Color::from(c) == Color(0xffff_ffff))
    );
    // Unmapped: the token of its own name, whatever its type.
    assert_eq!(t.role_by_name("gap"), Some(Token::Number(4.0)));
    assert_eq!(
        t.role_by_name("overlay").map(|t| t.value_type()),
        Some(ValueType::Color)
    );
    // Neither.
    assert_eq!(t.role_by_name("missing"), None);
    // A role of a Tailwind namespace whose own token has another type
    // reads nothing.
    let t = tokens_of(r##"{ "tokens": { "shadow-card": { "kind": "number", "value": 2 } } }"##);
    assert_eq!(t.role_by_name("shadow-card"), None);
}

#[test]
fn a_typed_read_of_another_type_gives_none_and_logs_role_type_once() {
    let c = core_with(DOCUMENT);
    let misread: Role<f32> = Role::new("popover");
    let (read, lines) = logged(|| {
        let first = c.tokens().role(misread);
        let second = c.tokens().role(misread);
        (first, second)
    });
    assert_eq!(read, (None, None));
    let typed = with_code(&lines, code::ROLE_TYPE);
    assert_eq!(typed.len(), 1, "{lines:?}");
    assert!(typed[0].contains("`surface`"), "{}", typed[0]);
    // The role read as what it is still reads.
    assert_eq!(popover(&c), Some(Color(0xffff_ffff)));
}

#[test]
fn a_declared_role_whose_own_token_has_another_type_reads_nothing() {
    let (c, lines) =
        logged(|| core_with(r##"{ "tokens": { "popover": { "kind": "number", "value": 3 } } }"##));
    let typed = with_code(&lines, code::ROLE_TYPE);
    assert_eq!(typed.len(), 1, "{lines:?}");
    assert!(
        typed[0].starts_with("role-type /tokens/popover: "),
        "{}",
        typed[0]
    );
    assert_eq!(popover(&c), None);
    assert_eq!(c.tokens().role_by_name("popover"), None);
    // The token is there, by name.
    assert_eq!(c.tokens().number("popover"), Some(3.0));
}

/// B5: the document does not map `popover`; the app's overrides map it
/// onto a number. The mapping is reported and left out, and `popover`
/// reads the token of its own name.
#[test]
fn b5_an_override_mapping_a_color_role_onto_a_number_is_left_out() {
    let document = r##"{ "tokens": {
        "popover": { "kind": "color", "value": "#dddddd" },
        "gap": { "kind": "number", "value": 4 } } }"##;
    let mut c = bare_core();
    let ((), lines) = logged(|| {
        assert!(
            c.set_app_theme(Some(document), Some(r#"{ "roles": { "popover": "gap" } }"#))
                .is_empty()
        );
        c.install_theme();
    });
    let typed = with_code(&lines, code::ROLE_TYPE);
    assert_eq!(typed.len(), 1, "{lines:?}");
    assert!(
        typed[0].starts_with("role-type /roles/popover: "),
        "{}",
        typed[0]
    );
    assert!(typed[0].contains("`gap`"), "{}", typed[0]);
    assert_eq!(popover(&c), Some(Color(0xdddd_ddff)));
    assert_eq!(c.tokens().role_name(role::POPOVER), "popover");
}

/// B5's second case: a wrong-typed patch mapping over a valid document
/// mapping reads the document's mapping, and changes no role.
#[test]
fn b5_a_wrong_typed_patch_mapping_over_a_valid_document_mapping_reads_the_documents() {
    let mut c = core_with(DOCUMENT);
    let ((), lines) = logged(|| {
        c.update_tokens(Patch::new().role("popover", "gap"));
        c.install_theme();
    });
    assert_eq!(with_code(&lines, code::ROLE_TYPE).len(), 1, "{lines:?}");
    assert_eq!(popover(&c), Some(Color(0xffff_ffff)));
    assert_eq!(c.tokens().mapped("popover"), Some("surface"));
    assert!(
        c.theme_changes().roles.is_empty(),
        "{:?}",
        c.theme_changes()
    );
    // A good one over it moves the role.
    c.update_tokens(Patch::new().role("popover", "overlay"));
    c.install_theme();
    assert_eq!(c.theme_changes().roles, ["popover"]);
    assert_eq!(popover(&c), Some(Color(0xeeee_eeff)));
}

#[test]
fn an_edited_document_that_removed_a_mapped_token_reads_the_role_s_own_token() {
    let mut c = core_with(DOCUMENT);
    assert_eq!(popover(&c), Some(Color(0xffff_ffff)));
    let edited = DOCUMENT.replace(
        r##""surface": { "kind": "color", "value": "#ffffff" },"##,
        "",
    );
    let said = c.set_theme(Some(&edited));
    assert!(
        said.iter().any(|d| d.code == code::ROLE_TOKEN_MISSING),
        "{said:?}"
    );
    c.install_theme();
    assert_eq!(popover(&c), Some(Color(0xdddd_ddff)));
    assert_eq!(c.theme_changes().roles, ["popover"]);
}

#[test]
fn update_tokens_mappings_are_checked_and_said_once() {
    let mut c = core_with(DOCUMENT);
    let ((), lines) = logged(|| {
        c.update_tokens(Patch::new().role("popover", "gap").role("gutter", "gap"));
        c.install_theme();
        // Installed again: said once.
        c.update_tokens(Patch::new().value("gap", 5.0));
        c.install_theme();
    });
    assert_eq!(with_code(&lines, code::ROLE_TYPE).len(), 1, "{lines:?}");
    // An undeclared role onto a number is known to no one: kept, with a
    // warning.
    assert_eq!(with_code(&lines, code::ROLE_UNKNOWN).len(), 1, "{lines:?}");
    assert_eq!(c.tokens().role_by_name("gutter"), Some(Token::Number(5.0)));
}

#[test]
fn the_hosts_mappings_are_checked_and_left_out() {
    let mut c = core_with(DOCUMENT);
    let ((), lines) = logged(|| {
        assert!(
            c.set_host_theme(Some(
                r#"{ "roles": { "popover": "gap", "primary": "overlay" } }"#
            ))
            .is_empty()
        );
        c.install_theme();
    });
    let typed = with_code(&lines, code::ROLE_TYPE);
    assert_eq!(typed.len(), 1, "{lines:?}");
    assert!(typed[0].contains("Patch 2"), "{}", typed[0]);
    assert_eq!(popover(&c), Some(Color(0xffff_ffff)));
    assert_eq!(
        c.tokens().role(role::PRIMARY).map(Color::from),
        Some(Color(0xeeee_eeff))
    );
}

#[test]
fn a_scopes_mappings_are_checked_and_the_stack_around_it_is_not_said_again() {
    let mut c = core_with(DOCUMENT);
    // The app's patch maps an undeclared role onto a number: said when
    // installed, not again for a scope.
    c.update_tokens(Patch::new().role("gutter", "gap"));
    c.install_theme();
    let (scope, said) = scoped(
        c.tokens(),
        None,
        Some(
            Patch::new()
                .role("popover", "gap")
                .role("accent", "overlay"),
        ),
    );
    let codes: Vec<&str> = said.iter().map(|d| d.code).collect();
    assert_eq!(codes, [code::ROLE_TYPE], "{said:?}");
    assert_eq!(
        scope.role(role::POPOVER).map(Color::from),
        Some(Color(0xffff_ffff))
    );
    assert_eq!(
        scope.role(role::ACCENT).map(Color::from),
        Some(Color(0xeeee_eeff))
    );
    // Around it, `accent` is not mapped.
    assert_eq!(c.tokens().role(role::ACCENT), None);
}

#[test]
fn a_tailwind_namespace_types_a_mapping_no_set_declares() {
    let (c, lines) = logged(|| {
        core_with(
            r##"{ "tokens": {
                "ink": { "kind": "color", "value": "#111111" },
                "corner": { "kind": "number", "value": 6 } },
              "roles": { "radius-card": "ink", "radius-chip": "corner", "ink-role": "ink" } }"##,
        )
    });
    let typed = with_code(&lines, code::ROLE_TYPE);
    assert_eq!(typed.len(), 1, "{lines:?}");
    assert!(typed[0].contains("radius-card"), "{}", typed[0]);
    // An undeclared role onto a color is known: no warning.
    assert!(!said(&lines, code::ROLE_UNKNOWN), "{lines:?}");
    assert_eq!(c.tokens().role_by_name("radius-card"), None);
    assert_eq!(
        c.tokens().role_by_name("radius-chip"),
        Some(Token::Number(6.0))
    );
    assert_eq!(
        c.tokens()
            .role(Role::<Hsla>::new("ink-role"))
            .map(Color::from),
        Some(Color(0x1111_11ff))
    );
    assert_eq!(color(c.tokens(), "ink"), Some(Color(0x1111_11ff)));
}

#[test]
fn a_role_read_in_code_follows_only_good_mappings() {
    let mut c = core_with(DOCUMENT);
    let _entered = c.test_runtime().enter();
    let read = || crate::element::role(role::POPOVER).map(Color::from);
    assert_eq!(read(), Some(Color(0xffff_ffff)));
    drop(_entered);
    c.update_tokens(Patch::new().role("popover", "gap"));
    c.install_theme();
    let _entered = c.test_runtime().enter();
    assert_eq!(read(), Some(Color(0xffff_ffff)));
}
