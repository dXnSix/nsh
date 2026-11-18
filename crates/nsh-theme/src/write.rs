//! Writing a document or a patch as JSON.
//!
//! The text has two-space indentation; the members of `tokens`, `modifiers`,
//! each modifier's `contexts` and `roles` in their order; references as
//! `{name}`; and the canonical forms: colors as lowercase `#rrggbb`, or
//! `#rrggbbaa` below full opacity, durations as `<n>ms`, whole numbers
//! without a fraction, easings and `lineHeight` in the form they were read,
//! compound members at their default left out. `read(&write(&doc))` gives
//! `doc` back, and writing a read document twice gives the same bytes.

use crate::{
    document::{Map, ThemeDocument},
    json::{Json, pretty},
    resolve::Patch,
    value,
};

fn object<V>(map: &Map<V>, json: impl Fn(&V) -> Json) -> Json {
    Json::Object(map.iter().map(|(k, v)| (k.to_owned(), json(v))).collect())
}

/// Writes `document` as JSON text.
pub fn write(document: &ThemeDocument) -> String {
    let mut members = Vec::new();
    if let Some(name) = &document.name {
        members.push(("name".to_owned(), Json::String(name.clone())));
    }
    let tokens = object(&document.tokens, |token| {
        let mut members = vec![
            (
                "kind".to_owned(),
                Json::String(token.value_type.name().to_owned()),
            ),
            ("value".to_owned(), value::to_json(&token.value)),
        ];
        if let Some(path) = &token.path {
            members.push(("path".to_owned(), Json::String(path.clone())));
        }
        if let Some(doc) = &token.doc {
            members.push(("doc".to_owned(), Json::String(doc.clone())));
        }
        Json::Object(members)
    });
    members.push(("tokens".to_owned(), tokens));
    if !document.modifiers.is_empty() {
        let modifiers = object(&document.modifiers, |modifier| {
            Json::Object(vec![
                ("default".to_owned(), Json::String(modifier.default.clone())),
                (
                    "contexts".to_owned(),
                    object(&modifier.contexts, |context| {
                        object(&context.values, value::to_json)
                    }),
                ),
            ])
        });
        members.push(("modifiers".to_owned(), modifiers));
    }
    if !document.modifiers.is_empty() || !document.resolution_order.is_empty() {
        let order = document
            .resolution_order
            .iter()
            .map(|name| Json::String(name.clone()))
            .collect();
        members.push(("resolutionOrder".to_owned(), Json::Array(order)));
    }
    if !document.roles.is_empty() {
        members.push((
            "roles".to_owned(),
            object(&document.roles, |token| Json::String(token.clone())),
        ));
    }
    pretty(&Json::Object(members))
}

/// Writes `patch` as JSON text, `values` and `roles` when they have entries.
/// `read_patch(&write_patch(&patch))` gives a read patch back.
pub fn write_patch(patch: &Patch) -> String {
    let mut members = Vec::new();
    if !patch.values.is_empty() {
        members.push(("values".to_owned(), object(&patch.values, Json::from_serde)));
    }
    if !patch.roles.is_empty() {
        members.push((
            "roles".to_owned(),
            object(&patch.roles, |token| Json::String(token.clone())),
        ));
    }
    pretty(&Json::Object(members))
}
