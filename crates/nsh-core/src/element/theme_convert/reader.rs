//! The old reader of `cssVars` theme files, moved from `theme_file.rs`:
//! shadcn's `cssVars` (`{ "cssVars": { "theme": { … }, "light": { … },
//! "dark": { … } } }`), values as CSS writes them (colors, `var(--ring)`,
//! `color-mix()`, lengths in `rem` or `px`, font family lists), read into
//! the old per-appearance [`Store`] by name, with the kinds a name's form
//! guesses; and theme documents of that format, whose `"tokens"` section
//! declares each token's kind, path and doc, read by those declarations.
//! What it cannot read is left out and listed ([`LeftOut`]).
//!
//! Only the converter reads with it: the runtime reads the theme format.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{
    modifiers::Modifiers,
    order::Ordered,
    store::{Kind, Pair, Store, TYPOGRAPHY_PARTS, easing},
};
use crate::{Color, JsonValue, color::Hsla};

/// Points per `rem`, as Tailwind's theme has them.
const REM: f32 = 16.0;

/// One thing the reader left out of a theme file: its sentence, with the
/// token it starts with (in backquotes).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LeftOut {
    token: Option<Box<str>>,
    sentence: String,
}

impl LeftOut {
    /// `` `token` `` followed by `rest` (`": no color value"`).
    pub(super) fn about(token: &str, rest: impl std::fmt::Display) -> Self {
        Self {
            token: Some(token.into()),
            sentence: format!("`{token}`{rest}"),
        }
    }

    /// A sentence about no one token.
    pub(super) fn said(sentence: impl Into<String>) -> Self {
        Self {
            token: None,
            sentence: sentence.into(),
        }
    }

    /// `name` is a name no theme document declares.
    fn undeclared(name: &str) -> Self {
        Self::about(name, ": no theme document declares it")
    }

    /// `name` is declared as another kind (`kind`) by `owner`.
    fn declared_by(name: &str, kind: DeclaredKind, owner: &str) -> Self {
        Self::about(
            name,
            format_args!(": {owner} declares it, as a {} token", kind.name()),
        )
    }

    /// The same, said of a part of the file at `at`
    /// (`modifiers.brand.contexts.forest: …`).
    pub(super) fn within(self, at: &str) -> Self {
        Self {
            sentence: format!("{at}: {}", self.sentence),
            ..self
        }
    }

    /// Whether its sentence starts with the token `name`.
    fn is_about(&self, name: &str) -> bool {
        self.token.as_deref() == Some(name)
    }

    /// The token its sentence starts with.
    pub(super) fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

impl std::fmt::Display for LeftOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.sentence)
    }
}

// ---------------------------------------------------------------------------
// Declared tokens (`theme.rs`'s `TokenKind`, `Declared`, `Declarations`)

/// What a declared token holds, as an old theme document's `kind` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum DeclaredKind {
    Color,
    Number,
    Radius,
    Spacing,
    Text,
    Weight,
    Tracking,
    Leading,
    Blur,
    Font,
    Shadow,
    Gradient,
    String,
    Boolean,
    Duration,
    Easing,
    Typography,
}

impl DeclaredKind {
    /// The kind the store keeps it as: a namespaced number is a number.
    fn stored(self) -> Kind {
        match self {
            DeclaredKind::Color => Kind::Color,
            DeclaredKind::Font => Kind::Font,
            DeclaredKind::Shadow => Kind::Shadow,
            DeclaredKind::Gradient => Kind::Gradient,
            DeclaredKind::String => Kind::String,
            DeclaredKind::Boolean => Kind::Boolean,
            DeclaredKind::Easing => Kind::Easing,
            _ => Kind::Number,
        }
    }

    /// Its name, as an old theme document writes it.
    pub(super) fn name(self) -> &'static str {
        match self {
            DeclaredKind::Color => "color",
            DeclaredKind::Number => "number",
            DeclaredKind::Radius => "radius",
            DeclaredKind::Spacing => "spacing",
            DeclaredKind::Text => "text",
            DeclaredKind::Weight => "weight",
            DeclaredKind::Tracking => "tracking",
            DeclaredKind::Leading => "leading",
            DeclaredKind::Blur => "blur",
            DeclaredKind::Font => "font",
            DeclaredKind::Shadow => "shadow",
            DeclaredKind::Gradient => "gradient",
            DeclaredKind::String => "string",
            DeclaredKind::Boolean => "boolean",
            DeclaredKind::Duration => "duration",
            DeclaredKind::Easing => "easing",
            DeclaredKind::Typography => "typography",
        }
    }

    /// Every kind's name, as a sentence lists them.
    const NAMES: &'static str = "color, number, radius, spacing, text, weight, tracking, \
        leading, blur, font, shadow, gradient, string, boolean, duration, easing, typography";

    /// The kind of this name.
    fn from_name(name: &str) -> Option<DeclaredKind> {
        Some(match name {
            "color" => DeclaredKind::Color,
            "number" => DeclaredKind::Number,
            "radius" => DeclaredKind::Radius,
            "spacing" => DeclaredKind::Spacing,
            "text" => DeclaredKind::Text,
            "weight" => DeclaredKind::Weight,
            "tracking" => DeclaredKind::Tracking,
            "leading" => DeclaredKind::Leading,
            "blur" => DeclaredKind::Blur,
            "font" => DeclaredKind::Font,
            "shadow" => DeclaredKind::Shadow,
            "gradient" => DeclaredKind::Gradient,
            "string" => DeclaredKind::String,
            "boolean" => DeclaredKind::Boolean,
            "duration" => DeclaredKind::Duration,
            "easing" => DeclaredKind::Easing,
            "typography" => DeclaredKind::Typography,
            _ => return None,
        })
    }

    /// Whether a role may read a token of this kind (a color, a number or
    /// a font family).
    fn reads_role(self) -> bool {
        !matches!(
            self,
            DeclaredKind::Shadow
                | DeclaredKind::Gradient
                | DeclaredKind::String
                | DeclaredKind::Boolean
                | DeclaredKind::Easing
        )
    }

    /// The namespace its names take; a color's and a number's is none.
    fn namespace(self) -> &'static str {
        match self {
            DeclaredKind::Color
            | DeclaredKind::Number
            | DeclaredKind::String
            | DeclaredKind::Boolean => "",
            DeclaredKind::Duration => "duration-",
            DeclaredKind::Easing => "ease-",
            DeclaredKind::Typography => "text-",
            DeclaredKind::Radius => "radius-",
            DeclaredKind::Spacing => "spacing-",
            DeclaredKind::Text => "text-",
            DeclaredKind::Weight => "font-weight-",
            DeclaredKind::Tracking => "tracking-",
            DeclaredKind::Leading => "leading-",
            DeclaredKind::Blur => "blur-",
            DeclaredKind::Font => "font-",
            DeclaredKind::Shadow => "shadow-",
            DeclaredKind::Gradient => GRADIENTS,
        }
    }

    /// Whether `tokens` hold `name` as this kind.
    fn holds(self, tokens: &Store, name: &str) -> bool {
        match self {
            DeclaredKind::Color => tokens.color(name).is_some(),
            DeclaredKind::Font => tokens.font(name).is_some(),
            DeclaredKind::Shadow => tokens.shadow(name).is_some(),
            DeclaredKind::Gradient => tokens.gradient(name).is_some(),
            DeclaredKind::String => tokens.string(name).is_some(),
            DeclaredKind::Boolean => tokens.boolean(name).is_some(),
            DeclaredKind::Easing => tokens.easing(name).is_some(),
            _ => tokens.number(name).is_some(),
        }
    }

    /// Whether a token of this kind may be named `name`.
    fn names(self, name: &str) -> bool {
        self.stored().names(name) && name.starts_with(self.namespace())
    }
}

/// One token an old theme document declares (its `tokens` section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Declared {
    /// Who declares it: the base's document or the app's theme file.
    pub(super) owner: Box<str>,
    pub(super) kind: DeclaredKind,
    pub(super) path: Option<Box<str>>,
    pub(super) doc: Box<str>,
    /// The typography token it is a part of (`text-title` of
    /// `text-title--line-height`).
    pub(super) part_of: Option<Box<str>>,
}

/// The tokens old theme documents declare: each name's kind, owner, path
/// and doc, and the values read by them, with the
/// roles they map.
#[derive(Debug, Clone, Default)]
pub(super) struct Declarations {
    names: FxHashMap<Box<str>, Declared>,
    defaults: Store,
}

impl Declarations {
    /// The declaration of token `name`.
    pub(super) fn get(&self, name: &str) -> Option<&Declared> {
        self.names.get(name)
    }

    fn kind_of(&self, name: &str) -> Option<DeclaredKind> {
        self.names.get(name).map(|d| d.kind)
    }

    /// The declared token at `path`.
    fn name_at(&self, path: &str) -> Option<&str> {
        self.names
            .iter()
            .find(|(_, d)| d.path.as_deref() == Some(path))
            .map(|(name, _)| &**name)
    }

    fn insert(&mut self, name: Box<str>, declared: Declared) {
        self.names.insert(name, declared);
    }

    /// These declarations with the tokens theme file `value` declares
    /// itself (owned by `owner`), and what of them was left out: a name
    /// another owner declares as the same kind is theirs, as another kind
    /// it is left out.
    fn with_file(&self, value: &JsonValue, owner: &str) -> (Declarations, Vec<LeftOut>) {
        let (declared, mut left_out) = declared_tokens(value, owner);
        let mut out = self.clone();
        for (name, d) in declared {
            match self.names.get(&name) {
                Some(other) if other.kind == d.kind => {}
                Some(other) => {
                    left_out.push(LeftOut::declared_by(&name, other.kind, &other.owner));
                }
                None => match d.path.as_deref().and_then(|path| self.name_at(path)) {
                    Some(other) => left_out.push(LeftOut::about(
                        &format!("tokens.{name}.path"),
                        format_args!(
                            ": `{}` is the path of `{other}`, which {} declares",
                            d.path.as_deref().unwrap_or_default(),
                            self.names[other].owner
                        ),
                    )),
                    None => out.insert(name, d),
                },
            }
        }
        (out, left_out)
    }

    /// Each role the documents map onto a token of another name: the
    /// role's name and the token's.
    fn renamed_roles(&self) -> impl Iterator<Item = (&str, &str)> {
        self.defaults
            .roles()
            .filter(|(role, token)| role != token && self.names.contains_key(*token))
    }

    /// Each declared name with its kind.
    fn names(&self) -> impl Iterator<Item = (&str, DeclaredKind)> {
        self.names.iter().map(|(name, d)| (&**name, d.kind))
    }

    /// The name a theme file's `key` sets: a declared name, else the token
    /// a role of that name (`primary`) is mapped onto.
    pub(super) fn target<'a>(&'a self, key: &'a str) -> Option<&'a str> {
        if self.names.contains_key(key) {
            return Some(key);
        }
        self.defaults
            .mapped(key)
            .filter(|name| self.names.contains_key(*name))
    }
}

// ---------------------------------------------------------------------------
// Reading (`theme_file.rs`)

/// The tokens of theme file `json` over `base`, with what it left out. A
/// color it names and does not set (`var(--ring)`) is `base`'s. `Err`:
/// `json` is not JSON.
pub(super) fn parse(json: &str, base: &Store) -> Result<(Store, Vec<LeftOut>), String> {
    let mut tokens = base.clone();
    let value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let ignored = read_into(&mut tokens, base, &value);
    Ok((tokens, ignored))
}

/// The tokens theme file `json` sets, and only those, with what of it was
/// left out: a modifier's context. A color it names and does not set is
/// `base`'s. `Err`: `json` is not JSON.
pub(super) fn overrides(json: &str, base: &Store) -> Result<(Store, Vec<LeftOut>), String> {
    let mut tokens = Store::default();
    let value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let ignored = read_into(&mut tokens, base, &value);
    Ok((tokens, ignored))
}

/// [`parse`] by declared tokens: each name the file sets must be a declared
/// one (a role's name, `primary`, sets the token the role is mapped onto),
/// and holds a value of its declared kind; any other is left out and listed.
pub(super) fn parse_declared(
    json: &str,
    base: &Store,
    set: &Declarations,
) -> Result<(Store, Vec<LeftOut>), String> {
    let (tokens, left_out) = overrides_declared(json, base, set)?;
    Ok((tokens.over(base), left_out))
}

/// The sections an NSH theme document has (and `$schema`, an editor's).
const SECTIONS: [&str; 5] = ["cssVars", "tokens", "modifiers", "roles", "$schema"];

/// A sentence for each top-level member of theme file `value` that no
/// reader knows, when it is an NSH theme document (it has `"tokens"` or
/// `"modifiers"`).
pub(super) fn unknown_sections(value: &JsonValue) -> Vec<LeftOut> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    if !object.contains_key("tokens") && !object.contains_key("modifiers") {
        return Vec::new();
    }
    object
        .keys()
        .filter(|k| !SECTIONS.contains(&k.as_str()) && k.as_str() != "name")
        .map(|k| {
            LeftOut::said(format!(
                "{k}: no such section of a theme (expected `cssVars`, `tokens`, `modifiers`, `roles`)"
            ))
        })
        .collect()
}

/// Whether theme file `value` declares its tokens: it has a `"tokens"`
/// section.
pub(super) fn declares_tokens(value: &JsonValue) -> bool {
    value
        .get("tokens")
        .and_then(JsonValue::as_object)
        .is_some_and(|tokens| !tokens.is_empty())
}

/// The tokens theme document `value` declares in its `"tokens"` section,
/// owned by `owner`, with what of it was left out: a kind there is not, a
/// name of another kind's form, a member of no known form.
fn declared_tokens(value: &JsonValue, owner: &str) -> (Vec<(Box<str>, Declared)>, Vec<LeftOut>) {
    let mut declared: Vec<(Box<str>, Declared)> = Vec::new();
    let mut left_out = Vec::new();
    let Some(section) = value.get("tokens") else {
        return (declared, left_out);
    };
    let Some(tokens) = section.as_object() else {
        left_out.push(LeftOut::said(
            "tokens: expected an object of token names and their declarations",
        ));
        return (declared, left_out);
    };
    for (name, entry) in tokens {
        let at = format!("tokens.{name}");
        let Some(entry) = entry.as_object() else {
            left_out.push(LeftOut::about(&at, ": expected `{ \"kind\": … }`"));
            continue;
        };
        let kind = match entry.get("kind").and_then(JsonValue::as_str) {
            Some(kind) => match DeclaredKind::from_name(kind) {
                Some(kind) => kind,
                None => {
                    left_out.push(LeftOut::about(
                        &at,
                        format_args!(": no kind `{kind}` ({})", DeclaredKind::NAMES),
                    ));
                    continue;
                }
            },
            None => {
                left_out.push(LeftOut::about(&at, ": no `kind`"));
                continue;
            }
        };
        if name.is_empty() || !kind.names(name) {
            left_out.push(LeftOut::about(
                &at,
                format_args!(
                    ": a {} token is named in its namespace `{}`, and {}",
                    kind.name(),
                    kind.namespace(),
                    Kind::FORMS
                ),
            ));
            continue;
        }
        let text = |key: &str| entry.get(key).and_then(JsonValue::as_str);
        // A path names one token.
        if let Some(path) = text("path") {
            let taken = declared
                .iter()
                .find(|(_, d)| d.path.as_deref() == Some(path));
            if let Some((other, _)) = taken {
                left_out.push(LeftOut::about(
                    &format!("{at}.path"),
                    format_args!(": `{path}` is the path of `{other}`"),
                ));
                continue;
            }
        }
        for (key, v) in entry {
            if !matches!(key.as_str(), "kind" | "path" | "doc") || !(key == "kind" || v.is_string())
            {
                left_out.push(LeftOut::about(
                    &format!("{at}.{key}"),
                    ": expected `kind`, `path` or `doc`, each a string",
                ));
            }
        }
        declared.push((
            Box::<str>::from(name.as_str()),
            Declared {
                owner: owner.into(),
                kind,
                path: text("path").map(Into::into),
                doc: text("doc").unwrap_or_default().into(),
                part_of: None,
            },
        ));
        // A text style declares its parts, numbers of their own names.
        if kind == DeclaredKind::Typography {
            for suffix in TYPOGRAPHY_PARTS {
                declared.push((
                    format!("{name}{suffix}").into(),
                    Declared {
                        owner: owner.into(),
                        kind: DeclaredKind::Number,
                        path: text("path").map(|p| format!("{p}.{}", &suffix[2..]).into()),
                        doc: Box::default(),
                        part_of: Some(name.as_str().into()),
                    },
                ));
            }
        }
    }
    (declared, left_out)
}

/// [`overrides`] by declared tokens, as [`parse_declared`] reads names.
pub(super) fn overrides_declared(
    json: &str,
    base: &Store,
    set: &Declarations,
) -> Result<(Store, Vec<LeftOut>), String> {
    let mut value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let (kept, mut left_out) = declared_names(&mut value, set);
    let mut read = Store::default();
    // Strings, booleans, durations and easings by their declared kind,
    // which their values cannot say.
    left_out.extend(read_plain(&mut read, &mut value, set));
    left_out.extend(read_into(&mut read, base, &value));
    let mut out = Store::default();
    for (name, kind) in set.names() {
        if kind.holds(&read, name) {
            out.copy_named(&read, name);
        } else if kept.iter().any(|k| k == name) && !left_out.iter().any(|l| l.is_about(name)) {
            // Set, and read as no value of its kind.
            left_out.push(LeftOut::about(
                name,
                format_args!(": no {} value", kind.name()),
            ));
        }
    }
    // The file's `"roles"`: a role no document maps may be mapped by the
    // file; a document's mapping is its owner's, and stays.
    let mut roles: Vec<(&str, &str)> = read.roles().collect();
    roles.sort_unstable();
    let mut mapped = Vec::new();
    for (role, token) in roles {
        if let Some(by_document) = set.target(role).filter(|token| *token != role) {
            left_out.push(LeftOut::about(
                &format!("roles.{role}"),
                format_args!(": a theme document maps it onto `{by_document}`"),
            ));
            continue;
        }
        if set.kind_of(token).is_some_and(DeclaredKind::reads_role) {
            mapped.push((role.to_owned(), token.to_owned()));
        }
    }
    if !mapped.is_empty() {
        out = out.with_roles(mapped);
    }
    Ok((out, left_out))
}

/// Makes the names of theme file `value` the declared ones: a role's name
/// becomes the token it is mapped onto (and so in `var()`s), a name nothing
/// declares is left out; returns the names kept and what was left out.
fn declared_names(value: &mut JsonValue, set: &Declarations) -> (Vec<String>, Vec<LeftOut>) {
    let mut left_out = Vec::new();
    let mut kept = Vec::new();
    // The roles the documents map onto tokens of other names.
    let renamed: Vec<(&str, &str)> = set.renamed_roles().collect();
    if let Some(vars) = value.get_mut("cssVars").and_then(JsonValue::as_object_mut) {
        for section in ["theme", "light", "dark"] {
            let Some(JsonValue::Object(fields)) = vars.get_mut(section) else {
                continue;
            };
            let old = std::mem::take(fields);
            // The tokens roles' names set in this section, with the role.
            let mut by_role: Vec<(&str, &str)> = Vec::new();
            let mut keys: Vec<(&String, &JsonValue)> = old.iter().collect();
            keys.sort_by(|a, b| a.0.cmp(b.0));
            for (key, v) in keys {
                match set.target(key) {
                    Some(name) if name == key => {
                        fields.insert(key.clone(), v.clone());
                        kept.push(key.clone());
                    }
                    Some(name) => {
                        if old.contains_key(name) {
                            left_out.push(LeftOut::about(
                                key,
                                format_args!(
                                    ": the token its role is mapped onto, `{name}`, is set too"
                                ),
                            ));
                            continue;
                        }
                        if let Some((_, first)) = by_role.iter().find(|(token, _)| *token == name) {
                            if fields.get(name) != Some(v) {
                                left_out.push(LeftOut::about(
                                    key,
                                    format_args!(
                                        ": `{first}` sets the token its role is mapped onto, `{name}`, too"
                                    ),
                                ));
                            }
                            continue;
                        }
                        by_role.push((name, key));
                        fields.insert(name.to_owned(), v.clone());
                        kept.push(name.to_owned());
                    }
                    None => left_out.push(LeftOut::undeclared(key)),
                }
            }
        }
        // A `var()` naming a role names the token it is mapped onto.
        if !renamed.is_empty() {
            for section in ["theme", "light", "dark"] {
                let Some(JsonValue::Object(fields)) = vars.get_mut(section) else {
                    continue;
                };
                for v in fields.values_mut() {
                    if let JsonValue::String(text) = v
                        && text.contains("var(")
                    {
                        *text = rename_vars(text, &renamed);
                    }
                }
            }
        }
    }
    (kept, left_out)
}

/// `value` with each `var()` that names one of `renamed`'s roles naming
/// its token. Every other character stays.
fn rename_vars(value: &str, renamed: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start + 4]);
        let inner = &rest[start + 4..];
        let mut depth = 1;
        let end = inner
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map(|(i, _)| i);
        let Some(end) = end else {
            rest = inner;
            break;
        };
        let (var, fallback) = match inner[..end].split_once(',') {
            Some((v, f)) => (v, Some(f)),
            None => (&inner[..end], None),
        };
        let token = var
            .trim()
            .strip_prefix("--")
            .and_then(|name| renamed.iter().find(|(role, _)| *role == name))
            .map(|(_, token)| *token);
        match token {
            Some(token) => {
                let name = var.trim();
                let at = var.find(name).unwrap_or(0);
                out.push_str(&var[..at]);
                out.push_str("--");
                out.push_str(token);
                out.push_str(&var[at + name.len()..]);
            }
            None => out.push_str(var),
        }
        if let Some(fallback) = fallback {
            out.push(',');
            out.push_str(&rename_vars(fallback, renamed));
        }
        out.push(')');
        rest = &inner[end + 1..];
    }
    out.push_str(rest);
    out
}

/// [`parse`] of a theme file already parsed.
fn read_into(tokens: &mut Store, base: &Store, value: &JsonValue) -> Vec<LeftOut> {
    let mut ignored = Vec::new();
    match value.get("cssVars") {
        Some(vars) if vars.is_object() => apply_css_vars(tokens, base, vars, &mut ignored),
        Some(_) => ignored.push(LeftOut::said("cssVars: expected an object")),
        None => ignored.push(LeftOut::said("no shadcn `{ \"cssVars\": { ... } }`")),
    }
    if let Some(roles) = value.get("roles") {
        apply_roles(tokens, base, roles, &mut ignored);
    }
    ignored
}

/// The `"roles"` section of a theme file, beside `cssVars`: each role's
/// name with the name of the token it is mapped onto. The token is one the
/// file sets or `base` has, a color, a number or a font family.
fn apply_roles(tokens: &mut Store, base: &Store, roles: &JsonValue, ignored: &mut Vec<LeftOut>) {
    let Some(roles) = roles.as_object() else {
        return ignored.push(LeftOut::said(
            "roles: expected an object of role names and the names of their tokens",
        ));
    };
    let mut names: Vec<&String> = roles.keys().collect();
    names.sort();
    let mut mapped = Vec::new();
    for role in names {
        let key = format!("roles.{role}");
        let Some(token) = roles[role].as_str() else {
            ignored.push(LeftOut::about(&key, ": expected the name of a token"));
            continue;
        };
        // A role reads what its token holds.
        match tokens.kind(token).or_else(|| base.kind(token)) {
            Some(Kind::Color | Kind::Number | Kind::Font) => {}
            Some(_) => {
                ignored.push(LeftOut::about(
                    &key,
                    format_args!(": no role reads what `{token}` holds"),
                ));
                continue;
            }
            None => {
                ignored.push(LeftOut::about(
                    &key,
                    format_args!(": the theme has no token `{token}`"),
                ));
                continue;
            }
        }
        mapped.push((role.clone(), token.to_owned()));
    }
    if !mapped.is_empty() {
        *tokens = std::mem::take(tokens).with_roles(mapped);
    }
}

/// A CSS color.
fn color(value: &str) -> Option<Hsla> {
    Color::parse(value).map(Hsla::from)
}

/// Whether `value` is written as a color, which a theme means as one: a hex
/// color, a CSS color function or a `var()` alone.
fn written_as_color(value: &str) -> bool {
    let value = value.trim();
    if value.starts_with('#') {
        return true;
    }
    let Some((function, rest)) = value.split_once('(') else {
        return false;
    };
    if !matches!(
        function.trim().to_ascii_lowercase().as_str(),
        "rgb"
            | "rgba"
            | "hsl"
            | "hsla"
            | "hwb"
            | "lab"
            | "lch"
            | "oklab"
            | "oklch"
            | "color"
            | "color-mix"
            | "light-dark"
            | "var"
    ) {
        return false;
    }
    if value.ends_with(')') {
        return true;
    }
    // A shadow may start with a color function. Its next component need
    // not be separated by whitespace, but an attached bare name other than
    // inset cannot follow the color.
    let mut depth = 1;
    for (i, c) in rest.char_indices() {
        depth += match c {
            '(' => 1,
            ')' => -1,
            _ => 0,
        };
        if depth == 0 {
            let suffix = &rest[i + 1..];
            let end = suffix
                .find(|c: char| c.is_ascii() && !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(suffix.len());
            let (name, tail) = suffix.split_at(end);
            return name
                .trim_start_matches('-')
                .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || !c.is_ascii())
                && !name.eq_ignore_ascii_case("inset")
                && !tail.starts_with(['(', '\\']);
        }
    }
    true
}

/// A CSS length in points: `0.625rem`, `10px`, `10`.
fn length(value: &str) -> Option<f32> {
    let value = value.trim();
    let (number, scale) = if let Some(n) = value.strip_suffix("rem") {
        (n, REM)
    } else if let Some(n) = value.strip_suffix("px") {
        (n, 1.0)
    } else {
        (value, 1.0)
    };
    number
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|n| n * scale)
}

/// The family of a CSS `font-family` list that the platform draws with:
/// its first, unquoted (`None`: a generic family, the system's font). The
/// old store kept this of a font token.
pub(super) fn font_family(value: &str) -> Option<&str> {
    let first = value.split(',').next()?.trim().trim_matches(['"', '\'']);
    let generic = matches!(
        first,
        "" | "sans-serif"
            | "serif"
            | "system-ui"
            | "ui-sans-serif"
            | "ui-serif"
            | "-apple-system"
            | "BlinkMacSystemFont"
    );
    (!generic).then_some(first)
}

/// A duration as a theme file writes it: `150ms`, `0.2s`, or a plain
/// number of milliseconds; in milliseconds.
fn duration(value: &str) -> Option<f32> {
    let value = value.trim();
    let ms = if let Some(n) = value.strip_suffix("ms") {
        n.trim().parse::<f32>().ok()?
    } else if let Some(n) = value.strip_suffix('s') {
        n.trim().parse::<f32>().ok()? * 1000.0
    } else {
        value.parse::<f32>().ok()?
    };
    (ms.is_finite() && ms >= 0.0).then_some(ms)
}

/// Takes the values of the tokens `set` declares as strings, booleans,
/// durations or easings out of theme file `value`'s `cssVars` and sets them
/// in `tokens`, each read as its kind. Returns what it left out.
fn read_plain(tokens: &mut Store, value: &mut JsonValue, set: &Declarations) -> Vec<LeftOut> {
    use DeclaredKind as K;
    let mut left_out = Vec::new();
    let Some(vars) = value.get_mut("cssVars").and_then(JsonValue::as_object_mut) else {
        return left_out;
    };
    // Each name's value in `theme`, `light` and `dark`.
    let mut found: FxHashMap<String, [Option<JsonValue>; 3]> = FxHashMap::default();
    for (i, section) in ["theme", "light", "dark"].into_iter().enumerate() {
        let Some(JsonValue::Object(fields)) = vars.get_mut(section) else {
            continue;
        };
        let names: Vec<String> = fields
            .keys()
            .filter(|n| {
                matches!(
                    set.kind_of(n),
                    Some(K::String | K::Boolean | K::Duration | K::Easing)
                )
            })
            .cloned()
            .collect();
        for name in names {
            let v = fields.remove(&name);
            found.entry(name).or_default()[i] = v;
        }
    }
    let mut names: Vec<&String> = found.keys().collect();
    names.sort();
    for name in names {
        let [theme, light, dark] = &found[name];
        let light = light.as_ref().or(theme.as_ref());
        let dark = dark.as_ref().or(light);
        let (Some(light), Some(dark)) = (light, dark) else {
            continue;
        };
        let text = |v: &JsonValue| match v {
            JsonValue::String(s) => Some(s.clone()),
            JsonValue::Number(n) => Some(n.to_string()),
            _ => None,
        };
        let kind = set.kind_of(name).unwrap_or(K::String);
        if kind == K::Duration {
            match (
                text(light).and_then(|v| duration(&v)),
                text(dark).and_then(|v| duration(&v)),
            ) {
                (Some(l), Some(d)) => {
                    tokens.set_number(name, l);
                    if d != l {
                        tokens.set_dark_number(name, d);
                    }
                }
                _ => left_out.push(LeftOut::about(
                    name,
                    format_args!(": {dark} is no duration (`150ms`, `0.2s`)"),
                )),
            }
            continue;
        }
        if light != dark {
            left_out.push(LeftOut::about(
                name,
                format_args!(": a {} is one value in both appearances", kind.name()),
            ));
            continue;
        }
        let ok = match kind {
            K::String => match light {
                JsonValue::String(s) => {
                    tokens.set_string(name, s.as_str());
                    true
                }
                _ => false,
            },
            K::Boolean => match light {
                JsonValue::Bool(b) => {
                    tokens.set_boolean(name, *b);
                    true
                }
                JsonValue::String(s) if s == "true" || s == "false" => {
                    tokens.set_boolean(name, s == "true");
                    true
                }
                _ => false,
            },
            _ => match text(light).and_then(|v| easing::parse(&v)) {
                Some(e) => {
                    tokens.set_easing(name, e);
                    true
                }
                None => false,
            },
        };
        if !ok {
            left_out.push(LeftOut::about(
                name,
                format_args!(": {light} is no {}", kind.name()),
            ));
        }
    }
    left_out
}

/// Applies shadcn's `cssVars`: `theme`'s values in both appearances,
/// `light`'s in the light one (and the dark one when `dark` leaves them
/// out), `dark`'s in the dark one. A color may name another (`var(--ring)`,
/// in `color-mix()` too) of the same appearance: the file's or the one
/// under it. A `shadow-*` value that reads as CSS `box-shadow` layers is a
/// shadow, a `background-image-*` one that reads as gradients a gradient,
/// an `ease-*` one an easing, a `duration-*` one a duration, a
/// `font-weight-*` one a number, any other `font-*` a family. What is of no
/// kind a token has is left out and listed in `ignored`.
fn apply_css_vars(tokens: &mut Store, base: &Store, vars: &JsonValue, ignored: &mut Vec<LeftOut>) {
    let mut section = |name: &str| -> FxHashMap<String, String> {
        let Some(v) = vars.get(name) else {
            return FxHashMap::default();
        };
        let JsonValue::Object(fields) = v else {
            ignored.push(LeftOut::said(format!("cssVars.{name}: expected an object")));
            return FxHashMap::default();
        };
        let mut out = FxHashMap::default();
        for (key, value) in fields {
            match value {
                JsonValue::Null => {}
                JsonValue::String(s) => {
                    out.insert(key.clone(), s.clone());
                }
                JsonValue::Number(n) => {
                    out.insert(key.clone(), n.to_string());
                }
                v => ignored.push(LeftOut::said(format!(
                    "cssVars.{name}.{key}: expected a CSS value, got {v}"
                ))),
            }
        }
        out
    };
    let (both, light, dark) = (section("theme"), section("light"), section("dark"));
    let mut light_vars = both.clone();
    light_vars.extend(light.iter().map(|(k, v)| (k.clone(), v.clone())));
    let mut dark_vars = light_vars.clone();
    dark_vars.extend(dark.iter().map(|(k, v)| (k.clone(), v.clone())));

    let mut names: Vec<&String> = light_vars.keys().chain(dark.keys()).collect();
    names.sort();
    names.dedup();
    for name in names {
        let value = dark_vars.get(name).or_else(|| light_vars.get(name));
        let Some(value) = value else { continue };
        // A box shadow, per appearance: a `shadow-*` value that reads as
        // CSS `box-shadow` layers.
        if name.starts_with("shadow-")
            && let Some(dark_layers) = dark_vars.get(name).and_then(|v| crate::css::box_shadows(v))
        {
            let light_layers = match light_vars.get(name) {
                Some(v) => crate::css::box_shadows(v),
                None => Some(dark_layers.clone()),
            };
            if let Some(light_layers) = light_layers {
                tokens.set_shadow(name, Pair::new(light_layers, dark_layers));
                continue;
            }
        }
        // A gradient, per appearance: a `background-image-*` value that
        // reads as CSS gradients, one or several. A `var()` in a stop is the
        // theme's color in that appearance.
        if name.starts_with(GRADIENTS) {
            let read = |vars: &FxHashMap<String, String>, dark: bool| {
                let css = substituted(name, vars, base, dark, &mut FxHashSet::default())?;
                Ok(css
                    .and_then(|css| crate::css::gradients(&css))
                    .filter(|layers| !layers.is_empty()))
            };
            match (read(&light_vars, false), read(&dark_vars, true)) {
                (Ok(Some(light)), Ok(Some(dark))) => {
                    tokens.set_gradient(name, Pair::new(light, dark));
                    continue;
                }
                (Err(e), _) | (_, Err(e)) => {
                    ignored.push(e);
                    continue;
                }
                // No gradient in an appearance: read as any other value.
                _ => {}
            }
        }
        let number = |v: &str,
                      vars: &FxHashMap<String, String>,
                      dark: bool,
                      parse: fn(&str) -> Option<f32>| {
            number_value(v, vars, base, dark, parse, &mut FxHashSet::default())
        };
        // An easing, one in both appearances.
        if name.starts_with("ease-") {
            let (l, d) = (light_vars.get(name), dark_vars.get(name));
            match (l.map(|v| easing::parse(v)), d.map(|v| easing::parse(v))) {
                (Some(Some(l)), Some(Some(d))) if l == d => tokens.set_easing(name, l),
                (None, Some(Some(d))) => tokens.set_easing(name, d),
                (Some(Some(_)), Some(Some(_))) => ignored.push(LeftOut::about(
                    name,
                    ": an easing is one value in both appearances",
                )),
                _ => ignored.push(LeftOut::about(
                    name,
                    format_args!(": {value} is no easing (`cubic-bezier(…)`, `ease-in-out`, …)"),
                )),
            }
            continue;
        }
        // A duration, per appearance, kept as a number of milliseconds.
        if name.starts_with("duration-") {
            let Some(dark_ms) = number(value, &dark_vars, true, duration) else {
                ignored.push(LeftOut::about(
                    name,
                    format_args!(": {value} is no duration (`150ms`, `0.2s`)"),
                ));
                continue;
            };
            let light_ms = light_vars
                .get(name)
                .and_then(|v| number(v, &light_vars, false, duration))
                .unwrap_or(dark_ms);
            tokens.set_number(name, light_ms);
            if dark_ms != light_ms {
                tokens.set_dark_number(name, dark_ms);
            }
            continue;
        }
        // A weight is a number; every other `font-*` a family.
        if name.starts_with("font-weight-") {
            let Some(dark_weight) = number(value, &dark_vars, true, length) else {
                let said = no_kind(value, "number", &dark_vars, base);
                ignored.push(LeftOut::about(name, format_args!(": {said}")));
                continue;
            };
            let light_weight = light_vars
                .get(name)
                .and_then(|v| number(v, &light_vars, false, length))
                .unwrap_or(dark_weight);
            tokens.set_number(name, light_weight);
            if dark_weight != light_weight {
                tokens.set_dark_number(name, dark_weight);
            }
            continue;
        }
        if name.starts_with("font-") {
            if written_as_color(value) || length(value).is_some() {
                let said = no_kind(value, "font family", &dark_vars, base);
                ignored.push(LeftOut::about(name, format_args!(": {said}")));
            } else if font_family(value).is_some() {
                // The old store kept `font_family(value)`; this one keeps the
                // whole list it comes from (`Store::font`).
                tokens.set_font(name, value.trim());
            }
            continue;
        }
        let resolve = |vars: &FxHashMap<String, String>, dark: bool| {
            resolve_color(name, vars, base, dark, &mut FxHashSet::default())
        };
        let (light, dark) = match (resolve(&light_vars, false), resolve(&dark_vars, true)) {
            (Ok(light), Ok(dark)) => (light, dark),
            (Err(e), _) | (_, Err(e)) => {
                ignored.push(e);
                continue;
            }
        };
        match (light, dark) {
            (Some(l), Some(d)) => {
                tokens.set_color(name, Pair::new(l, d));
            }
            (Some(c), None) | (None, Some(c)) => {
                // Set in one appearance only, the color is both's; given in
                // the other too, that value is no color.
                let (other, vars) = match light {
                    Some(_) => ("dark", &dark_vars),
                    None => ("light", &light_vars),
                };
                if let Some(v) = vars.get(name) {
                    ignored.push(LeftOut::about(
                        name,
                        format_args!(": {v} is no color in the {other} appearance"),
                    ));
                }
                tokens.set_color(name, Pair::both(c));
            }
            (None, None) => {
                let number = |v: &str, vars: &FxHashMap<String, String>, dark: bool| {
                    number_value(v, vars, base, dark, length, &mut FxHashSet::default())
                };
                if let Some(dark_n) = number(value, &dark_vars, true) {
                    let light_n = light_vars
                        .get(name)
                        .and_then(|v| number(v, &light_vars, false))
                        .unwrap_or(dark_n);
                    tokens.set_number(name, light_n);
                    if dark_n != light_n {
                        tokens.set_dark_number(name, dark_n);
                    }
                }
                // A value written as a color that is none is said, as is
                // one that is no number in an appearance where the other's
                // is; a `var()` copied as a number is not.
                let mut given: Vec<(&FxHashMap<String, String>, &String, bool)> = [
                    (&light_vars, light_vars.get(name), false),
                    (&dark_vars, dark_vars.get(name), true),
                ]
                .into_iter()
                .filter_map(|(vars, v, dark)| Some((vars, v?, dark)))
                .collect();
                given.dedup_by(|a, b| a.1 == b.1);
                let copied = |v: &str, vars, dark| {
                    v.trim_start().starts_with("var(") && number(v, vars, dark).is_some()
                };
                for (vars, v, _) in given
                    .into_iter()
                    .filter(|(vars, v, dark)| written_as_color(v) && !copied(v, vars, *dark))
                {
                    let said = no_kind(v, "color", vars, base);
                    ignored.push(LeftOut::about(name, format_args!(": {said}")));
                }
            }
        }
    }
}

/// A number value `value` of one appearance read by `parse`, or, when it is
/// a whole `var(--x)` or `var(--x, fallback)`, the number `x` is: the
/// file's own, else `base`'s, else the fallback.
fn number_value(
    value: &str,
    vars: &FxHashMap<String, String>,
    base: &Store,
    dark: bool,
    parse: fn(&str) -> Option<f32>,
    seen: &mut FxHashSet<String>,
) -> Option<f32> {
    let value = value.trim();
    let Some(inner) = value.strip_prefix("var(").and_then(|v| v.strip_suffix(')')) else {
        return parse(value);
    };
    let (var, fallback) = match inner.split_once(',') {
        Some((var, fallback)) => (var.trim(), Some(fallback.trim())),
        None => (inner.trim(), None),
    };
    let named = var.strip_prefix("--")?;
    let copied = match vars.get(named) {
        Some(own) if seen.insert(named.to_owned()) => {
            number_value(own, vars, base, dark, parse, seen)
        }
        Some(_) => None,
        None => base.number_in(named, dark),
    };
    copied.or_else(|| number_value(fallback?, vars, base, dark, parse, seen))
}

/// `value`, which a theme file gives a token that cannot take it, said as
/// what it is.
fn no_kind(value: &str, kind: &str, vars: &FxHashMap<String, String>, base: &Store) -> String {
    let mut rest = value;
    while let Some(start) = rest.find("var(") {
        let inner = &rest[start + 4..];
        let end = inner.find([',', ')']).unwrap_or(inner.len());
        if let Some(var) = inner[..end].trim().strip_prefix("--")
            && let Some(named) = named_kind(var, vars, base).filter(|k| *k != Kind::Color)
        {
            return match named {
                Kind::Number => format!("{value} names the number `{var}`, which is no {kind}"),
                _ => format!(
                    "{value} names the {} `{var}`, and var() copies only colors and numbers",
                    named.name()
                ),
            };
        }
        rest = &inner[end..];
    }
    format!("{value} is no {kind}")
}

/// The kind of token `name` as theme file `vars` (of one appearance) give
/// it, else as `base` has it.
fn named_kind(name: &str, vars: &FxHashMap<String, String>, base: &Store) -> Option<Kind> {
    let Some(value) = vars.get(name) else {
        return base.kind(name);
    };
    if name.starts_with("shadow-") && crate::css::box_shadows(value).is_some() {
        Some(Kind::Shadow)
    } else if name.starts_with(GRADIENTS)
        && crate::css::gradients(value).is_some_and(|layers| !layers.is_empty())
    {
        Some(Kind::Gradient)
    } else if Kind::Number.names(name) && length(value).is_some() {
        Some(Kind::Number)
    } else if Kind::Font.names(name) && !written_as_color(value) && font_family(value).is_some() {
        Some(Kind::Font)
    } else if written_as_color(value) {
        Some(Kind::Color)
    } else {
        None
    }
}

/// The namespace of gradient tokens, Tailwind's `--background-image-*`.
pub(super) const GRADIENTS: &str = "background-image-";

/// The color `name` is in `vars` (of one appearance), with the colors it
/// names resolved: the file's, else `base`'s. `None` when it is no color.
/// A name that names itself is an error.
fn resolve_color(
    name: &str,
    vars: &FxHashMap<String, String>,
    base: &Store,
    dark: bool,
    seen: &mut FxHashSet<String>,
) -> Result<Option<Hsla>, LeftOut> {
    if !vars.contains_key(name) {
        return Ok(base.color(name).map(|c| *c.get(dark)));
    }
    Ok(substituted(name, vars, base, dark, seen)?.and_then(|css| color(&css)))
}

/// The value of `name` in `vars` (of one appearance) with each `var(--x)`
/// and `var(--x, fallback)` in it written as the color it names, or its
/// fallback. `None` when `vars` have no `name`, or a variable in it is no
/// color and has no fallback.
fn substituted(
    name: &str,
    vars: &FxHashMap<String, String>,
    base: &Store,
    dark: bool,
    seen: &mut FxHashSet<String>,
) -> Result<Option<String>, LeftOut> {
    let Some(value) = vars.get(name) else {
        return Ok(None);
    };
    if !seen.insert(name.to_owned()) {
        return Err(LeftOut::about(name, " names itself"));
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value.as_str();
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let inner = &rest[start + 4..];
        let mut depth = 1;
        let end = inner
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map(|(i, _)| i)
            .ok_or_else(|| LeftOut::about(name, ": unclosed var()"))?;
        let (var, fallback) = match inner[..end].split_once(',') {
            Some((v, f)) => (v.trim(), Some(f.trim())),
            None => (inner[..end].trim(), None),
        };
        let Some(var) = var.strip_prefix("--") else {
            return Err(LeftOut::about(name, ": var() names no `--` variable"));
        };
        match resolve_color(var, vars, base, dark, seen)? {
            Some(c) => out.push_str(&format!("#{:08x}", Color::from(c).0)),
            None => match fallback {
                Some(f) => out.push_str(f),
                None => {
                    seen.remove(name);
                    return Ok(None);
                }
            },
        }
        rest = &inner[end + 1..];
    }
    out.push_str(rest);
    seen.remove(name);
    Ok(Some(out))
}

/// The name a whole `var(--x)` (or `var(--x, fallback)`) value names: `x`.
pub(super) fn whole_var(value: &str) -> Option<&str> {
    let inner = value.trim().strip_prefix("var(")?.strip_suffix(')')?;
    if inner.contains(['(', ')']) {
        return None;
    }
    let var = inner.split(',').next()?.trim();
    var.strip_prefix("--").filter(|name| !name.is_empty())
}

// ---------------------------------------------------------------------------
// One file over another, as the old runtime layered them: `theme.rs`'s
// `TokenProvider` (`read_left_out`, `with_modifiers`, `read_context`) and
// `Declarations::add_document`.

/// Who declares the tokens an app's own theme file declares.
const APP_THEME: &str = "the app's theme";
/// Who declares the tokens of the base a file is converted over.
const BASE_THEME: &str = "the base theme";

/// A file's text as `cssVars` writes it: its light value's (`theme`, else
/// `light`) and its dark value's (`dark`, else the light one's).
#[derive(Debug, Clone, Default)]
pub(super) struct Raw {
    pub(super) light: Option<String>,
    pub(super) dark: Option<String>,
}

/// What the old reader made of a theme file (over a base): the values under
/// no modifier's context, the modifiers, and what declares the tokens.
pub(super) struct Old {
    /// The `name` of the file, else of the base it was read over.
    pub(super) name: Option<String>,
    /// The tokens under no context.
    pub(super) under: Store,
    /// The modifiers, the base document's first.
    pub(super) modifiers: Option<Modifiers>,
    /// What declares the tokens, when they were read by declarations.
    pub(super) set: Option<Declarations>,
    /// The names in the order the files read write them, the base's first.
    pub(super) order: Vec<String>,
    /// Where a name is written (a JSON pointer into the file that sets it).
    pub(super) written: FxHashMap<String, String>,
    /// The `cssVars` text of each name, the file's over the base's.
    pub(super) raw: FxHashMap<String, Raw>,
    /// What was left out, each with whether it was the base's.
    pub(super) left_out: Vec<(bool, LeftOut)>,
}

impl Old {
    /// The tokens the old reader resolved with the contexts `picks` picked
    /// (each modifier's default where `picks` names none).
    #[cfg(test)]
    pub(super) fn values(&self, picks: &[(&str, &str)]) -> Store {
        let Some(modifiers) = &self.modifiers else {
            return self.under.clone();
        };
        let mut modifiers = modifiers.clone();
        for (name, context) in picks {
            if let Ok(picked) = modifiers.select(name, context) {
                modifiers = picked;
            }
        }
        modifiers.apply(&self.under)
    }
}

/// How the old runtime read an app's theme file (`theme.rs`'s
/// `TokenProvider`): with a UI set's own reader over its defaults while no
/// document declares tokens (nsh-base's `read_tokens`: `theme_file::parse`
/// over its default's tokens), or by the declared tokens of the documents
/// the sets registered.
enum Provider {
    /// The defaults of a UI set's own reader (none: a file read alone).
    Source(Store),
    /// The registered documents' declarations, and their modifiers.
    Declared(Declarations, Option<Modifiers>),
}

/// `Declarations::add_document`: the theme document `json` (`value`) of
/// `owner` as the only registered one, its declared tokens, roles, values
/// and modifiers. What the old registry refused it for is left out and
/// listed here; a declared token with no value is the writer's to report.
fn add_document(
    owner: &str,
    json: &str,
    value: &JsonValue,
) -> Result<(Provider, Vec<LeftOut>), String> {
    let (declared, mut left_out) = declared_tokens(value, owner);
    left_out.extend(unknown_sections(value));
    let mut own = Declarations::default();
    for (name, d) in declared {
        own.insert(name, d);
    }
    let (defaults, read_out) = overrides_declared(json, &Store::default(), &own)?;
    left_out.extend(read_out);
    // Its modifiers, each context read by its own declarations.
    let (modifiers, said) = Modifiers::read(json, value, &defaults, |context| {
        overrides_declared(context, &defaults, &own)
    });
    left_out.extend(said);
    own.defaults = defaults;
    Ok((Provider::Declared(own, modifiers), left_out))
}

/// Theme file `old` read as the old runtime read an app's theme over `over`,
/// a UI set's default theme (`None`: a file read alone, over no defaults):
/// a base that declares tokens is a registered theme document, and the app's
/// file is read by its declarations; one that declares none is the set's own
/// reader's defaults, which an app's file that declares no tokens goes over,
/// and one that declares tokens replaces. `Err`: which of the two is not
/// JSON (`true`: the base), and why.
pub(super) fn read_old(old: &str, over: Option<&str>) -> Result<Old, (bool, String)> {
    let not_json = |e: serde_json::Error| format!("not JSON: {e}");
    let mut left_out: Vec<(bool, LeftOut)> = Vec::new();
    let mut base: Option<(&str, JsonValue)> = None;
    let provider = match over {
        None => Provider::Source(Store::default()),
        Some(json) => {
            let value: JsonValue = serde_json::from_str(json).map_err(|e| (true, not_json(e)))?;
            let (provider, said) = if declares_tokens(&value) {
                add_document(BASE_THEME, json, &value).map_err(|e| (true, e))?
            } else {
                // The set's own reader reads its default with
                // `theme_file::parse` alone: no modifiers.
                let (tokens, said) = parse(json, &Store::default()).map_err(|e| (true, e))?;
                (Provider::Source(tokens), said)
            };
            left_out.extend(said.into_iter().map(|l| (true, l)));
            base = Some((json, value));
            provider
        }
    };
    let value: JsonValue = serde_json::from_str(old).map_err(|e| (false, not_json(e)))?;
    let mut said = unknown_sections(&value);
    let declares = declares_tokens(&value);
    let (under, modifiers, set, base_read) = match provider {
        Provider::Source(defaults) if !declares => {
            let (tokens, read_out) = parse(old, &defaults).map_err(|e| (false, e))?;
            said.extend(read_out);
            // The file's modifiers, each context read as a change of the
            // tokens is (`theme_file::overrides` against them).
            let (modifiers, read_out) =
                Modifiers::read(old, &value, &tokens, |context| overrides(context, &tokens));
            said.extend(read_out);
            let modifiers = modifiers.map(|m| m.rebased(&tokens));
            (tokens, modifiers, None, true)
        }
        provider => {
            // Read by declared tokens: the documents', or, with only a
            // set's own reader, none (`Declarations::default()`): the file
            // replaces the set's defaults.
            let (set, documents, base_read) = match provider {
                Provider::Declared(set, documents) => (set, documents, true),
                Provider::Source(_) => (Declarations::default(), None, false),
            };
            let (mut set, read_out) = set.with_file(&value, APP_THEME);
            said.extend(read_out);
            let (tokens, read_out) =
                parse_declared(old, &set.defaults, &set).map_err(|e| (false, e))?;
            said.extend(read_out);
            let (file, read_out) = Modifiers::read(old, &value, &tokens, |context| {
                overrides_declared(context, &tokens, &set)
            });
            said.extend(read_out);
            let modifiers = match (documents, file) {
                (Some(documents), Some(file)) => match documents.clone().merged(file) {
                    Ok(both) => Some(both),
                    Err(e) => {
                        said.push(LeftOut::said(format!("modifiers: {e}")));
                        Some(documents)
                    }
                },
                (documents, file) => documents.or(file),
            };
            let modifiers = modifiers.map(|m| m.rebased(&tokens));
            set.defaults = tokens.clone();
            (tokens, modifiers, Some(set), base_read)
        }
    };
    // A base the file replaced is no part of what the old runtime read.
    if !base_read {
        left_out.clear();
        base = None;
    }
    left_out.extend(said.into_iter().map(|l| (false, l)));

    let mut out = Old {
        name: None,
        under,
        modifiers,
        set,
        order: Vec::new(),
        written: FxHashMap::default(),
        raw: FxHashMap::default(),
        left_out,
    };
    for (json, value) in base.iter().map(|(j, v)| (*j, v)).chain([(old, &value)]) {
        out.record(json, value);
    }
    Ok(out)
}

impl Old {
    /// Records the names theme file `json` (`value`) writes, over those
    /// recorded before: their order, where each is written, their text, and
    /// the file's `name`. A key is the name the declarations read it as.
    fn record(&mut self, json: &str, value: &JsonValue) {
        if let Some(name) = value.get("name").and_then(JsonValue::as_str) {
            self.name = Some(name.to_owned());
        }
        let target = |key: &str| -> String {
            self.set
                .as_ref()
                .and_then(|s| s.target(key))
                .unwrap_or(key)
                .to_owned()
        };
        let text = |v: &JsonValue| match v {
            JsonValue::String(s) => Some(s.clone()),
            JsonValue::Number(n) => Some(n.to_string()),
            _ => None,
        };
        // The text: `theme`, then `light` over it, and `dark` (else the
        // light one) for the dark value; the base's where the file has none.
        let mut file: FxHashMap<String, Raw> = FxHashMap::default();
        for section in ["theme", "light", "dark"] {
            let Some(fields) = value
                .get("cssVars")
                .and_then(|v| v.get(section))
                .and_then(JsonValue::as_object)
            else {
                continue;
            };
            for (key, v) in fields {
                let Some(v) = text(v) else { continue };
                let raw = file.entry(target(key)).or_default();
                match section {
                    "dark" => raw.dark = Some(v),
                    _ => raw.light = Some(v),
                }
            }
        }
        for (name, raw) in file {
            let entry = self.raw.entry(name).or_default();
            if let Some(light) = raw.light.clone() {
                entry.light = Some(light);
            }
            entry.dark = raw.dark.or(raw.light);
        }
        // The order written: `cssVars`' sections, then `tokens`.
        let ordered = Ordered::parse(json);
        let mut here: FxHashMap<String, String> = FxHashMap::default();
        let mut add = |name: String, at: String, order: &mut Vec<String>| {
            if !order.contains(&name) {
                order.push(name.clone());
            }
            here.entry(name).or_insert(at);
        };
        for section in ["theme", "light", "dark"] {
            let keys = ordered
                .at(&["cssVars", section])
                .map(Ordered::keys)
                .unwrap_or_default();
            for key in keys {
                add(
                    target(key),
                    pointer(&["cssVars", section, key]),
                    &mut self.order,
                );
            }
        }
        for key in ordered.get("tokens").map(Ordered::keys).unwrap_or_default() {
            add(key.to_owned(), pointer(&["tokens", key]), &mut self.order);
        }
        self.written.extend(here);
    }
}

/// The RFC 6901 pointer of `path`.
pub(super) fn pointer(path: &[&str]) -> String {
    path.iter()
        .map(|p| format!("/{}", p.replace('~', "~0").replace('/', "~1")))
        .collect()
}
