//! The theme of NSH apps' UI: shadcn/ui's design tokens, exactly. The
//! surfaces and roles (`background`, `card`, `popover`, `primary`,
//! `secondary`, `muted`, `accent`), each with the color of what is drawn on
//! it (`-foreground`), `destructive`, `border`, `input`, `ring`, the charts'
//! colors (`chart-1` to `chart-5`), the sidebar's (`sidebar`, ...), each
//! with a light and a dark value, and the corner radius (`radius`): the
//! fields of [`Theme`], with its fonts, Tailwind's `font-sans` and
//! `font-mono`, which the default leaves to the system.
//!
//! A theme file is shadcn's `cssVars`, as a registry item has them (what
//! tweakcn and shadcn's themes export): `{ "cssVars": { "theme": { … },
//! "light": { "primary": "oklch(0.205 0 0)", … }, "dark": { … } } }`,
//! values as CSS writes them (colors, `var(--ring)`, `color-mix()`,
//! lengths in `rem` or `px`, font family lists). An app's
//! (`nsh.json` `"theme"`) goes over the default ([`DEFAULT_THEME`], shadcn's
//! `neutral`); names it adds (`surface`, `up`, `radius-card`) are the app's
//! own tokens, as variables a CSS theme adds are.
//!
//! The core keeps the result as design tokens ([`Tokens`]), which Tailwind
//! classes name (`bg-primary`, `bg-up/30`, `rounded-(--radius)`) and TS
//! reads (`token()` of `nsh`); it reads `nsh.json`'s theme with
//! [`tokens_of`], which `nsh/base`'s registry brings. [`theme()`] reads
//! each field through its role ([`role`]: the token a theme maps it onto,
//! else the token of its name), and the components that draw a look of their
//! own read it: `Editor`, `TextView`, `RichEditor`, `DiffView`, `Terminal`,
//! `Toast`, the scroll bar and the terminal looks of the controls.

use std::sync::{Arc, LazyLock};

#[cfg(test)]
use nsh_core::{Color, color::Hsla};
use nsh_core::{
    element::{AdaptiveColor, Role, SharedString, Tokens, theme_file},
    geometry::{Points, px},
};

/// The default theme file: shadcn/ui's `neutral` theme.
pub const DEFAULT_THEME: &str = include_str!("default-theme.json");

static DEFAULT: LazyLock<Theme> = LazyLock::new(|| Theme::of(Arc::new(DEFAULT_TOKENS.clone())));

/// A theme file that does not parse.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeError(pub String);

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeError {}

thread_local! {
    /// The theme of the tokens last read: `theme()` makes one only when
    /// the tokens change.
    static CURRENT: std::cell::RefCell<Option<Arc<Theme>>> = const { std::cell::RefCell::new(None) };
}

/// The theme the app is drawn with (reading it subscribes: a new theme
/// reaches what read it).
pub fn theme() -> Arc<Theme> {
    let tokens = nsh_core::element::tokens();
    CURRENT.with_borrow_mut(|current| match current {
        Some(theme) if Arc::ptr_eq(&theme.tokens, &tokens) => theme.clone(),
        _ => current.insert(Arc::new(Theme::of(tokens))).clone(),
    })
}

/// The tokens of theme file `json` over the default's (`None`: the
/// default's); what it cannot read is logged and left out.
pub fn tokens_of(json: Option<&str>) -> Result<Tokens, String> {
    let (tokens, ignored) = read_tokens(json)?;
    for what in ignored {
        log::warn!("theme: {what}, ignored");
    }
    Ok(tokens)
}

/// [`tokens_of`] with what it left out: how the core reads `nsh.json`'s
/// `"theme"`, and checks one an app sets while it runs (`setTheme`)
/// ([`nsh_core::element::Registry::tokens_from`]).
pub(crate) fn read_tokens(json: Option<&str>) -> Result<(Tokens, Vec<String>), String> {
    match json {
        Some(json) => {
            let (theme, ignored) = Theme::from_json_lenient(json).map_err(|e| e.0)?;
            Ok((theme.into_tokens(), ignored))
        }
        None => Ok((Theme::default().into_tokens(), Vec::new())),
    }
}

macro_rules! theme {
    ($( $(#[doc = $doc:literal])* $name:ident = $key:literal => $role:ident ),* $(,)?) => {
        /// The roles `nsh-base`'s theme and components draw with: shadcn/ui's
        /// tokens, each the role of its name. An app's own theme maps them
        /// onto its tokens ([`nsh_core::element::Tokens::with_role`]);
        /// without a mapping each reads the token of its name. The ones the
        /// core and the platforms' own controls draw with too
        /// ([`nsh_core::element::role`]: `POPOVER`, `PRIMARY`, …) are the
        /// same roles, as a role is its name.
        pub mod role {
            use nsh_core::element::{FontRole, NumberRole, Role};

            #[allow(unused_imports)]
            use super::Theme;

            $(
                $(#[doc = $doc])*
                #[doc = concat!("\n\nName `", $key, "`.")]
                pub const $role: Role = Role::new($key);
            )*
            /// The corner radius.
            ///
            /// Name `radius`.
            pub const RADIUS: NumberRole = Role::new("radius");
            /// The app's default font family.
            ///
            /// Name `font-sans`.
            pub const FONT_SANS: FontRole = nsh_core::element::role::FONT_SANS;
            /// The monospaced font family.
            ///
            /// Name `font-mono`.
            pub const FONT_MONO: FontRole = Role::new("font-mono");
            /// The highlight of a selection ([`Theme::selection`] draws
            /// `primary` at 22 % when the theme has none).
            ///
            /// Name `selection`.
            pub const SELECTION: Role = Role::new("selection");
        }

        /// A theme: shadcn/ui's tokens, each a field, and the app's own
        /// ones by name ([`Theme::color`], [`Theme::number`],
        /// [`Theme::font`]).
        #[derive(Debug, Clone, PartialEq)]
        #[non_exhaustive]
        pub struct Theme {
            $(
                $(#[doc = $doc])*
                #[doc = concat!("\n\nToken `", $key, "`.")]
                pub $name: AdaptiveColor,
            )*
            /// The corner radius.
            ///
            /// Token `radius`.
            pub radius: Points,
            /// The app's font family, which text is drawn with unless it
            /// sets its own (`None`: the system's): Tailwind's `--font-sans`,
            /// which a theme may set.
            ///
            /// Token `font-sans`.
            pub font_sans: Option<SharedString>,
            /// The monospaced font family (`None`: the system's): Tailwind's
            /// `--font-mono`.
            ///
            /// Token `font-mono`.
            pub font_mono: Option<SharedString>,
            tokens: Arc<Tokens>,
        }

        impl Theme {
            /// The theme of `tokens`, each field read through its role (the
            /// token a theme maps it onto, else the token of its name):
            /// shadcn's tokens they leave out (an embedding app's may) are
            /// the default's.
            fn of(tokens: Arc<Tokens>) -> Self {
                let color = |role: Role| {
                    tokens
                        .role(role)
                        .or_else(|| DEFAULT_TOKENS.color(role.name()))
                        .unwrap_or(AdaptiveColor::TRANSPARENT)
                };
                Theme {
                    $( $name: color(role::$role), )*
                    radius: px(tokens
                        .role(role::RADIUS)
                        .or_else(|| DEFAULT_TOKENS.number(role::RADIUS.name()))
                        .unwrap_or(0.0)),
                    font_sans: tokens.role(role::FONT_SANS).map(SharedString::from),
                    font_mono: tokens.role(role::FONT_MONO).map(SharedString::from),
                    tokens,
                }
            }
        }
    };
}

/// The default theme's tokens, which [`Theme::of`] falls back on (the
/// default [`Theme`] is made of them).
static DEFAULT_TOKENS: LazyLock<Tokens> = LazyLock::new(|| {
    let mut tokens = Tokens::default();
    let ignored = apply(&mut tokens, DEFAULT_THEME).expect("the default theme parses");
    debug_assert!(ignored.is_empty(), "the default theme: {ignored:?}");
    tokens
});

theme! {
    /// The app's background.
    background = "background" => BACKGROUND,
    /// Text and icons on [`Theme::background`].
    foreground = "foreground" => FOREGROUND,
    /// Cards.
    card = "card" => CARD,
    /// Text on [`Theme::card`].
    card_foreground = "card-foreground" => CARD_FOREGROUND,
    /// Popovers, menus, tooltips.
    popover = "popover" => POPOVER,
    /// Text on [`Theme::popover`].
    popover_foreground = "popover-foreground" => POPOVER_FOREGROUND,
    /// The main action.
    primary = "primary" => PRIMARY,
    /// Text on [`Theme::primary`].
    primary_foreground = "primary-foreground" => PRIMARY_FOREGROUND,
    /// Secondary actions.
    secondary = "secondary" => SECONDARY,
    /// Text on [`Theme::secondary`].
    secondary_foreground = "secondary-foreground" => SECONDARY_FOREGROUND,
    /// Muted surfaces.
    muted = "muted" => MUTED,
    /// Secondary text: hints, captions, placeholders.
    muted_foreground = "muted-foreground" => MUTED_FOREGROUND,
    /// What is hovered or focused: menu items, list rows.
    accent = "accent" => ACCENT,
    /// Text on [`Theme::accent`].
    accent_foreground = "accent-foreground" => ACCENT_FOREGROUND,
    /// Destructive actions and errors.
    destructive = "destructive" => DESTRUCTIVE,
    /// Borders and dividers.
    border = "border" => BORDER,
    /// The borders of inputs.
    input = "input" => INPUT,
    /// Focus rings.
    ring = "ring" => RING,
    /// A chart's first series.
    chart_1 = "chart-1" => CHART_1,
    /// A chart's second series.
    chart_2 = "chart-2" => CHART_2,
    /// A chart's third series.
    chart_3 = "chart-3" => CHART_3,
    /// A chart's fourth series.
    chart_4 = "chart-4" => CHART_4,
    /// A chart's fifth series.
    chart_5 = "chart-5" => CHART_5,
    /// The sidebar.
    sidebar = "sidebar" => SIDEBAR,
    /// Text on [`Theme::sidebar`].
    sidebar_foreground = "sidebar-foreground" => SIDEBAR_FOREGROUND,
    /// The sidebar's main action.
    sidebar_primary = "sidebar-primary" => SIDEBAR_PRIMARY,
    /// Text on [`Theme::sidebar_primary`].
    sidebar_primary_foreground = "sidebar-primary-foreground" => SIDEBAR_PRIMARY_FOREGROUND,
    /// What is hovered or focused in the sidebar.
    sidebar_accent = "sidebar-accent" => SIDEBAR_ACCENT,
    /// Text on [`Theme::sidebar_accent`].
    sidebar_accent_foreground = "sidebar-accent-foreground" => SIDEBAR_ACCENT_FOREGROUND,
    /// The sidebar's borders.
    sidebar_border = "sidebar-border" => SIDEBAR_BORDER,
    /// Focus rings in the sidebar.
    sidebar_ring = "sidebar-ring" => SIDEBAR_RING,
}

impl Default for Theme {
    /// The default theme ([`DEFAULT_THEME`]).
    fn default() -> Self {
        DEFAULT.clone()
    }
}

impl Theme {
    /// The default theme with theme file `json` over it. What it cannot
    /// read (a section or value of a form it does not know, a color naming
    /// itself) is left out and the rest applies: a bundle made by a newer
    /// NSH keeps its theme on an older runtime. Only text that is not JSON
    /// is an error.
    pub fn from_json(json: &str) -> Result<Self, ThemeError> {
        Self::from_json_lenient(json).map(|(theme, _)| theme)
    }

    /// [`Theme::from_json`], with what it left out, which the toolchain
    /// warns of (a newer NSH's, or a typo) and the runtime logs.
    pub fn from_json_lenient(json: &str) -> Result<(Self, Vec<String>), ThemeError> {
        let mut tokens = DEFAULT_TOKENS.clone();
        let ignored = apply(&mut tokens, json)?;
        Ok((Self::of(Arc::new(tokens)), ignored))
    }

    /// The color named `name` (any the theme sets: shadcn's, the app's).
    pub fn color(&self, name: &str) -> Option<AdaptiveColor> {
        self.tokens.color(name)
    }

    /// The highlight of a selection, the one every component draws: the
    /// app's `selection` color if the theme sets one, else `primary` at
    /// 22 %.
    pub fn selection(&self) -> AdaptiveColor {
        self.tokens
            .role(role::SELECTION)
            .unwrap_or_else(|| self.primary.opacity(0.22))
    }

    /// The number named `name` (any the theme sets).
    pub fn number(&self, name: &str) -> Option<f32> {
        self.tokens.number(name)
    }

    /// The font family named `name` (any the theme sets; its sans and mono
    /// ones are [`Theme::font_sans`] and [`Theme::font_mono`]).
    pub fn font(&self, name: &str) -> Option<Arc<str>> {
        self.tokens.font(name).cloned()
    }

    /// Sets the color named `name` (one of shadcn's sets its field).
    pub fn set_color(&mut self, name: &str, color: AdaptiveColor) {
        self.edit(|t| t.set_color(name, color));
    }

    /// Sets the number named `name` (`radius` sets its field).
    pub fn set_number(&mut self, name: &str, value: f32) {
        self.edit(|t| t.set_number(name, value));
    }

    /// Sets the font family named `name`.
    pub fn set_font(&mut self, name: &str, family: impl Into<Arc<str>>) {
        self.edit(|t| t.set_font(name, family));
    }

    /// Its design tokens.
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// Its design tokens, for the core ([`nsh_core::element::Root::tokens`],
    /// an engine's host theme).
    pub fn into_tokens(self) -> Tokens {
        Arc::unwrap_or_clone(self.tokens)
    }

    /// Changes the tokens with `f`, and the fields with them.
    fn edit(&mut self, f: impl FnOnce(&mut Tokens)) {
        let mut tokens = std::mem::take(&mut self.tokens);
        f(Arc::make_mut(&mut tokens));
        *self = Self::of(tokens);
    }
}

impl From<Theme> for Tokens {
    fn from(theme: Theme) -> Self {
        theme.into_tokens()
    }
}

/// Applies theme file `json` (shadcn's `cssVars`) over `tokens`.
fn apply(tokens: &mut Tokens, json: &str) -> Result<Vec<String>, ThemeError> {
    let (out, ignored) = theme_file::parse(json, tokens).map_err(ThemeError)?;
    *tokens = out;
    Ok(ignored)
}

/// The tokens theme file `json` sets, and only those: an embedding app's
/// overrides of the app's theme (the platforms' `AppView`s, the iOS and
/// Android SDKs), which win over the app's tokens of the same names and
/// leave its others as they are (`Core::set_host_tokens`). The file is the
/// one [`Theme::from_json`] reads, shadcn's `cssVars`; a color it names
/// (`var(--ring)`) and does not set is the default theme's.
pub fn theme_overrides(json: &str) -> Result<Tokens, ThemeError> {
    let (tokens, ignored) = theme_file::overrides(json, &DEFAULT_TOKENS).map_err(ThemeError)?;
    for what in ignored {
        log::warn!("theme overrides: {what}, ignored");
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(c: Hsla) -> u32 {
        Color::from(c).0 >> 8
    }

    /// shadcn's names, every one: the default theme has these and no
    /// other.
    const SHADCN: &[&str] = &[
        "background",
        "foreground",
        "card",
        "card-foreground",
        "popover",
        "popover-foreground",
        "primary",
        "primary-foreground",
        "secondary",
        "secondary-foreground",
        "muted",
        "muted-foreground",
        "accent",
        "accent-foreground",
        "destructive",
        "border",
        "input",
        "ring",
        "chart-1",
        "chart-2",
        "chart-3",
        "chart-4",
        "chart-5",
        "sidebar",
        "sidebar-foreground",
        "sidebar-primary",
        "sidebar-primary-foreground",
        "sidebar-accent",
        "sidebar-accent-foreground",
        "sidebar-border",
        "sidebar-ring",
    ];

    #[test]
    fn the_default_theme_is_shadcns_neutral() {
        let theme = Theme::default();
        let mut colors: Vec<&str> = theme.tokens().colors().map(|(k, _)| k).collect();
        colors.sort_unstable();
        let mut expected = SHADCN.to_vec();
        expected.sort_unstable();
        assert_eq!(colors, expected);
        let numbers: Vec<_> = theme.tokens().numbers().collect();
        assert_eq!(numbers, [("radius", 10.0)]);
        assert_eq!(theme.tokens().fonts().count(), 0);
        // oklch(0.205 0 0) / oklch(0.922 0 0).
        assert_eq!(hex(theme.primary.light), 0x171717);
        assert_eq!(hex(theme.primary.dark), 0xe5e5e5);
        assert_eq!(theme.radius, px(10.0));
        // oklch(1 0 0 / 10%).
        assert!((theme.border.dark.a - 0.1).abs() < 0.01);
        // Tokens without them read the defaults.
        assert_eq!(Theme::of(Arc::default()).ring, theme.ring);
        assert_eq!(theme.font_sans, None);
    }

    #[test]
    fn css_vars_go_over_the_default() {
        let theme = Theme::from_json(
            r##"{ "cssVars": {
                "theme": { "font-sans": "Inter, sans-serif", "font-mono": "'JetBrains Mono', monospace",
                           "radius-card": "14px", "shadow-sm": "0 1px 2px #0000" },
                "light": { "primary": "#7c3aed", "surface": "#ffffff", "up": "#138a5a",
                           "up-subtle": "color-mix(in srgb, var(--up) 50%, transparent)",
                           "sidebar-ring": "var(--primary)", "radius": "0.5rem" },
                "dark": { "surface": "#18191c" }
            } }"##,
        )
        .unwrap();
        assert_eq!(hex(theme.primary.light), 0x7c3aed);
        // Only light: both.
        assert_eq!(hex(theme.primary.dark), 0x7c3aed);
        let surface = theme.color("surface").unwrap();
        assert_eq!(
            (hex(surface.light), hex(surface.dark)),
            (0xffffff, 0x18191c)
        );
        assert_eq!(hex(theme.sidebar_ring.light), 0x7c3aed);
        assert_eq!(theme.radius, px(8.0));
        assert_eq!(theme.number("radius-card"), Some(14.0));
        assert_eq!(theme.font_sans.as_deref(), Some("Inter"));
        assert_eq!(theme.font_mono.as_deref(), Some("JetBrains Mono"));
        // Of no kind a token has.
        assert_eq!(theme.tokens().get("shadow-sm"), None);
        // Keys left out keep the default.
        assert_eq!(theme.muted, Theme::default().muted);
        assert!(Theme::from_json("not json").is_err());
    }

    #[test]
    fn what_it_cannot_read_is_left_out() {
        let (theme, mut ignored) = Theme::from_json_lenient(
            r##"{ "cssVars": {
                "theme": { "radius": "0.5rem" },
                "light": { "a": "var(--b)", "b": "var(--a)", "primary": "#e11d48", "odd": [1] },
                "dark": "nope"
            } }"##,
        )
        .unwrap();
        assert_eq!(hex(theme.primary.light), 0xe11d48);
        assert_eq!(theme.color("a"), None);
        ignored.sort();
        assert_eq!(
            ignored,
            [
                "`a` names itself",
                "`b` names itself",
                "cssVars.dark: expected an object",
                "cssVars.light.odd: expected a CSS value, got [1]",
            ]
        );
        // Another form of theme (a newer NSH's): the default's tokens.
        let (theme, ignored) = Theme::from_json_lenient(r#"{ "colors": {} }"#).unwrap();
        assert_eq!(theme, Theme::default());
        assert_eq!(ignored.len(), 1);
    }

    #[test]
    fn a_color_that_is_none_is_listed() {
        let (theme, mut ignored) = Theme::from_json_lenient(
            r##"{ "cssVars": {
                "theme": { "shadow-sm": "0 1px 2px #0000", "letter-spacing": "0em" },
                "light": { "brand": "#e11d48", "edge": "#zzz", "glow": "lab(50% 40 59)",
                           "ink": "var(--nothing)", "accent": "#7c3aed" },
                "dark": { "brand": "#12", "accent": "oklch(0.7 0.1" }
            } }"##,
        )
        .unwrap();
        ignored.sort();
        assert_eq!(
            ignored,
            [
                "`accent`: oklch(0.7 0.1 is no color in the dark appearance",
                "`brand`: #12 is no color in the dark appearance",
                "`edge`: #zzz is no color",
                "`glow`: lab(50% 40 59) is no color",
                "`ink`: var(--nothing) is no color",
            ]
        );
        // As before: a color given in one appearance only is both's, and
        // what is none of a token's kinds is left out.
        let brand = theme.color("brand").unwrap();
        assert_eq!((hex(brand.light), hex(brand.dark)), (0xe11d48, 0xe11d48));
        assert_eq!(theme.color("edge"), None);
        assert_eq!(theme.tokens().get("shadow-sm"), None);
        // A file read whole lists nothing.
        let (_, ignored) = read_tokens(Some(DEFAULT_THEME)).unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
    }

    #[test]
    fn a_var_falls_back() {
        let theme = Theme::from_json(
            r##"{ "cssVars": { "light": { "edge": "var(--nothing, #ff0000)", "line": "var(--border)" } } }"##,
        )
        .unwrap();
        assert_eq!(hex(theme.color("edge").unwrap().light), 0xff0000);
        // The default's, in each appearance.
        assert_eq!(theme.color("line").unwrap(), Theme::default().border);
    }

    #[test]
    fn overrides_are_the_tokens_the_file_sets() {
        let tokens = theme_overrides(
            r##"{ "cssVars": {
                "theme": { "radius": "0.75rem" },
                "light": { "primary": "#e11d48", "ring": "var(--primary)", "focus": "var(--border)" },
                "dark": { "primary": "#fb7185" }
            } }"##,
        )
        .unwrap();
        let primary = tokens.color("primary").unwrap();
        assert_eq!(
            (hex(primary.light), hex(primary.dark)),
            (0xe11d48, 0xfb7185)
        );
        // The file's own, per appearance.
        assert_eq!(tokens.color("ring").unwrap(), primary);
        // One it does not set: the default's.
        assert_eq!(tokens.color("focus").unwrap(), Theme::default().border);
        assert_eq!(tokens.number("radius"), Some(12.0));
        // Nothing else: the app's tokens stay.
        assert_eq!(tokens.colors().count(), 3);
        assert_eq!(tokens.color("background"), None);
        assert_eq!(theme_overrides("{}").unwrap().colors().count(), 0);
        assert!(theme_overrides("not json").is_err());
    }
}
