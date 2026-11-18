//! `nsh_base::Theme` over the theme format: its fields read through roles
//! from tokens resolved of a theme document and its patches.

use nsh_base::Theme;
use nsh_core::{Color, prelude::*};

fn hex(c: AdaptiveColor) -> String {
    format!("#{:08x}", Color::from(c.light).0)
}

/// Each of `set_color` and `set_number` changes its token and keeps every
/// edit made before it (an embedding app sets them one by one). A patch never
/// adds a name: `set_font` of a font the document doesn't declare changes
/// nothing, and keeps the earlier edits too.
#[test]
fn successive_theme_edits_keep_earlier_ones() {
    let mut theme = Theme::default();
    theme.set_color("primary", AdaptiveColor::rgb(0x123456, 0x123456));
    theme.set_color("background", AdaptiveColor::rgb(0x654321, 0x654321));
    theme.set_number("radius", 7.0);
    theme.set_font("font-sans", "Inter");
    theme.set_font("font-mono", "Iosevka");

    assert_eq!(hex(theme.primary), "#123456ff");
    assert_eq!(hex(theme.background), "#654321ff");
    assert_eq!(theme.radius.0, 7.0);
    assert_eq!(theme.font_sans, None);
    assert_eq!(theme.font_mono, None);
    assert_eq!(hex(theme.color("primary").unwrap()), "#123456ff");
    assert_eq!(theme.number("radius"), Some(7.0));
}
