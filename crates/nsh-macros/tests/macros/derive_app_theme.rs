//! `#[derive(AppTheme)]`: the proposal's design system as fields. Names in
//! kebab case with their namespaces, nested and flattened types, skipped
//! fields, roles and docs; a round trip gives the theme back and a patch
//! writes only the fields it sets. A nested type's namespaced tokens keep
//! their namespace in front (`radius-button-card`), so they register and
//! their classes read them.

use nsh_core::{
    element::{
        AdaptiveColor, AdaptiveShadow, AppTheme, Registry, Role, SharedString, TokenKind, Tokens,
    },
    geometry::{Points, px},
};

#[derive(Debug, Clone, PartialEq, AppTheme)]
pub(crate) struct Surface {
    /// The app's background.
    #[token(color, role = "background")]
    pub(crate) canvas: AdaptiveColor,
    /// Cards.
    #[token(color, role = "card")]
    pub(crate) raised: AdaptiveColor,
    /// Popovers and menus.
    #[token(color, role = "popover")]
    pub(crate) overlay: AdaptiveColor,
}

impl Default for Surface {
    fn default() -> Self {
        Surface {
            canvas: AdaptiveColor::rgb(0xffffff, 0x101113),
            raised: AdaptiveColor::rgb(0xf7f7f8, 0x18191c),
            overlay: AdaptiveColor::rgb(0xffffff, 0x202124),
        }
    }
}

#[derive(Debug, Clone, PartialEq, AppTheme)]
pub(crate) struct Odds {
    #[token(color)]
    pub(crate) odds_up: AdaptiveColor,
}

impl Default for Odds {
    fn default() -> Self {
        Odds {
            odds_up: AdaptiveColor::rgb(0x16a34a, 0x4ade80),
        }
    }
}

#[derive(Debug, Clone, PartialEq, AppTheme)]
pub(crate) struct DsTheme {
    #[token(nested)]
    pub(crate) surface: Surface,
    #[token(flatten)]
    pub(crate) odds: Odds,
    /// The main action.
    #[token(color, role = "primary")]
    pub(crate) action: AdaptiveColor,
    #[token(radius)]
    pub(crate) control: Points,
    #[token(radius)]
    pub(crate) card: f32,
    #[token(spacing)]
    pub(crate) gutter: Points,
    #[token(weight)]
    pub(crate) strong: f32,
    #[token(shadow)]
    pub(crate) raised: AdaptiveShadow,
    #[token(font, role = "font-sans")]
    pub(crate) body: SharedString,
    #[token(color, name = "brand-ink")]
    pub(crate) ink: AdaptiveColor,
    #[token(skip)]
    pub(crate) cached: u32,
}

impl Default for DsTheme {
    fn default() -> Self {
        DsTheme {
            surface: Surface::default(),
            odds: Odds::default(),
            action: AdaptiveColor::rgb(0x2563eb, 0x60a5fa),
            control: px(8.0),
            card: 14.0,
            gutter: px(16.0),
            strong: 650.0,
            raised: AdaptiveShadow::from([nsh_core::BoxShadow {
                offset_y: px(1.0),
                blur: px(2.0),
                ..Default::default()
            }]),
            body: "Inter".into(),
            ink: AdaptiveColor::rgb(0x111111, 0xeeeeee),
            cached: 7,
        }
    }
}

#[test]
fn names_are_kebab_case_with_their_namespaces() {
    let names: Vec<&str> = DsTheme::token_specs().iter().map(|s| &*s.name).collect();
    assert_eq!(
        names,
        [
            "surface-canvas",
            "surface-raised",
            "surface-overlay",
            "odds-up",
            "action",
            "radius-control",
            "radius-card",
            "spacing-gutter",
            "font-weight-strong",
            "shadow-raised",
            "font-body",
            "brand-ink",
        ]
    );
    let spec = |name: &str| {
        DsTheme::token_specs()
            .iter()
            .find(|s| s.name == name)
            .unwrap()
            .clone()
    };
    assert_eq!(spec("surface-overlay").role, Some(Role::Popover));
    assert_eq!(spec("surface-overlay").doc, "Popovers and menus.");
    assert_eq!(spec("action").role, Some(Role::Primary));
    assert_eq!(spec("font-body").role, Some(Role::FontSans));
    assert_eq!(spec("radius-card").kind, TokenKind::Radius);
    assert_eq!(spec("font-weight-strong").kind, TokenKind::Weight);
    assert_eq!(spec("shadow-raised").kind, TokenKind::Shadow);
    // The specs are made once.
    assert!(std::ptr::eq(DsTheme::token_specs(), DsTheme::token_specs()));
}

#[test]
fn a_round_trip_gives_the_theme_back() {
    let theme = DsTheme {
        card: 20.0,
        surface: Surface {
            overlay: AdaptiveColor::rgb(0x333333, 0x444444),
            ..Surface::default()
        },
        cached: 0,
        ..DsTheme::default()
    };
    let tokens = theme.to_tokens();
    assert_eq!(tokens.role_name(Role::Popover), "surface-overlay");
    assert_eq!(tokens.number("radius-card"), Some(20.0));
    assert_eq!(tokens.font("font-body").map(|f| &**f), Some("Inter"));
    let read = DsTheme::read_tokens(&tokens, "");
    // A skipped field keeps `Default`'s value.
    assert_eq!(read, DsTheme { cached: 7, ..theme });
    // Missing names keep the defaults.
    assert_eq!(
        DsTheme::read_tokens(&Tokens::default(), ""),
        DsTheme::default()
    );
}

#[test]
fn a_patch_writes_only_the_fields_it_sets() {
    let patch = DsThemePatch {
        action: Some(AdaptiveColor::rgb(0xff0000, 0xff0000)),
        surface: Some(SurfacePatch {
            raised: Some(AdaptiveColor::rgb(0x00ff00, 0x00ff00)),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut out = Tokens::default();
    DsTheme::write_patch(&patch, "", &mut out);
    let names: Vec<&str> = {
        let mut n: Vec<&str> = out.colors().map(|(n, _)| n).collect();
        n.sort();
        n
    };
    assert_eq!(names, ["action", "surface-raised"]);
    assert_eq!(out.numbers().count(), 0);
}

#[test]
fn a_registered_derived_type_is_the_apps_theme() {
    let mut registry = Registry::new();
    registry.theme::<DsTheme>();
    let (tokens, _) = registry.tokens(None).unwrap().unwrap();
    assert_eq!(
        tokens.color("surface-raised"),
        Some(Surface::default().raised)
    );
    assert_eq!(tokens.role_name(Role::Primary), "action");
    assert_eq!(
        tokens.role_font(Role::FontSans).map(|f| &**f),
        Some("Inter")
    );
}

/// Namespaced kinds, to nest.
#[derive(Debug, Clone, PartialEq, AppTheme)]
pub(crate) struct Control {
    #[token(font)]
    pub(crate) body: SharedString,
    #[token(radius)]
    pub(crate) card: Points,
    #[token(shadow)]
    pub(crate) raised: AdaptiveShadow,
    #[token(color)]
    pub(crate) ink: AdaptiveColor,
}

impl Default for Control {
    fn default() -> Self {
        Control {
            body: "Inter".into(),
            card: px(4.0),
            raised: AdaptiveShadow::from([nsh_core::BoxShadow {
                blur: px(3.0),
                ..Default::default()
            }]),
            ink: AdaptiveColor::rgb(0x000000, 0xffffff),
        }
    }
}

#[derive(Debug, Clone, PartialEq, AppTheme)]
pub(crate) struct Kit {
    #[token(nested)]
    pub(crate) button: Control,
}

/// Its own default for the nested field, which names the tokens lack do not
/// take.
impl Default for Kit {
    fn default() -> Self {
        Kit {
            button: Control {
                card: px(99.0),
                ..Control::default()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
pub(crate) struct App {
    #[token(nested)]
    pub(crate) kit: Kit,
}

#[test]
fn nested_names_go_after_their_namespace() {
    let names: Vec<&str> = Kit::token_specs().iter().map(|s| &*s.name).collect();
    assert_eq!(
        names,
        [
            "font-button-body",
            "radius-button-card",
            "shadow-button-raised",
            "button-ink",
        ]
    );
    let names: Vec<&str> = App::token_specs().iter().map(|s| &*s.name).collect();
    assert_eq!(
        names,
        [
            "font-kit-button-body",
            "radius-kit-button-card",
            "shadow-kit-button-raised",
            "kit-button-ink",
        ]
    );
    // The specs, the writes, the reads and the patch agree on the names.
    let app = App::default();
    let tokens = app.to_tokens();
    assert_eq!(tokens.number("radius-kit-button-card"), Some(99.0));
    assert_eq!(
        tokens.font("font-kit-button-body").map(|f| &**f),
        Some("Inter")
    );
    assert!(tokens.shadow("shadow-kit-button-raised").is_some());
    assert_eq!(tokens.color("kit-button-ink"), Some(Control::default().ink));
    assert_eq!(App::read_tokens(&tokens, ""), app);
    let mut out = Tokens::default();
    App::write_patch(
        &AppPatch {
            kit: Some(KitPatch {
                button: Some(ControlPatch {
                    card: Some(px(2.0)),
                    ..Default::default()
                }),
            }),
        },
        "",
        &mut out,
    );
    assert_eq!(
        out.numbers().map(|(n, _)| n).collect::<Vec<_>>(),
        ["radius-kit-button-card"]
    );
}

#[test]
fn a_nested_font_shadow_and_radius_register() {
    let mut registry = Registry::new();
    registry.theme::<App>();
    let (tokens, _) = registry.tokens(None).unwrap().unwrap();
    assert_eq!(tokens.number("radius-kit-button-card"), Some(99.0));
    assert!(tokens.shadow("shadow-kit-button-raised").is_some());
    assert!(tokens.font("font-kit-button-body").is_some());
}

#[test]
fn a_nested_fields_missing_names_take_the_inner_default() {
    assert_eq!(Kit::default().button.card, px(99.0));
    // Read from no tokens, the nested field is `Control::default()`, not
    // the `button` that `Kit::default()` gives.
    assert_eq!(
        Kit::read_tokens(&Tokens::default(), ""),
        Kit {
            button: Control::default()
        }
    );
}
