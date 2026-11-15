//! The token dictionary as data, for tools and code that does not know the
//! tokens' names ahead (TS's `tokens.get`, `tokens.kind`, `tokens.names` and
//! `tokens.entries`): each token's kind, its values in both appearances and
//! what declares it. A token's kind is the one declared for it
//! ([`super::Declarations`]: `radius`, `spacing`, …), else the kind it is
//! kept as (`color`, `number`, `font`, `shadow`, `gradient`).

use super::{Token, Tokens, tokens::Kind};
use crate::{Color, JsonValue};

/// A token as [`Tokens::infos`] lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    /// Its name.
    pub name: Box<str>,
    /// Its kind ([`Tokens::kind_name`]).
    pub kind: &'static str,
    /// Its place in the designer's tree, when it is declared with one.
    pub path: Option<Box<str>>,
    /// Its doc, when it is declared with one.
    pub doc: Option<Box<str>>,
    /// Who declares it, when it is declared.
    pub owner: Option<Box<str>>,
}

impl TokenInfo {
    /// It as JSON: `{ name, kind, path?, doc?, owner? }`.
    pub fn to_json(&self) -> JsonValue {
        let mut out = serde_json::Map::new();
        out.insert("name".into(), (*self.name).into());
        out.insert("kind".into(), self.kind.into());
        for (key, value) in [
            ("path", &self.path),
            ("doc", &self.doc),
            ("owner", &self.owner),
        ] {
            if let Some(value) = value {
                out.insert(key.into(), (**value).into());
            }
        }
        JsonValue::Object(out)
    }
}

impl Tokens {
    /// The kind of token `name`: its declared kind's name
    /// ([`super::TokenKind::name`]), else the kind it is kept as (`color`,
    /// `number`, `font`, `shadow` or `gradient`). `None`: there is none.
    pub fn kind_name(&self, name: &str) -> Option<&'static str> {
        let kept = self.kind(name)?;
        if let Some(declared) = self.declarations().and_then(|d| d.kind_of(name)) {
            return Some(declared.name());
        }
        Some(match kept {
            Kind::Color => "color",
            Kind::Number => "number",
            Kind::Font => "font",
            Kind::Shadow => "shadow",
            Kind::Gradient => "gradient",
            Kind::String => "string",
            Kind::Boolean => "boolean",
            Kind::Easing => "easing",
        })
    }

    /// Each token, sorted by name, with its kind and what declares it.
    pub fn infos(&self) -> Vec<TokenInfo> {
        let mut names: Vec<&str> = self.names().collect();
        names.sort_unstable();
        names
            .into_iter()
            .filter_map(|name| {
                let declared = self.declarations().and_then(|d| d.get(name));
                Some(TokenInfo {
                    name: name.into(),
                    kind: self.kind_name(name)?,
                    path: declared.and_then(|d| d.path.clone()),
                    doc: declared
                        .map(|d| d.doc.clone())
                        .filter(|doc| !doc.is_empty()),
                    owner: declared.map(|d| d.owner.clone()),
                })
            })
            .collect()
    }

    /// Token `name` as JSON: `{ kind, light, dark }`, its value in each
    /// appearance as TS's `token()` gives it (a color as `#rrggbbaa`, a
    /// number in points, a duration in milliseconds, a font family, shadows,
    /// gradients and an easing as CSS, a string, a boolean; a typography
    /// token as `{ size, lineHeight?, letterSpacing?, fontWeight? }`).
    /// `None`: there is none.
    pub fn value_json(&self, name: &str) -> Option<JsonValue> {
        let kind = self.kind_name(name)?;
        // A text style: its size and the parts it has.
        if kind == "typography" {
            let style = |dark: bool| {
                let t = self.typography_in(name, dark)?;
                let mut out = serde_json::Map::new();
                out.insert("size".into(), t.size.0.into());
                for (key, value) in [
                    ("lineHeight", t.line_height),
                    ("letterSpacing", t.letter_spacing.map(|p| p.0)),
                    ("fontWeight", t.font_weight),
                ] {
                    if let Some(value) = value {
                        out.insert(key.into(), value.into());
                    }
                }
                Some(JsonValue::Object(out))
            };
            return Some(
                serde_json::json!({ "kind": kind, "light": style(false)?, "dark": style(true)? }),
            );
        }
        let (light, dark): (JsonValue, JsonValue) = match self.get(name)? {
            Token::Color(c) => {
                let hex = |c| JsonValue::from(format!("#{:08x}", Color::from(c).0));
                (hex(c.light), hex(c.dark))
            }
            Token::Number(_) => (
                self.number_in(name, false)?.into(),
                self.number_in(name, true)?.into(),
            ),
            Token::Font(f) => (JsonValue::from(&*f), JsonValue::from(&*f)),
            Token::Shadow(s) => (s.css(false).into(), s.css(true).into()),
            Token::Gradient(g) => (g.css(false).into(), g.css(true).into()),
            Token::String(s) => (JsonValue::from(&*s), JsonValue::from(&*s)),
            Token::Boolean(b) => (b.into(), b.into()),
            Token::Easing(e) => (e.css().into(), e.css().into()),
        };
        Some(serde_json::json!({ "kind": kind, "light": light, "dark": dark }))
    }

    /// Whether these tokens and `other` have the same names, each of the
    /// same kind: what a list of the tokens shows.
    pub fn same_names(&self, other: &Tokens) -> bool {
        let count = |t: &Tokens| t.names().count();
        count(self) == count(other) && self.names().all(|n| self.kind(n) == other.kind(n))
    }
}
