//! `theme_keys!`: a typed key for each token a theme document declares, in
//! modules by its path, typed by its kind, reading the tokens the document
//! makes.

use nsh_core::{
    element::{AdaptiveColor, AdaptiveShadow, Registry, SharedString, TokenKey},
    geometry::Points,
};

mod keys {
    nsh_core::element::theme_keys!("tests/macros/theme_keys/theme.json");
}

#[test]
fn keys_follow_the_paths_and_kinds() {
    let primary: TokenKey<AdaptiveColor> = keys::colors::backgrounds::PRIMARY;
    assert_eq!(primary.name(), "colors-backgrounds-primary");
    // A token without a path is named after its name.
    let ink: TokenKey<AdaptiveColor> = keys::INK;
    assert_eq!(ink.name(), "ink");
    let card: TokenKey<Points> = keys::radius::CARD;
    assert_eq!(card.name(), "radius-card");
    // Segments become Rust names: `type-scale` a module `type_scale`,
    // `type` a raw one, a digit first takes `_`.
    let gutter: TokenKey<Points> = keys::type_scale::GUTTER;
    assert_eq!(gutter.name(), "spacing-gutter");
    let strong: TokenKey<f32> = keys::r#type::STRONG;
    assert_eq!(strong.name(), "font-weight-strong");
    let body: TokenKey<SharedString> = keys::r#type::body::FAMILY;
    assert_eq!(body.name(), "font-body");
    let raised: TokenKey<AdaptiveShadow> = keys::effects::_2xl::RAISED;
    assert_eq!(raised.name(), "shadow-raised");
}

#[test]
fn keys_read_the_tokens_the_document_declares() {
    let mut registry = Registry::ui_set("ds/native");
    registry.theme_document(include_str!("theme_keys/theme.json"));
    let tokens = registry.tokens(None).unwrap().unwrap().0;
    assert_eq!(
        tokens.key(keys::colors::backgrounds::PRIMARY),
        Some(AdaptiveColor::rgb(0xffffff, 0x0b0b0f))
    );
    assert_eq!(tokens.key(keys::radius::CARD), Some(Points(12.0)));
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
    let mut registry = Registry::ui_set("ds/native");
    registry.theme_document(include_str!("theme_keys/theme.json"));
    let tokens = registry.tokens(None).unwrap().unwrap().0;
    assert_eq!(
        tokens.key(keys::INK),
        Some(AdaptiveColor::rgb(0x101010, 0xf0f0f0))
    );
    let modifiers = tokens.modifiers().unwrap();
    assert_eq!(modifiers.selected(Brand::NAME), Some(Brand::DEFAULT.name()));
    let forest = modifiers.select(Brand::NAME, Brand::Forest.name()).unwrap();
    assert_eq!(
        forest.tokens().key(keys::INK),
        Some(AdaptiveColor::rgb(0x16a34a, 0x4ade80))
    );
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
    let mut registry = Registry::ui_set("ds/native");
    registry.theme_document(include_str!("theme_keys/theme.json"));
    let tokens = registry.tokens(None).unwrap().unwrap().0;
    let (forest, said) = tokens.picked(&ScopeModifiers::from(Brand::Forest).0);
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(
        forest.key(keys::INK),
        Some(AdaptiveColor::rgb(0x16a34a, 0x4ade80))
    );
}
