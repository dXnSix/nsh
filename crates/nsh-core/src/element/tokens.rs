//! Design tokens: the values of the active theme document, resolved, by
//! name (`muted-foreground`, `radius-card`, `font-sans`). The core gives no
//! name a meaning but `font-sans`, the app's default font (as Tailwind's
//! `--font-sans` is a page's), and has none of its own: a theme document
//! declares them (`docs/dev/theme-format.md`), a UI set or the app brings it
//! ([`super::Registry::theme_document`], [`super::Root::theme`]).
//!
//! [`Tokens`] is one map, name → [`Token`], of one value per name, typed by
//! its declaration: a name decides nothing about a type. The core makes it
//! with `nsh_theme::resolve` once per change, never per read: the document's
//! values, the picked modifier contexts, the app's and the host's overrides
//! ([`super::Patch`]), references last.
//!
//! Tailwind classes name every token as they name a theme variable
//! (`bg-muted`, `rounded-(--radius)`; `tailwind/theme.rs`), TS reads them
//! with `nsh`'s `token()`, and Rust components with [`super::token`], typed
//! keys ([`super::TokenKey`]) and roles ([`super::role()`]).

use std::{
    marker::PhantomData,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use nsh_theme::{
    Patch, Resolved, RoleDeclaration, RolePattern, Selection, ThemeDocument, ValueType,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Blur, Border, TransitionToken, Typography};
use crate::{BoxShadow, color::Hsla, effects::Gradient};

/// The app's default font family's token: Tailwind's `--font-sans`.
pub const FONT_SANS: &str = "font-sans";

/// The monospaced font family's token: Tailwind's `--font-mono`.
pub const FONT_MONO: &str = "font-mono";

/// What a [`Tokens`] was resolved from: the active document, the selection
/// and the patches over it (the app's, the host's, outer scopes'), so a
/// [`super::ThemeScope`] resolves its subtree with the same stack, and the
/// roles its mappings resolve against.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeLayers {
    pub(crate) document: Arc<ThemeDocument>,
    pub(crate) selection: Selection,
    pub(crate) patches: Vec<Arc<Patch>>,
    pub(crate) roles: Arc<RoleTable>,
}

impl ThemeLayers {
    /// The layers' patches as `resolve` takes them.
    pub(crate) fn patch_refs(&self) -> Vec<&Patch> {
        self.patches.iter().map(|p| &**p).collect()
    }
}

/// The design tokens: one resolved value per token name, the resolved role
/// mapping, and the active document's declarations ([`Tokens::kind_of`],
/// [`Tokens::path_of`], [`Tokens::names`]). The default has none.
///
/// Roles, what NSH's own parts draw with, read the token a theme maps them
/// onto ([`Tokens::role`]): a theme whose surface for popovers is
/// `surface-overlay` maps [`role::POPOVER`] onto it. Without a mapping a
/// role reads the token of its own name (`popover`). A mapping onto a token
/// of a type the role doesn't read is left out (`role-type`), and the role
/// resolves through the layer below (`docs/dev/theme-format.md`, "Roles").
#[derive(Debug, Clone, Default)]
pub struct Tokens {
    values: FxHashMap<Box<str>, Token>,
    /// A role's name → the name of the token it reads (`None`: none), for
    /// the roles a document or a patch maps and the declared roles whose
    /// own token has the wrong type, resolved once per change with the
    /// declarations. Any other role reads the token of its name.
    roles: FxHashMap<Box<str>, Option<Box<str>>>,
    /// What they were resolved from (`None`: made of values alone).
    layers: Option<Arc<ThemeLayers>>,
    /// Whether the app has a theme of its own ([`Tokens::is_themed`]): not
    /// a token, so not compared.
    themed: bool,
}

impl PartialEq for Tokens {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
            && self.roles == other.roles
            && self.layers.as_ref().map(|l| &l.selection)
                == other.layers.as_ref().map(|l| &l.selection)
    }
}

/// What one of NSH's own parts draws with: a surface, the color of what is
/// on it, a radius, a font family. `nsh-base`'s theme reads them
/// (`theme().popover`), as do the platforms' own controls (Linux's menus,
/// the mobile tab bar, iOS's and Android's alerts) and the default font.
///
/// A role is a constant of the crate that draws with it: the core's and the
/// platforms' are in [`role`], `nsh-base`'s in `nsh_base::role`, and a
/// design system's components have their own, declared with [`crate::roles!`]
/// and registered with [`super::Registry::roles`]. It reads the token a
/// theme maps it onto, else the token of its name ([`Role::name`],
/// `docs/dev/theme-format.md`, "Roles"). A role is its name: two constants
/// of one name are one role. It is no token and no class: `bg-primary`
/// exists when a theme has a token `primary`.
///
/// `V` is what it reads ([`RoleValue`]): a color unless it says otherwise
/// ([`NumberRole`], [`FontRole`], `Role<Typography>`, …).
pub struct Role<V = Hsla> {
    name: &'static str,
    value: PhantomData<fn() -> V>,
}

/// A role that reads a number (`radius`).
pub type NumberRole = Role<f32>;

/// A role that reads a font family (`font-sans`).
pub type FontRole = Role<Arc<str>>;

impl<V> Role<V> {
    /// The role named `name`: the token it reads unless a theme maps it
    /// (`popover-foreground`).
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            value: PhantomData,
        }
    }

    /// Its name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<V: RoleValue> Role<V> {
    /// The value type of the tokens it reads.
    pub const VALUE_TYPE: ValueType = V::TYPE;

    /// Its declaration, as a UI set registers it
    /// ([`super::Registry::roles`]): its name, its value type, `doc` as a
    /// doc comment writes it (each paragraph's lines joined, the paragraph
    /// that only names it, ``Name `popover`.``, left out; none when that
    /// leaves nothing), and whether every theme should map it.
    pub fn declaration(self, doc: &str, required: bool) -> RoleSpec {
        let declaration = RoleDeclaration::new(self.name, V::TYPE).with_required(required);
        match doc_text(doc) {
            Some(doc) => declaration.with_doc(doc),
            None => declaration,
        }
    }
}

/// A doc comment as one text: the lines of each paragraph joined, without
/// the paragraph that only names the role (``Name `popover`.``); `None`
/// when nothing is left.
fn doc_text(doc: &str) -> Option<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in doc.lines().chain([""]) {
        let line = line.trim();
        if !line.is_empty() {
            current.push(line);
        } else if !current.is_empty() {
            paragraphs.push(current.join(" "));
            current.clear();
        }
    }
    paragraphs.retain(|p| !p.starts_with("Name `"));
    (!paragraphs.is_empty()).then(|| paragraphs.join("\n\n"))
}

impl<V> Clone for Role<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for Role<V> {}

impl<V> PartialEq for Role<V> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<V> Eq for Role<V> {}

impl<V> std::hash::Hash for Role<V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl<V> std::fmt::Debug for Role<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Role").field(&self.name).finish()
    }
}

/// What a [`Role`] reads: the Rust type of one value type
/// (`element/theme_values.rs`' table): [`Hsla`] (color), `f32` (number),
/// `Arc<str>` (font), `String`, `bool`, [`Duration`], [`CubicBezier`]
/// (easing), [`Typography`], `Arc<[BoxShadow]>` (shadow), [`Gradient`],
/// [`Blur`], [`Border`] and [`TransitionToken`] (transition). Sealed: no
/// other type is one.
pub trait RoleValue: super::TokenValue {}

impl RoleValue for Hsla {}
impl RoleValue for f32 {}
impl RoleValue for Arc<str> {}
impl RoleValue for String {}
impl RoleValue for bool {}
impl RoleValue for Duration {}
impl RoleValue for CubicBezier {}
impl RoleValue for Typography {}
impl RoleValue for Arc<[BoxShadow]> {}
impl RoleValue for Gradient {}
impl RoleValue for Blur {}
impl RoleValue for Border {}
impl RoleValue for TransitionToken {}

/// A role as a UI set registers it ([`super::Registry::roles`]): its name,
/// the value type of the tokens it reads, its doc and whether every theme
/// should map it (`nsh_theme::RoleDeclaration`). The toolchain types TS's
/// `role()` from these, and the core checks a theme's mappings against
/// them.
pub type RoleSpec = RoleDeclaration;

/// Declares role constants with their docs, and `all()`, which lists their
/// declarations ([`RoleSpec`]) for a UI set's registry
/// ([`super::Registry::roles`]): `pub NAME: Type = "name"`, each under its
/// doc comment, with `required` after the name for a role every theme
/// should map (`role-unmapped` otherwise). `Type` is a [`Role`] of any
/// [`RoleValue`] (`Role`, `NumberRole`, `FontRole`, `Role<Typography>`).
///
/// ```
/// mod role {
///     use nsh_core::element::{NumberRole, Role};
///
///     nsh_core::roles! {
///         /// A banner's surface.
///         pub BANNER: Role = "banner" required,
///         /// How round a chip is.
///         pub CHIP_RADIUS: NumberRole = "chip-radius",
///     }
/// }
/// let all = role::all();
/// assert!(all[0].required && !all[1].required);
/// assert_eq!(all[0].doc.as_deref(), Some("A banner's surface."));
/// ```
#[macro_export]
macro_rules! roles {
    (@required) => { false };
    (@required required) => { true };
    ($( $(#[doc = $doc:literal])* $vis:vis $role:ident: $ty:ty = $name:literal $($required:ident)? ),* $(,)?) => {
        $(
            $(#[doc = $doc])*
            $vis const $role: $ty = <$ty>::new($name);
        )*

        /// These roles' declarations with their docs, for a UI set's
        /// registry (`Registry::roles`).
        pub fn all() -> ::std::vec::Vec<$crate::element::RoleSpec> {
            ::std::vec![
                $( $role.declaration(
                    ::core::concat!($($doc, "\n"),*),
                    $crate::roles!(@required $($required)?),
                ) ),*
            ]
        }
    };
}

/// The roles the core and the platforms' own controls draw with, none
/// required. A UI set has more of its own (`nsh_base::role`). The core
/// checks a theme's mappings against them whatever the sets register.
pub mod role {
    use super::{FontRole, Role};

    crate::roles! {
        /// Popovers, menus, tooltips, alerts.
        pub POPOVER: Role = "popover",
        /// Text on popovers.
        pub POPOVER_FOREGROUND: Role = "popover-foreground",
        /// The main action: buttons, the tab bar's and alerts' tint.
        pub PRIMARY: Role = "primary",
        /// What is hovered or highlighted: menu items, list rows.
        pub ACCENT: Role = "accent",
        /// Text on what is highlighted.
        pub ACCENT_FOREGROUND: Role = "accent-foreground",
        /// Secondary text: hints, captions, an alert's message.
        pub MUTED_FOREGROUND: Role = "muted-foreground",
        /// Borders and dividers.
        pub BORDER: Role = "border",
        /// Destructive actions and errors.
        pub DESTRUCTIVE: Role = "destructive",
        /// The app's default font family.
        pub FONT_SANS: FontRole = "font-sans",
    }
}

/// The roles a theme's mappings are checked and resolved against: the
/// core's own ([`role::all`]) and the served sets' declarations, collected
/// once when a host gives the core its sets, and Tailwind's namespaces as
/// patterns (`radius-` reads numbers, …).
#[derive(Debug)]
pub(crate) struct RoleTable {
    pub(crate) declarations: Vec<RoleDeclaration>,
    pub(crate) patterns: Vec<RolePattern>,
    /// The declared roles' value types, by name.
    declared: FxHashMap<Box<str>, ValueType>,
    /// The `role-type` lines typed reads logged ([`Tokens::role`]), each
    /// once.
    logged: Mutex<FxHashSet<String>>,
}

impl PartialEq for RoleTable {
    fn eq(&self, other: &Self) -> bool {
        self.declarations == other.declarations && self.patterns == other.patterns
    }
}

impl Default for RoleTable {
    /// The core's own roles alone.
    fn default() -> Self {
        RoleTable::of(&[], &[])
    }
}

impl RoleTable {
    /// The core's roles, then `specs` (of the sets `sets` names, one per
    /// spec, `None`: the app's own components), each once: a role listed
    /// again with another value type gives `role-conflict`, logged, and the
    /// first is kept.
    pub(crate) fn of(specs: &[RoleSpec], sets: &[Option<&'static str>]) -> Self {
        let mut declarations: Vec<RoleDeclaration> = Vec::new();
        let mut owners: Vec<Option<&'static str>> = Vec::new();
        let core = role::all();
        let listed = core.into_iter().map(|d| (d, Some("nsh"))).chain(
            specs
                .iter()
                .cloned()
                .zip(sets.iter().copied().chain(std::iter::repeat(None))),
        );
        let mut said = Vec::new();
        for (declaration, set) in listed {
            if let Some(conflict) = merge_role(&mut declarations, &mut owners, declaration, set) {
                said.push(conflict);
            }
        }
        super::theme::report(&said);
        let declared = declarations
            .iter()
            .map(|d| (d.name.as_str().into(), d.value_type))
            .collect();
        RoleTable {
            declarations,
            patterns: crate::tailwind::role_patterns(),
            declared,
            logged: Mutex::new(FxHashSet::default()),
        }
    }

    /// Whether the token `name` may serve the undeclared role of its own
    /// name: no pattern matches it, or the longest that does accepts `ty`.
    fn own_accepts(&self, name: &str, ty: ValueType) -> bool {
        self.patterns
            .iter()
            .filter(|p| name.starts_with(p.prefix.as_str()))
            .max_by_key(|p| p.prefix.len())
            .is_none_or(|p| p.accepts.contains(&ty))
    }

    /// Logs `diagnostic` unless it was.
    fn report_once(&self, diagnostic: nsh_theme::Diagnostic) {
        let line = diagnostic.to_string();
        let fresh = match self.logged.lock() {
            Ok(mut logged) => logged.insert(line),
            Err(_) => true,
        };
        if fresh {
            super::theme::report(&[diagnostic]);
        }
    }
}

/// Adds `declaration` (listed by `set`) to `declarations` unless a role of
/// its name is there: of the same type it is the same role; of another, the
/// `role-conflict` warning is returned and the first kept. Never panics.
pub(crate) fn merge_role(
    declarations: &mut Vec<RoleDeclaration>,
    owners: &mut Vec<Option<&'static str>>,
    declaration: RoleDeclaration,
    set: Option<&'static str>,
) -> Option<nsh_theme::Diagnostic> {
    let Some(i) = declarations.iter().position(|d| d.name == declaration.name) else {
        declarations.push(declaration);
        owners.push(set);
        return None;
    };
    let listed = &declarations[i];
    if listed.value_type == declaration.value_type {
        return None;
    }
    let whose = |set: Option<&str>| match set {
        Some(set) => format!("`{set}`"),
        None => "the app's components".to_owned(),
    };
    Some(nsh_theme::Diagnostic::warning(
        nsh_theme::code::ROLE_CONFLICT,
        "",
        format!(
            "The role `{}` is registered reading a {} by {} and a {} by {}; the first is kept.",
            declaration.name,
            listed.value_type,
            whose(owners.get(i).copied().flatten()),
            declaration.value_type,
            whose(set),
        ),
    ))
}

/// The core's own roles and Tailwind's patterns, for tokens resolved
/// without a core ([`Tokens::resolve`]).
static CORE_ROLES: LazyLock<Arc<RoleTable>> = LazyLock::new(|| Arc::new(RoleTable::default()));

/// A token's value: one variant per value type
/// ([`nsh_theme::ValueType`]), each in its Rust type
/// (`element/theme_values.rs`' table).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Token {
    /// A color.
    Color(Hsla),
    /// A number: a length in points, or a unitless quantity.
    Number(f32),
    /// A font family list, whole.
    Font(Arc<str>),
    /// A string.
    String(String),
    /// A boolean.
    Boolean(bool),
    /// A duration.
    Duration(Duration),
    /// An easing curve.
    Easing(CubicBezier),
    /// A text style.
    Typography(Typography),
    /// Box shadows, the first on top.
    Shadow(Arc<[BoxShadow]>),
    /// A gradient.
    Gradient(Gradient),
    /// A blur.
    Blur(Blur),
    /// A border.
    Border(Border),
    /// A transition.
    Transition(TransitionToken),
}

impl Token {
    /// Its value type.
    pub fn value_type(&self) -> ValueType {
        match self {
            Token::Color(_) => ValueType::Color,
            Token::Number(_) => ValueType::Number,
            Token::Font(_) => ValueType::Font,
            Token::String(_) => ValueType::String,
            Token::Boolean(_) => ValueType::Boolean,
            Token::Duration(_) => ValueType::Duration,
            Token::Easing(_) => ValueType::Easing,
            Token::Typography(_) => ValueType::Typography,
            Token::Shadow(_) => ValueType::Shadow,
            Token::Gradient(_) => ValueType::Gradient,
            Token::Blur(_) => ValueType::Blur,
            Token::Border(_) => ValueType::Border,
            Token::Transition(_) => ValueType::Transition,
        }
    }
}

/// An easing curve: a cubic Bézier `x1, y1, x2, y2`, as CSS's
/// `cubic-bezier()` and a transition's easing ([`crate::effects::Transition`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier(pub [f32; 4]);

impl CubicBezier {
    /// CSS `linear`.
    pub const LINEAR: CubicBezier = CubicBezier([0.0, 0.0, 1.0, 1.0]);
    /// CSS `ease`.
    pub const EASE: CubicBezier = CubicBezier([0.25, 0.1, 0.25, 1.0]);
    /// CSS `ease-in`.
    pub const EASE_IN: CubicBezier = CubicBezier([0.42, 0.0, 1.0, 1.0]);
    /// CSS `ease-out`.
    pub const EASE_OUT: CubicBezier = CubicBezier([0.0, 0.0, 0.58, 1.0]);
    /// CSS `ease-in-out`.
    pub const EASE_IN_OUT: CubicBezier = CubicBezier([0.42, 0.0, 0.58, 1.0]);

    /// A CSS easing: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`
    /// or `cubic-bezier(x1, y1, x2, y2)` (`x1` and `x2` in `0..=1`).
    pub fn parse(css: &str) -> Option<CubicBezier> {
        let css = css.trim();
        Some(match css {
            "linear" => Self::LINEAR,
            "ease" => Self::EASE,
            "ease-in" => Self::EASE_IN,
            "ease-out" => Self::EASE_OUT,
            "ease-in-out" => Self::EASE_IN_OUT,
            _ => {
                let args = css.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
                let n: Vec<f32> = args
                    .split(',')
                    .map(|a| a.trim().parse::<f32>().ok().filter(|n| n.is_finite()))
                    .collect::<Option<_>>()?;
                let [x1, y1, x2, y2] = n.as_slice() else {
                    return None;
                };
                if !(0.0..=1.0).contains(x1) || !(0.0..=1.0).contains(x2) {
                    return None;
                }
                CubicBezier([*x1, *y1, *x2, *y2])
            }
        })
    }

    /// As CSS writes it: `cubic-bezier(x1, y1, x2, y2)`.
    pub fn css(self) -> String {
        let [a, b, c, d] = self.0;
        format!("cubic-bezier({a}, {b}, {c}, {d})")
    }
}

/// A modifier of the active theme document, as [`super::modifiers`] lists
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierAxis {
    /// Its name (`brand`).
    pub name: Box<str>,
    /// Its contexts (`ocean`, `forest`).
    pub contexts: Vec<Box<str>>,
    /// The context picked when nothing else is.
    pub default: Box<str>,
}

impl Tokens {
    /// The tokens of `document` under `selection` and `patches` (a later
    /// patch winning), with references resolved: what the core draws with
    /// when that document is active. Its roles resolve against the core's
    /// own declarations and Tailwind's patterns (a core's against its
    /// sets' too). The diagnostics of resolving are returned, not logged.
    pub fn resolve(
        document: Arc<ThemeDocument>,
        selection: Selection,
        patches: Vec<Arc<Patch>>,
    ) -> (Tokens, Vec<nsh_theme::Diagnostic>) {
        let layers = Arc::new(ThemeLayers {
            document,
            selection,
            patches,
            roles: CORE_ROLES.clone(),
        });
        let (tokens, resolved) = Tokens::resolve_layers(layers);
        (tokens, resolved.diagnostics)
    }

    /// The tokens of `layers`, and what resolving them gave.
    pub(crate) fn resolve_layers(layers: Arc<ThemeLayers>) -> (Tokens, Resolved) {
        let resolved =
            nsh_theme::resolve(&layers.document, &layers.selection, &layers.patch_refs());
        let tokens = Tokens::from_resolved(&resolved, layers, None);
        (tokens, resolved)
    }

    /// The tokens of `resolved`, made from `layers`. With `previous` (the
    /// tokens before and what changed since), only the changed names are
    /// converted; the others are taken from `previous`.
    pub(crate) fn from_resolved(
        resolved: &Resolved,
        layers: Arc<ThemeLayers>,
        previous: Option<(&Tokens, &nsh_theme::Changes)>,
    ) -> Tokens {
        let values = match previous {
            Some((before, changes)) => {
                let mut values = before.values.clone();
                for name in &changes.tokens {
                    match resolved
                        .values
                        .get(name)
                        .and_then(super::theme_values::token_of)
                    {
                        Some(token) => {
                            values.insert(name.as_str().into(), token);
                        }
                        None => {
                            values.remove(name.as_str());
                        }
                    }
                }
                values
            }
            None => resolved
                .values
                .iter()
                .filter_map(|(name, value)| {
                    Some((name.into(), super::theme_values::token_of(value)?))
                })
                .collect(),
        };
        let roles = resolve_roles(&layers);
        Tokens {
            values,
            roles,
            layers: Some(layers),
            themed: false,
        }
    }

    /// What they were resolved from.
    pub(crate) fn layers(&self) -> Option<&Arc<ThemeLayers>> {
        self.layers.as_ref()
    }

    /// The role checks of the layers these were resolved from
    /// (`nsh_theme::check_roles`, against their declarations and patterns),
    /// `resolved` being what resolving them gave.
    pub(crate) fn check_roles(&self, resolved: &Resolved) -> Vec<nsh_theme::Diagnostic> {
        let Some(layers) = self.layers.as_deref() else {
            return Vec::new();
        };
        nsh_theme::check_roles(
            &layers.document,
            &layers.patch_refs(),
            resolved,
            &layers.roles.declarations,
            &layers.roles.patterns,
        )
    }

    /// The roles whose resolved token, or that token's value, differs from
    /// `before`'s, `changed` being the token names whose values differ:
    /// the roles some layer maps, before or now, and the declared ones
    /// that read nothing. With whether a role reads another token.
    pub(crate) fn changed_roles(&self, before: &Tokens, changed: &[String]) -> (Vec<String>, bool) {
        let changed: FxHashSet<&str> = changed.iter().map(String::as_str).collect();
        let mut names: Vec<&str> = self
            .roles
            .keys()
            .chain(before.roles.keys().filter(|r| !self.roles.contains_key(*r)))
            .map(|r| &**r)
            .collect();
        names.sort_unstable();
        let mut moved = false;
        let mut out = Vec::new();
        for role in names {
            let now = self.role_target(role);
            if now != before.role_target(role) {
                moved = true;
                out.push(role.to_owned());
            } else if now.is_some_and(|token| changed.contains(token)) {
                out.push(role.to_owned());
            }
        }
        (out, moved)
    }

    /// The token named `name`.
    pub fn get(&self, name: &str) -> Option<Token> {
        self.values.get(name).cloned()
    }

    /// The token named `name`, borrowed.
    pub fn value(&self, name: &str) -> Option<&Token> {
        self.values.get(name)
    }

    /// Each token with its value, in no order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Token)> {
        self.values.iter().map(|(name, token)| (&**name, token))
    }

    /// The value type token `name` is declared with in the active
    /// document (made of values alone: its value's type).
    pub fn kind_of(&self, name: &str) -> Option<ValueType> {
        match self.document() {
            Some(document) => document.token(name).map(|t| t.value_type),
            None => self.values.get(name).map(Token::value_type),
        }
    }

    /// The path token `name` is declared at in the active document.
    pub fn path_of(&self, name: &str) -> Option<&str> {
        self.document()?.token(name)?.path.as_deref()
    }

    /// The doc token `name` is declared with in the active document.
    pub fn doc_of(&self, name: &str) -> Option<&str> {
        self.document()?.token(name)?.doc.as_deref()
    }

    /// The declared token at `path` in the active document.
    pub fn name_at(&self, path: &str) -> Option<&str> {
        self.document()?
            .tokens
            .iter()
            .find(|(_, token)| token.path.as_deref() == Some(path))
            .map(|(name, _)| name)
    }

    /// Each token's name: the active document's, in its order (made of
    /// values alone: theirs, in no order).
    pub fn names(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self.document() {
            Some(document) => Box::new(document.tokens.keys()),
            None => Box::new(self.values.keys().map(|name| &**name)),
        }
    }

    /// Whether it has no token.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The active document they were resolved from.
    pub fn document(&self) -> Option<&ThemeDocument> {
        self.layers.as_deref().map(|l| &*l.document)
    }

    /// The modifier contexts they were resolved with.
    pub fn selection(&self) -> Option<&Selection> {
        self.layers.as_deref().map(|l| &l.selection)
    }

    /// The context modifier `name` shows: the one picked, else its
    /// default. `None`: the active document has no such modifier.
    pub fn modifier(&self, name: &str) -> Option<&str> {
        let layers = self.layers.as_deref()?;
        let modifier = layers.document.modifiers.get(name)?;
        Some(
            layers
                .selection
                .get(name)
                .filter(|context| modifier.contexts.contains_key(context))
                .unwrap_or(modifier.default.as_str()),
        )
    }

    /// The active document's modifiers, each with its contexts and
    /// default, in its order.
    pub fn modifiers(&self) -> Vec<ModifierAxis> {
        let Some(document) = self.document() else {
            return Vec::new();
        };
        document
            .modifiers
            .iter()
            .map(|(name, modifier)| ModifierAxis {
                name: name.into(),
                contexts: modifier.contexts.keys().map(Into::into).collect(),
                default: modifier.default.as_str().into(),
            })
            .collect()
    }

    /// The color named `name`.
    pub fn color(&self, name: &str) -> Option<Hsla> {
        match self.values.get(name)? {
            Token::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// The number named `name`.
    pub fn number(&self, name: &str) -> Option<f32> {
        match self.values.get(name)? {
            Token::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The font family named `name` ([`FONT_SANS`], [`FONT_MONO`], ...).
    pub fn font(&self, name: &str) -> Option<&Arc<str>> {
        match self.values.get(name)? {
            Token::Font(f) => Some(f),
            _ => None,
        }
    }

    /// The string named `name`.
    pub fn string(&self, name: &str) -> Option<&str> {
        match self.values.get(name)? {
            Token::String(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean named `name`.
    pub fn boolean(&self, name: &str) -> Option<bool> {
        match self.values.get(name)? {
            Token::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// The duration named `name`.
    pub fn duration(&self, name: &str) -> Option<Duration> {
        match self.values.get(name)? {
            Token::Duration(d) => Some(*d),
            _ => None,
        }
    }

    /// The easing curve named `name`.
    pub fn easing(&self, name: &str) -> Option<CubicBezier> {
        match self.values.get(name)? {
            Token::Easing(e) => Some(*e),
            _ => None,
        }
    }

    /// The text style named `name`.
    pub fn typography(&self, name: &str) -> Option<&Typography> {
        match self.values.get(name)? {
            Token::Typography(t) => Some(t),
            _ => None,
        }
    }

    /// The box shadows named `name`.
    pub fn shadow(&self, name: &str) -> Option<&Arc<[BoxShadow]>> {
        match self.values.get(name)? {
            Token::Shadow(s) => Some(s),
            _ => None,
        }
    }

    /// The gradient named `name`.
    pub fn gradient(&self, name: &str) -> Option<&Gradient> {
        match self.values.get(name)? {
            Token::Gradient(g) => Some(g),
            _ => None,
        }
    }

    /// The blur named `name`.
    pub fn blur(&self, name: &str) -> Option<Blur> {
        match self.values.get(name)? {
            Token::Blur(b) => Some(*b),
            _ => None,
        }
    }

    /// The border named `name`.
    pub fn border(&self, name: &str) -> Option<Border> {
        match self.values.get(name)? {
            Token::Border(b) => Some(*b),
            _ => None,
        }
    }

    /// The transition named `name`.
    pub fn transition(&self, name: &str) -> Option<TransitionToken> {
        match self.values.get(name)? {
            Token::Transition(t) => Some(*t),
            _ => None,
        }
    }

    /// Each role a document or a patch maps, with the token it resolves
    /// to: its highest mapping onto a token of a type it reads, else the
    /// token of its own name.
    pub fn roles(&self) -> impl Iterator<Item = (&str, &str)> {
        self.roles
            .iter()
            .filter_map(|(role, token)| Some((&**role, token.as_deref()?)))
    }

    /// The name of the token the role named `role` resolves to, if a theme
    /// maps it: for a role known by its name alone, as TS's `role()` knows
    /// one ([`Tokens::role`] reads a role constant). `None` when no layer
    /// maps it, or it resolves to nothing.
    pub fn mapped(&self, role: &str) -> Option<&str> {
        self.roles.get(role)?.as_deref()
    }

    /// Whether `other` resolves the same roles to the same tokens.
    pub fn same_roles(&self, other: &Tokens) -> bool {
        self.roles == other.roles
    }

    /// The name of the token `role` reads: the one it resolves to through
    /// a mapping, else its own ([`Role::name`]).
    pub fn role_name<V>(&self, role: Role<V>) -> &str {
        self.role_target(role.name()).unwrap_or(role.name())
    }

    /// The token the role named `role` resolves to (`docs/dev/theme-format.md`,
    /// "Roles"): its highest mapping onto a token of a type it reads, else
    /// the token of its own name; `None` when that token has the wrong
    /// type. Made once per change, never per read.
    pub(crate) fn role_target<'a>(&'a self, role: &'a str) -> Option<&'a str> {
        if let Some(token) = self.roles.get(role) {
            return token.as_deref();
        }
        // No mapping: its own name, unless it is no declared role and a
        // Tailwind pattern refuses its own token's type.
        let Some(layers) = self.layers.as_deref() else {
            return Some(role);
        };
        let table = &layers.roles;
        let refused = !table.declared.contains_key(role)
            && self
                .kind_of(role)
                .is_some_and(|ty| !table.own_accepts(role, ty));
        (!refused).then_some(role)
    }

    /// The token the role named `name` reads, whatever its type: the token
    /// it resolves to ([`Tokens::role`]), `None` when there is none or the
    /// token of its own name has the wrong type.
    pub fn role_by_name(&self, name: &str) -> Option<Token> {
        self.values.get(self.role_target(name)?).cloned()
    }

    /// What `role` draws with: the token a theme maps it onto (its highest
    /// mapping onto a token of the type it reads), else the one of its own
    /// name. A color for a [`Role`], a number for a [`NumberRole`], a font
    /// family for a [`FontRole`]. `None` when there is no such token or it
    /// has another type than `V`, which logs `role-type` once.
    pub fn role<V: RoleValue>(&self, role: Role<V>) -> Option<V> {
        let token = self.role_target(role.name())?;
        self.typed_role(role, token)
    }

    /// The value of `token`, which `role` resolved to, as a `V`; a token of
    /// another type is `None` and logs `role-type` once.
    pub(crate) fn typed_role<V: RoleValue>(&self, role: Role<V>, token: &str) -> Option<V> {
        let value = self.values.get(token)?;
        if let Some(value) = V::from_token(value) {
            return Some(value);
        }
        if let Some(layers) = self.layers.as_deref() {
            layers.roles.report_once(nsh_theme::Diagnostic::error(
                nsh_theme::code::ROLE_TYPE,
                format!(
                    "/roles/{}",
                    role.name().replace('~', "~0").replace('/', "~1")
                ),
                format!(
                    "Role `{}` is read as a {}, and the token `{token}` it resolves to is a {}; \
                     the read gives nothing.",
                    role.name(),
                    V::TYPE,
                    value.value_type()
                ),
            ));
        }
        None
    }

    /// Whether the app is drawn with a theme of its own: a document of the
    /// app's or of a UI set (not only a fallback), or overrides. The core
    /// says so on the tokens it draws with, which the platforms get
    /// (`Command::Tokens`): their own controls take the theme's colors only
    /// then (iOS's and Android's alerts), and keep the system's for an app
    /// without one. `false` on tokens made elsewhere.
    pub fn is_themed(&self) -> bool {
        self.themed
    }

    /// Says whether the app has a theme of its own ([`Tokens::is_themed`]).
    pub(crate) fn set_themed(&mut self, themed: bool) {
        self.themed = themed;
    }
}

/// The role mapping of `layers` (`nsh_theme::resolve_role`, against their
/// declarations and patterns): for each role a layer maps, the token it
/// resolves to; for each declared role no layer maps whose own token has
/// another type, none. Once per change, never per read.
fn resolve_roles(layers: &ThemeLayers) -> FxHashMap<Box<str>, Option<Box<str>>> {
    let document = &*layers.document;
    let patches = layers.patch_refs();
    let table = &*layers.roles;
    let mut roles: FxHashMap<Box<str>, Option<Box<str>>> = FxHashMap::default();
    let mapped = document
        .roles
        .keys()
        .chain(patches.iter().flat_map(|p| p.roles.keys()));
    for role in mapped {
        if roles.contains_key(role) {
            continue;
        }
        let token = nsh_theme::resolve_role(
            role,
            document,
            &patches,
            &table.declarations,
            &table.patterns,
        );
        roles.insert(role.into(), token.map(Into::into));
    }
    for declaration in &table.declarations {
        let role = declaration.name.as_str();
        if !roles.contains_key(role)
            && document
                .token(role)
                .is_some_and(|own| own.value_type != declaration.value_type)
        {
            roles.insert(role.into(), None);
        }
    }
    roles
}

/// Setting values directly: for the core's tests, which make tokens
/// without a document. A value given per appearance (`AdaptiveColor`,
/// `AdaptiveShadow`, `AdaptiveGradient`, as the tests written before the
/// theme format give them) sets its light value: a token has one.
#[cfg(test)]
impl Tokens {
    pub(crate) fn insert(&mut self, name: &str, token: Token) {
        self.values.insert(name.into(), token);
    }

    pub(crate) fn remove_named(&mut self, name: &str) {
        self.values.remove(name);
    }

    pub(crate) fn set_color(&mut self, name: &str, color: impl Into<super::AdaptiveColor>) {
        self.insert(name, Token::Color(color.into().light));
    }

    /// `self` with the tokens of `base` it does not have, and the roles of
    /// both, its own winning.
    pub(crate) fn over(&self, base: &Tokens) -> Tokens {
        let mut out = base.clone();
        for (name, token) in &self.values {
            out.values.insert(name.clone(), token.clone());
        }
        for (role, token) in &self.roles {
            out.roles.insert(role.clone(), token.clone());
        }
        out
    }

    pub(crate) fn set_number(&mut self, name: &str, value: f32) {
        self.insert(name, Token::Number(value));
    }

    pub(crate) fn set_font(&mut self, name: &str, family: impl Into<Arc<str>>) {
        self.insert(name, Token::Font(family.into()));
    }

    pub(crate) fn set_shadow(&mut self, name: &str, layers: impl Into<super::AdaptiveShadow>) {
        self.insert(name, Token::Shadow(layers.into().light));
    }

    pub(crate) fn set_gradient(
        &mut self,
        name: &str,
        gradient: impl Into<super::AdaptiveGradient>,
    ) {
        match gradient.into().light.first() {
            Some(g) => self.insert(name, Token::Gradient(g.clone())),
            None => self.remove_named(name),
        }
    }

    pub(crate) fn set_easing(&mut self, name: &str, value: CubicBezier) {
        self.insert(name, Token::Easing(value));
    }

    pub(crate) fn set_typography(&mut self, name: &str, value: Typography) {
        self.insert(name, Token::Typography(value));
    }

    /// These tokens with `role` mapped onto the token named `token`.
    pub(crate) fn with_role<V>(self, role: Role<V>, token: impl Into<Box<str>>) -> Self {
        self.with_roles([(role, token)])
    }

    /// These tokens with each of `roles` (a [`Role`] or a name) mapped onto
    /// the token named.
    pub(crate) fn with_roles<R: RoleName, S: Into<Box<str>>>(
        mut self,
        roles: impl IntoIterator<Item = (R, S)>,
    ) -> Self {
        for (role, token) in roles {
            self.roles
                .insert(role.role_name().into(), Some(token.into()));
        }
        self
    }

    /// A patch of these tokens' values and roles: how a test overrides.
    pub(crate) fn to_patch(&self) -> Patch {
        let mut patch = Patch::new();
        let mut names: Vec<&str> = self.values.keys().map(|n| &**n).collect();
        names.sort_unstable();
        for name in names {
            patch.set(name, &super::theme_values::value_of(&self.values[name]));
        }
        let mut roles: Vec<(&str, &str)> = self.roles().collect();
        roles.sort_unstable();
        for (role, token) in roles {
            patch.set_role(role, token);
        }
        patch
    }

    /// A theme document that declares each of these tokens with its value
    /// and maps their roles: how a test hands values to a core.
    pub(crate) fn to_document(&self) -> ThemeDocument {
        let mut names: Vec<&str> = self.values.keys().map(|n| &**n).collect();
        names.sort_unstable();
        let mut document = ThemeDocument::new();
        for name in names {
            let token = &self.values[name];
            document.insert_token(
                name,
                nsh_theme::Token::new(token.value_type(), super::theme_values::value_of(token)),
            );
        }
        let mut roles: Vec<(&str, &str)> = self.roles().collect();
        roles.sort_unstable();
        for (role, token) in roles {
            if document.token(token).is_some() {
                document.set_role(role, token);
            }
        }
        document
    }
}

/// A role by its constant or its name, for the tests' [`Tokens::with_roles`].
#[cfg(test)]
pub(crate) trait RoleName {
    fn role_name(&self) -> &str;
}

#[cfg(test)]
impl<V> RoleName for Role<V> {
    fn role_name(&self) -> &str {
        self.name
    }
}

#[cfg(test)]
impl RoleName for &str {
    fn role_name(&self) -> &str {
        self
    }
}

/// Tokens as apps have them (`nsh-base`'s default theme's light values and
/// a few of an app's own), for the core's tests.
#[cfg(test)]
pub(crate) fn sample() -> Tokens {
    let mut tokens = Tokens::default();
    for (key, value) in SAMPLE_COLORS {
        tokens.set_color(key, crate::color::rgb(*value));
    }
    for (key, n) in [("radius", 12.0), ("radius-lg", 16.0), ("body-size", 16.0)] {
        tokens.set_number(key, n);
    }
    tokens
}

/// The sample's colors: name and value.
#[cfg(test)]
const SAMPLE_COLORS: &[(&str, u32)] = &[
    ("background", 0xffffff),
    ("foreground", 0x0a0a0a),
    ("border", 0xe5e5e5),
    ("ring", 0xa3a3a3),
    ("muted", 0xf5f5f5),
    ("muted-foreground", 0x737373),
    ("accent", 0xf5f5f5),
    ("primary", 0x171717),
    ("primary-active", 0x0a0a0a),
    ("primary-foreground", 0xfafafa),
    ("popover", 0xffffff),
    ("popover-foreground", 0x0a0a0a),
];
