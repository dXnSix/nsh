//! A design system's token playing several roles: one surface for cards
//! and popovers. `theme().card` and `theme().popover` are that token, and
//! both follow a change of it.

use std::time::Duration;

use nsh_base::theme;
use nsh_core::{
    Color, Size,
    element::{Patch, PatchValues, Registry, Root, TokenKey},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;

/// One surface for cards and popovers.
const FLAT: &str = r##"{
    "tokens": { "surface": { "kind": "color", "value": "#0a0b0c" } },
    "roles": { "card": "surface", "popover": "surface" }
}"##;

const SURFACE: TokenKey<nsh_core::Hsla> = TokenKey::new("surface");

fn hex(c: nsh_core::Hsla) -> String {
    format!("#{:08x}", Color::from(c).0)
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
        update_tokens(Patch::new().key(SURFACE, AdaptiveColor::rgb(0x123456, 0x123456).light));
    });
    text(move || format!("first {first}, now {}", shown()))
}

#[test]
fn theme_reads_one_token_through_both_roles_and_both_follow_it() {
    let mut ds = Registry::new();
    ds.theme_document(FLAT);
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
