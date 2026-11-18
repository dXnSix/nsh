//! `theme_keys!` on the theme format (`docs/dev/theme-format.md`): keys by
//! path and by name, each of its value type's Rust type, modifier enums in
//! the document's order, a document under `OUT_DIR`, and a document whose
//! warnings don't stop it. Its refusals are compile errors, tested as
//! `compile_fail` doctests in nsh-core (`element/token_keys.rs`,
//! `theme_keys_refusals`).

use std::{sync::Arc, time::Duration};

use nsh_core::{
    BoxShadow, Gradient, Hsla,
    color::rgb,
    element::{
        Blur, BlurKind, Border, CubicBezier, Registry, TokenKey, TokenValue, Tokens,
        TransitionToken, Typography,
    },
    geometry::Points,
};

/// The tokens of the document `json` that the set `set` registers, its
/// default contexts picked.
fn tokens_of(set: &'static str, json: &'static str) -> Arc<Tokens> {
    let mut registry = Registry::ui_set(set);
    registry.theme_document(json);
    let document = registry
        .theme_document_of_sets()
        .0
        .expect("the set's document is active");
    Arc::new(Tokens::resolve(document, Default::default(), Vec::new()).0)
}

/// The value type `key`'s Rust type is, by name.
fn value_type<V: TokenValue>(_: TokenKey<V>) -> &'static str {
    V::TYPE.name()
}

mod values {
    nsh_core::element::theme_keys!("tests/macros/theme_format_keys/values.json");
}

mod modes {
    nsh_core::element::theme_keys!("tests/macros/theme_format_keys/modifiers.json");
}

mod generated {
    nsh_core::element::theme_keys!("$OUT_DIR/theme_format_keys.json");
}

mod unknown_member {
    nsh_core::element::theme_keys!("tests/macros/theme_format_keys/unknown_member.json");
}

const VALUES: &str = include_str!("theme_format_keys/values.json");

/// A token with a path is a key in modules by it; one without is at the
/// top, named after its name, whatever separators the name has.
#[test]
fn keys_are_placed_by_path_or_at_the_top_by_name() {
    assert_eq!(values::surface::APP.name(), "surface-app");
    assert_eq!(values::motion::duration::FAST.name(), "duration-fast");
    assert_eq!(values::motion::REDUCED.name(), "motion-reduced");
    assert_eq!(values::SPACE_3.name(), "space/3");
    assert_eq!(values::SURFACE_RAISED.name(), "Surface Raised");
    // A segment that starts with a digit takes `_`.
    assert_eq!(modes::space::_3.name(), "space-3");
}

/// Each key is of its value type's Rust type (`element/theme_values.rs`'
/// table), the type its token's kind is.
#[test]
fn each_value_type_has_its_rust_type() {
    let color: TokenKey<Hsla> = values::surface::APP;
    let number: TokenKey<f32> = values::SPACE_3;
    let font: TokenKey<Arc<str>> = values::font::INTER;
    let string: TokenKey<String> = values::label::CONFIRM;
    let boolean: TokenKey<bool> = values::motion::REDUCED;
    let duration: TokenKey<Duration> = values::motion::duration::FAST;
    let easing: TokenKey<CubicBezier> = values::motion::ease::STANDARD;
    let typography: TokenKey<Typography> = values::text::TITLE;
    let shadow: TokenKey<Arc<[BoxShadow]>> = values::shadow::CARD;
    let gradient: TokenKey<Gradient> = values::gradient::HERO;
    let blur: TokenKey<Blur> = values::blur::GLASS;
    let border: TokenKey<Border> = values::border::CARD;
    let transition: TokenKey<TransitionToken> = values::motion::PRESS;
    let tokens = tokens_of("ds/values", VALUES);
    let kinds = [
        (color.name(), value_type(color)),
        (number.name(), value_type(number)),
        (font.name(), value_type(font)),
        (string.name(), value_type(string)),
        (boolean.name(), value_type(boolean)),
        (duration.name(), value_type(duration)),
        (easing.name(), value_type(easing)),
        (typography.name(), value_type(typography)),
        (shadow.name(), value_type(shadow)),
        (gradient.name(), value_type(gradient)),
        (blur.name(), value_type(blur)),
        (border.name(), value_type(border)),
        (transition.name(), value_type(transition)),
    ];
    for (name, value_type) in kinds {
        assert_eq!(tokens.kind_name(name), Some(value_type), "{name}");
    }
    let mut every: Vec<&str> = kinds.iter().map(|(_, t)| *t).collect();
    every.sort_unstable();
    every.dedup();
    assert_eq!(every.len(), nsh_core::element::ValueType::ALL.len());
}

/// The keys read the values the document gives, references resolved.
#[test]
fn keys_read_the_documents_values() {
    let tokens = tokens_of("ds/values", VALUES);
    assert_eq!(tokens.key(values::surface::APP), Some(rgb(0xfcfcfc).into()));
    assert_eq!(
        tokens.key(values::SURFACE_RAISED),
        tokens.key(values::surface::APP)
    );
    assert_eq!(tokens.key(values::SPACE_3), Some(12.0));
    assert_eq!(
        tokens.key(values::font::INTER).as_deref(),
        Some("Inter, system-ui, sans-serif")
    );
    assert_eq!(tokens.key(values::label::CONFIRM).as_deref(), Some("OK"));
    assert_eq!(tokens.key(values::motion::REDUCED), Some(false));
    assert_eq!(
        tokens.key(values::motion::duration::FAST),
        Some(Duration::from_millis(150))
    );
    assert_eq!(
        tokens.key(values::motion::ease::STANDARD),
        Some(CubicBezier([0.2, 0.0, 0.0, 1.0]))
    );
    assert_eq!(
        tokens.key(values::motion::ease::EXIT),
        Some(CubicBezier::EASE_IN)
    );
    let title = tokens.key(values::text::TITLE).expect("a text style");
    assert_eq!(title.size, Points(20.0));
    assert_eq!(title.font_weight, Some(600.0));
    assert_eq!(tokens.key(values::shadow::CARD).map(|s| s.len()), Some(1));
    assert_eq!(
        tokens.key(values::gradient::HERO).map(|g| g.stops.len()),
        Some(2)
    );
    assert_eq!(
        tokens.key(values::blur::GLASS),
        Some(Blur::new(BlurKind::Background, 16.0))
    );
    let border = tokens.key(values::border::CARD).expect("a border");
    assert_eq!(border.width, Points(1.0));
    assert_eq!(border.color, rgb(0xe8e8e8).into());
    assert_eq!(
        tokens.key(values::motion::PRESS),
        Some(
            TransitionToken::new(Duration::from_millis(100))
                .with_easing(CubicBezier([0.2, 0.0, 0.0, 1.0]))
        )
    );
}

/// Each modifier is an enum of its contexts in the document's order, the
/// light/dark modifier as any other.
#[test]
fn each_modifier_is_an_enum_of_its_contexts_in_the_documents_order() {
    use modes::modifiers::{ColorMode, Density, HighContrast};
    assert_eq!(ColorMode::NAME, "colorMode");
    assert_eq!(ColorMode::ALL, [ColorMode::Light, ColorMode::Dark]);
    assert_eq!(ColorMode::DEFAULT, ColorMode::Light);
    assert_eq!(Density::NAME, "density");
    assert_eq!(
        Density::ALL,
        [Density::Compact, Density::Regular, Density::_2x]
    );
    assert_eq!(Density::DEFAULT, Density::Regular);
    assert_eq!(Density::_2x.name(), "2x");
    assert_eq!(HighContrast::NAME, "high-contrast");
    assert_eq!(HighContrast::DEFAULT.name(), "off");
    assert_eq!(HighContrast::ALL, [HighContrast::Off, HighContrast::On]);
}

/// A modifier's context, picked by its enum, gives the values the context
/// sets; the defaults are read without a pick.
#[test]
fn a_modifiers_enum_picks_its_contexts_values() {
    use modes::modifiers::{ColorMode, Density};
    use nsh_core::element::ScopeModifiers;
    let tokens = tokens_of(
        "ds/modifiers",
        include_str!("theme_format_keys/modifiers.json"),
    );
    assert_eq!(tokens.key(modes::space::_3), Some(12.0));
    assert_eq!(
        tokens.modifier(Density::NAME),
        Some(Density::DEFAULT.name())
    );
    let picks = ScopeModifiers::from(ColorMode::Dark).and(Density::Compact);
    let (picked, said) = tokens.scoped(Some(&picks), None);
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(picked.key(modes::space::_3), Some(8.0));
    assert_eq!(picked.key(modes::surface::APP), Some(rgb(0x111111).into()));
}

/// A path that starts with `$OUT_DIR/` is read under the crate's `OUT_DIR`:
/// here the document this crate's build script writes.
#[test]
fn a_document_under_out_dir_is_read() {
    use generated::modifiers::Density;
    let accent: TokenKey<Hsla> = generated::generated::ACCENT;
    let gap: TokenKey<f32> = generated::GENERATED_GAP;
    assert_eq!(accent.name(), "generated-accent");
    assert_eq!(gap.name(), "generated-gap");
    assert_eq!(Density::ALL, [Density::Regular, Density::Compact]);
}

/// Warnings don't stop generation: a document with unknown members (at the
/// top and in entries) compiles, and its keys read its tokens.
#[test]
fn a_document_with_unknown_members_compiles() {
    let surface: TokenKey<Hsla> = unknown_member::surface::APP;
    let space: TokenKey<f32> = unknown_member::SPACE_3;
    let tokens = tokens_of(
        "ds/unknown",
        include_str!("theme_format_keys/unknown_member.json"),
    );
    assert_eq!(tokens.key(surface), Some(rgb(0xfcfcfc).into()));
    assert_eq!(tokens.key(space), Some(12.0));
}
