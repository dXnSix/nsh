//! The JSON tree the reader works on: objects keep their members' order, and
//! a duplicate member is reported where it is written (the first is kept).
//!
//! It is built by a serde visitor over `serde_json::Deserializer`, so the
//! workspace's `serde_json` features stay as they are. serde_json's own
//! recursion limit (128 levels) refuses deeper input with an error, which
//! the reader turns into `not-a-document`, so nothing here recurses deeper
//! than that.

use std::{cell::RefCell, collections::HashSet, fmt};

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::diagnostic::{Diagnostic, code, pointer};

/// A JSON value with ordered objects.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The JSON type's name, for messages.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a boolean",
            Json::Number(_) => "a number",
            Json::String(_) => "a string",
            Json::Array(_) => "an array",
            Json::Object(_) => "an object",
        }
    }

    /// The member `name` of an object.
    pub(crate) fn member(&self, name: &str) -> Option<&Json> {
        match self {
            Json::Object(members) => members.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// From a `serde_json` value (a patch's values), numbers as `f64`.
    pub(crate) fn from_serde(value: &serde_json::Value) -> Json {
        match value {
            serde_json::Value::Null => Json::Null,
            serde_json::Value::Bool(b) => Json::Bool(*b),
            serde_json::Value::Number(n) => n.as_f64().map_or(Json::Null, Json::Number),
            serde_json::Value::String(s) => Json::String(s.clone()),
            serde_json::Value::Array(items) => {
                Json::Array(items.iter().map(Json::from_serde).collect())
            }
            serde_json::Value::Object(members) => Json::Object(
                members
                    .iter()
                    .map(|(k, v)| (k.clone(), Json::from_serde(v)))
                    .collect(),
            ),
        }
    }

    /// To a `serde_json` value. A number JSON cannot hold (not finite)
    /// becomes `null`.
    pub(crate) fn to_serde(&self) -> serde_json::Value {
        match self {
            Json::Null => serde_json::Value::Null,
            Json::Bool(b) => serde_json::Value::Bool(*b),
            Json::Number(n) => serde_json::Number::from_f64(*n)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            Json::String(s) => serde_json::Value::String(s.clone()),
            Json::Array(items) => {
                serde_json::Value::Array(items.iter().map(Json::to_serde).collect())
            }
            Json::Object(members) => serde_json::Value::Object(
                members
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_serde()))
                    .collect(),
            ),
        }
    }
}

/// Parses `text` into a tree, reporting duplicate members into
/// `diagnostics`. `skip` names top-level members whose content is checked as
/// JSON but not kept (`cssVars`). `None` when the text is not JSON.
pub(crate) fn parse(text: &str, skip: &[&str], diagnostics: &mut Vec<Diagnostic>) -> Option<Json> {
    let found = RefCell::new(Vec::new());
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let seed = Seed {
        path: Vec::new(),
        found: &found,
        skip,
    };
    let json = seed.deserialize(&mut deserializer).ok()?;
    deserializer.end().ok()?;
    diagnostics.extend(found.into_inner());
    Some(json)
}

struct Seed<'a> {
    path: Vec<String>,
    found: &'a RefCell<Vec<Diagnostic>>,
    skip: &'a [&'a str],
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Json;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
        Ok(Json::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
        Ok(Json::Number(v as f64))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
        Ok(Json::Number(v as f64))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
        Ok(Json::Number(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Json, E> {
        Ok(Json::String(v.to_owned()))
    }

    fn visit_string<E>(self, v: String) -> Result<Json, E> {
        Ok(Json::String(v))
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_none<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        loop {
            let mut path = self.path.clone();
            path.push(items.len().to_string());
            let seed = Seed {
                path,
                found: self.found,
                skip: &[],
            };
            match seq.next_element_seed(seed)? {
                Some(item) => items.push(item),
                None => break,
            }
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut members: Vec<(String, Json)> = Vec::new();
        let mut seen = HashSet::new();
        let top = self.path.is_empty();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                // The first is kept: the second is checked as JSON only.
                map.next_value::<IgnoredAny>()?;
                if !(top && self.skip.contains(&key.as_str())) {
                    let at = pointer(self.path.iter().map(String::as_str).chain([key.as_str()]));
                    let message =
                        format!("The member `{key}` is written twice; the first is kept.");
                    self.found.borrow_mut().push(Diagnostic::error(
                        code::DUPLICATE_MEMBER,
                        at,
                        message,
                    ));
                }
                continue;
            }
            if top && self.skip.contains(&key.as_str()) {
                map.next_value::<IgnoredAny>()?;
                members.push((key, Json::Null));
                continue;
            }
            let mut path = self.path.clone();
            path.push(key.clone());
            let seed = Seed {
                path,
                found: self.found,
                skip: &[],
            };
            let value = map.next_value_seed(seed)?;
            members.push((key, value));
        }
        Ok(Json::Object(members))
    }
}

/// Writes `json` with two-space indentation: objects and arrays of objects
/// over several lines, arrays of plain values on one.
pub(crate) fn pretty(json: &Json) -> String {
    let mut out = String::new();
    write_pretty(json, 0, &mut out);
    out.push('\n');
    out
}

fn write_pretty(json: &Json, indent: usize, out: &mut String) {
    match json {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) => out.push_str(&number_text(*n)),
        Json::String(s) => out.push_str(&string_text(s)),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Array(items)
            if items
                .iter()
                .all(|i| !matches!(i, Json::Array(_) | Json::Object(_))) =>
        {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_pretty(item, indent, out);
            }
            out.push(']');
        }
        Json::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                push_indent(indent + 1, out);
                write_pretty(item, indent + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            push_indent(indent, out);
            out.push(']');
        }
        Json::Object(members) if members.is_empty() => out.push_str("{}"),
        Json::Object(members) => {
            out.push_str("{\n");
            for (i, (key, value)) in members.iter().enumerate() {
                push_indent(indent + 1, out);
                out.push_str(&string_text(key));
                out.push_str(": ");
                write_pretty(value, indent + 1, out);
                out.push_str(if i + 1 < members.len() { ",\n" } else { "\n" });
            }
            push_indent(indent, out);
            out.push('}');
        }
    }
}

fn push_indent(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn string_text(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}

/// A number as the format writes it: the shortest digits that read back as
/// the same `f64`, never with an exponent or a fraction of zero (`12`, never
/// `12.0`; `0.000001`, never `1e-6`), so durations (`<n>ms`) and `<n>px`
/// read back; `null` for a number JSON cannot hold.
pub(crate) fn number_text(n: f64) -> String {
    if !n.is_finite() {
        return "null".to_owned();
    }
    if n == 0.0 {
        // `-0` reads as 0 anyway.
        return "0".to_owned();
    }
    // Rust's `Display` for `f64` writes no exponent and no `.0`.
    format!("{n}")
}
