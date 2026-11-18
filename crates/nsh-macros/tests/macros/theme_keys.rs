//! `theme_keys!`: a typed key for each token a theme document declares, in
//! modules by its path, typed by its value type, reading the tokens the
//! document makes. The fixtures are the old `cssVars` ones, converted.

use std::sync::Arc;

use nsh_core::{
    BoxShadow, Hsla,
    color::rgb,
    element::{Registry, TokenKey, Tokens},
    geometry::Points,
};

/// The tokens of the document `json` that the set `set` registers, its
/// default contexts picked.
fn tokens_of(set: &'static str, json: &'static str) -> Arc<Tokens> {
    let mut registry = Registry::ui_set(set);
    registry.theme_document(json);
    let document = registry.theme_document_of_sets().0.unwrap();
    Arc::new(Tokens::resolve(document, Default::default(), Vec::new()).0)
}

mod keys {
    nsh_core::element::theme_keys!("tests/macros/theme_keys/theme.json");
}

#[test]
fn keys_follow_the_paths_and_kinds() {
    let primary: TokenKey<Hsla> = keys::colors::backgrounds::PRIMARY;
    assert_eq!(primary.name(), "colors-backgrounds-primary");
    // A token without a path is at the top, named after its name.
    let ink: TokenKey<Hsla> = keys::INK;
    assert_eq!(ink.name(), "ink");
    let forest: TokenKey<Hsla> = keys::INK_BRAND_FOREST;
    assert_eq!(forest.name(), "ink.brand.forest");
    let card: TokenKey<f32> = keys::radius::CARD;
    assert_eq!(card.name(), "radius-card");
    // Segments become Rust names: `type-scale` a module `type_scale`,
    // `type` a raw one, a digit first takes `_`.
    let gutter: TokenKey<f32> = keys::type_scale::GUTTER;
    assert_eq!(gutter.name(), "spacing-gutter");
    let strong: TokenKey<f32> = keys::r#type::STRONG;
    assert_eq!(strong.name(), "font-weight-strong");
    let body: TokenKey<Arc<str>> = keys::r#type::body::FAMILY;
    assert_eq!(body.name(), "font-body");
    let raised: TokenKey<Arc<[BoxShadow]>> = keys::effects::_2xl::RAISED;
    assert_eq!(raised.name(), "shadow-raised");
}

#[test]
fn keys_read_the_tokens_the_document_declares() {
    let tokens = tokens_of("ds/native", include_str!("theme_keys/theme.json"));
    assert_eq!(
        tokens.key(keys::colors::backgrounds::PRIMARY),
        Some(rgb(0xffffff).into())
    );
    assert_eq!(tokens.key(keys::radius::CARD), Some(12.0));
    assert_eq!(tokens.key(keys::r#type::STRONG), Some(650.0));
    assert_eq!(
        tokens.key(keys::r#type::body::FAMILY).as_deref(),
        Some("Inter")
    );
    assert!(tokens.key(keys::effects::_2xl::RAISED).is_some());
}

#[test]
fn each_modifier_is_an_enum_of_its_contexts() {
    use keys::modifiers::{Brand, HighContrast};
    assert_eq!(Brand::NAME, "brand");
    assert_eq!(Brand::ALL, [Brand::Ocean, Brand::Forest]);
    assert_eq!(Brand::DEFAULT, Brand::Ocean);
    assert_eq!(Brand::Forest.name(), "forest");
    assert_eq!(HighContrast::NAME, "high-contrast");
    assert_eq!(HighContrast::DEFAULT.name(), "off");
    assert_eq!(HighContrast::ALL, [HighContrast::Off, HighContrast::On]);
}

#[test]
fn the_documents_default_contexts_are_read_and_another_is_picked() {
    use keys::modifiers::Brand;
    let tokens = tokens_of("ds/native", include_str!("theme_keys/theme.json"));
    assert_eq!(tokens.key(keys::INK), Some(rgb(0x101010).into()));
    assert_eq!(tokens.modifier(Brand::NAME), Some(Brand::DEFAULT.name()));
    let (forest, _) = tokens.scoped(Some(&Brand::Forest.into()), None);
    assert_eq!(forest.key(keys::INK), Some(rgb(0x16a34a).into()));
}

/// A modifier's enum is what a `ThemeScope` picks: `Brand::Forest.into()`,
/// and `.and(…)` for another modifier.
#[test]
fn a_modifiers_enum_picks_a_scopes_context() {
    use keys::modifiers::{Brand, HighContrast};
    use nsh_core::element::ScopeModifiers;
    let picks: ScopeModifiers = Brand::Forest.into();
    assert_eq!(picks, ScopeModifiers::new([("brand", "forest")]));
    let both = picks.and(HighContrast::On).and(Brand::Ocean);
    assert_eq!(
        both,
        ScopeModifiers::new([("high-contrast", "on"), ("brand", "ocean")])
    );
    let tokens = tokens_of("ds/native", include_str!("theme_keys/theme.json"));
    let (forest, said) = tokens.scoped(Some(&ScopeModifiers::from(Brand::Forest)), None);
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(forest.key(keys::INK), Some(rgb(0x16a34a).into()));
}

mod kinds {
    nsh_core::element::theme_keys!("tests/macros/theme_keys/kinds.json");
}

/// A string, a boolean, a duration, an easing and a text style each get a
/// key of their own type, which reads the document's value.
#[test]
fn the_newer_kinds_are_typed() {
    use nsh_core::element::{CubicBezier, Typography};
    let name: TokenKey<String> = kinds::brand::NAME;
    let beta: TokenKey<bool> = kinds::features::BETA;
    let fast: TokenKey<std::time::Duration> = kinds::motion::FAST;
    let snappy: TokenKey<CubicBezier> = kinds::motion::SNAPPY;
    let title: TokenKey<Typography> = kinds::r#type::TITLE;
    let tokens = tokens_of("ds/kinds", include_str!("theme_keys/kinds.json"));
    assert_eq!(tokens.key(name).as_deref(), Some("Acme"));
    assert_eq!(tokens.key(beta), Some(true));
    assert_eq!(
        tokens.key(fast),
        Some(std::time::Duration::from_millis(120))
    );
    assert_eq!(tokens.key(snappy), Some(CubicBezier([0.2, 0.0, 0.0, 1.0])));
    assert_eq!(
        tokens.key(title),
        Some(Typography {
            size: Points(24.0),
            line_height: Some(nsh_core::LineHeight::Multiple(1.25)),
            ..Typography::default()
        })
    );
}
