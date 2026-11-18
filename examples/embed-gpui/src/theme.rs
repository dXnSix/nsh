//! GPUI Kit's theme as NSH's: the host's patch for its embedded apps.
//!
//! The embedded app draws with `nsh/base`'s default theme, whose tokens are
//! shadcn/ui's, and GPUI Kit's colors have the same roles under its own
//! names, so the conversion sets each of shadcn's tokens to the GPUI Kit
//! color of its role, as GPUI Kit resolved it (with its fallbacks), and
//! `radius` to GPUI Kit's: a theme patch (`docs/dev/theme-format.md`,
//! section 8) the surfaces take as their host layer
//! ([`nsh_platform_gpui::AppView::set_theme_overrides`]), so the embedded
//! components match the host's widgets exactly. A patch holds in every
//! context the app picks, light and dark alike, so it is the current mode's
//! colors, sent again on every change of the host's theme or mode. A patch
//! never adds a token, so GPUI Kit's fonts, which the default theme has no
//! token for, stay the system's.

use gpui_kit::{
    Hsla,
    component::{Theme, ThemeMode},
};
use nsh_platform_gpui::{Appearance, Color};

/// A GPUI color as NSH's: the same `Hsla`, channel for channel.
fn color(c: Hsla) -> nsh_platform_gpui::Hsla {
    nsh_platform_gpui::Hsla {
        h: c.h,
        s: c.s,
        l: c.l,
        a: c.a,
    }
}

/// A GPUI color in the theme format's form: `#rrggbb`, with its alpha
/// (`#rrggbbaa`) below full opacity.
fn hex(c: Hsla) -> String {
    let Color(rgba) = Color::from(color(c));
    if rgba & 0xff == 0xff {
        format!("#{:06x}", rgba >> 8)
    } else {
        format!("#{rgba:08x}")
    }
}

/// The host's light or dark mode, for [`nsh_platform_gpui::AppView::set_appearance`].
pub fn appearance(mode: ThemeMode) -> Appearance {
    if mode.is_dark() {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

/// `theme` (GPUI Kit's active theme, in its current mode) as a theme patch
/// of shadcn's tokens, for
/// [`nsh_platform_gpui::AppView::set_theme_overrides`].
pub fn host_patch(theme: &Theme) -> String {
    let colors = &theme.colors;
    let mut values = serde_json::Map::new();
    for (name, c) in [
        ("background", colors.background),
        ("foreground", colors.foreground),
        ("card", colors.group_box),
        ("card-foreground", colors.group_box_foreground),
        ("popover", colors.popover),
        ("popover-foreground", colors.popover_foreground),
        ("primary", colors.primary),
        ("primary-foreground", colors.primary_foreground),
        ("secondary", colors.secondary),
        ("secondary-foreground", colors.secondary_foreground),
        ("muted", colors.muted),
        ("muted-foreground", colors.muted_foreground),
        ("accent", colors.accent),
        ("accent-foreground", colors.accent_foreground),
        ("destructive", colors.danger),
        ("border", colors.border),
        ("input", colors.input),
        ("ring", colors.ring),
        ("chart-1", colors.chart_1),
        ("chart-2", colors.chart_2),
        ("chart-3", colors.chart_3),
        ("chart-4", colors.chart_4),
        ("chart-5", colors.chart_5),
        ("sidebar", colors.sidebar),
        ("sidebar-foreground", colors.sidebar_foreground),
        ("sidebar-primary", colors.sidebar_primary),
        (
            "sidebar-primary-foreground",
            colors.sidebar_primary_foreground,
        ),
        ("sidebar-accent", colors.sidebar_accent),
        (
            "sidebar-accent-foreground",
            colors.sidebar_accent_foreground,
        ),
        ("sidebar-border", colors.sidebar_border),
        ("sidebar-ring", colors.ring),
    ] {
        values.insert(name.to_owned(), hex(c).into());
    }
    values.insert("radius".to_owned(), f32::from(theme.radius).into());
    serde_json::json!({ "values": values }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_take_the_theme_formats_form() {
        assert_eq!(hex(gpui_kit::rgb(0x336699).into()), "#336699");
        assert_eq!(hex(gpui_kit::transparent_black()), "#00000000");
    }

    /// The patch is the current mode's resolved colors under shadcn's
    /// names.
    #[test]
    fn the_current_modes_resolved_colors_are_the_patchs_values() {
        let mut theme = Theme::default();
        theme.mode = ThemeMode::Dark;
        theme.radius = gpui_kit::px(6.0);
        theme.colors.primary = gpui_kit::rgb(0x7c3aed).into();
        let patch: serde_json::Value = serde_json::from_str(&host_patch(&theme)).unwrap();
        assert_eq!(patch["values"]["primary"], "#7c3aed");
        assert_eq!(patch["values"]["radius"], 6.0);
    }

    /// Every name the patch sets is a token of `nsh/base`'s default theme,
    /// of its type: nothing is left out.
    #[test]
    fn the_patch_fits_nsh_bases_default_theme() {
        let (document, _) = nsh_core::element::theme::read_document(nsh_base::DEFAULT_THEME);
        let document = document.expect("nsh/base's default theme");
        let (patch, read) = nsh_core::element::theme::read_patch(&host_patch(&Theme::default()));
        assert!(read.is_empty(), "{read:?}");
        let checked = patch.check(&document);
        assert!(checked.is_empty(), "{checked:?}");
    }
}
