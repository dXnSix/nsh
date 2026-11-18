//! Roles: the names a library reads, mapped onto a theme's tokens.
//!
//! A library registers [`RoleDeclaration`]s (a name, a value type, a doc and
//! whether a theme must map it). A role resolves through its layers, highest
//! first: the patches' `roles` (a later patch first), then the document's.
//! A mapping refused at its layer (onto an undeclared token, or a token of a
//! type the role doesn't accept) is dropped from that layer, and the role
//! resolves through the next layer down. With no mapping left, the role
//! reads the token of its own name, unless that token has the wrong type.

use crate::{
    diagnostic::{Diagnostic, code, pointer},
    document::ThemeDocument,
    resolve::{Patch, Resolved},
    value::ValueType,
};

/// A role a library registers: what it reads, of which type.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoleDeclaration {
    /// The role's name (`primary`, `surface-card`).
    pub name: String,
    /// The type of the token it reads.
    pub value_type: ValueType,
    /// What the role is for.
    pub doc: Option<String>,
    /// Whether every theme should map it (`role-unmapped` otherwise).
    pub required: bool,
}

impl RoleDeclaration {
    /// A role `name` of `value_type`, with no doc, not required.
    pub fn new(name: impl Into<String>, value_type: ValueType) -> Self {
        Self {
            name: name.into(),
            value_type,
            doc: None,
            required: false,
        }
    }

    /// With this doc.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Required or not.
    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

/// A family of roles no library lists one by one: every role whose name
/// starts with `prefix` reads a token of one of `accepts` (nsh-core passes
/// Tailwind's namespaces: `radius-` numbers, `shadow-` shadows, …).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RolePattern {
    /// The role name's prefix.
    pub prefix: String,
    /// The value types its tokens may have.
    pub accepts: Vec<ValueType>,
}

impl RolePattern {
    /// Roles starting with `prefix`, reading tokens of `accepts`.
    pub fn new(prefix: impl Into<String>, accepts: &[ValueType]) -> Self {
        Self {
            prefix: prefix.into(),
            accepts: accepts.to_vec(),
        }
    }
}

/// Which types a role accepts, as the declarations and patterns say.
enum Expected<'a> {
    /// A declaration names it.
    Declared(ValueType),
    /// A pattern's prefix matches it (the longest).
    Pattern(&'a [ValueType]),
    /// Nothing declares or matches it: a mapping onto a color is known
    /// (Tailwind's bare color form reads any name), any other is unknown.
    Unknown,
}

impl Expected<'_> {
    fn of<'a>(
        role: &str,
        declarations: &'a [RoleDeclaration],
        patterns: &'a [RolePattern],
    ) -> Expected<'a> {
        if let Some(declaration) = declarations.iter().find(|d| d.name == role) {
            return Expected::Declared(declaration.value_type);
        }
        patterns
            .iter()
            .filter(|p| role.starts_with(p.prefix.as_str()))
            .max_by_key(|p| p.prefix.len())
            .map_or(Expected::Unknown, |p| Expected::Pattern(&p.accepts))
    }

    fn accepts(&self, ty: ValueType) -> bool {
        match self {
            Expected::Declared(expected) => *expected == ty,
            Expected::Pattern(accepts) => accepts.contains(&ty),
            Expected::Unknown => true,
        }
    }

    fn describe(&self) -> String {
        match self {
            Expected::Declared(ty) => format!("a {ty}"),
            Expected::Pattern(accepts) => {
                let names: Vec<&str> = accepts.iter().map(|t| t.name()).collect();
                format!("one of {}", names.join(", "))
            }
            Expected::Unknown => "any type".to_owned(),
        }
    }
}

/// The mappings of `role`, highest layer first, with where each is.
fn mappings<'a>(
    role: &str,
    document: &'a ThemeDocument,
    patches: &[&'a Patch],
) -> Vec<(&'a str, Option<usize>)> {
    let mut found: Vec<(&str, Option<usize>)> = patches
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(i, patch)| patch.roles.get(role).map(|token| (token.as_str(), Some(i))))
        .collect();
    if let Some(token) = document.roles.get(role) {
        found.push((token, None));
    }
    found
}

/// The token the role `name` reads, for the runtime: its highest mapping
/// onto a declared token of an accepted type, else the token of its own
/// name; `None` when there is none, or the own-name token has the wrong type.
pub fn resolve_role(
    name: &str,
    document: &ThemeDocument,
    patches: &[&Patch],
    declarations: &[RoleDeclaration],
    patterns: &[RolePattern],
) -> Option<String> {
    let expected = Expected::of(name, declarations, patterns);
    for (token, _) in mappings(name, document, patches) {
        if let Some(declared) = document.tokens.get(token)
            && expected.accepts(declared.value_type)
        {
            return Some(token.to_owned());
        }
    }
    let own = document.tokens.get(name)?;
    expected.accepts(own.value_type).then(|| name.to_owned())
}

/// Checks a theme's roles against what libraries register:
///
/// - `role-type` (error) for a mapping onto a token of a type its
///   declaration or pattern doesn't accept (the role resolves through the
///   next layer down), and for a declared role that falls back to a token of
///   its own name of the wrong type;
/// - `role-unknown` (warning) for a mapping onto a non-color token that no
///   declaration or pattern covers; it is kept, another library may read it;
/// - `role-unmapped` (warning) for a `required` role that resolves to no
///   token with a value in `resolved`.
///
/// Mappings onto undeclared tokens are left to `read` and [`Patch::check`]
/// (`role-token-missing`), and not reported again.
pub fn check_roles(
    document: &ThemeDocument,
    patches: &[&Patch],
    resolved: &Resolved,
    declarations: &[RoleDeclaration],
    patterns: &[RolePattern],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let layers = std::iter::once((None, &document.roles)).chain(
        patches
            .iter()
            .enumerate()
            .map(|(i, patch)| (Some(i), &patch.roles)),
    );
    for (patch, roles) in layers {
        let whose = patch.map_or("The document".to_owned(), |i| format!("Patch {}", i + 1));
        for (role, token) in roles.iter() {
            let Some(declared) = document.tokens.get(token) else {
                continue;
            };
            let at = pointer(["roles", role]);
            let expected = Expected::of(role, declarations, patterns);
            match expected {
                Expected::Unknown if declared.value_type != ValueType::Color => {
                    diagnostics.push(Diagnostic::warning(
                        code::ROLE_UNKNOWN,
                        at,
                        format!(
                            "{whose} maps role `{role}`, which nothing declares or matches, onto the {} token \
                             `{token}`; the mapping is kept.",
                            declared.value_type
                        ),
                    ));
                }
                _ if !expected.accepts(declared.value_type) => {
                    diagnostics.push(Diagnostic::error(
                        code::ROLE_TYPE,
                        at,
                        format!(
                            "{whose} maps role `{role}` onto the {} token `{token}`, and the role reads {}; the \
                             mapping is left out.",
                            declared.value_type,
                            expected.describe()
                        ),
                    ));
                }
                _ => {}
            }
        }
    }
    for declaration in declarations {
        let role = declaration.name.as_str();
        match resolve_role(role, document, patches, declarations, patterns) {
            None => {
                let wrong_own = document
                    .tokens
                    .get(role)
                    .filter(|own| own.value_type != declaration.value_type);
                if let Some(own) = wrong_own {
                    diagnostics.push(Diagnostic::error(
                        code::ROLE_TYPE,
                        pointer(["tokens", role]),
                        format!(
                            "Role `{role}` reads a {}, and the token of its own name is a {}; the role reads nothing.",
                            declaration.value_type, own.value_type
                        ),
                    ));
                } else if declaration.required {
                    diagnostics.push(unmapped(role));
                }
            }
            Some(token) if declaration.required && resolved.get(&token).is_none() => {
                diagnostics.push(unmapped(role));
            }
            Some(_) => {}
        }
    }
    diagnostics
}

fn unmapped(role: &str) -> Diagnostic {
    Diagnostic::warning(
        code::ROLE_UNMAPPED,
        pointer(["roles", role]),
        format!("Role `{role}` is required, and the theme resolves it to no token."),
    )
}
