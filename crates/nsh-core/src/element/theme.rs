//! Theme types: an app's or a design system's own design tokens as a Rust
//! type ([`AppTheme`]), registered by its UI set ([`super::Registry::theme`])
//! and read where it draws ([`theme_of`]). Several sets may each register
//! types; their token names merge into one theme, which replaces the default
//! token source (`nsh-base`'s shadcn theme). A type maps [`Role`]s, what
//! NSH's own parts draw with, onto its tokens, so `nsh-base`'s components,
//! the platforms' menus and alerts draw in it.
//!
//! The tokens stay flat and by name, as every theme's are: classes name them
//! (`bg-surface-raised`, `rounded-card`), TS reads them with `token()`, the
//! app's theme file (`nsh.json` `"theme"`) sets them by name, and a type is a
//! typed view of its names, read once per change of the tokens.

use std::{
    any::{Any, TypeId},
    borrow::Cow,
    sync::Arc,
};

use rustc_hash::FxHashMap;

use super::{AnyRole, Role, RoleKind, RoleValue, Tokens};

/// A theme type: design tokens as the fields of a Rust type. Its token
/// names are [`AppTheme::token_specs`]'; a field of a nested type is that
/// type's names under a prefix, after their kind's namespace
/// ([`TokenKind::prefixed`]: `surface-` with `raised` is `surface-raised`,
/// with `shadow-raised` `shadow-surface-raised`).
/// `#[derive(AppTheme)]` writes it of a struct's fields; written by hand,
/// its four methods agree on the names.
///
/// ```
/// use nsh_core::{
///     element::{AdaptiveColor, AppTheme, role},
///     geometry::Points,
/// };
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     /// Popovers and menus.
///     #[token(color, role = role::POPOVER)]
///     overlay: AdaptiveColor,
///     #[token(radius)]
///     card: Points,
/// }
///
/// let names: Vec<_> = Ds::token_specs()
///     .iter()
///     .map(|s| s.name.to_string())
///     .collect();
/// assert_eq!(names, ["overlay", "radius-card"]);
/// ```
///
/// A role is a constant of the crate that draws with it, named by its
/// path: the core's ([`super::role`]), a UI set's (`nsh_base::role`), or a
/// component library's own. A token plays several roles with a `role` for
/// each: one surface for cards and popovers.
///
/// ```
/// use nsh_core::element::{AdaptiveColor, AppTheme, Role, role};
///
/// /// What a library's cards draw on.
/// const CARD: Role = Role::new("card");
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Flat {
///     #[token(color, role = CARD, role = role::POPOVER)]
///     surface: AdaptiveColor,
/// }
///
/// let roles: Vec<_> = Flat::token_specs()[0]
///     .roles
///     .iter()
///     .map(|r| r.name())
///     .collect();
/// assert_eq!(roles, ["card", "popover"]);
/// ```
///
/// The derive refuses, at the attribute, a name given twice in one type:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color)]
///     ink: AdaptiveColor,
///     #[token(color, name = "ink")]
///     other: AdaptiveColor,
/// }
/// ```
///
/// a kind that does not fit the field's type:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(radius)]
///     card: AdaptiveColor,
/// }
/// ```
///
/// a kind there is not:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(colour)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// a role's name in place of its constant:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, role = "popover")]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// and the compiler a role there is not:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme, role};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, role = role::SURFACE)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// and a role that reads another kind than the token holds (a radius is a
/// number):
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme, NumberRole, Role};
///
/// const RADIUS: NumberRole = Role::new("radius");
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, role = RADIUS)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// The derive refuses a role on a shadow token, which no role reads:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveShadow, AppTheme, role};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(shadow, role = role::POPOVER)]
///     raised: AdaptiveShadow,
/// }
/// ```
///
/// a role mapped twice, onto two tokens:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme, role};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, role = role::POPOVER)]
///     ink: AdaptiveColor,
///     #[token(color, role = role::POPOVER)]
///     paper: AdaptiveColor,
/// }
/// ```
///
/// or twice onto one:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme, role};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, role = role::POPOVER, role = role::POPOVER)]
///     paper: AdaptiveColor,
/// }
/// ```
///
/// a name of the wrong form for its kind (only a font is `font-*`):
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color)]
///     font_accent: AdaptiveColor,
/// }
/// ```
///
/// an empty name:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, name = "")]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// two kinds:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color, shadow)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// a `name` or `role` on a field that is no token:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme, role};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(skip, role = role::POPOVER)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// a kind with `nested`, `flatten` or `skip`:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(skip, color)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// two of `nested`, `flatten` and `skip`:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(nested, skip)]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// a second `#[token]`:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(color)]
///     #[token(color, name = "paper")]
///     ink: AdaptiveColor,
/// }
/// ```
///
/// a nested field whose prefix gives its names another kind's form
/// (`font-…`, `weight-…`):
///
/// ```compile_fail
/// use nsh_core::element::AppTheme;
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Fonts {
///     #[token(font)]
///     body: String,
/// }
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(nested)]
///     weight: Fonts,
/// }
/// ```
///
/// ```compile_fail
/// use nsh_core::element::AppTheme;
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Fonts {
///     #[token(font)]
///     body: String,
/// }
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(nested)]
///     font: Fonts,
/// }
/// ```
///
/// a namespaced kind's name outside its namespace:
///
/// ```compile_fail
/// use nsh_core::{element::AppTheme, geometry::Points};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds {
///     #[token(radius, name = "card")]
///     card: Points,
/// }
/// ```
///
/// and a generic struct:
///
/// ```compile_fail
/// use nsh_core::element::{AdaptiveColor, AppTheme};
///
/// #[derive(Clone, PartialEq, Default, AppTheme)]
/// struct Ds<T: Clone + PartialEq + Default + 'static> {
///     #[token(skip)]
///     extra: T,
///     #[token(color)]
///     ink: AdaptiveColor,
/// }
/// ```
pub trait AppTheme: Clone + PartialEq + Default + 'static {
    /// Some of its fields, each `None` for "as it is" ([`update_theme`]).
    type Patch: Default + Clone + 'static;

    /// Its tokens: names (under no prefix), kinds, docs and the roles they
    /// play, flattened through nested types.
    fn token_specs() -> &'static [TokenSpec];

    /// Writes its tokens into `out`, each name under `prefix`
    /// ([`TokenKind::prefixed`]).
    fn write_tokens(&self, prefix: &str, out: &mut Tokens);

    /// The theme of `tokens`' names under `prefix`; a name they lack keeps
    /// [`Default`]'s value (a nested type's field, the nested type's
    /// `Default`).
    fn read_tokens(tokens: &Tokens, prefix: &str) -> Self;

    /// Writes the fields `patch` sets into `out`, each name under `prefix`.
    fn write_patch(patch: &Self::Patch, prefix: &str, out: &mut Tokens);

    /// Its tokens, with the roles its specs map onto them.
    fn to_tokens(&self) -> Tokens {
        let mut out = Tokens::default();
        self.write_tokens("", &mut out);
        out.with_roles(roles_of(Self::token_specs()))
    }
}

/// One token of a theme type: its name, kind, doc and the roles it plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSpec {
    /// Its name (`surface-raised`, `radius-card`).
    pub name: Cow<'static, str>,
    /// What it holds.
    pub kind: TokenKind,
    /// Its doc comment (the field's), for the toolchain's declarations.
    pub doc: &'static str,
    /// The roles it plays (`role::POPOVER`), if any. A design system with
    /// one surface for cards and popovers has one token play both, so both
    /// follow every change of it; a role is played by one token.
    pub roles: Vec<AnyRole>,
}

impl TokenSpec {
    /// A token named `name` of `kind`.
    pub fn new(name: impl Into<Cow<'static, str>>, kind: TokenKind) -> Self {
        Self {
            name: name.into(),
            kind,
            doc: "",
            roles: Vec::new(),
        }
    }

    /// With the doc comment `doc`.
    pub fn doc(mut self, doc: &'static str) -> Self {
        self.doc = doc;
        self
    }

    /// Playing `role`, beside the roles it plays already. The role reads
    /// what the token holds: a color token plays a [`Role`], a number a
    /// [`super::NumberRole`], a font a [`super::FontRole`] (registering a
    /// type whose token plays a role of another kind panics).
    pub fn role<V: RoleValue>(mut self, role: Role<V>) -> Self {
        let role = AnyRole::from(role);
        if !self.roles.contains(&role) {
            self.roles.push(role);
        }
        self
    }
}

/// What a token holds, which decides how a theme file's value is read and
/// how TS types it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenKind {
    /// A color per appearance.
    Color,
    /// A number (a unitless quantity, or a length in points).
    Number,
    /// A corner radius (`radius-*`: `rounded-*`).
    Radius,
    /// A spacing (`spacing-*`: `p-*`, `gap-*`, …).
    Spacing,
    /// A font size (`text-*`).
    Text,
    /// A font weight (`font-weight-*`: `font-*`).
    Weight,
    /// A letter spacing in points (`tracking-*`).
    Tracking,
    /// A line height (`leading-*`).
    Leading,
    /// A blur radius (`blur-*`).
    Blur,
    /// A font family.
    Font,
    /// Box shadows per appearance (`shadow-*`).
    Shadow,
}

impl TokenKind {
    /// Its name, as the toolchain writes it.
    pub fn name(self) -> &'static str {
        match self {
            TokenKind::Color => "color",
            TokenKind::Number => "number",
            TokenKind::Radius => "radius",
            TokenKind::Spacing => "spacing",
            TokenKind::Text => "text",
            TokenKind::Weight => "weight",
            TokenKind::Tracking => "tracking",
            TokenKind::Leading => "leading",
            TokenKind::Blur => "blur",
            TokenKind::Font => "font",
            TokenKind::Shadow => "shadow",
        }
    }

    /// The kind of this name ([`TokenKind::name`]).
    pub fn from_name(name: &str) -> Option<TokenKind> {
        Some(match name {
            "color" => TokenKind::Color,
            "number" => TokenKind::Number,
            "radius" => TokenKind::Radius,
            "spacing" => TokenKind::Spacing,
            "text" => TokenKind::Text,
            "weight" => TokenKind::Weight,
            "tracking" => TokenKind::Tracking,
            "leading" => TokenKind::Leading,
            "blur" => TokenKind::Blur,
            "font" => TokenKind::Font,
            "shadow" => TokenKind::Shadow,
            _ => return None,
        })
    }

    /// What a role its token plays reads (`None`: no role reads box
    /// shadows).
    pub(crate) fn role_kind(self) -> Option<RoleKind> {
        match self {
            TokenKind::Color => Some(RoleKind::Color),
            TokenKind::Font => Some(RoleKind::Font),
            TokenKind::Shadow => None,
            _ => Some(RoleKind::Number),
        }
    }

    /// Whether its value is a number token.
    pub fn is_number(self) -> bool {
        !matches!(self, TokenKind::Color | TokenKind::Font | TokenKind::Shadow)
    }

    /// The namespace its names take, which its classes read them by
    /// (`radius-`: `rounded-*`); a color's and a number's is none.
    pub fn namespace(self) -> &'static str {
        match self {
            TokenKind::Color | TokenKind::Number => "",
            TokenKind::Radius => "radius-",
            TokenKind::Spacing => "spacing-",
            TokenKind::Text => "text-",
            TokenKind::Weight => "font-weight-",
            TokenKind::Tracking => "tracking-",
            TokenKind::Leading => "leading-",
            TokenKind::Blur => "blur-",
            TokenKind::Font => "font-",
            TokenKind::Shadow => "shadow-",
        }
    }

    /// Its token `name` under `prefix` (an [`AppTheme`] nested in another):
    /// after the namespace when `name` is in it, so the classes still find
    /// it (`radius-card` under `button-`: `radius-button-card`, which is
    /// `rounded-button-card`), else in front (`raised`: `button-raised`).
    /// A namespaced kind's names keep to its namespace (the derive refuses
    /// others), so a name nested twice is the same whether the prefixes
    /// are applied one at a time or joined.
    pub fn prefixed(self, prefix: &str, name: &str) -> String {
        let namespace = self.namespace();
        match name.strip_prefix(namespace) {
            Some(rest) if !namespace.is_empty() => format!("{namespace}{prefix}{rest}"),
            _ => format!("{prefix}{name}"),
        }
    }

    /// Whether `tokens` hold `name` as this kind.
    pub(crate) fn holds(self, tokens: &Tokens, name: &str) -> bool {
        match self {
            TokenKind::Color => tokens.color(name).is_some(),
            TokenKind::Font => tokens.font(name).is_some(),
            TokenKind::Shadow => tokens.shadow(name).is_some(),
            _ => tokens.number(name).is_some(),
        }
    }
}

/// A registered theme type, as data: its name, specs and defaults
/// ([`super::Registry::theme`]).
#[derive(Clone)]
pub struct ThemeSpec {
    type_name: &'static str,
    type_id: fn() -> TypeId,
    specs: fn() -> &'static [TokenSpec],
    defaults: fn() -> Tokens,
}

impl ThemeSpec {
    /// The spec of `T`.
    pub fn of<T: AppTheme>() -> Self {
        Self {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>,
            specs: T::token_specs,
            defaults: || T::default().to_tokens(),
        }
    }

    /// The type's name (`dxn_ds::DsTheme`).
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// The type's id.
    pub fn type_id(&self) -> TypeId {
        (self.type_id)()
    }

    /// Its tokens.
    pub fn token_specs(&self) -> &'static [TokenSpec] {
        (self.specs)()
    }

    /// Its default tokens (with the roles its specs map).
    pub fn defaults(&self) -> Tokens {
        (self.defaults)()
    }
}

impl std::fmt::Debug for ThemeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThemeSpec")
            .field("type", &self.type_name)
            .finish()
    }
}

/// The roles `specs` map, onto their names.
fn roles_of(specs: &[TokenSpec]) -> Vec<(AnyRole, Box<str>)> {
    specs
        .iter()
        .flat_map(|s| {
            s.roles
                .iter()
                .map(|role| (role.clone(), s.name.as_ref().into()))
        })
        .collect()
}

/// One theme of every registered type: their token names (each owned by
/// one type), kinds, the roles they map and the union of their defaults.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeSet {
    /// Name → (owner type, kind).
    names: FxHashMap<Box<str>, (Box<str>, TokenKind)>,
    /// The types, in registration order.
    types: Vec<Box<str>>,
    /// The defaults of every type, with the role aliases.
    defaults: Tokens,
}

impl ThemeSet {
    /// The set of `specs`' types.
    ///
    /// # Panics
    ///
    /// When two types declare one token name or map one role, naming both.
    #[track_caller]
    pub(crate) fn new(specs: &[ThemeSpec]) -> Self {
        let mut set = Self::empty();
        for spec in specs {
            let tokens = spec
                .token_specs()
                .iter()
                .map(|s| (s.name.as_ref(), s.kind, s.roles.as_slice()));
            if let Err(e) = set.try_add(spec.type_name(), tokens, spec.defaults()) {
                panic!("{e}");
            }
        }
        set
    }

    pub(crate) fn empty() -> Self {
        Self {
            names: FxHashMap::default(),
            types: Vec::new(),
            defaults: Tokens::default(),
        }
    }

    /// Adds type `type_name`'s tokens (name, kind, roles) and `defaults`
    /// (whose roles are set from the tokens'). A type already added is
    /// skipped. `Err`: a name or a role another type claims, naming both,
    /// or a role two of its tokens play.
    pub(crate) fn try_add<'a>(
        &mut self,
        type_name: &str,
        tokens: impl IntoIterator<Item = (&'a str, TokenKind, &'a [AnyRole])>,
        defaults: Tokens,
    ) -> Result<(), String> {
        if self.types.iter().any(|t| **t == *type_name) {
            return Ok(());
        }
        let tokens: Vec<_> = tokens.into_iter().collect();
        let mut roles = Vec::new();
        for (i, (name, kind, plays)) in tokens.iter().enumerate() {
            // A theme file's value is read by the name's form: the name
            // says what its kind is, as a CSS variable's does.
            let read_as = |prefix: &str| name.starts_with(prefix);
            let readable = match kind {
                TokenKind::Font => read_as("font-") && !read_as("font-weight-"),
                TokenKind::Shadow => read_as("shadow-"),
                TokenKind::Color => !read_as("font-"),
                _ => !read_as("font-") || read_as("font-weight-"),
            };
            if !readable {
                return Err(format!(
                    "`{type_name}`: the {} token `{name}` is named as no {} is (a font is `font-*`, a shadow \
                     `shadow-*`, a weight `font-weight-*`)",
                    kind.name(),
                    kind.name()
                ));
            }
            if tokens[..i].iter().any(|(n, ..)| n == name) {
                return Err(format!(
                    "`{type_name}` declares the theme token `{name}` twice"
                ));
            }
            if let Some((owner, _)) = self.names.get(*name) {
                return Err(format!(
                    "the theme token `{name}` is declared by both `{owner}` and `{type_name}`"
                ));
            }
            // A token may play several roles; a role is played by one token.
            for (j, role) in plays.iter().enumerate() {
                if plays[..j].contains(role) {
                    continue;
                }
                // A role reads what its token holds.
                if kind.role_kind() != Some(role.kind()) {
                    return Err(format!(
                        "`{type_name}`: the role `{}` reads {}, and `{name}` is a {} token",
                        role.name(),
                        match role.kind() {
                            RoleKind::Color => "a color",
                            RoleKind::Number => "a number",
                            RoleKind::Font => "a font family",
                        },
                        kind.name()
                    ));
                }
                // By name: a role is its name, whoever declares it.
                let named = |r: &AnyRole| r.name() == role.name();
                if tokens[..i].iter().any(|(_, _, r)| r.iter().any(named)) {
                    return Err(format!(
                        "`{type_name}` maps the role `{}` twice",
                        role.name()
                    ));
                }
                if let Some(owner) = self.role_owner(role.name()) {
                    return Err(format!(
                        "the role `{}` is mapped by both `{owner}` and `{type_name}`",
                        role.name()
                    ));
                }
                roles.push((role.clone(), Box::<str>::from(*name)));
            }
        }
        for (name, kind, _) in tokens {
            self.names.insert(name.into(), (type_name.into(), kind));
        }
        self.defaults = defaults.with_roles(roles).over(&self.defaults);
        self.types.push(type_name.into());
        Ok(())
    }

    /// The type mapping the role named `role`, if any.
    fn role_owner(&self, role: &str) -> Option<&str> {
        self.defaults
            .mapped(role)
            .and_then(|name| self.names.get(name))
            .map(|(owner, _)| &**owner)
    }

    /// Each role the types map onto a token of another name: the role's
    /// name and the token's.
    pub(crate) fn renamed_roles(&self) -> impl Iterator<Item = (&str, &str)> {
        self.defaults
            .roles()
            .filter(|(role, token)| role != token && self.names.contains_key(*token))
    }

    /// Each declared name, with its owner type and kind.
    pub(crate) fn names(&self) -> impl Iterator<Item = (&str, &str, TokenKind)> {
        self.names
            .iter()
            .map(|(name, (owner, kind))| (&**name, &**owner, *kind))
    }

    /// The name a theme file's `key` sets: a declared name, else the token
    /// a role of that name (`primary`) is mapped onto.
    pub(crate) fn target<'a>(&'a self, key: &'a str) -> Option<&'a str> {
        if self.names.contains_key(key) {
            return Some(key);
        }
        self.defaults
            .mapped(key)
            .filter(|name| self.names.contains_key(*name))
    }
}

/// How the core reads a theme file: with a UI set's functions
/// ([`super::Registry::tokens_from`], [`super::Registry::token_overrides_from`]),
/// or with the registered theme types.
#[derive(Clone)]
pub(crate) enum TokenProvider {
    Source {
        read: super::TokenSource,
        overrides: Option<super::TokenOverrides>,
    },
    Themes(Arc<ThemeSet>),
}

impl std::fmt::Debug for TokenProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenProvider::Source { .. } => f.write_str("TokenProvider::Source"),
            TokenProvider::Themes(set) => {
                f.debug_tuple("TokenProvider::Themes").field(set).finish()
            }
        }
    }
}

impl TokenProvider {
    /// The tokens of theme file `json` over the defaults (`None`: the
    /// defaults), with what of it was left out.
    pub(crate) fn read(&self, json: Option<&str>) -> Result<(Tokens, Vec<String>), String> {
        let (tokens, left_out) = self.read_left_out(json)?;
        Ok((tokens, super::theme_file::sentences(left_out)))
    }

    /// [`TokenProvider::read`], each thing left out with what the core
    /// asks of it (whether it is only a name no type declares).
    pub(crate) fn read_left_out(
        &self,
        json: Option<&str>,
    ) -> Result<(Tokens, Vec<super::theme_file::LeftOut>), String> {
        match self {
            TokenProvider::Source { read, .. } => read(json).map(said),
            TokenProvider::Themes(set) => match json {
                None => Ok((set.defaults.clone(), Vec::new())),
                Some(json) => super::theme_file::parse_declared(json, &set.defaults, set),
            },
        }
    }

    /// The tokens theme file `json` sets, and only those (`var()` against
    /// `base`); `None`: the set reads no changes.
    pub(crate) fn overrides(
        &self,
        json: &str,
        base: &Tokens,
    ) -> Option<Result<(Tokens, Vec<String>), String>> {
        Some(
            self.overrides_left_out(json, base)?
                .map(|(tokens, left_out)| (tokens, super::theme_file::sentences(left_out))),
        )
    }

    /// [`TokenProvider::overrides`], as [`TokenProvider::read_left_out`].
    pub(crate) fn overrides_left_out(
        &self,
        json: &str,
        base: &Tokens,
    ) -> Option<Result<(Tokens, Vec<super::theme_file::LeftOut>), String>> {
        match self {
            TokenProvider::Source { overrides, .. } => Some((*overrides)?(json, base).map(said)),
            TokenProvider::Themes(set) => {
                Some(super::theme_file::overrides_declared(json, base, set))
            }
        }
    }
}

/// A UI set's own reader says what it left out in sentences alone.
fn said((tokens, left_out): (Tokens, Vec<String>)) -> (Tokens, Vec<super::theme_file::LeftOut>) {
    let left_out = left_out
        .into_iter()
        .map(super::theme_file::LeftOut::said)
        .collect();
    (tokens, left_out)
}

/// The theme of type `T` the surface is drawn with now: its names in the
/// tokens ([`super::tokens`]), each a field (a name they lack keeps
/// [`Default`]'s). Reading it subscribes, as [`super::tokens`] does: a
/// binding reading it runs again when the tokens change. It is read once
/// per change of the tokens, per surface.
pub fn theme_of<T: AppTheme>() -> Arc<T> {
    let tokens = super::tokens();
    let cached = super::ambient::with_theme_cache(|cache| {
        let (seen, theme) = cache.get(&TypeId::of::<T>())?;
        Arc::ptr_eq(seen, &tokens)
            .then(|| theme.downcast_ref::<Arc<T>>().cloned())
            .flatten()
    });
    if let Some(theme) = cached {
        return theme;
    }
    // The type's own code runs with the cache let go: it may read the
    // tokens or another type.
    let theme = Arc::new(T::read_tokens(&tokens, ""));
    super::ambient::with_theme_cache(|cache| {
        cache.insert(
            TypeId::of::<T>(),
            (tokens.clone(), Box::new(theme.clone()) as Box<dyn Any>),
        );
    });
    theme
}

/// Draws the app with `theme`: its names take its values, every other
/// token stays (another type's, the app's own). In order with the app's
/// other actions, as [`super::update_tokens`].
///
/// A number field is its token's light value. A number `theme` leaves at
/// the value it has keeps the dark value it has, so a theme read
/// ([`theme_of`]), changed in one field and set changes that field only;
/// one `theme` changes is that value in both appearances, as
/// [`update_theme`] makes a number it sets.
pub fn set_theme<T: AppTheme>(theme: T) {
    super::ambient::set_theme_tokens(theme_tokens(&theme));
}

/// `theme`'s tokens, without the roles its specs map: the roles are the
/// registered types' (a type set while it runs does not map them again).
pub(crate) fn theme_tokens<T: AppTheme>(theme: &T) -> Tokens {
    let mut out = Tokens::default();
    theme.write_tokens("", &mut out);
    out
}

/// Changes the fields of `T` that `patch` sets, the others staying.
pub fn update_theme<T: AppTheme>(patch: T::Patch) {
    super::update_tokens(patch_tokens::<T>(&patch));
}

/// Puts `T`'s tokens back to the app's theme's (`nsh.json`'s over the
/// types' defaults, or the one last set whole), undoing [`set_theme`] and
/// [`update_theme`].
pub fn reset_theme<T: AppTheme>() {
    super::reset_tokens(T::token_specs().iter().map(|s| s.name.as_ref()));
}

/// The tokens `patch` sets.
pub(crate) fn patch_tokens<T: AppTheme>(patch: &T::Patch) -> Tokens {
    let mut out = Tokens::default();
    T::write_patch(patch, "", &mut out);
    out
}
