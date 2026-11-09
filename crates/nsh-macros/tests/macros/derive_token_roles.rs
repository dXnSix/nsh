//! `#[derive(AppTheme)]` with a token playing several roles: `role` once
//! for each. One surface for cards and popovers, one ink on both; a nested
//! type's token keeps its roles under the prefix; registered, each role is
//! mapped onto the one token.

use nsh_core::element::{AdaptiveColor, AppTheme, Registry, Role};

/// One surface for cards and popovers, and one ink on both: each token
/// plays two roles.
#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
struct Flat {
    #[token(color, role = "card", role = "popover")]
    surface: AdaptiveColor,
    #[token(color, role = "card-foreground", role = "popover-foreground")]
    ink: AdaptiveColor,
    #[token(nested)]
    side: Side,
}

/// A nested type's token playing two roles too.
#[derive(Debug, Clone, PartialEq, Default, AppTheme)]
struct Side {
    #[token(color, role = "sidebar", role = "sidebar-accent")]
    bar: AdaptiveColor,
}

#[test]
fn a_token_plays_each_role_it_names() {
    let specs = Flat::token_specs();
    let roles = |name: &str| {
        specs
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.roles.clone())
            .unwrap_or_default()
    };
    assert_eq!(roles("surface"), [Role::Card, Role::Popover]);
    assert_eq!(
        roles("ink"),
        [Role::CardForeground, Role::PopoverForeground]
    );
    // A nested type's token keeps its roles under the prefix.
    assert_eq!(roles("side-bar"), [Role::Sidebar, Role::SidebarAccent]);
    let theme = Flat {
        surface: AdaptiveColor::rgb(0x123456, 0x654321),
        ..Flat::default()
    };
    let tokens = theme.to_tokens();
    for role in [Role::Card, Role::Popover] {
        assert_eq!(tokens.role_name(role), "surface");
        assert_eq!(tokens.role_color(role), Some(theme.surface));
    }
    assert_eq!(tokens.role_name(Role::SidebarAccent), "side-bar");
    // Registered, it is the app's theme, each role mapped.
    let mut registry = Registry::new();
    registry.theme::<Flat>();
    let (defaults, _) = registry.tokens(None).unwrap().unwrap();
    assert_eq!(defaults.role_name(Role::Popover), "surface");
    assert_eq!(defaults.role_name(Role::PopoverForeground), "ink");
}
