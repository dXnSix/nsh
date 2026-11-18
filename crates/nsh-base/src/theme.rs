//! The theme of NSH apps' UI: shadcn/ui's design tokens, as roles. The
//! surfaces (`background`, `card`, `popover`, `primary`, `secondary`,
//! `muted`, `accent`), each with the color of what is drawn on it
//! (`-foreground`), `destructive`, `border`, `input`, `ring`, the charts'
//! colors (`chart-1` to `chart-5`), the sidebar's (`sidebar`, ...) and the
//! corner radius (`radius`) are the fields of [`Theme`], with its fonts,
//! Tailwind's `font-sans` and `font-mono`, which the default leaves to the
//! system. Each field reads its role ([`role`]).
//!
//! A theme is a theme document (`docs/dev/theme-format.md`). `nsh/base`'s
//! registry gives its default, [`DEFAULT_THEME`] (shadcn's `neutral`, its
//! dark values in the `colorMode` modifier's `dark` context), as the
//! fallback theme document (`Registry::fallback_theme_document`): the
//! active document of an app whose own theme and sets have none. An app's
//! theme (`nsh.json`'s `"theme"`) is a document of its own: it names its
//! tokens as it likes and maps `nsh/base`'s roles onto them (`roles`).
//!
//! The core keeps the active document's values as design tokens
//! ([`Tokens`]), which Tailwind classes name (`bg-primary`,
//! `rounded-(--radius)`) and TS reads (`token()` of `nsh`). [`theme()`]
//! reads each field through its role, in the active document alone, as any
//! library's roles resolve: the token the document maps it onto, else the
//! token of its name. A role that resolves to nothing reads transparent
//! for a color and 0 for `radius`. The components that draw a look of their
//! own read it: `Editor`, `TextView`, `RichEditor`, `DiffView`, `Terminal`,
//! `Toast`, the scroll bar and the terminal looks of the controls.

use std::{
    cell::{OnceCell, RefCell},
    rc::Rc,
    sync::{Arc, LazyLock},
};

use nsh_core::{
    color::{Hsla, transparent_black},
    element::{
        Memo, Owner, Patch, PatchValues, Role, RoleValue, SharedString, Tokens, computed,
        scope_tokens, try_global, untracked,
    },
    geometry::{Points, px},
};

/// The default theme document (`docs/dev/theme-format.md`): shadcn/ui's
/// `neutral` theme, light by default, with its dark values in the
/// `colorMode` modifier's `dark` context. Converted once from shadcn's
/// `cssVars` file with `nsh theme convert`; `nsh/base` registers it as the
/// fallback theme document.
pub const DEFAULT_THEME: &str = include_str!("default-theme.json");

/// The default theme: [`DEFAULT_THEME`] in its default contexts (light).
static DEFAULT: LazyLock<Theme> = LazyLock::new(|| {
    let tokens = Arc::new(resolve(DEFAULT_THEME));
    Theme::read(&*tokens, tokens.clone())
});

/// A theme file that does not parse.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeError(pub String);

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeError {}

/// The theme the app is drawn with (reading it subscribes: a new theme
/// reaches what read it). Each field is its role's value in the active
/// document ([`role`]): the token the document maps the role onto, else the
/// token of its name; transparent for a color and 0 for `radius` when it
/// resolves to nothing.
///
/// Outside a [`nsh_core::element::ThemeScope`] it is a memo over its role
/// reads: what read it runs again when the value or the mapping of one of
/// its roles changes, and not when a token none of them reads does. In a
/// scope it is the scope's, made once per change of the scope's tokens.
/// Outside an engine (a unit test of a component's model) it is the
/// default theme.
pub fn theme() -> Arc<Theme> {
    let Some(surface) = surface_theme() else {
        return Arc::new(Theme::default());
    };
    match scope_tokens() {
        Some(tokens) => scoped(tokens),
        None => surface.get(),
    }
}

/// The surface's theme memo, made on its first read (`None` outside an
/// engine).
#[derive(Default)]
struct SurfaceTheme(Rc<OnceCell<Memo<Arc<Theme>>>>);

/// The memo of the surface's theme: its fields read through their roles
/// ([`nsh_core::element::role()`]), each read subscribing to its token and
/// the role mappings alone. Made once per surface, under no owner, so no
/// scope is around its reads.
fn surface_theme() -> Option<Memo<Arc<Theme>>> {
    let cell = try_global::<SurfaceTheme>()?.with_untracked(|g| g.0.clone());
    Some(*cell.get_or_init(|| {
        Owner::detached().run(|| {
            computed(|| {
                let tokens = untracked(nsh_core::element::tokens);
                Arc::new(Theme::read(&Ambient, tokens))
            })
        })
    }))
}

thread_local! {
    /// The theme of the scope tokens last read: [`theme()`] makes one only
    /// when they change.
    static SCOPED: RefCell<Option<Arc<Theme>>> = const { RefCell::new(None) };
}

/// The theme of a scope's `tokens`.
fn scoped(tokens: Arc<Tokens>) -> Arc<Theme> {
    SCOPED.with_borrow_mut(|current| match current {
        Some(theme) if Arc::ptr_eq(&theme.tokens, &tokens) => theme.clone(),
        _ => {
            let theme = Arc::new(Theme::read(&*tokens, tokens.clone()));
            *current = Some(theme.clone());
            theme
        }
    })
}

/// What a theme's fields are read from: tokens, or the surface's ambient
/// role reads.
trait ReadRole {
    /// What `role` reads, `None` when it resolves to nothing.
    fn read<V: RoleValue>(&self, role: Role<V>) -> Option<V>;
}

impl ReadRole for Tokens {
    fn read<V: RoleValue>(&self, role: Role<V>) -> Option<V> {
        self.role(role)
    }
}

/// The surface's roles, each read subscribing to its token alone.
struct Ambient;

impl ReadRole for Ambient {
    fn read<V: RoleValue>(&self, role: Role<V>) -> Option<V> {
        nsh_core::element::role(role)
    }
}

/// What a color role that resolves to nothing reads.
const TRANSPARENT: Hsla = transparent_black();

macro_rules! theme {
    ($( $(#[doc = $doc:literal])* $name:ident = $key:literal => $role:ident ),* $(,)?) => {
        /// The roles `nsh-base`'s theme and components draw with: shadcn/ui's
        /// tokens and the components' own colors, each declared with its
        /// value type, none required. An app's theme document maps them
        /// onto its tokens (its `roles`); without a mapping each reads the
        /// token of its name in the active document. The ones the core and
        /// the platforms' own controls draw with too
        /// ([`nsh_core::element::role`]: `POPOVER`, `PRIMARY`, …) are the
        /// same roles, as a role is its name.
        pub mod role {
            use nsh_core::element::FontRole;

            /// The roles of the theme's colors.
            mod colors {
                use nsh_core::element::Role;

                #[allow(unused_imports)]
                use super::super::Theme;

                nsh_core::roles! {
                    $( $(#[doc = $doc])* pub $role: Role = $key, )*
                }
            }
            pub use colors::{ $( $role ),* };

            /// The app's default font family.
            ///
            /// Name `font-sans`.
            pub const FONT_SANS: FontRole = nsh_core::element::role::FONT_SANS;

            /// The roles beside the theme's fields.
            mod more {
                use nsh_core::element::{FontRole, NumberRole, Role};

                #[allow(unused_imports)]
                use super::super::Theme;

                nsh_core::roles! {
                    /// The corner radius.
                    ///
                    /// Name `radius`.
                    pub RADIUS: NumberRole = "radius",
                    /// The monospaced font family.
                    ///
                    /// Name `font-mono`.
                    pub FONT_MONO: FontRole = "font-mono",
                    /// The highlight of a selection ([`Theme::selection`] draws
                    /// `primary` at 22 % when the theme has none).
                    ///
                    /// Name `selection`.
                    pub SELECTION: Role = "selection",
                    /// What succeeded: a `[!TIP]` alert in a `TextView`. No token
                    /// of the default theme: the component draws its own green
                    /// when the theme has none.
                    ///
                    /// Name `success`.
                    pub SUCCESS: Role = "success",
                    /// What to watch out for: an `Editor`'s warning diagnostics, a
                    /// `[!WARNING]` alert in a `TextView`. No token of the default
                    /// theme: each draws its own amber when the theme has none.
                    ///
                    /// Name `warning`.
                    pub WARNING: Role = "warning",
                    /// What is worth knowing: an `Editor`'s info diagnostics, a
                    /// `[!NOTE]` alert in a `TextView`. No token of the default
                    /// theme: each draws its own blue when the theme has none.
                    ///
                    /// Name `info`.
                    pub INFO: Role = "info",
                    /// What is marked: an `Editor`'s search matches, marked text
                    /// (`<mark>`) in a `TextView`. No token of the
                    /// default theme: each draws its own yellow when the theme has
                    /// none. The components draw it translucent; the token is the
                    /// solid color.
                    ///
                    /// Name `highlight`.
                    pub HIGHLIGHT: Role = "highlight",
                    /// A diff's added lines: a `DiffView`'s row tint, indicators,
                    /// changed words and file counts. No token of the default
                    /// theme: the view draws its own green when the theme has
                    /// none. The view draws the tints translucent; the token is
                    /// the solid color.
                    ///
                    /// Name `diff-added`.
                    pub DIFF_ADDED: Role = "diff-added",
                    /// A diff's deleted lines, as [`DIFF_ADDED`] its added ones:
                    /// the view draws its own red when the theme has none.
                    ///
                    /// Name `diff-deleted`.
                    pub DIFF_DELETED: Role = "diff-deleted",
                    /// Comments in highlighted code: an `Editor`'s, a `DiffView`'s
                    /// and a `TextView`'s code blocks'. No token of the default
                    /// theme, as none of the `SYNTAX_*` roles has: the highlighter
                    /// draws its own palette where the theme has none.
                    ///
                    /// Name `syntax-comment`.
                    pub SYNTAX_COMMENT: Role = "syntax-comment",
                    /// Keywords, the words of operators, headings.
                    ///
                    /// Name `syntax-keyword`.
                    pub SYNTAX_KEYWORD: Role = "syntax-keyword",
                    /// Strings, regular expressions, escapes.
                    ///
                    /// Name `syntax-string`.
                    pub SYNTAX_STRING: Role = "syntax-string",
                    /// Numbers, constants and literals.
                    ///
                    /// Name `syntax-number`.
                    pub SYNTAX_NUMBER: Role = "syntax-number",
                    /// Names of functions and classes.
                    ///
                    /// Name `syntax-function`.
                    pub SYNTAX_FUNCTION: Role = "syntax-function",
                    /// Attributes, parameters, properties and keys.
                    ///
                    /// Name `syntax-property`.
                    pub SYNTAX_PROPERTY: Role = "syntax-property",
                    /// Types, built-ins and tags.
                    ///
                    /// Name `syntax-type`.
                    pub SYNTAX_TYPE: Role = "syntax-type",
                }
            }
            pub use more::{
                RADIUS, FONT_MONO, SELECTION, SUCCESS, WARNING, INFO, HIGHLIGHT, DIFF_ADDED, DIFF_DELETED, SYNTAX_COMMENT, SYNTAX_KEYWORD, SYNTAX_STRING, SYNTAX_NUMBER, SYNTAX_FUNCTION, SYNTAX_PROPERTY, SYNTAX_TYPE,
            };

            /// Every role `nsh-base`'s theme and components draw with, with
            /// its value type and doc: what its registry lists for the
            /// toolchain and the core's checks (`Registry::roles`), which
            /// types TS's `role()` from them.
            pub fn all() -> Vec<nsh_core::element::RoleSpec> {
                let mut all = colors::all();
                all.push(FONT_SANS.declaration(" The app's default font family.\n", false));
                all.extend(more::all());
                all
            }
        }

        /// A theme: shadcn/ui's colors, its radius and fonts, each a single
        /// value read through its role ([`role`]), and the tokens it was
        /// made of, which [`Theme::color`], [`Theme::number`] and
        /// [`Theme::font`] read by name. Two themes are equal when their
        /// fields are.
        #[derive(Debug, Clone)]
        #[non_exhaustive]
        pub struct Theme {
            $(
                $(#[doc = $doc])*
                #[doc = concat!("\n\nRole `", $key, "`.")]
                pub $name: Hsla,
            )*
            /// The corner radius (0 when its role resolves to nothing).
            ///
            /// Role `radius`.
            pub radius: Points,
            /// The app's font family, which text is drawn with unless it
            /// sets its own (`None`: the system's): Tailwind's `--font-sans`,
            /// which a theme may set.
            ///
            /// Role `font-sans`.
            pub font_sans: Option<SharedString>,
            /// The monospaced font family (`None`: the system's): Tailwind's
            /// `--font-mono`.
            ///
            /// Role `font-mono`.
            pub font_mono: Option<SharedString>,
            /// The `selection` role's color, if it resolves to one.
            selection: Option<Hsla>,
            tokens: Arc<Tokens>,
        }

        impl Theme {
            /// The theme whose fields `roles` reads, made of `tokens`.
            fn read(roles: &impl ReadRole, tokens: Arc<Tokens>) -> Self {
                let color = |role: Role| roles.read(role).unwrap_or(TRANSPARENT);
                Theme {
                    $( $name: color(role::$role), )*
                    radius: px(roles.read(role::RADIUS).unwrap_or(0.0)),
                    font_sans: roles.read(role::FONT_SANS).map(SharedString::from),
                    font_mono: roles.read(role::FONT_MONO).map(SharedString::from),
                    selection: roles.read(role::SELECTION),
                    tokens,
                }
            }
        }

        impl PartialEq for Theme {
            fn eq(&self, other: &Self) -> bool {
                $( self.$name == other.$name && )*
                    self.radius == other.radius
                    && self.font_sans == other.font_sans
                    && self.font_mono == other.font_mono
                    && self.selection == other.selection
            }
        }
    };
}

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
    /// The default theme ([`DEFAULT_THEME`]) in its default contexts
    /// (light).
    fn default() -> Self {
        DEFAULT.clone()
    }
}

impl Theme {
    /// The color named `name` in the tokens the theme was made of (any the
    /// document declares). [`theme()`]'s follows its roles alone: read a
    /// token no role reads where it is used, with
    /// [`nsh_core::element::token`].
    pub fn color(&self, name: &str) -> Option<Hsla> {
        self.tokens.color(name)
    }

    /// The highlight of a selection, the one every component draws: the
    /// color the `selection` role reads if it resolves to one, else
    /// `primary` at 22 %.
    pub fn selection(&self) -> Hsla {
        self.selection.unwrap_or_else(|| self.primary.opacity(0.22))
    }

    /// The number named `name` in the tokens the theme was made of.
    pub fn number(&self, name: &str) -> Option<f32> {
        self.tokens.number(name)
    }

    /// The font family named `name` in the tokens the theme was made of
    /// (its sans and mono ones are [`Theme::font_sans`] and
    /// [`Theme::font_mono`]).
    pub fn font(&self, name: &str) -> Option<Arc<str>> {
        self.tokens.font(name).cloned()
    }

    /// Sets the color token `name` (a field when a role reads it).
    pub fn set_color(&mut self, name: &str, color: Hsla) {
        self.edit(Patch::new().value(name, color));
    }

    /// Sets the number token `name` (`radius` sets its field).
    pub fn set_number(&mut self, name: &str, value: f32) {
        self.edit(Patch::new().value(name, value));
    }

    /// Sets the font family token `name`.
    pub fn set_font(&mut self, name: &str, family: impl Into<Arc<str>>) {
        self.edit(Patch::new().value(name, family.into()));
    }

    /// The design tokens it was made of.
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// The design tokens it was made of.
    pub fn into_tokens(self) -> Tokens {
        Arc::unwrap_or_clone(self.tokens)
    }

    /// Changes the tokens with `patch`, and the fields with them: its
    /// tokens resolved again with `patch` after every patch they were
    /// resolved with, so earlier edits hold. A patch never adds a token the
    /// document doesn't declare.
    fn edit(&mut self, patch: Patch) {
        let (tokens, _) = self.tokens.scoped(None, Some(patch));
        *self = Self::read(&*tokens, tokens.clone());
    }
}

impl From<Theme> for Tokens {
    fn from(theme: Theme) -> Self {
        theme.into_tokens()
    }
}

/// The tokens of theme document `text`, its default contexts picked.
fn resolve(text: &str) -> Tokens {
    let read = nsh_theme::read(text);
    Tokens::resolve(Arc::new(read.document), Default::default(), Vec::new()).0
}

/// The roles `nsh-base` lists for the toolchain ([`role::all`]): every
/// constant of [`role`], once, with what it reads and its doc.
#[cfg(test)]
mod listed_roles {
    use nsh_core::element::{ValueType, description::SetDescription};

    use super::role;

    #[test]
    fn every_role_is_listed_once_with_its_kind() {
        let all = role::all();
        let mut names: Vec<&str> = all.iter().map(|spec| spec.name.as_str()).collect();
        // The theme's 31 colors, its radius and two fonts, the selection,
        // and the 13 roles of its components' own colors.
        assert_eq!(names.len(), 31 + 3 + 1 + 13);
        names.sort_unstable();
        let listed = names.len();
        names.dedup();
        assert_eq!(names.len(), listed, "a role is listed twice");
        let kind = |name: &str| {
            all.iter()
                .find(|spec| spec.name == name)
                .map(|spec| spec.value_type)
        };
        assert_eq!(kind(role::POPOVER.name()), Some(ValueType::Color));
        assert_eq!(kind(role::WARNING.name()), Some(ValueType::Color));
        assert_eq!(kind(role::SYNTAX_KEYWORD.name()), Some(ValueType::Color));
        assert_eq!(kind(role::RADIUS.name()), Some(ValueType::Number));
        assert_eq!(kind(role::FONT_SANS.name()), Some(ValueType::Font));
        assert_eq!(kind(role::FONT_MONO.name()), Some(ValueType::Font));
        // Each says what it is for.
        assert!(all.iter().all(|spec| spec.doc.is_some()));
    }

    #[test]
    fn the_registry_describes_them() {
        let described = SetDescription::of(&crate::registry()).unwrap();
        assert_eq!(described.roles.len(), role::all().len());
        let popover = described
            .roles
            .iter()
            .find(|r| r.name == "popover")
            .unwrap();
        assert_eq!(popover.value_type, ValueType::Color);
        let doc = popover.doc.as_deref().unwrap_or_default();
        assert!(!doc.contains("Name `"), "{doc}");
        let warning = described
            .roles
            .iter()
            .find(|r| r.name == "warning")
            .unwrap();
        let doc = warning.doc.as_deref().unwrap_or_default();
        assert!(doc.starts_with("What to watch out for"), "{doc}");
    }
}
