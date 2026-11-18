//! The token store as data, for tools and code that does not know the
//! tokens' names ahead (TS's `tokens.get`, `tokens.kind`, `tokens.names` and
//! `tokens.entries`): each token's value type, its value and what the active
//! document declares of it.

use super::{Token, Tokens};
use crate::{Color, JsonValue};

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

    /// Token `name` as JSON: `{ kind, light, dark }`, its value as TS's
    /// `token()` gives it, the same under both keys (a color as
    /// `#rrggbbaa`, a number in points, a duration in milliseconds, a font
    /// family, shadows, a gradient and an easing as CSS, a string, a
    /// boolean; a typography token as `{ size, lineHeight?, letterSpacing?,
    /// fontWeight?, fontFamily? }`, a blur, a border and a transition as
    /// objects of their members). `None`: there is none.
    pub fn value_json(&self, name: &str) -> Option<JsonValue> {
        let kind = self.kind_name(name)?;
        let value = token_json(self.value(name)?);
        Some(serde_json::json!({ "kind": kind, "light": value.clone(), "dark": value }))
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

/// A token's value as TS's `token()` gives it.
fn token_json(token: &Token) -> JsonValue {
    let hex = |c: crate::color::Hsla| JsonValue::from(format!("#{:08x}", Color::from(c).0));
    let millis = |d: std::time::Duration| JsonValue::from(super::theme_values::millis(d));
    match token {
        Token::Color(c) => hex(*c),
        Token::Number(n) => (*n).into(),
        Token::Font(f) => JsonValue::from(&**f),
        Token::String(s) => JsonValue::from(s.as_str()),
        Token::Boolean(b) => (*b).into(),
        Token::Duration(d) => millis(*d),
        Token::Easing(e) => e.css().into(),
        Token::Typography(t) => {
            let mut out = serde_json::Map::new();
            out.insert("size".into(), t.size.0.into());
            match t.line_height {
                Some(crate::LineHeight::Multiple(m)) => {
                    out.insert("lineHeight".into(), m.into());
                }
                Some(crate::LineHeight::Points(p)) => {
                    out.insert("lineHeight".into(), format!("{}px", p.0).into());
                }
                _ => {}
            }
            if let Some(spacing) = t.letter_spacing {
                out.insert("letterSpacing".into(), spacing.0.into());
            }
            if let Some(weight) = t.font_weight {
                out.insert("fontWeight".into(), weight.into());
            }
            if let Some(family) = &t.font_family {
                out.insert("fontFamily".into(), JsonValue::from(&**family));
            }
            JsonValue::Object(out)
        }
        Token::Shadow(layers) => super::AdaptiveShadow::from(layers.iter().copied())
            .css(false)
            .into(),
        Token::Gradient(g) => g.css().into(),
        Token::Blur(b) => serde_json::json!({
            "type": match b.kind {
                super::BlurKind::Background => "background",
                _ => "layer",
            },
            "radius": b.radius.0,
        }),
        Token::Border(b) => serde_json::json!({
            "color": hex(b.color),
            "width": b.width.0,
            "style": match b.style {
                crate::style::BorderStyle::Dashed => "dashed",
                crate::style::BorderStyle::Dotted => "dotted",
                crate::style::BorderStyle::Solid => "solid",
            },
        }),
        Token::Transition(t) => serde_json::json!({
            "duration": millis(t.duration),
            "delay": millis(t.delay),
            "easing": t.easing.css(),
        }),
    }
}
