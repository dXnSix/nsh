//! The token store as data, for tools and code that does not know the
//! tokens' names ahead (TS's `tokens.get`, `tokens.kind`, `tokens.names` and
//! `tokens.entries`): each token's value type, its value and what the active
//! document declares of it.

use super::{Token, Tokens};
use crate::JsonValue;

/// A token as [`Tokens::infos`] lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    /// Its name.
    pub name: Box<str>,
    /// Its value type's name ([`Tokens::kind_name`]).
    pub kind: &'static str,
    /// Its place in the designer's tree, when it is declared with one.
    pub path: Option<Box<str>>,
    /// Its doc, when it is declared with one.
    pub doc: Option<Box<str>>,
}

impl TokenInfo {
    /// It as JSON: `{ name, kind, path?, doc? }`.
    pub fn to_json(&self) -> JsonValue {
        let mut out = serde_json::Map::new();
        out.insert("name".into(), (*self.name).into());
        out.insert("kind".into(), self.kind.into());
        for (key, value) in [("path", &self.path), ("doc", &self.doc)] {
            if let Some(value) = value {
                out.insert(key.into(), (**value).into());
            }
        }
        JsonValue::Object(out)
    }
}

impl Tokens {
    /// The name of token `name`'s value type ([`Tokens::kind_of`]).
    /// `None`: there is none.
    pub fn kind_name(&self, name: &str) -> Option<&'static str> {
        self.kind_of(name).map(|kind| kind.name())
    }

    /// Each token with a value, sorted by name, with its value type and
    /// what the active document declares of it.
    pub fn infos(&self) -> Vec<TokenInfo> {
        let mut names: Vec<&str> = self.iter().map(|(name, _)| name).collect();
        names.sort_unstable();
        names
            .into_iter()
            .filter_map(|name| {
                Some(TokenInfo {
                    name: name.into(),
                    kind: self.kind_name(name)?,
                    path: self.path_of(name).map(Into::into),
                    doc: self.doc_of(name).filter(|d| !d.is_empty()).map(Into::into),
                })
            })
            .collect()
    }

    /// Token `name` as JSON: `{ kind, value }`, its value type's name and
    /// its value in the canonical form ([`Tokens::json`]), as TS's
    /// `tokens.get` gives it. `None`: there is none.
    pub fn value_json(&self, name: &str) -> Option<JsonValue> {
        let kind = self.kind_name(name)?;
        let value = self.json(name)?;
        Some(serde_json::json!({ "kind": kind, "value": value }))
    }

    /// Whether these tokens and `other` have the same names, each of the
    /// same value type: what a list of the tokens shows.
    pub fn same_names(&self, other: &Tokens) -> bool {
        let count = |t: &Tokens| t.iter().count();
        count(self) == count(other)
            && self
                .iter()
                .all(|(n, t)| other.value(n).map(Token::value_type) == Some(t.value_type()))
    }
}
