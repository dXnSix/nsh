//! `nsh_base::Theme`'s setters under an app's own theme type: a field
//! whose role the type maps sets the token the role is mapped onto, as
//! `theme()` reads it, and a theme read and set makes no shadcn token of
//! what the app did not change.

use std::time::Duration;

use nsh_base::{Theme, role, theme};
use nsh_core::{
    Color, Size,
    element::{AppTheme, Registry, Root, TokenKind, TokenSpec, Tokens},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

/// A design system that maps `popover` onto its `surface-overlay`, and no
/// other role.
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

fn hex(c: AdaptiveColor) -> String {
    format!("{:06x}", Color::from(c.light).0 >> 8)
}

/// `theme()`'s popover and border, and the app's color tokens by name.
fn shown() -> String {
    let tokens = tokens();
    let mut names: Vec<&str> = tokens.colors().map(|(n, _)| n).collect();
    names.sort();
    let t = theme();
    format!(
        "popover {} border {} tokens {}",
        hex(t.popover),
        hex(t.border),
        names.join(" ")
    )
}

fn start<C: Component<Props = ()>>() -> Headless {
    let mut ds = Registry::new();
    ds.theme::<Ds>();
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        ui_sets: vec![nsh_base::registry(), ds],
        ..EngineConfig::new(Root::of::<C>())
    })
    .unwrap();
    app.first_render().unwrap();
    app
}

/// Reads `theme()`, changes the popover and the border, and sets it.
#[component]
fn SetsTheTheme() -> impl IntoElement {
    let first = shown();
    set_timeout(Duration::from_millis(50), || {
        let mut t: Theme = (*theme()).clone();
        t.popover = AdaptiveColor::rgb(0xff0000, 0xff0000);
        t.border = AdaptiveColor::rgb(0x00ff00, 0x00ff00);
        set_theme(t);
    });
    text(move || format!("first {first}, now {}", shown()))
}

#[test]
fn a_theme_read_and_set_changes_what_its_fields_read_and_adds_no_other_token() {
    let mut app = start::<SetsTheTheme>();
    // The popover is the design system's overlay, which the role reads.
    // The border, which no type maps, is the token of its name. Every
    // other shadcn name stays no token.
    app.wait_for(
        "text:first popover 0a0b0c border e5e5e5 tokens surface-overlay, \
         now popover ff0000 border 00ff00 tokens border surface-overlay",
    )
    .unwrap();
}

/// Changes the popover by name with a patch, then puts the theme back.
#[component]
fn UpdatesAndResets() -> impl IntoElement {
    let step = signal("first");
    set_timeout(Duration::from_millis(50), move || {
        let mut patch = Tokens::default();
        patch.set_color("popover", AdaptiveColor::rgb(0x0000ff, 0x0000ff));
        patch.set_color("border", AdaptiveColor::rgb(0x00ff00, 0x00ff00));
        update_theme::<Theme>(patch);
        step.set("updated");
    });
    set_timeout(Duration::from_millis(200), move || {
        reset_theme::<Theme>();
        step.set("reset");
    });
    text(move || format!("{} {}", step.get(), shown()))
}

#[test]
fn a_patch_and_a_reset_of_the_theme_go_through_its_roles() {
    let mut app = start::<UpdatesAndResets>();
    app.wait_for("text:updated popover 0000ff border 00ff00 tokens border surface-overlay")
        .unwrap();
    app.wait_for("text:reset popover 0a0b0c border e5e5e5 tokens surface-overlay")
        .unwrap();
}
