//! `nsh_base::Theme` as a theme type: shadcn's tokens, each playing the
//! role of its name, unregistered so it stays the default. `theme()` is
//! `theme_of::<Theme>()`; a shadcn app changes it with
//! `update_theme::<Theme>`; under an app's registered type its fields are
//! the tokens that type's roles are mapped onto.

use std::time::Duration;

use nsh_base::{Theme, role, theme};
use nsh_core::{
    Color, Size,
    element::{AppTheme, Registry, Root, TokenKind, TokenSpec, Tokens},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

fn hex(c: AdaptiveColor) -> String {
    format!("#{:08x}", Color::from(c.light).0)
}

fn start<C: Component<Props = ()>>(ui_sets: Vec<Registry>) -> Headless {
    let mut config = EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ..EngineConfig::new(Root::of::<C>())
    };
    if !ui_sets.is_empty() {
        config.ui_sets = ui_sets;
    }
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

#[test]
fn theme_is_shadcns_tokens_each_playing_its_role() {
    let specs = Theme::token_specs();
    assert_eq!(specs.len(), 34);
    for spec in specs {
        assert_eq!(
            spec.roles.iter().map(|r| r.name()).collect::<Vec<_>>(),
            [&*spec.name],
            "{}",
            spec.name
        );
    }
    assert_eq!(
        specs.iter().find(|s| s.name == "radius").map(|s| s.kind),
        Some(TokenKind::Radius)
    );
    // Written and read back, a theme's fields are its own.
    let theme = Theme::default();
    let read = Theme::read_tokens(&theme.to_tokens(), "");
    assert_eq!(
        (read.primary, read.popover, read.radius),
        (theme.primary, theme.popover, theme.radius)
    );
    for spec in specs.iter().filter(|s| s.kind == TokenKind::Color) {
        assert_eq!(
            read.color(&spec.name),
            theme.color(&spec.name),
            "{}",
            spec.name
        );
    }
}

/// Shows `theme()`'s primary, and whether two reads are one theme; then
/// changes the primary with `update_theme::<Theme>`.
#[component]
fn Updates() -> impl IntoElement {
    set_timeout(Duration::from_millis(50), || {
        let mut patch = Tokens::default();
        patch.set_color("primary", AdaptiveColor::rgb(0x123456, 0x123456));
        update_theme::<Theme>(patch);
    });
    text(move || {
        let (a, b) = (theme(), theme());
        format!(
            "primary {} same {}",
            hex(a.primary),
            std::sync::Arc::ptr_eq(&a, &b)
        )
    })
}

#[test]
fn theme_is_theme_of_and_update_theme_changes_it() {
    let mut app = start::<Updates>(Vec::new());
    app.wait_for("text:primary #123456ff same true").unwrap();
}

/// A design system's type mapping `popover` onto its `surface-overlay`.
#[derive(Debug, Clone, PartialEq)]
struct Ds {
    overlay: AdaptiveColor,
}

impl Default for Ds {
    fn default() -> Self {
        Ds {
            overlay: AdaptiveColor::rgb(0x0a0b0c, 0x0a0b0c),
        }
    }
}

impl AppTheme for Ds {
    type Patch = ();

    fn token_specs() -> &'static [TokenSpec] {
        static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
            vec![TokenSpec::new("surface-overlay", TokenKind::Color).role(role::POPOVER)]
        });
        &SPECS
    }

    fn write_tokens(&self, prefix: &str, out: &mut Tokens) {
        out.set_color(&format!("{prefix}surface-overlay"), self.overlay);
    }

    fn read_tokens(tokens: &Tokens, prefix: &str) -> Self {
        Ds {
            overlay: tokens
                .color(&format!("{prefix}surface-overlay"))
                .unwrap_or(Ds::default().overlay),
        }
    }

    fn write_patch(_: &(), _: &str, _: &mut Tokens) {}
}

#[component]
fn Fields() -> impl IntoElement {
    text(move || {
        let t = theme();
        format!("popover {} primary {}", hex(t.popover), hex(t.primary))
    })
}

/// Under a registered type, `theme()` reads the tokens its roles are mapped
/// onto, and shadcn's tokens no type has are the default theme's.
#[test]
fn under_a_registered_type_theme_reads_its_roles() {
    let mut ds = Registry::new();
    ds.theme::<Ds>();
    let mut app = start::<Fields>(vec![nsh_base::registry(), ds]);
    app.wait_for(&format!(
        "text:popover #0a0b0cff primary {}",
        hex(Theme::default().primary)
    ))
    .unwrap();
}
