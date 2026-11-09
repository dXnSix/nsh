//! Design tokens: colors with a light and a dark value, numbers and font
//! families, by name (`muted-foreground`, `radius`, `font-sans`). The core
//! gives no name a meaning but `font-sans`, the app's default font (as
//! Tailwind's `--font-sans` is a page's), and has none of its own: a UI set
//! brings them (`nsh-base`'s theme, through
//! [`super::Registry::tokens_from`]), an app sets its own, an embedding app
//! its own over those ([`crate::Core::set_host_tokens`]).
//!
//! A name is a CSS custom property's without its `--`: Tailwind classes name
//! every token as they name a theme variable (`bg-muted`, `bg-up/30`,
//! `rounded-(--radius)`, `font-(family-name:--font-mono)`;
//! `tailwind/theme.rs`), TS reads them with `nsh`'s `token()`, and Rust
//! components with [`super::tokens`].

use std::sync::Arc;

use rustc_hash::FxHashMap;

use super::style::{AdaptiveColor, AdaptiveShadow};

/// The app's default font family's token: Tailwind's `--font-sans`.
pub const FONT_SANS: &str = "font-sans";

/// The monospaced font family's token: Tailwind's `--font-mono`.
pub const FONT_MONO: &str = "font-mono";

/// Design tokens by name: colors with a value per appearance, numbers and
/// font families. The default has none.
///
/// They may map [`Role`]s, what NSH's own parts draw with, onto tokens of
/// other names ([`Tokens::with_roles`]): a theme whose surface for popovers
/// is `surface-overlay` maps [`Role::Popover`] onto it, and every part that
/// draws a popover reads that token ([`Tokens::role_color`]). Without a
/// mapping a role reads the token of its own name (`popover`).
#[derive(Debug, Clone, Default)]
pub struct Tokens {
    colors: FxHashMap<Box<str>, AdaptiveColor>,
    /// The numbers' light values, which the dark appearance has too unless
    /// [`Self::dark_numbers`] has another.
    numbers: FxHashMap<Box<str>, f32>,
    /// The dark values of the numbers that have one of their own.
    dark_numbers: FxHashMap<Box<str>, f32>,
    fonts: FxHashMap<Box<str>, Arc<str>>,
    /// Box shadows, per appearance.
    shadows: FxHashMap<Box<str>, AdaptiveShadow>,
    /// Role -> the name of the token it reads (`None`: each its own name).
    roles: Option<Arc<FxHashMap<Role, Box<str>>>>,
    /// Whether the app has a theme of its own ([`Tokens::is_themed`]): not
    /// a token, so not compared.
    themed: bool,
}

impl PartialEq for Tokens {
    fn eq(&self, other: &Self) -> bool {
        self.colors == other.colors
            && self.numbers == other.numbers
            && self.dark_numbers == other.dark_numbers
            && self.fonts == other.fonts
            && self.shadows == other.shadows
            && self.roles == other.roles
    }
}

macro_rules! roles {
    ($( $(#[doc = $doc:literal])* $variant:ident = $name:literal ),* $(,)?) => {
        /// What NSH's own parts draw with: `nsh-base`'s theme
        /// (`theme().popover`), the platforms' own controls (Linux's menus,
        /// the mobile tab bar, iOS's and Android's alerts), the default
        /// font. Each reads the token of its name ([`Role::name`],
        /// shadcn/ui's), or the token a theme maps it onto
        /// ([`Tokens::with_roles`]). A role is no token and no class:
        /// `bg-primary` exists when a theme has a token `primary`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum Role {
            $(
                $(#[doc = $doc])*
                #[doc = concat!("\n\nName `", $name, "`.")]
                $variant,
            )*
        }

        impl Role {
            /// Every role.
            pub const ALL: &'static [Role] = &[$(Role::$variant),*];

            /// Its name: the token it reads unless a theme maps it, as
            /// shadcn/ui spells it (`popover-foreground`).
            pub fn name(self) -> &'static str {
                match self {
                    $(Role::$variant => $name,)*
                }
            }

            /// The role of this name (`"popover"`), if any.
            pub fn from_name(name: &str) -> Option<Role> {
                match name {
                    $($name => Some(Role::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

roles! {
    /// The app's background.
    Background = "background",
    /// Text and icons on the background.
    Foreground = "foreground",
    /// Cards.
    Card = "card",
    /// Text on cards.
    CardForeground = "card-foreground",
    /// Popovers, menus, tooltips, alerts.
    Popover = "popover",
    /// Text on popovers.
    PopoverForeground = "popover-foreground",
    /// The main action: buttons, the tab bar's and alerts' tint.
    Primary = "primary",
    /// Text on the main action.
    PrimaryForeground = "primary-foreground",
    /// Secondary actions.
    Secondary = "secondary",
    /// Text on secondary actions.
    SecondaryForeground = "secondary-foreground",
    /// Muted surfaces.
    Muted = "muted",
    /// Secondary text: hints, captions, an alert's message.
    MutedForeground = "muted-foreground",
    /// What is hovered or highlighted: menu items, list rows.
    Accent = "accent",
    /// Text on what is highlighted.
    AccentForeground = "accent-foreground",
    /// Destructive actions and errors.
    Destructive = "destructive",
    /// Borders and dividers.
    Border = "border",
    /// The borders of inputs.
    Input = "input",
    /// Focus rings.
    Ring = "ring",
    /// A chart's first series.
    Chart1 = "chart-1",
    /// A chart's second series.
    Chart2 = "chart-2",
    /// A chart's third series.
    Chart3 = "chart-3",
    /// A chart's fourth series.
    Chart4 = "chart-4",
    /// A chart's fifth series.
    Chart5 = "chart-5",
    /// The sidebar.
    Sidebar = "sidebar",
    /// Text on the sidebar.
    SidebarForeground = "sidebar-foreground",
    /// The sidebar's main action.
    SidebarPrimary = "sidebar-primary",
    /// Text on the sidebar's main action.
    SidebarPrimaryForeground = "sidebar-primary-foreground",
    /// What is highlighted in the sidebar.
    SidebarAccent = "sidebar-accent",
    /// Text on what is highlighted in the sidebar.
    SidebarAccentForeground = "sidebar-accent-foreground",
    /// The sidebar's borders.
    SidebarBorder = "sidebar-border",
    /// Focus rings in the sidebar.
    SidebarRing = "sidebar-ring",
    /// The highlight of a selection (`nsh-base` draws `primary` at 22 %
    /// when the theme has none).
    Selection = "selection",
    /// The corner radius.
    Radius = "radius",
    /// The app's default font family.
    FontSans = "font-sans",
    /// The monospaced font family.
    FontMono = "font-mono",
}

/// A token's value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Token {
    /// A color, per appearance.
    Color(AdaptiveColor),
    /// A number: a length in points, or a unitless quantity.
    Number(f32),
    /// A font family.
    Font(Arc<str>),
    /// Box shadows, per appearance.
    Shadow(AdaptiveShadow),
}

/// Makes the tokens of a theme file's content (`Some`), or the defaults
/// (`None`), with what of the file it left out (a value it cannot read):
/// how a UI set reads the app's theme ([`super::Registry::tokens_from`]).
/// `Err`: the content is no theme file at all.
pub type TokenSource = fn(Option<&str>) -> Result<(Tokens, Vec<String>), String>;

/// Reads the tokens theme file `json` sets, and only those (a value it
/// cannot read is left out, so the token keeps the value it has); a color
/// it names and does not set (`var(--ring)`) is `base`'s, the tokens drawn
/// with now. Returns them with what of the file it left out: how a UI set
/// reads a change of some of the app's tokens
/// ([`super::Registry::token_overrides_from`], `nsh`'s `updateTheme`).
/// `Err`: the content is no theme file at all.
pub type TokenOverrides = fn(&str, &Tokens) -> Result<(Tokens, Vec<String>), String>;

/// The tokens a surface is drawn with once the app's theme file reaches its
/// root ([`crate::Core::theme_tokens`]). The entry hands the theme to the
/// renderer in the turn of its first render, before the theme's op reaches
/// the core: JS reads the tokens from here meanwhile, so its first render
/// already has them (`token()` of `nsh`).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ThemeTokens {
    /// How theme files are read (`None`: no UI set reads them).
    provider: Option<super::theme::TokenProvider>,
    /// The embedding app's tokens, which win over the app's.
    host: Option<Arc<Tokens>>,
}

/// What of `left_out` refuses a file read whole: every value that could not
/// be read, `; `-separated, but no name only that no theme type declares.
fn refused(left_out: &[String]) -> Option<String> {
    let what: Vec<&str> = left_out
        .iter()
        .filter(|w| !super::theme_file::is_undeclared(w))
        .map(String::as_str)
        .collect();
    (!what.is_empty()).then(|| what.join("; "))
}

impl ThemeTokens {
    pub(crate) fn new(
        provider: Option<super::theme::TokenProvider>,
        host: Option<Arc<Tokens>>,
    ) -> Self {
        Self { provider, host }
    }

    /// The tokens drawn with when the app's theme file is `json`: its own
    /// under the embedding app's, as the core makes them. `None`: no UI set
    /// reads themes, or `json` is not one (the core reports it when the
    /// theme reaches it).
    pub fn of(&self, json: &str) -> Option<Tokens> {
        let (app, _) = self.provider.as_ref()?.read(Some(json)).ok()?;
        Some(self.under_host(app))
    }

    /// [`ThemeTokens::of`] for a theme file that must be read whole (`nsh`'s
    /// `setTheme`): `Err` names each value it cannot read, `; `-separated.
    /// Under theme types a name no type declares is left out, not refused
    /// (#1190). `None`: no UI set reads themes.
    pub fn checked(&self, json: &str) -> Option<Result<Tokens, String>> {
        Some(match self.provider.as_ref()?.read(Some(json)) {
            Ok((app, left_out)) => match refused(&left_out) {
                None => Ok(self.under_host(app)),
                Some(what) => Err(what),
            },
            Err(e) => Err(e),
        })
    }

    /// The tokens drawn with once theme file `json` changes some of the
    /// app's (`nsh`'s `updateTheme`): the values it sets over `current` (the
    /// tokens drawn with now, which a `var()` it does not set names), under
    /// the embedding app's. Read whole: `Err` as [`ThemeTokens::checked`].
    /// `None`: no UI set reads theme changes.
    pub fn updated(&self, current: &Tokens, json: &str) -> Option<Result<Tokens, String>> {
        Some(match self.provider.as_ref()?.overrides(json, current)? {
            Ok((part, left_out)) => match refused(&left_out) {
                None => Ok(self.under_host(part).over(current)),
                Some(what) => Err(what),
            },
            Err(e) => Err(e),
        })
    }

    /// The tokens drawn with once the tokens `names` (`None`: every one)
    /// go back to their values in the app's theme file `json` (`nsh`'s
    /// `resetTheme`), the others staying as in `current`: a name the file
    /// and the defaults lack is gone. `None`: no UI set reads themes, or
    /// `json` is not one.
    pub fn reset(
        &self,
        current: &Tokens,
        json: &str,
        names: Option<&[Box<str>]>,
    ) -> Option<Tokens> {
        let theme = self.of(json)?;
        let Some(names) = names else {
            return Some(theme);
        };
        let mut out = current.clone();
        for name in names {
            out.copy_named(&theme, name);
        }
        Some(out)
    }

    /// `app` under the embedding app's tokens.
    fn under_host(&self, app: Tokens) -> Tokens {
        match &self.host {
            Some(host) => host.over(&app),
            None => app,
        }
    }
}

impl Tokens {
    /// The color named `name`.
    pub fn color(&self, name: &str) -> Option<AdaptiveColor> {
        self.colors.get(name).copied()
    }

    /// Sets the color named `name`.
    pub fn set_color(&mut self, name: &str, color: AdaptiveColor) {
        self.colors.insert(name.into(), color);
    }

    /// The number named `name` (its light value, the only one of most
    /// numbers).
    pub fn number(&self, name: &str) -> Option<f32> {
        self.numbers.get(name).copied()
    }

    /// The number named `name` in the dark appearance (`dark`) or the
    /// light one: its dark value when it has one of its own, else the one
    /// value it has.
    pub fn number_in(&self, name: &str, dark: bool) -> Option<f32> {
        dark.then(|| self.dark_numbers.get(name).copied())
            .flatten()
            .or_else(|| self.number(name))
    }

    /// Sets the number named `name`, in both appearances (a dark value it
    /// had goes).
    pub fn set_number(&mut self, name: &str, value: f32) {
        self.numbers.insert(name.into(), value);
        self.dark_numbers.remove(name);
    }

    /// Gives the number named `name` a value of its own in the dark
    /// appearance (its light one is [`Tokens::set_number`]'s).
    pub fn set_dark_number(&mut self, name: &str, value: f32) {
        self.dark_numbers.insert(name.into(), value);
    }

    /// The font family named `name` ([`FONT_SANS`], [`FONT_MONO`], ...).
    pub fn font(&self, name: &str) -> Option<&Arc<str>> {
        self.fonts.get(name)
    }

    /// Sets the font family named `name`.
    pub fn set_font(&mut self, name: &str, family: impl Into<Arc<str>>) {
        self.fonts.insert(name.into(), family.into());
    }

    /// The box shadows named `name` (`shadow-raised`).
    pub fn shadow(&self, name: &str) -> Option<&AdaptiveShadow> {
        self.shadows.get(name)
    }

    /// Sets the box shadows named `name`.
    pub fn set_shadow(&mut self, name: &str, shadow: impl Into<AdaptiveShadow>) {
        self.shadows.insert(name.into(), shadow.into());
    }

    /// Each shadow by name.
    pub fn shadows(&self) -> impl Iterator<Item = (&str, &AdaptiveShadow)> {
        self.shadows.iter().map(|(k, s)| (&**k, s))
    }

    /// The token named `name`, of any kind (a color first, as classes take
    /// it).
    pub fn get(&self, name: &str) -> Option<Token> {
        if let Some(c) = self.colors.get(name) {
            return Some(Token::Color(*c));
        }
        if let Some(n) = self.numbers.get(name) {
            return Some(Token::Number(*n));
        }
        if let Some(f) = self.fonts.get(name) {
            return Some(Token::Font(f.clone()));
        }
        self.shadows.get(name).cloned().map(Token::Shadow)
    }

    /// These tokens with `roles` mapped onto the tokens named (over the
    /// roles they map already): [`Tokens::role_color`] and the others
    /// read a role's token through them.
    pub fn with_roles<S: Into<Box<str>>>(
        mut self,
        roles: impl IntoIterator<Item = (Role, S)>,
    ) -> Self {
        let mut map = self.roles.as_deref().cloned().unwrap_or_default();
        map.extend(roles.into_iter().map(|(role, name)| (role, name.into())));
        self.roles = (!map.is_empty()).then(|| Arc::new(map));
        self
    }

    /// Each role mapped onto a token of another name, with that name.
    pub fn roles(&self) -> impl Iterator<Item = (Role, &str)> {
        self.roles
            .iter()
            .flat_map(|r| r.iter())
            .map(|(role, name)| (*role, &**name))
    }

    /// The name of the token `role` reads: the one a theme mapped it onto,
    /// else its own ([`Role::name`]).
    pub fn role_name(&self, role: Role) -> &str {
        self.roles
            .as_ref()
            .and_then(|r| r.get(&role))
            .map_or(role.name(), |name| name)
    }

    /// The color `role` draws with: its token's ([`Tokens::role_name`]),
    /// else the one of its own name.
    pub fn role_color(&self, role: Role) -> Option<AdaptiveColor> {
        self.color(self.role_name(role))
            .or_else(|| self.color(role.name()))
    }

    /// The number `role` draws with, as [`Tokens::role_color`].
    pub fn role_number(&self, role: Role) -> Option<f32> {
        self.number(self.role_name(role))
            .or_else(|| self.number(role.name()))
    }

    /// The font family `role` draws with, as [`Tokens::role_color`].
    pub fn role_font(&self, role: Role) -> Option<&Arc<str>> {
        self.font(self.role_name(role))
            .or_else(|| self.font(role.name()))
    }

    /// Whether the app is drawn with a theme of its own: tokens other than
    /// its UI set's defaults (`nsh-base`'s theme), its own or an embedding
    /// app's. The core says so on the tokens it draws with, which the
    /// platforms get (`Command::Tokens`): their own controls take the
    /// theme's colors only then (iOS's and Android's alerts), and keep the
    /// system's for an app without one. `false` on tokens made elsewhere.
    pub fn is_themed(&self) -> bool {
        self.themed
    }

    /// Says whether the app has a theme of its own ([`Tokens::is_themed`]).
    pub fn set_themed(&mut self, themed: bool) {
        self.themed = themed;
    }

    /// Whether it has no token.
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
            && self.numbers.is_empty()
            && self.dark_numbers.is_empty()
            && self.fonts.is_empty()
            && self.shadows.is_empty()
    }

    /// Each color by name.
    pub fn colors(&self) -> impl Iterator<Item = (&str, AdaptiveColor)> {
        self.colors.iter().map(|(k, c)| (&**k, *c))
    }

    /// Each number by name.
    pub fn numbers(&self) -> impl Iterator<Item = (&str, f32)> {
        self.numbers.iter().map(|(k, n)| (&**k, *n))
    }

    /// Each number's dark value of its own, by name.
    pub fn dark_numbers(&self) -> impl Iterator<Item = (&str, f32)> {
        self.dark_numbers.iter().map(|(k, n)| (&**k, *n))
    }

    /// Each font family by name.
    pub fn fonts(&self) -> impl Iterator<Item = (&str, &Arc<str>)> {
        self.fonts.iter().map(|(k, f)| (&**k, f))
    }

    /// Makes token `name` what it is in `from`, of whatever kind, or
    /// removes it when `from` has none of that name.
    pub(crate) fn copy_named(&mut self, from: &Tokens, name: &str) {
        fn copy<V: Clone>(
            to: &mut FxHashMap<Box<str>, V>,
            from: &FxHashMap<Box<str>, V>,
            name: &str,
        ) {
            match from.get(name) {
                Some(value) => {
                    to.insert(name.into(), value.clone());
                }
                None => {
                    to.remove(name);
                }
            }
        }
        copy(&mut self.colors, &from.colors, name);
        copy(&mut self.numbers, &from.numbers, name);
        copy(&mut self.dark_numbers, &from.dark_numbers, name);
        copy(&mut self.fonts, &from.fonts, name);
        copy(&mut self.shadows, &from.shadows, name);
    }

    /// `self` with the tokens of `base` it does not have.
    pub fn over(&self, base: &Tokens) -> Tokens {
        let mut out = base.clone();
        out.colors
            .extend(self.colors.iter().map(|(k, c)| (k.clone(), *c)));
        // A number of `self` is its values: a dark one of `base`'s goes
        // unless `self` has its own.
        for (k, n) in &self.numbers {
            out.numbers.insert(k.clone(), *n);
            out.dark_numbers.remove(k);
        }
        out.dark_numbers
            .extend(self.dark_numbers.iter().map(|(k, n)| (k.clone(), *n)));
        out.fonts
            .extend(self.fonts.iter().map(|(k, f)| (k.clone(), f.clone())));
        out.shadows
            .extend(self.shadows.iter().map(|(k, s)| (k.clone(), s.clone())));
        if let Some(roles) = &self.roles {
            out.roles = match out.roles {
                Some(base) => {
                    let mut map = Arc::unwrap_or_clone(base);
                    map.extend(roles.iter().map(|(r, n)| (*r, n.clone())));
                    Some(Arc::new(map))
                }
                None => Some(roles.clone()),
            };
        }
        out
    }
}

/// Tokens as apps have them (`nsh-base`'s default theme's and a few of an
/// app's own), for the core's tests.
#[cfg(test)]
pub(crate) fn sample() -> Tokens {
    let mut tokens = Tokens::default();
    for (key, light, dark) in [
        ("background", 0xffffff, 0x0a0a0a),
        ("foreground", 0x0a0a0a, 0xfafafa),
        ("border", 0xe5e5e5, 0x262626),
        ("ring", 0xa3a3a3, 0x525252),
        ("muted", 0xf5f5f5, 0x262626),
        ("muted-foreground", 0x737373, 0xa3a3a3),
        ("accent", 0xf5f5f5, 0x262626),
        ("primary", 0x171717, 0xfafafa),
        ("primary-active", 0x0a0a0a, 0xe5e5e5),
        ("primary-foreground", 0xfafafa, 0x171717),
        ("popover", 0xffffff, 0x0a0a0a),
        ("popover-foreground", 0x0a0a0a, 0xfafafa),
    ] {
        tokens.set_color(key, AdaptiveColor::rgb(light, dark));
    }
    for (key, n) in [("radius", 12.0), ("radius-lg", 16.0), ("font-size", 16.0)] {
        tokens.set_number(key, n);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::rgb;

    #[test]
    fn tokens_over_others_win_and_keep_the_rest() {
        let mut base = Tokens::default();
        base.set_color("primary", rgb(0x111111).into());
        base.set_color("ring", rgb(0x222222).into());
        base.set_number("radius", 12.0);
        base.set_font(FONT_SANS, "Inter");
        let mut top = Tokens::default();
        top.set_color("primary", rgb(0x333333).into());
        top.set_number("radius-card", 10.0);
        top.set_font(FONT_MONO, "Iosevka");
        let tokens = top.over(&base);
        assert_eq!(tokens.color("primary"), Some(rgb(0x333333).into()));
        assert_eq!(tokens.color("ring"), Some(rgb(0x222222).into()));
        assert_eq!(tokens.number("radius"), Some(12.0));
        assert_eq!(tokens.number("radius-card"), Some(10.0));
        assert_eq!(tokens.font(FONT_SANS).map(|f| &**f), Some("Inter"));
        assert_eq!(tokens.font(FONT_MONO).map(|f| &**f), Some("Iosevka"));
        assert!(Tokens::default().is_empty());
    }
}

/// Roles: what NSH's own parts draw with, read through a theme's mapping.
#[cfg(test)]
mod theme_roles {
    use super::*;
    use crate::color::rgb;

    #[test]
    fn a_role_reads_the_token_of_its_name_without_a_mapping() {
        let tokens = sample();
        assert_eq!(tokens.role_color(Role::Popover), tokens.color("popover"));
        assert_eq!(tokens.role_number(Role::Radius), tokens.number("radius"));
        assert_eq!(
            tokens.role_name(Role::PopoverForeground),
            "popover-foreground"
        );
        assert_eq!(tokens.roles().count(), 0);
    }

    #[test]
    fn a_role_reads_the_token_a_theme_maps_it_onto() {
        let mut tokens = Tokens::default();
        tokens.set_color("surface-overlay", rgb(0x123456).into());
        tokens.set_color("popover", rgb(0xffffff).into());
        tokens.set_number("radius-control", 6.0);
        tokens.set_font("font-body", "Inter");
        let tokens = tokens.with_roles([
            (Role::Popover, "surface-overlay"),
            (Role::Radius, "radius-control"),
            (Role::FontSans, "font-body"),
            (Role::Primary, "action"),
        ]);
        assert_eq!(tokens.role_color(Role::Popover), Some(rgb(0x123456).into()));
        assert_eq!(tokens.role_number(Role::Radius), Some(6.0));
        assert_eq!(
            tokens.role_font(Role::FontSans).map(|f| &**f),
            Some("Inter")
        );
        // The token it maps onto is missing: the role's own name.
        assert_eq!(tokens.role_color(Role::Primary), None);
        // A role is no token.
        assert_eq!(
            tokens.get("popover"),
            Some(Token::Color(rgb(0xffffff).into()))
        );
        assert!(tokens.color("primary").is_none());
    }

    #[test]
    fn over_keeps_the_roles_of_both_and_the_tops_win() {
        let base = Tokens::default().with_roles([(Role::Popover, "a"), (Role::Card, "b")]);
        let top = Tokens::default().with_roles([(Role::Popover, "c")]);
        let tokens = top.over(&base);
        assert_eq!(tokens.role_name(Role::Popover), "c");
        assert_eq!(tokens.role_name(Role::Card), "b");
        let plain = Tokens::default().over(&base);
        assert_eq!(plain.role_name(Role::Card), "b");
    }

    #[test]
    fn names_are_shadcns_and_read_back() {
        for role in Role::ALL {
            assert_eq!(Role::from_name(role.name()), Some(*role));
        }
        assert_eq!(Role::Selection.name(), "selection");
        assert_eq!(Role::from_name("surface"), None);
    }

    #[test]
    fn being_themed_is_no_token() {
        let mut themed = sample();
        themed.set_themed(true);
        assert_eq!(themed, sample());
        assert!(!sample().is_themed());
    }
}
