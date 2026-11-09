//! A design system's token playing several roles: one surface for cards
//! and popovers. `theme().card` and `theme().popover` are that token, and
//! both follow a change of it.

use std::time::Duration;

use nsh_base::{role, theme};
use nsh_core::{
    Color, Size,
    element::{AppTheme, Registry, Root, TokenKind, TokenSpec, Tokens},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

/// One surface for cards and popovers.
#[derive(Debug, Clone, PartialEq)]
struct Flat {
    surface: AdaptiveColor,
}

impl Default for Flat {
    fn default() -> Self {
        Flat {
            surface: AdaptiveColor::rgb(0x0a0b0c, 0x0a0b0c),
        }
    }
}

impl AppTheme for Flat {
    type Patch = AdaptiveColor;

    fn token_specs() -> &'static [TokenSpec] {
        static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
            vec![
                TokenSpec::new("surface", TokenKind::Color)
                    .role(role::CARD)
                    .role(role::POPOVER),
            ]
        });
        &SPECS
    }

    fn write_tokens(&self, prefix: &str, out: &mut Tokens) {
        out.set_color(&format!("{prefix}surface"), self.surface);
    }

    fn read_tokens(tokens: &Tokens, prefix: &str) -> Self {
        Flat {
            surface: tokens
                .color(&format!("{prefix}surface"))
                .unwrap_or(Flat::default().surface),
        }
    }

    fn write_patch(patch: &AdaptiveColor, prefix: &str, out: &mut Tokens) {
        out.set_color(&format!("{prefix}surface"), *patch);
    }
}

fn hex(c: AdaptiveColor) -> String {
    format!("#{:08x}", Color::from(c.light).0)
}

/// `theme()`'s card and popover.
fn shown() -> String {
    let t = theme();
    format!("card {} popover {}", hex(t.card), hex(t.popover))
}

/// Shows them as they are at first and as they are now, and changes the
/// one token.
#[component]
fn Surfaces() -> impl IntoElement {
    let first = shown();
    set_timeout(Duration::from_millis(50), || {
        update_theme::<Flat>(AdaptiveColor::rgb(0x123456, 0x123456));
    });
    text(move || format!("first {first}, now {}", shown()))
}

#[test]
fn theme_reads_one_token_through_both_roles_and_both_follow_it() {
    let mut ds = Registry::new();
    ds.theme::<Flat>();
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ui_sets: vec![nsh_base::registry(), ds],
        ..EngineConfig::new(Root::of::<Surfaces>())
    })
    .unwrap();
    app.first_render().unwrap();
    app.wait_for(
        "text:first card #0a0b0cff popover #0a0b0cff, now card #123456ff popover #123456ff",
    )
    .unwrap();
}
