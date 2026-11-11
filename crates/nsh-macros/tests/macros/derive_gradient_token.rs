//! `#[derive(AppTheme)]` with a gradient token: `#[token(gradient)]` on an
//! `AdaptiveGradient` field is a token in Tailwind's `background-image-*`
//! namespace, the class `bg-*`. It registers, reads back from a theme file
//! and keeps its namespace in front when nested.

use nsh_core::element::{AdaptiveGradient, AppTheme, Registry, TokenKind, Tokens, theme_file};

fn gradient(css: &str) -> AdaptiveGradient {
    AdaptiveGradient::parse(css).unwrap()
}

#[derive(Debug, Clone, PartialEq, AppTheme)]
struct Banner {
    /// The landing page's background.
    #[token(gradient)]
    hero: AdaptiveGradient,
    #[token(gradient, name = "background-image-fade")]
    scrim: AdaptiveGradient,
}

impl Default for Banner {
    fn default() -> Self {
        Banner {
            hero: gradient("linear-gradient(135deg, #7c3aed, #db2777)"),
            scrim: gradient("linear-gradient(to top, #000000cc, #00000000)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
struct Kit {
    #[token(nested)]
    banner: Banner,
}

#[test]
fn a_gradient_field_is_a_token_of_the_background_image_namespace() {
    let specs = Banner::token_specs();
    let names: Vec<&str> = specs.iter().map(|s| &*s.name).collect();
    assert_eq!(names, ["background-image-hero", "background-image-fade"]);
    assert!(specs.iter().all(|s| s.kind == TokenKind::Gradient));
    assert_eq!(specs[0].doc, "The landing page's background.");
    assert!(specs.iter().all(|s| s.roles.is_empty()));
    // Nested, the namespace stays in front, so the class still finds it.
    let nested: Vec<&str> = Kit::token_specs().iter().map(|s| &*s.name).collect();
    assert_eq!(
        nested,
        [
            "background-image-banner-hero",
            "background-image-banner-fade"
        ]
    );
}

#[test]
fn a_gradient_token_round_trips_and_registers() {
    let theme = Banner {
        hero: gradient("radial-gradient(#ffffff, #000000)"),
        ..Banner::default()
    };
    let tokens = theme.to_tokens();
    assert_eq!(
        tokens.gradient("background-image-hero"),
        Some(&gradient("radial-gradient(#ffffff, #000000)"))
    );
    assert_eq!(Banner::read_tokens(&tokens, ""), theme);
    // A name the tokens lack keeps the default's.
    assert_eq!(
        Banner::read_tokens(&Tokens::default(), ""),
        Banner::default()
    );
    // Registered, a theme file sets it per appearance and the rest stay
    // the type's defaults.
    let mut registry = Registry::ui_set("test/gradients");
    registry.theme::<Kit>();
    let file = r##"{ "cssVars": {
        "light": { "background-image-banner-hero": "linear-gradient(90deg, #111111, #222222)" },
        "dark": { "background-image-banner-hero": "linear-gradient(90deg, #eeeeee, #dddddd)" } } }"##;
    let (tokens, left_out) = registry.tokens(Some(file)).unwrap().unwrap();
    assert_eq!(left_out, Vec::<String>::new());
    let hero = tokens.gradient("background-image-banner-hero").unwrap();
    assert_eq!(
        hero.css(false),
        "linear-gradient(90deg, #111111ff, #222222ff)"
    );
    assert_eq!(
        hero.css(true),
        "linear-gradient(90deg, #eeeeeeff, #ddddddff)"
    );
    assert_eq!(
        tokens.gradient("background-image-banner-fade"),
        Some(&Banner::default().scrim)
    );
    // A value that is no gradient is said, as any value of the wrong kind.
    let wrong = r##"{ "cssVars": { "theme": { "background-image-banner-hero": "#ff0000" } } }"##;
    let (_, left_out) = registry.tokens(Some(wrong)).unwrap().unwrap();
    assert_eq!(left_out.len(), 1, "{left_out:?}");
    assert!(left_out[0].contains("no gradient value"), "{left_out:?}");
    // The defaults written as a theme file read back.
    let written = theme_file::write(&Kit::default().to_tokens());
    let (read, _) = theme_file::parse(&written, &Tokens::default()).unwrap();
    assert_eq!(
        read.gradient("background-image-banner-hero"),
        Some(&Banner::default().hero)
    );
}
