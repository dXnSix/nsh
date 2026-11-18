//! Tailwind's theme classes in the theme format (TW-ROLES, TW-COMPOUNDS,
//! TW-OVERRIDE, TW-SINGLE): a namespaced class reads its role, through a
//! mapping or a token of its own name; compounds are read whole; the theme
//! wins over Tailwind's built-in values; one rule per class, resolved again
//! when what it read changes.

use std::{rc::Rc, sync::Arc};

use nsh_theme::{Selection, code};

use super::{theme::NAMESPACES, *};
use crate::{
    effects::Filter,
    element::Tokens,
    geometry::px,
    style::{Color, FontFamily, Style},
};

/// The tokens of the theme document `json` as the core resolves them: its
/// roles against the core's declarations and Tailwind's namespaces.
pub(super) fn resolved(json: &str) -> Tokens {
    resolved_with(json, Selection::new(), &[])
}

/// [`resolved`] under `selection`, with the patches `patches` (JSON) over
/// it, a later one winning.
fn resolved_with(json: &str, selection: Selection, patches: &[&str]) -> Tokens {
    let read = nsh_theme::read(json);
    assert!(read.diagnostics.is_empty(), "{:?}", read.diagnostics);
    let patches = patches
        .iter()
        .map(|text| {
            let (patch, said) = nsh_theme::read_patch(text);
            assert!(said.is_empty(), "{said:?}");
            Arc::new(patch)
        })
        .collect();
    Tokens::resolve(Arc::new(read.document), selection, patches).0
}

fn theme_of(tokens: &Tokens) -> Theme {
    let mut theme = Theme::default();
    theme.set_tokens(tokens);
    theme
}

/// `classes` parsed with `tokens`, every one known.
fn list(tokens: &Tokens, classes: &str) -> ClassList {
    let list = parse(&theme_of(tokens), classes);
    assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
    list
}

/// The classes of `classes` that `tokens` leave unknown.
fn unknown(tokens: &Tokens, classes: &str) -> Vec<String> {
    parse(&theme_of(tokens), classes)
        .unknown()
        .iter()
        .map(|c| c.to_string())
        .collect()
}

/// The style `classes` give with `tokens` in the appearance `dark`.
fn styled(tokens: &Tokens, classes: &str, dark: bool) -> Style {
    let theme = theme_of(tokens);
    let list = parse(&theme, classes);
    assert!(list.unknown().is_empty(), "{classes}: {:?}", list.unknown());
    let mut style = Style::default();
    let group = |_: &str| 0;
    let context = Context {
        env: ClassEnvironment {
            dark,
            ..Default::default()
        },
        window: crate::geometry::size_pt(390.0, 844.0),
        rtl: false,
        states: 0,
        position: 0,
        group: &group,
    };
    list.apply(
        &context,
        &theme,
        &mut style,
        &mut ClassProps::default(),
        |_| {},
    );
    style
}

/// `named` and `arbitrary` give the same style with `tokens`.
fn same(tokens: &Tokens, named: &str, arbitrary: &str) {
    assert_eq!(
        format!("{:?}", styled(tokens, named, false)),
        format!("{:?}", styled(tokens, arbitrary, false)),
        "{named} as {arbitrary}"
    );
}

fn effects(style: &Style) -> crate::effects::Effects {
    style.view.effects.as_deref().cloned().unwrap_or_default()
}

/// Every row of TW-ROLES' table, each by a token of the role's own name.
const OWN: &str = r##"{ "tokens": {
    "radius-card": { "kind": "number", "value": 14 },
    "spacing-gutter": { "kind": "number", "value": 18 },
    "text-title": { "kind": "typography", "value": { "fontFamily": "Inter", "fontSize": 20,
        "fontWeight": 600, "lineHeight": 1.4, "letterSpacing": -0.5 } },
    "text-caption": { "kind": "number", "value": 11 },
    "font-display": { "kind": "font", "value": "Fraunces" },
    "font-weight-strong": { "kind": "number", "value": 650 },
    "tracking-label": { "kind": "number", "value": 0.5 },
    "leading-body": { "kind": "number", "value": 1.3 },
    "blur-soft": { "kind": "number", "value": 6 },
    "blur-glass": { "kind": "blur", "value": { "type": "background", "radius": 8 } },
    "duration-quick": { "kind": "duration", "value": "120ms" },
    "ease-snappy": { "kind": "easing", "value": [0.2, 0, 0, 1] },
    "shadow-raised": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 2 },
        { "color": "#ffffff80", "y": 1, "blur": 1, "inset": true }] },
    "background-image-hero": { "kind": "gradient", "value": { "type": "linear", "angle": 90,
        "stops": [{ "color": "#7c3aed", "position": 0 }, { "color": "#db2777", "position": 1 }] } },
    "brand": { "kind": "color", "value": "#2563eb" } } }"##;

/// The same values under names of the theme's own, each row's role mapped
/// onto one.
const MAPPED: &str = r##"{ "tokens": {
    "corner": { "kind": "number", "value": 14 },
    "gap": { "kind": "number", "value": 18 },
    "title": { "kind": "typography", "value": { "fontFamily": "Inter", "fontSize": 20,
        "fontWeight": 600, "lineHeight": 1.4, "letterSpacing": -0.5 } },
    "caption-size": { "kind": "number", "value": 11 },
    "display": { "kind": "font", "value": "Fraunces" },
    "strong": { "kind": "number", "value": 650 },
    "label-spacing": { "kind": "number", "value": 0.5 },
    "body-leading": { "kind": "number", "value": 1.3 },
    "soft": { "kind": "number", "value": 6 },
    "glass": { "kind": "blur", "value": { "type": "background", "radius": 8 } },
    "quick": { "kind": "duration", "value": "120ms" },
    "snappy": { "kind": "easing", "value": [0.2, 0, 0, 1] },
    "lift": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 2 },
        { "color": "#ffffff80", "y": 1, "blur": 1, "inset": true }] },
    "dawn": { "kind": "gradient", "value": { "type": "linear", "angle": 90,
        "stops": [{ "color": "#7c3aed", "position": 0 }, { "color": "#db2777", "position": 1 }] } },
    "ink": { "kind": "color", "value": "#2563eb" } },
  "roles": {
    "radius-card": "corner", "spacing-gutter": "gap", "text-title": "title",
    "text-caption": "caption-size", "font-display": "display", "font-weight-strong": "strong",
    "tracking-label": "label-spacing", "leading-body": "body-leading", "blur-soft": "soft",
    "blur-glass": "glass", "duration-quick": "quick", "ease-snappy": "snappy",
    "shadow-raised": "lift", "background-image-hero": "dawn", "brand": "ink" } }"##;

/// Both themes, each with the token a role resolves to.
fn themes() -> [Tokens; 2] {
    [resolved(OWN), resolved(MAPPED)]
}

/// The name of the token `role` reads in `tokens`.
fn read_by(tokens: &Tokens, role: &str) -> String {
    tokens.role_target(role).unwrap_or(role).to_owned()
}

/// `class` with `tokens` reads the token `role` resolves to, and the class
/// reads the theme.
fn reads_role(tokens: &Tokens, class: &str, role: &str) {
    let l = list(tokens, class);
    assert!(
        l.reads_token(&read_by(tokens, role)),
        "{class} reads {role}"
    );
    assert_ne!(l.deps() & deps::THEME, 0, "{class}");
}

#[test]
fn rounded_reads_its_radius_role() {
    for t in themes() {
        same(
            &t,
            "rounded-card rounded-tl-card",
            "rounded-[14px] rounded-tl-[14px]",
        );
        reads_role(&t, "rounded-b-card", "radius-card");
    }
}

#[test]
fn the_spacing_utilities_read_their_spacing_role() {
    for t in themes() {
        same(
            &t,
            "p-gutter -mt-gutter gap-x-gutter w-gutter max-h-gutter inset-x-gutter space-x-gutter",
            "p-[18px] -mt-[18px] gap-x-[18px] w-[18px] max-h-[18px] inset-x-[18px] space-x-[18px]",
        );
        reads_role(&t, "size-gutter", "spacing-gutter");
    }
}

#[test]
fn text_reads_a_typography_whole_or_a_number_for_the_size() {
    for t in themes() {
        let s = styled(&t, "text-title", false).text;
        assert_eq!(s.font_size, Some(px(20.0)));
        assert_eq!(s.font_weight, Some(600));
        assert_eq!(s.line_height_multiple, Some(1.4));
        assert_eq!(s.letter_spacing, Some(px(-0.5)));
        assert_eq!(s.font_family, Some(FontFamily::new("Inter")));
        reads_role(&t, "text-title", "text-title");
        // A number is the size alone.
        same(&t, "text-caption", "text-[11px]");
        assert_eq!(styled(&t, "text-caption", false).text.font_weight, None);
        reads_role(&t, "text-caption", "text-caption");
    }
}

#[test]
fn font_reads_a_font_role_and_a_weight_role() {
    for t in themes() {
        assert_eq!(
            styled(&t, "font-display", false).text.font_family,
            Some(FontFamily::new("Fraunces"))
        );
        reads_role(&t, "font-display", "font-display");
        same(&t, "font-strong", "font-[650]");
        reads_role(&t, "font-strong", "font-weight-strong");
    }
}

#[test]
fn tracking_reads_points_and_leading_a_multiple() {
    for t in themes() {
        same(&t, "tracking-label", "tracking-[0.5px]");
        same(&t, "leading-body", "leading-[1.3]");
        reads_role(&t, "tracking-label", "tracking-label");
        reads_role(&t, "leading-body", "leading-body");
    }
}

#[test]
fn blur_and_backdrop_blur_read_a_number_or_a_blurs_radius() {
    for t in themes() {
        let e = effects(&styled(&t, "blur-soft backdrop-blur-glass", false));
        assert_eq!(e.filters, [Filter::Blur(px(6.0))]);
        assert_eq!(e.backdrop_filters, [Filter::Blur(px(8.0))]);
        // The utility says where the radius goes, not the blur's kind.
        let e = effects(&styled(&t, "blur-glass backdrop-blur-soft", false));
        assert_eq!(e.filters, [Filter::Blur(px(8.0))]);
        assert_eq!(e.backdrop_filters, [Filter::Blur(px(6.0))]);
        same(&t, "blur-soft", "blur-[6px]");
        reads_role(&t, "backdrop-blur-glass", "blur-glass");
    }
}

#[test]
fn duration_delay_and_ease_read_their_roles() {
    for t in themes() {
        let transition = styled(
            &t,
            "transition duration-quick delay-quick ease-snappy",
            false,
        )
        .view
        .transition
        .expect("a transition");
        assert_eq!(transition.duration, 120.0);
        assert_eq!(transition.delay, 120.0);
        assert_eq!(transition.easing, [0.2, 0.0, 0.0, 1.0]);
        reads_role(&t, "delay-quick", "duration-quick");
        reads_role(&t, "ease-snappy", "ease-snappy");
    }
}

#[test]
fn shadow_reads_its_layers_inset_ones_included() {
    for t in themes() {
        let layers = t
            .role_by_name("shadow-raised")
            .and_then(|token| match token {
                crate::element::Token::Shadow(layers) => Some(layers),
                _ => None,
            })
            .expect("a shadow");
        assert_eq!(layers.len(), 2);
        assert!(layers[1].inset);
        assert_eq!(
            effects(&styled(&t, "shadow-raised", false)).box_shadows,
            layers.to_vec()
        );
        reads_role(&t, "shadow-raised", "shadow-raised");
    }
}

#[test]
fn bg_reads_a_gradient_role_and_a_color_role() {
    for t in themes() {
        let gradient = t
            .gradient(&read_by(&t, "background-image-hero"))
            .cloned()
            .expect("a gradient");
        assert_eq!(
            effects(&styled(&t, "bg-hero", false)).background_images,
            [gradient]
        );
        reads_role(&t, "bg-hero", "background-image-hero");
        let s = styled(&t, "bg-brand text-brand/50 border-t-brand", false);
        assert_eq!(s.view.background_color, Color(0x2563_ebff));
        assert_eq!(s.text.color, Some(Color(0x2563_ebff).with_alpha(0.5)));
        reads_role(&t, "border-t-brand", "brand");
    }
}

#[test]
fn a_mapped_role_and_a_token_of_its_name_draw_the_same() {
    let [own, mapped] = themes();
    let classes = "rounded-card p-gutter text-title font-display font-strong tracking-label \
                   leading-body blur-soft backdrop-blur-glass transition duration-quick ease-snappy \
                   shadow-raised bg-hero text-brand";
    assert_eq!(
        format!("{:?}", styled(&own, classes, false)),
        format!("{:?}", styled(&mapped, classes, false))
    );
    // `corner-card` mapped as `radius-card`, or a token `radius-card`.
    let declared = resolved(
        r##"{ "tokens": { "corner-card": { "kind": "number", "value": 9 } },
              "roles": { "radius-card": "corner-card" } }"##,
    );
    let named = resolved(r##"{ "tokens": { "radius-card": { "kind": "number", "value": 9 } } }"##);
    assert_eq!(
        styled(&declared, "rounded-card", false).view.border_radius,
        px(9.0)
    );
    assert_eq!(
        format!("{:?}", styled(&declared, "rounded-card", false)),
        format!("{:?}", styled(&named, "rounded-card", false))
    );
}

#[test]
fn a_typographys_parts_give_way_to_the_classes_of_their_own() {
    let t = resolved(
        r##"{ "tokens": { "text-title": { "kind": "typography", "value": { "fontFamily": "Inter",
            "fontSize": 20, "fontWeight": 600, "lineHeight": "28px", "letterSpacing": -0.5 } } } }"##,
    );
    let s = styled(&t, "text-title", false).text;
    assert_eq!(s.line_height, Some(px(28.0)));
    // `leading-*`, `tracking-*` and `font-*` win wherever they are, as
    // Tailwind 4's `--text-*--line-height` and friends.
    let s = styled(
        &t,
        "leading-tight text-title tracking-wide font-bold font-mono",
        false,
    )
    .text;
    assert_eq!(s.font_size, Some(px(20.0)));
    assert_eq!(s.line_height_multiple, Some(1.25));
    assert_eq!(s.letter_spacing_em, Some(0.025));
    assert_eq!(s.font_weight, Some(700));
    assert_eq!(s.font_family, Some(FontFamily::new("monospace")));
}

#[test]
fn gradients_of_every_kind_are_read_whole() {
    let t = resolved(
        r##"{ "tokens": {
            "background-image-glow": { "kind": "gradient", "value": { "type": "radial",
                "stops": [{ "color": "#ffffff" }, { "color": "#ffffff00" }] } },
            "background-image-spin": { "kind": "gradient", "value": { "type": "conic", "angle": 90,
                "stops": [{ "color": "#0090ff" }, { "color": "#0090ff00" }] } } } }"##,
    );
    for name in ["glow", "spin"] {
        let gradient = t
            .gradient(&format!("background-image-{name}"))
            .cloned()
            .expect("a gradient");
        assert_eq!(
            effects(&styled(&t, &format!("bg-{name}"), false)).background_images,
            [gradient],
            "{name}"
        );
    }
}

/// The longer utility first: `inset-shadow-card` is the inset shadow's
/// color `card`, whatever `spacing-shadow-card` is.
#[test]
fn the_longer_utility_comes_first() {
    let t = resolved(
        r##"{ "tokens": { "card": { "kind": "color", "value": "#ff0000" },
              "spacing-shadow-card": { "kind": "number", "value": 5 } } }"##,
    );
    same(&t, "inset-shadow-card", "inset-shadow-[color:#ff0000ff]");
    let l = list(&t, "inset-shadow-card");
    assert!(l.reads_token("card") && !l.reads_token("spacing-shadow-card"));
}

/// At one utility, a namespaced role before a bare color of the same name.
#[test]
fn a_namespaced_role_comes_before_a_bare_color() {
    let t = resolved(
        r##"{ "tokens": { "raised": { "kind": "color", "value": "#ff0000" },
              "shadow-raised": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 4 }] } } }"##,
    );
    let shadows = effects(&styled(&t, "shadow-raised", false)).box_shadows;
    assert_eq!(shadows.len(), 1);
    assert_eq!(shadows[0].color, Color(0x0000_001a));
    assert!(list(&t, "shadow-raised").reads_token("shadow-raised"));
    // The color is the class's where no shadow is.
    let colored =
        resolved(r##"{ "tokens": { "raised": { "kind": "color", "value": "#ff0000" } } }"##);
    same(
        &colored,
        "shadow-md shadow-raised",
        "shadow-md shadow-[color:#ff0000]",
    );
}

/// `text-<n>`: a color, then a typography, then a number for the size.
#[test]
fn text_takes_a_color_then_a_typography_then_a_number() {
    let both = resolved(
        r##"{ "tokens": { "title": { "kind": "color", "value": "#ff0000" },
              "text-title": { "kind": "typography", "value": { "fontSize": 20 } } } }"##,
    );
    let s = styled(&both, "text-title", false).text;
    assert_eq!(s.color, Some(Color(0xff00_00ff)));
    assert_eq!(s.font_size, None);
    let typography = resolved(
        r##"{ "tokens": { "text-title": { "kind": "typography", "value": { "fontSize": 20 } } } }"##,
    );
    assert_eq!(
        styled(&typography, "text-title", false).text.font_size,
        Some(px(20.0))
    );
    let number = resolved(
        r##"{ "tokens": { "title": { "kind": "color", "value": "#ff0000" },
        "text-small": { "kind": "number", "value": 11 } } }"##,
    );
    let s = styled(&number, "text-small text-title", false).text;
    assert_eq!(s.font_size, Some(px(11.0)));
    assert_eq!(s.color, Some(Color(0xff00_00ff)));
}

/// `font-<n>`: a font, then a weight; `bg-<n>`: a color, then a gradient.
#[test]
fn font_takes_a_font_then_a_weight_and_bg_a_color_then_a_gradient() {
    let t = resolved(
        r##"{ "tokens": {
            "font-heavy": { "kind": "font", "value": "Heavy Sans" },
            "font-weight-heavy": { "kind": "number", "value": 850 },
            "font-weight-thick": { "kind": "number", "value": 750 },
            "hero": { "kind": "color", "value": "#00ff00" },
            "background-image-hero": { "kind": "gradient", "value": { "type": "linear",
                "stops": [{ "color": "#000000" }, { "color": "#ffffff" }] } },
            "background-image-dusk": { "kind": "gradient", "value": { "type": "linear",
                "stops": [{ "color": "#000000" }, { "color": "#ffffff" }] } } } }"##,
    );
    let s = styled(&t, "font-heavy", false).text;
    assert_eq!(s.font_family, Some(FontFamily::new("Heavy Sans")));
    assert_eq!(s.font_weight, None);
    assert_eq!(styled(&t, "font-thick", false).text.font_weight, Some(750));
    let s = styled(&t, "bg-hero", false);
    assert_eq!(s.view.background_color, Color(0x00ff_00ff));
    assert!(effects(&s).background_images.is_empty());
    assert_eq!(
        effects(&styled(&t, "bg-dusk", false))
            .background_images
            .len(),
        1
    );
}

/// A role of a type its row doesn't take is skipped there: the class falls
/// to the next row, else to Tailwind's built-in utility, else is unknown.
#[test]
fn a_wrong_typed_role_is_skipped_for_its_row() {
    // The theme's mapping is refused at its layer: `font-heavy` reads no
    // font, and `font-heavy` is the weight `font-weight-heavy`.
    let t = resolved(
        r##"{ "tokens": {
            "seven": { "kind": "number", "value": 7 },
            "red": { "kind": "color", "value": "#ff0000" },
            "font-weight-heavy": { "kind": "number", "value": 850 } },
          "roles": { "font-heavy": "seven", "shadow-md": "seven", "radius-pill": "red" } }"##,
    );
    assert_eq!(styled(&t, "font-heavy", false).text.font_weight, Some(850));
    // `shadow-md` mapped onto a number draws Tailwind's `shadow-md`.
    let plain = Tokens::default();
    assert_eq!(
        effects(&styled(&t, "shadow-md", false)).box_shadows,
        effects(&styled(&plain, "shadow-md", false)).box_shadows
    );
    assert!(!list(&t, "shadow-md").reads_token("seven"));
    // Neither a namespace nor Tailwind's: unknown.
    assert_eq!(unknown(&t, "rounded-pill"), ["rounded-pill"]);
    // A store made without a document's checks (a test's) is skipped by the
    // row's type too.
    let mut unchecked =
        Tokens::default().with_roles([("shadow-md", "seven"), ("radius-pill", "red")]);
    unchecked.set_number("seven", 7.0);
    unchecked.set_color("red", crate::color::rgb(0xff0000));
    assert_eq!(
        effects(&styled(&unchecked, "shadow-md", false)).box_shadows,
        effects(&styled(&plain, "shadow-md", false)).box_shadows
    );
    assert_eq!(unknown(&unchecked, "rounded-pill"), ["rounded-pill"]);
}

/// The namespaces are the role patterns the core checks a theme's mappings
/// against: a wrong-typed one is reported, a right one is not.
#[test]
fn tailwinds_mappings_are_checked_as_role_patterns() {
    let patterns = role_patterns();
    assert_eq!(patterns.len(), NAMESPACES.len());
    for namespace in NAMESPACES {
        assert!(
            patterns
                .iter()
                .any(|p| p.prefix == namespace.role && p.accepts == namespace.accepts),
            "{}",
            namespace.role
        );
    }
    let read = nsh_theme::read(
        r##"{ "tokens": {
            "seven": { "kind": "number", "value": 7 },
            "red": { "kind": "color", "value": "#ff0000" },
            "title": { "kind": "typography", "value": { "fontSize": 20 } } },
          "roles": { "shadow-md": "seven", "radius-card": "red", "text-title": "title",
                     "spacing-gutter": "seven" } }"##,
    );
    let document = read.document;
    let resolved = nsh_theme::resolve(&document, &Selection::new(), &[]);
    let said = nsh_theme::check_roles(&document, &[], &resolved, &[], &patterns);
    let mut typed: Vec<&str> = said
        .iter()
        .filter(|d| d.code == code::ROLE_TYPE)
        .map(|d| d.pointer.as_str())
        .collect();
    typed.sort_unstable();
    assert_eq!(
        typed,
        ["/roles/radius-card", "/roles/shadow-md"],
        "{said:?}"
    );
    assert_eq!(said.len(), 2, "{said:?}");
}

/// TW-OVERRIDE: a theme's role or token whose class is one of Tailwind's
/// built-in values wins over it.
#[test]
fn the_themes_names_win_over_tailwinds_built_in_values() {
    let t = resolved(
        r##"{ "tokens": {
            "shadow-sm": { "kind": "shadow", "value": [{ "color": "#ff000080", "y": 9 }] },
            "lifted": { "kind": "shadow", "value": [{ "color": "#00ff0080", "y": 7 }] },
            "radius-lg": { "kind": "number", "value": 21 },
            "text-lg": { "kind": "typography", "value": { "fontSize": 30, "lineHeight": 1.1 } },
            "blur-md": { "kind": "number", "value": 3 },
            "ease-in": { "kind": "easing", "value": [0.5, 0, 1, 1] },
            "duration-150": { "kind": "duration", "value": "400ms" },
            "spacing-4": { "kind": "number", "value": 99 } },
          "roles": { "shadow-md": "lifted" } }"##,
    );
    for (class, role) in [("shadow-sm", "shadow-sm"), ("shadow-md", "shadow-md")] {
        let token = t.role_by_name(role);
        let Some(crate::element::Token::Shadow(layers)) = token else {
            panic!("{role}: {token:?}");
        };
        // `token("shadow-md")` and the class `shadow-md` draw the same.
        assert_eq!(
            effects(&styled(&t, class, false)).box_shadows,
            layers.to_vec(),
            "{class}"
        );
    }
    assert_eq!(styled(&t, "rounded-lg", false).view.border_radius, px(21.0));
    let s = styled(&t, "text-lg", false).text;
    assert_eq!(s.font_size, Some(px(30.0)));
    assert_eq!(s.line_height_multiple, Some(1.1));
    assert_eq!(
        effects(&styled(&t, "blur-md", false)).filters,
        [Filter::Blur(px(3.0))]
    );
    let transition = styled(&t, "transition ease-in duration-150", false)
        .view
        .transition
        .expect("a transition");
    assert_eq!(transition.easing, [0.5, 0.0, 1.0, 1.0]);
    assert_eq!(transition.duration, 400.0);
    same(&t, "p-4 mt-4", "p-[99px] mt-[99px]");
    assert!(list(&t, "p-4").reads_token("spacing-4"));
}

/// Without a theme's name, Tailwind's built-in value is unchanged.
#[test]
fn the_built_in_values_a_theme_does_not_name_stay_tailwinds() {
    let plain = Tokens::default();
    let t = resolved(
        r##"{ "tokens": { "shadow-raised": { "kind": "shadow", "value": [{ "color": "#ff0000", "y": 9 }] },
              "radius-card": { "kind": "number", "value": 21 } } }"##,
    );
    let classes =
        "shadow-sm shadow-md rounded-lg text-lg blur-md transition ease-in duration-150 p-4";
    assert_eq!(
        format!("{:?}", styled(&t, classes, false)),
        format!("{:?}", styled(&plain, classes, false))
    );
    let l = list(&t, classes);
    assert_eq!(l.deps() & deps::THEME, 0);
    assert_eq!(
        styled(&plain, "rounded-lg", false).view.border_radius,
        px(8.0)
    );
}

/// A class the theme cannot serve is told without a parse: no token read,
/// nothing declared.
#[test]
fn a_class_the_theme_cannot_serve_reads_nothing() {
    let t = resolved(OWN);
    let theme = theme_of(&t);
    let tokens = theme.tokens();
    let mut read = Vec::new();
    for class in [
        "p-4",
        "rounded-lg",
        "flex",
        "bg-red-500",
        "bg-linear-to-r",
        "text-sm/6",
        "w-1/2",
        "p-[4px]",
        "bg-(--brand)",
    ] {
        assert_eq!(tokens.class(&theme, class, &mut read), None, "{class}");
    }
    assert!(read.is_empty());
    // A theme with no names serves none.
    let empty = ClassTokens::new(&Tokens::default());
    assert_eq!(empty.class(&theme, "bg-brand", &mut read), None);
    assert!(tokens.class(&theme, "bg-brand", &mut read).is_some());
}

/// TW-SINGLE: one rule per class, with the values of now; the appearance is
/// no dependency of it.
#[test]
fn a_token_class_is_one_rule() {
    for t in themes() {
        let l = list(
            &t,
            "bg-brand rounded-card shadow-raised text-title bg-(--brand)",
        );
        assert_eq!(l.rules.len(), 5);
        assert!(l.rules.iter().all(|r| r.theme && r.cond.dark.is_none()));
        assert_eq!(l.deps() & deps::DARK, 0);
        assert_eq!(
            format!("{:?}", styled(&t, "bg-brand rounded-card", false)),
            format!("{:?}", styled(&t, "bg-brand rounded-card", true))
        );
    }
}

/// `dark:` follows the color scheme, whatever the theme's modifiers are.
#[test]
fn dark_follows_the_color_scheme() {
    let t = resolved(
        r##"{ "tokens": { "brand": { "kind": "color", "value": "#2563eb" },
              "night": { "kind": "color", "value": "#111111" } },
          "modifiers": { "colorMode": { "default": "light",
              "contexts": { "light": {}, "dark": { "brand": "#93c5fd" } } } },
          "resolutionOrder": ["colorMode"] }"##,
    );
    let classes = "bg-brand dark:bg-night";
    assert_eq!(
        styled(&t, classes, false).view.background_color,
        Color(0x2563_ebff)
    );
    assert_eq!(
        styled(&t, classes, true).view.background_color,
        Color(0x1111_11ff)
    );
    let l = list(&t, classes);
    assert_ne!(l.deps() & deps::DARK, 0);
}

/// After a pick of a modifier's context, the classes reading a token it
/// changed resolve again, and the others don't.
#[test]
fn a_pick_resolves_again_the_classes_reading_what_it_changed() {
    let document = r##"{ "tokens": { "brand": { "kind": "color", "value": "#2563eb" },
          "radius-card": { "kind": "number", "value": 14 } },
      "modifiers": { "colorMode": { "default": "light",
          "contexts": { "light": {}, "dark": { "brand": "#93c5fd" } } } },
      "resolutionOrder": ["colorMode"] }"##;
    let mut tailwind = Tailwind::new();
    tailwind.set_tokens(&resolved(document));
    let brand = tailwind.classes("p-2 bg-brand");
    let card = tailwind.classes("rounded-card");
    let dark = resolved_with(document, Selection::new().with("colorMode", "dark"), &[]);
    let changed = tailwind.set_tokens(&dark).expect("a change");
    assert!(matches!(changed, TokenChange::Values(_)), "{changed:?}");
    assert!(brand.reads(&changed) && !card.reads(&changed));
    assert!(Rc::ptr_eq(&card, &tailwind.classes("rounded-card")));
    let again = tailwind.classes("p-2 bg-brand");
    assert!(!Rc::ptr_eq(&brand, &again));
    let theme = tailwind.theme().clone();
    let mut style = Style::default();
    let group = |_: &str| 0;
    let context = Context {
        env: ClassEnvironment::default(),
        window: crate::geometry::size_pt(390.0, 844.0),
        rtl: false,
        states: 0,
        position: 0,
        group: &group,
    };
    again.apply(
        &context,
        &theme,
        &mut style,
        &mut ClassProps::default(),
        |_| {},
    );
    assert_eq!(style.view.background_color, Color(0x93c5_fdff));
}

/// A patch that only re-maps a role resolves again the classes reading
/// that role, and no other.
#[test]
fn a_role_only_patch_resolves_again_the_classes_reading_the_role() {
    let document = r##"{ "tokens": { "corner": { "kind": "number", "value": 14 },
          "corner-small": { "kind": "number", "value": 4 },
          "brand": { "kind": "color", "value": "#2563eb" } },
      "roles": { "radius-card": "corner" } }"##;
    let mut tailwind = Tailwind::new();
    tailwind.set_tokens(&resolved(document));
    let card = tailwind.classes("rounded-card");
    let brand = tailwind.classes("bg-brand");
    let patched = resolved_with(
        document,
        Selection::new(),
        &[r#"{ "roles": { "radius-card": "corner-small" } }"#],
    );
    let changed = tailwind.set_tokens(&patched).expect("a change");
    assert!(matches!(changed, TokenChange::Values(_)), "{changed:?}");
    assert!(card.reads(&changed) && !brand.reads(&changed));
    assert!(Rc::ptr_eq(&brand, &tailwind.classes("bg-brand")));
    let now = tailwind.classes("rounded-card");
    assert!(now.reads_token("corner-small") && !now.reads_token("corner"));
    // Unmapped again, `radius-card` reads no token: every class may be
    // another one.
    let unmapped = resolved(
        r##"{ "tokens": { "corner": { "kind": "number", "value": 14 },
              "brand": { "kind": "color", "value": "#2563eb" } } }"##,
    );
    assert_eq!(tailwind.set_tokens(&unmapped), Some(TokenChange::All));
    assert_eq!(tailwind.classes("rounded-card").unknown().len(), 1);
}

/// `describe` names the token a class reads through its role.
#[test]
fn describe_names_the_token_a_role_resolves_to() {
    let t = resolved(MAPPED);
    let theme = theme_of(&t);
    let tokens = |class: &str| describe(&theme, class).expect("a class").tokens;
    assert_eq!(tokens("rounded-card"), ["corner"]);
    assert_eq!(tokens("hover:shadow-raised"), ["lift"]);
    assert_eq!(tokens("bg-brand/50"), ["ink"]);
    assert_eq!(tokens("bg-(--brand)"), ["ink"]);
    assert!(describe(&theme, "text-title").expect("a class").reads_theme);
}

/// The modifiers Tailwind's `text-<n>` and `shadow-<n>` take, on a theme's:
/// a line height and an alpha, so one name has one value with them or
/// without.
const MODIFIED: &str = r##"{ "tokens": {
    "text-sm": { "kind": "number", "value": 15 },
    "text-title": { "kind": "typography", "value": { "fontSize": 20, "lineHeight": 1.4,
        "fontWeight": 600 } },
    "shadow-md": { "kind": "shadow", "value": [{ "color": "#ff000080", "y": 9 }] },
    "shadow-raised": { "kind": "shadow", "value": [{ "color": "#0000ff", "y": 3 },
        { "color": "#00ff0040", "y": 1, "inset": true }] },
    "radius-card": { "kind": "number", "value": 14 } } }"##;

/// `text-<n>/<line>` with a theme's number: its size, and the modifier's
/// line height, as `text-[<size>px]/<line>`; the theme's name over
/// Tailwind's (`text-sm/6`) as well as one of its own.
#[test]
fn a_themes_text_size_takes_a_line_height_modifier() {
    let t = resolved(MODIFIED);
    same(&t, "text-sm/6", "text-[15px]/6");
    same(&t, "text-sm/[1.5]", "text-[15px]/[1.5]");
    let s = styled(&t, "text-sm/6", false).text;
    assert_eq!(s.font_size, Some(px(15.0)));
    assert_eq!(s.line_height, Some(px(24.0)));
    // With the modifier or without, the theme's size.
    assert_eq!(styled(&t, "text-sm", false).text.font_size, Some(px(15.0)));
    assert!(list(&t, "text-sm/6").reads_token("text-sm"));
    // Without the theme's name, Tailwind's.
    let plain = Tokens::default();
    assert_eq!(
        styled(&plain, "text-sm/6", false).text.font_size,
        Some(px(14.0))
    );
}

/// `text-<n>/<line>` with a theme's typography: the style whole, with the
/// modifier's line height in place of its own.
#[test]
fn a_themes_typography_takes_a_line_height_modifier() {
    let t = resolved(MODIFIED);
    let s = styled(&t, "text-title/6", false).text;
    assert_eq!(s.font_size, Some(px(20.0)));
    assert_eq!(s.font_weight, Some(600));
    assert_eq!(s.line_height, Some(px(24.0)));
    assert_eq!(s.line_height_multiple, None);
    let s = styled(&t, "text-title/[1.25]", false).text;
    assert_eq!(s.line_height_multiple, Some(1.25));
    assert_eq!(
        styled(&t, "text-title", false).text.line_height_multiple,
        Some(1.4)
    );
    // A line height Tailwind cannot read leaves the class unknown.
    assert_eq!(unknown(&t, "text-title/nope"), ["text-title/nope"]);
}

/// `shadow-<n>/<alpha>` with a theme's shadow: its layers at the
/// modifier's alpha, as Tailwind's sizes take one, and that alpha for a
/// shadow color; the theme's name over Tailwind's (`shadow-md/20`) as well
/// as one of its own.
#[test]
fn a_themes_shadow_takes_an_alpha_modifier() {
    let t = resolved(MODIFIED);
    for (class, layers) in [("shadow-md/20", 1), ("shadow-raised/20", 2)] {
        let shadows = effects(&styled(&t, class, false)).box_shadows;
        assert_eq!(shadows.len(), layers, "{class}");
        assert!(
            shadows.iter().all(|l| l.color.0 & 0xff == 0x33),
            "{class}: {shadows:?}"
        );
    }
    let raised = effects(&styled(&t, "shadow-raised/20", false)).box_shadows;
    assert_eq!(raised[0].color, Color(0x0000_ff33));
    assert!(raised[1].inset);
    assert_eq!(raised[0].offset_y, px(3.0));
    // With the modifier or without, the theme's layers.
    let md = effects(&styled(&t, "shadow-md", false)).box_shadows;
    assert_eq!(md.len(), 1);
    assert_eq!(md[0].offset_y, px(9.0));
    assert_eq!(
        effects(&styled(&t, "shadow-md/20", false)).box_shadows[0].offset_y,
        px(9.0)
    );
    // The alpha reaches a shadow color, as Tailwind's `--tw-shadow-alpha`
    // does for its sizes.
    let plain = Tokens::default();
    let alpha = |tokens: &Tokens, classes: &str| {
        effects(&styled(tokens, classes, false)).box_shadows[0]
            .color
            .0
            & 0xff
    };
    assert_eq!(
        alpha(&t, "shadow-raised/20 shadow-red-500"),
        alpha(&plain, "shadow-md/20 shadow-red-500")
    );
    let l = list(&t, "shadow-md/20 shadow-raised/[0.5]");
    assert!(l.reads_token("shadow-md") && l.reads_token("shadow-raised"));
    // Without the theme's name, Tailwind's.
    let plain = Tokens::default();
    assert_eq!(
        effects(&styled(&plain, "shadow-md/20", false))
            .box_shadows
            .len(),
        2
    );
}

/// The other rows take no modifier, as Tailwind's own utilities of them
/// do not.
#[test]
fn the_other_rows_refuse_a_modifier() {
    let t = resolved(MODIFIED);
    assert_eq!(unknown(&t, "rounded-card/20"), ["rounded-card/20"]);
    assert_eq!(unknown(&t, "shadow-raised/nope"), ["shadow-raised/nope"]);
}
