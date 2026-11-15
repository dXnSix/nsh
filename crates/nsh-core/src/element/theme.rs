//! Declared tokens: what theme documents say their tokens are. A theme
//! document is a theme file (`cssVars`, the values) with a `tokens` section
//! that declares each token's kind, its path in the designer's tree and its
//! doc, and a `roles` section that maps the roles NSH's own parts draw with
//! onto its tokens:
//!
//! ```json
//! {
//!   "cssVars": { "theme": { "radius-card": "12px" }, "light": { "surface": "#fff" } },
//!   "tokens": {
//!     "surface": { "kind": "color", "path": "colors.surface", "doc": "Cards and popovers." },
//!     "radius-card": { "kind": "radius", "path": "radius.card" }
//!   },
//!   "roles": { "popover": "surface" }
//! }
//! ```
//!
//! A UI set ships its tokens as a document ([`super::Registry::theme_document`]),
//! and an app's own theme file (`nsh.json`'s `"theme"`, TS's `setTheme`)
//! may declare tokens too. Declared tokens replace the default token source
//! (`nsh-base`'s shadcn theme), and a theme file is read by them
//! ([`Declarations`]): a name no document declares is left out, a value is
//! read as its token's kind.
//!
//! The tokens stay flat and by name: classes name them (`bg-surface`,
//! `rounded-card`), TS reads them with `token()`, Rust with
//! [`super::token`] or through typed keys ([`super::TokenKey`], which
//! `theme_keys!` makes of a document).

use std::sync::Arc;

use rustc_hash::FxHashMap;

use super::{AnyRole, RoleKind, Tokens};

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
    /// Gradients per appearance (`background-image-*`: `bg-*`).
    Gradient,
}

impl TokenKind {
    /// The kind [`Tokens`] keeps it as: a namespaced number is a number.
    pub(crate) fn stored(self) -> super::tokens::Kind {
        use super::tokens::Kind;
        match self {
            TokenKind::Color => Kind::Color,
            TokenKind::Font => Kind::Font,
            TokenKind::Shadow => Kind::Shadow,
            TokenKind::Gradient => Kind::Gradient,
            _ => Kind::Number,
        }
    }

    /// Its name, as a theme document and the toolchain write it.
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
            TokenKind::Gradient => "gradient",
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
            "gradient" => TokenKind::Gradient,
            _ => return None,
        })
    }

    /// What a role its token plays reads (`None`: no role reads box
    /// shadows or gradients).
    pub(crate) fn role_kind(self) -> Option<RoleKind> {
        match self {
            TokenKind::Color => Some(RoleKind::Color),
            TokenKind::Font => Some(RoleKind::Font),
            TokenKind::Shadow | TokenKind::Gradient => None,
            _ => Some(RoleKind::Number),
        }
    }

    /// Whether its value is a number token.
    pub fn is_number(self) -> bool {
        !matches!(
            self,
            TokenKind::Color | TokenKind::Font | TokenKind::Shadow | TokenKind::Gradient
        )
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
            TokenKind::Gradient => super::theme_file::GRADIENTS,
        }
    }

    /// Whether `tokens` hold `name` as this kind.
    pub(crate) fn holds(self, tokens: &Tokens, name: &str) -> bool {
        match self {
            TokenKind::Color => tokens.color(name).is_some(),
            TokenKind::Font => tokens.font(name).is_some(),
            TokenKind::Shadow => tokens.shadow(name).is_some(),
            TokenKind::Gradient => tokens.gradient(name).is_some(),
            _ => tokens.number(name).is_some(),
        }
    }

    /// Whether a token of this kind may be named `name`: its form says the
    /// kind, as a theme file reads it, and a namespaced kind's name is in
    /// its namespace, which its classes read it by.
    pub(crate) fn names(self, name: &str) -> bool {
        self.stored().names(name) && name.starts_with(self.namespace())
    }
}

/// One token a theme document declares (its `tokens` section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// Who declares it: the UI set whose document it is, or the app's
    /// theme file.
    pub owner: Box<str>,
    /// What it holds.
    pub kind: TokenKind,
    /// Its place in the designer's tree (`colors.backgrounds.primary`), if
    /// the document gives one.
    pub path: Option<Box<str>>,
    /// Its doc, for the toolchain's declarations (`tokens.d.ts`).
    pub doc: Box<str>,
}

/// The tokens theme documents declare, merged: each name's kind, owner,
/// path and doc (one owner per name), the roles they map (one token per
/// role) and the union of their default values. Tokens read by them carry
/// them ([`Tokens::declarations`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Declarations {
    names: FxHashMap<Box<str>, Declared>,
    /// The defaults of every document, with the roles they map.
    defaults: Tokens,
}

impl Declarations {
    /// The kind token `name` is declared as.
    pub fn kind_of(&self, name: &str) -> Option<TokenKind> {
        self.names.get(name).map(|d| d.kind)
    }

    /// The path token `name` is declared at.
    pub fn path_of(&self, name: &str) -> Option<&str> {
        self.names.get(name)?.path.as_deref()
    }

    /// The declared token at `path`.
    pub fn name_at(&self, path: &str) -> Option<&str> {
        self.names
            .iter()
            .find(|(_, d)| d.path.as_deref() == Some(path))
            .map(|(name, _)| &**name)
    }

    /// The declaration of token `name`.
    pub fn get(&self, name: &str) -> Option<&Declared> {
        self.names.get(name)
    }

    /// Each declared token, sorted by name.
    pub fn tokens(&self) -> Vec<(&str, &Declared)> {
        let mut all: Vec<_> = self.names.iter().map(|(n, d)| (&**n, d)).collect();
        all.sort_unstable_by(|a, b| a.0.cmp(b.0));
        all
    }

    /// Whether no token is declared.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The default values of every document, with the roles they map: the
    /// tokens the app is drawn with when its theme file sets none.
    pub fn defaults(&self) -> &Tokens {
        &self.defaults
    }

    /// The tokens of theme file `json` (`None`: the defaults) read by these
    /// declarations, with what of it was left out: how the toolchain reads
    /// `nsh.json`'s theme as the core does. `Err`: `json` is not JSON.
    pub fn read(&self, json: Option<&str>) -> Result<(Tokens, Vec<String>), String> {
        TokenProvider::Declared(Arc::new(self.clone())).read(json)
    }

    /// Adds the theme document `json` of `owner`: its declared tokens, its
    /// roles and its values, which become defaults. `Err`: the document is
    /// no theme document, says what it cannot (a kind there is not, a value
    /// of another kind, a name it does not declare), or declares a name or
    /// maps a role another owner has, naming both.
    pub(crate) fn add_document(&mut self, owner: &str, json: &str) -> Result<(), String> {
        let value: crate::JsonValue =
            serde_json::from_str(json).map_err(|e| format!("{owner}: not JSON: {e}"))?;
        if !super::theme_file::declares_tokens(&value) {
            return Err(format!(
                "{owner}: a theme document declares its tokens (`\"tokens\"`)"
            ));
        }
        let (declared, mut left_out) = super::theme_file::declared_tokens(&value, owner);
        let mut own = Declarations::default();
        own.extend(owner, declared.clone(), &[])?;
        let (defaults, read_out) =
            super::theme_file::overrides_declared(json, &Tokens::default(), &own)?;
        left_out.extend(read_out);
        for (name, d) in &declared {
            if !d.kind.holds(&defaults, name) && !left_out.iter().any(|l| l.is_about(name)) {
                left_out.push(super::theme_file::LeftOut::no_value(name));
            }
        }
        if !left_out.is_empty() {
            let what: Vec<String> = left_out.iter().map(ToString::to_string).collect();
            return Err(format!("{owner}'s theme document: {}", what.join("; ")));
        }
        let plays: Vec<(String, String)> = defaults
            .roles()
            .map(|(role, token)| (role.to_owned(), token.to_owned()))
            .collect();
        self.extend(owner, declared, &plays)?;
        self.defaults = defaults.over(&self.defaults);
        Ok(())
    }

    /// Checks that each role of `roles` (what UI sets list, and the core's)
    /// a document maps reads what its token holds. `Err` names the role
    /// and the token.
    pub(crate) fn check_roles(&self, roles: &[super::RoleSpec]) -> Result<(), String> {
        let core = super::role::all();
        for spec in roles.iter().chain(&core) {
            let name = spec.role.name();
            let Some(token) = self.defaults.mapped(name) else {
                continue;
            };
            let Some(kind) = self.kind_of(token) else {
                continue;
            };
            if kind.role_kind() != Some(spec.role.kind()) {
                return Err(format!(
                    "the role `{name}` reads {}, and `{token}` is a {} token",
                    match spec.role.kind() {
                        RoleKind::Color => "a color",
                        RoleKind::Number => "a number",
                        RoleKind::Font => "a font family",
                    },
                    kind.name()
                ));
            }
        }
        Ok(())
    }

    /// Adds `declared`, owned by `owner`, and the roles `plays` maps onto
    /// them (role, token). `Err`: a name or a role another owner has,
    /// naming both, a role mapped twice, or one that reads another kind
    /// than its token holds.
    fn extend(
        &mut self,
        owner: &str,
        declared: Vec<(Box<str>, Declared)>,
        plays: &[(String, String)],
    ) -> Result<(), String> {
        for (i, (name, d)) in declared.iter().enumerate() {
            if declared[..i].iter().any(|(n, _)| n == name) {
                return Err(format!("{owner} declares the theme token `{name}` twice"));
            }
            if let Some(other) = self.names.get(name) {
                return Err(format!(
                    "the theme token `{name}` is declared by both {} and {owner}",
                    other.owner
                ));
            }
            debug_assert!(d.kind.names(name));
        }
        let mut roles = Vec::new();
        for (role, token) in plays {
            let Some((_, d)) = declared.iter().find(|(n, _)| **n == **token) else {
                return Err(format!(
                    "{owner}: the role `{role}` is mapped onto `{token}`, which it does not declare"
                ));
            };
            let Some(kind) = d.kind.role_kind() else {
                return Err(format!(
                    "{owner}: the role `{role}` is mapped onto `{token}`, a {} token, which no role reads",
                    d.kind.name()
                ));
            };
            if let Some(other) = self.role_owner(role) {
                return Err(format!(
                    "the role `{role}` is mapped by both {other} and {owner}"
                ));
            }
            roles.push((AnyRole::new(role.clone(), kind), Box::<str>::from(&**token)));
        }
        for (name, d) in declared {
            self.names.insert(name, d);
        }
        if !roles.is_empty() {
            self.defaults = std::mem::take(&mut self.defaults).with_roles(roles);
        }
        Ok(())
    }

    /// These declarations with the tokens theme file `value` declares
    /// itself (the app's own, owned by `owner`), and what of them was left
    /// out: a name another owner declares as the same kind is theirs, as
    /// another kind it is left out.
    pub(crate) fn with_file(
        &self,
        value: &crate::JsonValue,
        owner: &str,
    ) -> (Declarations, Vec<super::theme_file::LeftOut>) {
        let (declared, mut left_out) = super::theme_file::declared_tokens(value, owner);
        let mut out = self.clone();
        let mut own = Vec::new();
        for (name, d) in declared {
            match self.names.get(&name) {
                Some(other) if other.kind == d.kind => {}
                Some(other) => left_out.push(super::theme_file::LeftOut::declared_by(
                    &name,
                    other.kind,
                    &other.owner,
                )),
                None => own.push((name, d)),
            }
        }
        if !own.is_empty() {
            // Only names and roles no other owner has are left: this adds.
            let _ = out.extend(owner, own, &[]);
        }
        (out, left_out)
    }

    /// The owner mapping the role named `role`, if any.
    fn role_owner(&self, role: &str) -> Option<&str> {
        self.defaults
            .mapped(role)
            .and_then(|name| self.names.get(name))
            .map(|d| &*d.owner)
    }

    /// Each role the documents map onto a token of another name: the role's
    /// name and the token's.
    pub(crate) fn renamed_roles(&self) -> impl Iterator<Item = (&str, &str)> {
        self.defaults
            .roles()
            .filter(|(role, token)| role != token && self.names.contains_key(*token))
    }

    /// Each declared name with its kind.
    pub(crate) fn names(&self) -> impl Iterator<Item = (&str, TokenKind)> {
        self.names.iter().map(|(name, d)| (&**name, d.kind))
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

/// Who declares the tokens an app's own theme file declares.
pub(crate) const APP_THEME: &str = "the app's theme";

/// How the core reads a theme file: with a UI set's functions
/// ([`super::Registry::tokens_from`], [`super::Registry::token_overrides_from`])
/// while nothing declares tokens, or by the declared tokens.
#[derive(Clone)]
pub(crate) enum TokenProvider {
    Source {
        read: super::TokenSource,
        overrides: Option<super::TokenOverrides>,
    },
    Declared(Arc<Declarations>),
}

impl std::fmt::Debug for TokenProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenProvider::Source { .. } => f.write_str("TokenProvider::Source"),
            TokenProvider::Declared(set) => {
                f.debug_tuple("TokenProvider::Declared").field(set).finish()
            }
        }
    }
}

impl TokenProvider {
    /// The token a theme file's `name` names: under declared tokens the
    /// token a role of that name is mapped onto ([`Declarations::target`]),
    /// as `setTheme` and `updateTheme` read it; any other name, and every
    /// name without declared tokens, is its own.
    pub(crate) fn token_name<'a>(&'a self, name: &'a str) -> &'a str {
        match self {
            TokenProvider::Declared(set) => set.target(name).unwrap_or(name),
            TokenProvider::Source { .. } => name,
        }
    }

    /// The tokens of theme file `json` over the defaults (`None`: the
    /// defaults), with what of it was left out.
    pub(crate) fn read(&self, json: Option<&str>) -> Result<(Tokens, Vec<String>), String> {
        let (tokens, left_out) = self.read_left_out(json)?;
        Ok((tokens, super::theme_file::sentences(left_out)))
    }

    /// [`TokenProvider::read`], each thing left out with what the core
    /// asks of it (whether it is only a name nothing declares). A theme
    /// file that declares tokens is read by them, with the documents' own,
    /// and replaces a UI set's defaults.
    pub(crate) fn read_left_out(
        &self,
        json: Option<&str>,
    ) -> Result<(Tokens, Vec<super::theme_file::LeftOut>), String> {
        let set = match self {
            TokenProvider::Declared(set) => set.clone(),
            TokenProvider::Source { read, .. } => {
                let declares = json
                    .and_then(|json| serde_json::from_str::<crate::JsonValue>(json).ok())
                    .is_some_and(|value| super::theme_file::declares_tokens(&value));
                if !declares {
                    return read(json).map(said);
                }
                Arc::new(Declarations::default())
            }
        };
        let Some(json) = json else {
            return Ok((with_declarations(set.defaults.clone(), &set), Vec::new()));
        };
        let value: crate::JsonValue =
            serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
        let (set, mut left_out) = set.with_file(&value, APP_THEME);
        let set = Arc::new(set);
        let (tokens, read_out) = super::theme_file::parse_declared(json, &set.defaults, &set)?;
        left_out.extend(read_out);
        Ok((with_declarations(tokens, &set), left_out))
    }

    /// The tokens theme file `json` sets, and only those (`var()` against
    /// `base`); `None`: nothing reads changes.
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
    /// The tokens drawn with now (`base`) are read by what declares them
    /// (the documents and the app's theme file); a change may declare
    /// tokens of its own beside them.
    pub(crate) fn overrides_left_out(
        &self,
        json: &str,
        base: &Tokens,
    ) -> Option<Result<(Tokens, Vec<super::theme_file::LeftOut>), String>> {
        let set = match (base.declarations_arc(), self) {
            (Some(set), _) => set,
            (None, TokenProvider::Declared(set)) => set.clone(),
            (None, TokenProvider::Source { overrides, .. }) => {
                return Some((*overrides)?(json, base).map(said));
            }
        };
        Some((|| {
            let value: crate::JsonValue =
                serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
            let (set, mut left_out) = set.with_file(&value, APP_THEME);
            let set = Arc::new(set);
            let (tokens, read_out) = super::theme_file::overrides_declared(json, base, &set)?;
            left_out.extend(read_out);
            Ok((with_declarations(tokens, &set), left_out))
        })())
    }
}

/// `tokens`, read by `set`, carrying it.
fn with_declarations(mut tokens: Tokens, set: &Arc<Declarations>) -> Tokens {
    tokens.set_declarations(Some(set.clone()));
    tokens
}

/// A UI set's own reader says what it left out in sentences alone.
fn said((tokens, left_out): (Tokens, Vec<String>)) -> (Tokens, Vec<super::theme_file::LeftOut>) {
    let left_out = left_out
        .into_iter()
        .map(super::theme_file::LeftOut::said)
        .collect();
    (tokens, left_out)
}
