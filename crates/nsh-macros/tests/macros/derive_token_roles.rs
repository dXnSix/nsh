//! `#[derive(AppTheme)]` with a token playing several roles: `role` once
//! for each, a constant's path. One surface for cards and popovers, one ink on both; a nested
//! type's token keeps its roles under the prefix; registered, each role is
//! mapped onto the one token.

use nsh_core::element::{AdaptiveColor, AppTheme, Registry};

/// The roles these themes map: the core's, and a UI set's own as
/// `nsh-base` declares its.
mod role {
    use nsh_core::element::Role;
    pub(crate) use nsh_core::element::role::*;

    pub(crate) const CARD: Role = Role::new("card");

    pub(crate) const CARD_FOREGROUND: Role = Role::new("card-foreground");

    pub(crate) const SIDEBAR: Role = Role::new("sidebar");

    pub(crate) const SIDEBAR_ACCENT: Role = Role::new("sidebar-accent");
}

/// One surface for cards and popovers, and one ink on both: each token
/// plays two roles.
#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
struct Flat {
    #[token(color, role = role::CARD, role = role::POPOVER)]
    surface: AdaptiveColor,
    #[token(color, role = role::CARD_FOREGROUND, role = role::POPOVER_FOREGROUND)]
    ink: AdaptiveColor,
    #[token(nested)]
    side: Side,
}

/// A nested type's token playing two roles too.
#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
struct Side {
    #[token(color, role = role::SIDEBAR, role = role::SIDEBAR_ACCENT)]
    bar: AdaptiveColor,
}

#[test]
fn a_token_plays_each_role_it_names() {
    let specs = Flat::token_specs();
    let roles = |name: &str| {
        specs
            .iter()
            .find(|s| s.name == name)
            .map(|s| {
                s.roles
                    .iter()
                    .map(|r| r.name().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    assert_eq!(roles("surface"), ["card", "popover"]);
    assert_eq!(roles("ink"), ["card-foreground", "popover-foreground"]);
    // A nested type's token keeps its roles under the prefix.
    assert_eq!(roles("side-bar"), ["sidebar", "sidebar-accent"]);
    let theme = Flat {
        surface: AdaptiveColor::rgb(0x123456, 0x654321),
        ..Flat::default()
    };
    let tokens = theme.to_tokens();
    for role in [role::CARD, role::POPOVER] {
        assert_eq!(tokens.role_name(role), "surface");
        assert_eq!(tokens.role(role), Some(theme.surface));
    }
    assert_eq!(tokens.role_name(role::SIDEBAR_ACCENT), "side-bar");
    // Registered, it is the app's theme, each role mapped.
    let mut registry = Registry::new();
    registry.theme::<Flat>();
    let (defaults, _) = registry.tokens(None).unwrap().unwrap();
    assert_eq!(defaults.role_name(role::POPOVER), "surface");
    assert_eq!(defaults.role_name(role::POPOVER_FOREGROUND), "ink");
}
