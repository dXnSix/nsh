//! Reading a theme document or a patch from JSON, with diagnostics, and
//! checking a document edited in memory by the same rules.
//!
//! Reading has two steps. The first turns the JSON into the model: the
//! document's shape (sections, entries, `kind`, `value`, each value's JSON
//! form). The second applies the rules on the model: names, ranges, unique
//! paths, references, contexts, modifiers, `resolutionOrder` and roles.
//! [`check`] runs the second step alone on a document in memory, so it gives
//! the codes and pointers `read` gives for the document's
//! [`write`](crate::write()).
//!
//! An entry that depends on a token left out for its own error (a context
//! entry for it, a reference to it, a role mapped onto it) is left out with
//! it, without a diagnostic of its own: the token's diagnostic explains it.

use std::collections::HashSet;

use crate::{
    diagnostic::{Diagnostic, code, pointer},
    document::{Context, Map, Modifier, ThemeDocument, Token},
    json::{self, Json},
    resolve::Patch,
    value::{self, Value, ValueType},
};

/// What [`read`] gives: the document, with every entry an error refused left
/// out, and the diagnostics.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Read {
    /// The document as read.
    pub document: ThemeDocument,
    /// Errors (their entries left out) and warnings, in the order found.
    pub diagnostics: Vec<Diagnostic>,
}

/// The members of JSON object `text` in the order it writes them, each
/// value written back as JSON text that keeps its own members' order: how
/// a document or a patch is taken out of a larger object (a bundle's
/// `{ "document": …, "overrides": … }`) for [`read`] and [`read_patch`]
/// without a `serde_json::Value`, whose objects sort their members unless
/// serde_json's `preserve_order` feature is on. A member written twice
/// keeps its first value and is reported (`duplicate-member`, its pointer
/// from `text`'s root). `None` when `text` is no JSON object. Never
/// panics.
///
/// ```
/// let (members, said) = nsh_theme::object_members(r#"{ "b": { "z": 1, "a": 2 }, "a": null }"#);
/// let members = members.unwrap();
/// assert_eq!(members[0].0, "b");
/// assert!(members[0].1.find("\"z\"").unwrap() < members[0].1.find("\"a\"").unwrap());
/// assert_eq!(members[1], ("a".to_owned(), "null\n".to_owned()));
/// assert!(said.is_empty());
/// ```
pub fn object_members(text: &str) -> (Option<Vec<(String, String)>>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let Some(Json::Object(members)) = json::parse(text, &[], &mut diagnostics) else {
        return (None, diagnostics);
    };
    let members = members
        .into_iter()
        .map(|(key, value)| (key, json::pretty(&value)))
        .collect();
    (Some(members), diagnostics)
}

/// Reads a theme document. Never panics, whatever `text` holds.
///
/// Text that is not JSON, or not a JSON object, gives one `not-a-document`
/// error and an empty document; an object with a top-level `cssVars` and no
/// `tokens` gives one `legacy-format` error and an empty document.
pub fn read(text: &str) -> Read {
    let mut diagnostics = Vec::new();
    let Some(Json::Object(members)) = json::parse(text, &["cssVars"], &mut diagnostics) else {
        return Read {
            document: ThemeDocument::new(),
            diagnostics: vec![not_a_document("theme document")],
        };
    };
    let has = |name: &str| members.iter().any(|(k, _)| k == name);
    if has("cssVars") && !has("tokens") {
        return Read {
            document: ThemeDocument::new(),
            diagnostics: vec![Diagnostic::error(
                code::LEGACY_FORMAT,
                "",
                "The document has `cssVars` and no `tokens`: it is the old theme format, which \
                 `nsh theme convert` converts.",
            )],
        };
    }

    let mut document = ThemeDocument::new();
    let mut state = State::default();
    let (mut tokens, mut modifiers, mut order, mut roles) = (None, None, None, None);
    for (key, json) in &members {
        match key.as_str() {
            "$schema" => {}
            "name" => match json {
                Json::String(name) => document.name = Some(name.clone()),
                other => diagnostics.push(Diagnostic::error(
                    code::BAD_VALUE,
                    pointer(["name"]),
                    format!(
                        "The document's `name` is {}, not a string; it is left out.",
                        other.kind()
                    ),
                )),
            },
            "tokens" => tokens = Some(json),
            "modifiers" => modifiers = Some(json),
            "resolutionOrder" => order = Some(json),
            "roles" => roles = Some(json),
            _ => diagnostics.push(unknown_member(&[key], "the document")),
        }
    }

    // Tokens first: contexts read their entries with the tokens' types.
    if let Some(json) = tokens {
        match json {
            Json::Object(entries) => {
                for (name, entry) in entries {
                    match read_token(name, entry, &mut diagnostics) {
                        Some(token) => {
                            document.tokens.insert(name.clone(), token);
                        }
                        None => {
                            state.left_out.insert(name.clone());
                        }
                    }
                }
            }
            other => diagnostics.push(bad_section("tokens", "an object", other)),
        }
    }
    if let Some(json) = modifiers {
        match json {
            Json::Object(entries) => {
                for (name, entry) in entries {
                    state.modifier_names.insert(name.clone());
                    if let Some(modifier) =
                        read_modifier(name, entry, &document, &state, &mut diagnostics)
                    {
                        document.modifiers.insert(name.clone(), modifier);
                    }
                }
            }
            other => diagnostics.push(bad_section("modifiers", "an object", other)),
        }
    }
    if let Some(json) = order {
        match json {
            Json::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    match item {
                        Json::String(name) => state.order.push((i, name.clone())),
                        other => diagnostics.push(Diagnostic::error(
                            code::BAD_VALUE,
                            pointer(["resolutionOrder", &i.to_string()]),
                            format!(
                                "Item {i} of `resolutionOrder` is {}, not a modifier's name; it is ignored.",
                                other.kind()
                            ),
                        )),
                    }
                }
            }
            other => diagnostics.push(bad_section("resolutionOrder", "an array", other)),
        }
    }
    if let Some(json) = roles {
        match json {
            Json::Object(entries) => {
                for (role, token) in entries {
                    match token {
                        Json::String(token) => {
                            document.roles.insert(role.clone(), token.clone());
                        }
                        other => diagnostics.push(Diagnostic::error(
                            code::BAD_VALUE,
                            pointer(["roles", role]),
                            format!(
                                "Role `{role}` maps onto {}, not a token's name; it is left out.",
                                other.kind()
                            ),
                        )),
                    }
                }
            }
            other => diagnostics.push(bad_section("roles", "an object", other)),
        }
    }

    apply_rules(&mut document, state, &mut diagnostics);
    Read {
        document,
        diagnostics,
    }
}

/// Applies `read`'s rules to a document in memory, with the codes and
/// pointers `read` would give for its [`write`](crate::write()).
///
/// The rules of the JSON text itself (`not-a-document`, `legacy-format`,
/// `duplicate-member`, `unknown-member`, `bad-kind`, `missing-value`) cannot
/// fail on a document in memory.
pub fn check(document: &ThemeDocument) -> Vec<Diagnostic> {
    let mut copy = document.clone();
    let state = State {
        left_out: HashSet::new(),
        modifier_names: document.modifiers.keys().map(str::to_owned).collect(),
        order: document
            .resolution_order
            .iter()
            .cloned()
            .enumerate()
            .collect(),
    };
    copy.resolution_order.clear();
    let mut diagnostics = Vec::new();
    apply_rules(&mut copy, state, &mut diagnostics);
    diagnostics
}

/// Reads an override patch, `{ "values": { token → value }, "roles": { role →
/// token } }`. Never panics. The patch is stored as given: its values are
/// read against a document's types by [`Patch::check`] and
/// [`resolve`](crate::resolve()).
pub fn read_patch(text: &str) -> (Patch, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut patch = Patch::new();
    let Some(Json::Object(members)) = json::parse(text, &[], &mut diagnostics) else {
        return (patch, vec![not_a_document("patch")]);
    };
    for (key, json) in &members {
        match (key.as_str(), json) {
            ("values", Json::Object(entries)) => {
                for (name, value) in entries {
                    patch.values.insert(name.clone(), value.to_serde());
                }
            }
            ("roles", Json::Object(entries)) => {
                for (role, token) in entries {
                    match token {
                        Json::String(token) => {
                            patch.roles.insert(role.clone(), token.clone());
                        }
                        other => diagnostics.push(Diagnostic::error(
                            code::BAD_VALUE,
                            pointer(["roles", role]),
                            format!(
                                "The patch maps role `{role}` onto {}, not a token's name; it is left out.",
                                other.kind()
                            ),
                        )),
                    }
                }
            }
            ("values" | "roles", other) => diagnostics.push(bad_section(key, "an object", other)),
            _ => diagnostics.push(unknown_member(&[key], "the patch")),
        }
    }
    (patch, diagnostics)
}

fn not_a_document(what: &str) -> Diagnostic {
    Diagnostic::error(
        code::NOT_A_DOCUMENT,
        "",
        format!("The text is not a {what}: it is not JSON, or not a JSON object."),
    )
}

fn unknown_member(path: &[&str], owner: &str) -> Diagnostic {
    let name = path.last().copied().unwrap_or_default();
    Diagnostic::warning(
        code::UNKNOWN_MEMBER,
        pointer(path.iter().copied()),
        format!("`{name}` is not a member of {owner}; it is ignored."),
    )
}

fn bad_section(name: &str, expected: &str, found: &Json) -> Diagnostic {
    Diagnostic::error(
        code::BAD_VALUE,
        pointer([name]),
        format!(
            "`{name}` is {}, not {expected}; it is ignored.",
            found.kind()
        ),
    )
}

/// What the second step needs to know of the text.
#[derive(Default)]
struct State {
    /// Tokens written but left out for their own errors.
    left_out: HashSet<String>,
    /// The modifiers' names as written, kept or not.
    modifier_names: HashSet<String>,
    /// `resolutionOrder`'s string items with their positions in the text.
    order: Vec<(usize, String)>,
}

fn read_token(name: &str, entry: &Json, diagnostics: &mut Vec<Diagnostic>) -> Option<Token> {
    let at = pointer(["tokens", name]);
    if !value::valid_name(name) {
        diagnostics.push(Diagnostic::error(
            code::BAD_NAME,
            at,
            format!("`{name}` is not a token name: a name is a non-empty string without `{{` or `}}`; it is left out."),
        ));
        return None;
    }
    let Json::Object(members) = entry else {
        diagnostics.push(Diagnostic::error(
            code::BAD_VALUE,
            at,
            format!(
                "Token `{name}` is {}, not an object; it is left out.",
                entry.kind()
            ),
        ));
        return None;
    };
    for (key, _) in members {
        if !matches!(key.as_str(), "kind" | "value" | "path" | "doc") {
            diagnostics.push(unknown_member(
                &["tokens", name, key],
                &format!("token `{name}`"),
            ));
        }
    }
    let value_type = match entry.member("kind") {
        Some(Json::String(kind)) => ValueType::from_name(kind),
        _ => None,
    };
    let Some(value_type) = value_type else {
        let found = match entry.member("kind") {
            None => "has no `kind`".to_owned(),
            Some(Json::String(kind)) => format!("has the unknown kind `{kind}`"),
            Some(other) => format!("has a `kind` that is {}", other.kind()),
        };
        diagnostics.push(Diagnostic::error(
            code::BAD_KIND,
            at,
            format!("Token `{name}` {found}; it is left out."),
        ));
        return None;
    };
    let Some(json) = entry.member("value") else {
        diagnostics.push(Diagnostic::error(
            code::MISSING_VALUE,
            at,
            format!("Token `{name}` has no `value`; it is left out."),
        ));
        return None;
    };
    let text_member = |member: &str| match entry.member(member) {
        None => Ok(None),
        Some(Json::String(text)) => Ok(Some(text.clone())),
        Some(other) => Err(Diagnostic::error(
            code::BAD_VALUE,
            at.clone(),
            format!(
                "Token `{name}`'s `{member}` is {}, not a string; it is left out.",
                other.kind()
            ),
        )),
    };
    let (path, doc) = match (text_member("path"), text_member("doc")) {
        (Ok(path), Ok(doc)) => (path, doc),
        (Err(e), _) | (_, Err(e)) => {
            diagnostics.push(e);
            return None;
        }
    };
    let value = read_value(
        value_type,
        json,
        &at,
        &["tokens", name, "value"],
        &format!("token `{name}`"),
        diagnostics,
    )?;
    Some(Token {
        value_type,
        value,
        path,
        doc,
    })
}

/// Reads a value of `value_type`; `None` (with a `bad-value` at `at`) when its
/// JSON form doesn't fit. Unknown compound members are warned of at `base`.
fn read_value(
    value_type: ValueType,
    json: &Json,
    at: &str,
    base: &[&str],
    what: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Value> {
    let mut unknown = Vec::new();
    match value::parse(value_type, json, &mut unknown) {
        Ok(value) => {
            for path in unknown {
                let full: Vec<&str> = base
                    .iter()
                    .copied()
                    .chain(path.iter().map(String::as_str))
                    .collect();
                diagnostics.push(unknown_member(
                    &full,
                    &format!("the {value_type} of {what}"),
                ));
            }
            Some(value)
        }
        Err(reason) => {
            diagnostics.push(bad_value(at, what, value_type, &reason));
            None
        }
    }
}

pub(crate) fn bad_value(at: &str, what: &str, value_type: ValueType, reason: &str) -> Diagnostic {
    Diagnostic::error(
        code::BAD_VALUE,
        at,
        format!("The value of {what} is not a {value_type}: {reason}; it is left out."),
    )
}

fn read_modifier(
    name: &str,
    entry: &Json,
    document: &ThemeDocument,
    state: &State,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Modifier> {
    let at = pointer(["modifiers", name]);
    let Json::Object(members) = entry else {
        diagnostics.push(Diagnostic::error(
            code::BAD_VALUE,
            at,
            format!(
                "Modifier `{name}` is {}, not an object; it is left out.",
                entry.kind()
            ),
        ));
        return None;
    };
    for (key, _) in members {
        if !matches!(key.as_str(), "default" | "contexts") {
            diagnostics.push(unknown_member(
                &["modifiers", name, key],
                &format!("modifier `{name}`"),
            ));
        }
    }
    let default = match entry.member("default") {
        Some(Json::String(default)) => default.clone(),
        found => {
            // Every condition that holds is reported: too few contexts too.
            let count = match entry.member("contexts") {
                Some(Json::Object(contexts)) => contexts.len(),
                _ => 0,
            };
            if count < 2 {
                diagnostics.push(too_few_contexts(name, count));
            }
            let found = found.map_or("no `default`".to_owned(), |f| {
                format!("a `default` that is {}", f.kind())
            });
            diagnostics.push(Diagnostic::error(
                code::BAD_DEFAULT,
                at,
                format!("Modifier `{name}` has {found}, not a context's name; it is left out."),
            ));
            return None;
        }
    };
    let mut modifier = Modifier::new(default);
    let contexts = match entry.member("contexts") {
        None => return Some(modifier),
        Some(Json::Object(contexts)) => contexts,
        Some(other) => {
            diagnostics.push(Diagnostic::error(
                code::BAD_VALUE,
                at,
                format!(
                    "Modifier `{name}`'s `contexts` is {}, not an object; it is left out.",
                    other.kind()
                ),
            ));
            return None;
        }
    };
    for (context_name, json) in contexts {
        let context_at = pointer(["modifiers", name, "contexts", context_name]);
        let Json::Object(entries) = json else {
            diagnostics.push(Diagnostic::error(
                code::BAD_VALUE,
                context_at,
                format!(
                    "Context `{context_name}` of modifier `{name}` is {}, not an object; it is left out.",
                    json.kind()
                ),
            ));
            continue;
        };
        let mut context = Context::new();
        for (token_name, json) in entries {
            let entry_at = pointer(["modifiers", name, "contexts", context_name, token_name]);
            let Some(token) = document.tokens.get(token_name) else {
                if !state.left_out.contains(token_name) {
                    diagnostics.push(undeclared(
                        &entry_at,
                        token_name,
                        &format!("context `{context_name}` of modifier `{name}`"),
                    ));
                }
                continue;
            };
            let what =
                format!("token `{token_name}` in context `{context_name}` of modifier `{name}`");
            let base = ["modifiers", name, "contexts", context_name, token_name];
            if let Some(value) =
                read_value(token.value_type, json, &entry_at, &base, &what, diagnostics)
            {
                context.values.insert(token_name.clone(), value);
            }
        }
        modifier.contexts.insert(context_name.clone(), context);
    }
    Some(modifier)
}

pub(crate) fn undeclared(at: &str, token: &str, place: &str) -> Diagnostic {
    Diagnostic::error(
        code::UNDECLARED_TOKEN,
        at,
        format!(
            "The entry for `{token}` in {place} names no token of the document; it is left out."
        ),
    )
}

/// Why a reference in a value is refused, if it is.
pub(crate) enum RefProblem {
    /// The target is undeclared.
    Missing(String),
    /// The target was left out for its own error.
    LeftOut,
    /// The target has another type.
    Type(String, ValueType, ValueType),
}

/// Checks the references of `value` (of a token of type `own`) against the
/// document's tokens.
pub(crate) fn check_references(
    own: ValueType,
    value: &Value,
    tokens: &Map<Token>,
    left_out: &HashSet<String>,
) -> Result<(), RefProblem> {
    let mut found = Vec::new();
    value::references(own, value, &mut found);
    for (target, needed) in found {
        match tokens.get(target) {
            None if left_out.contains(target) => return Err(RefProblem::LeftOut),
            None => return Err(RefProblem::Missing(target.to_owned())),
            Some(token) if token.value_type != needed => {
                return Err(RefProblem::Type(
                    target.to_owned(),
                    needed,
                    token.value_type,
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

pub(crate) fn reference_diagnostic(
    problem: RefProblem,
    at: &str,
    what: &str,
) -> Option<Diagnostic> {
    match problem {
        RefProblem::LeftOut => None,
        RefProblem::Missing(target) => Some(Diagnostic::error(
            code::REFERENCE_MISSING,
            at,
            format!(
                "The value of {what} refers to `{target}`, which the document doesn't declare; it is left out."
            ),
        )),
        RefProblem::Type(target, needed, found) => Some(Diagnostic::error(
            code::REFERENCE_TYPE,
            at,
            format!(
                "The value of {what} refers to `{target}`, a {found} token, where a {needed} is needed; it is left out."
            ),
        )),
    }
}

/// The second step: the rules on the model. Entries an error refuses are
/// removed from `document`.
fn apply_rules(document: &mut ThemeDocument, mut state: State, diagnostics: &mut Vec<Diagnostic>) {
    // Names, values and paths.
    let mut paths = HashSet::new();
    let mut refused = Vec::new();
    for (name, token) in document.tokens.iter() {
        let at = pointer(["tokens", name]);
        if !value::valid_name(name) {
            diagnostics.push(Diagnostic::error(
                code::BAD_NAME,
                at,
                format!("`{name}` is not a token name: a name is a non-empty string without `{{` or `}}`; it is left out."),
            ));
            refused.push(name.to_owned());
        } else if let Err(reason) = value::validate(token.value_type, &token.value) {
            diagnostics.push(bad_value(
                &at,
                &format!("token `{name}`"),
                token.value_type,
                &reason,
            ));
            refused.push(name.to_owned());
        } else if let Some(path) = &token.path
            && !paths.insert(path.clone())
        {
            diagnostics.push(Diagnostic::error(
                code::DUPLICATE_PATH,
                at,
                format!("Token `{name}` has the path `{path}`, which an earlier token has; it is left out."),
            ));
            refused.push(name.to_owned());
        }
    }
    for name in refused {
        document.tokens.remove(&name);
        state.left_out.insert(name);
    }

    // References between tokens, until no refusal leaves another reference
    // dangling.
    loop {
        let mut refused = Vec::new();
        for (name, token) in document.tokens.iter() {
            if let Err(problem) = check_references(
                token.value_type,
                &token.value,
                &document.tokens,
                &state.left_out,
            ) {
                let at = pointer(["tokens", name]);
                diagnostics.extend(reference_diagnostic(
                    problem,
                    &at,
                    &format!("token `{name}`"),
                ));
                refused.push(name.to_owned());
            }
        }
        if refused.is_empty() {
            break;
        }
        for name in refused {
            document.tokens.remove(&name);
            state.left_out.insert(name);
        }
    }

    // Context entries, then each modifier as a whole.
    let tokens = &document.tokens;
    let left_out = &state.left_out;
    document.modifiers.retain(|name, modifier| {
        for (context_name, context) in modifier.contexts.iter_mut() {
            context.values.retain(|token_name, value| {
                let at = pointer(["modifiers", name, "contexts", context_name, token_name]);
                let place = format!("context `{context_name}` of modifier `{name}`");
                let Some(token) = tokens.get(token_name) else {
                    if !left_out.contains(token_name) {
                        diagnostics.push(undeclared(&at, token_name, &place));
                    }
                    return false;
                };
                let what = format!("token `{token_name}` in {place}");
                if let Err(reason) = value::validate(token.value_type, value) {
                    diagnostics.push(bad_value(&at, &what, token.value_type, &reason));
                    return false;
                }
                if let Err(problem) = check_references(token.value_type, value, tokens, left_out) {
                    diagnostics.extend(reference_diagnostic(problem, &at, &what));
                    return false;
                }
                true
            });
        }
        let at = pointer(["modifiers", name]);
        // Every condition that holds is reported; the modifier is left out
        // once.
        let too_few = modifier.contexts.len() < 2;
        if too_few {
            diagnostics.push(too_few_contexts(name, modifier.contexts.len()));
        }
        let no_default = !modifier.contexts.contains_key(&modifier.default);
        if no_default {
            diagnostics.push(Diagnostic::error(
                code::BAD_DEFAULT,
                at,
                format!(
                    "Modifier `{name}`'s default `{}` names none of its contexts; it is left out.",
                    modifier.default
                ),
            ));
        }
        !(too_few || no_default)
    });

    // resolutionOrder: each modifier exactly once.
    let mut listed = HashSet::new();
    let mut order = Vec::new();
    for (i, name) in &state.order {
        let at = pointer(["resolutionOrder", &i.to_string()]);
        if !state.modifier_names.contains(name) {
            diagnostics.push(Diagnostic::error(
                code::ORDER_UNKNOWN,
                at,
                format!(
                    "Item {i} of `resolutionOrder`, `{name}`, names no modifier; it is ignored."
                ),
            ));
        } else if !listed.insert(name.clone()) {
            diagnostics.push(Diagnostic::error(
                code::ORDER_DUPLICATE,
                at,
                format!(
                    "Item {i} of `resolutionOrder` lists `{name}` a second time; it is ignored."
                ),
            ));
        } else if document.modifiers.contains_key(name) {
            order.push(name.clone());
        }
    }
    document.modifiers.retain(|name, _| {
        if listed.contains(name) {
            return true;
        }
        diagnostics.push(Diagnostic::error(
            code::ORDER_MISSING,
            pointer(["modifiers", name]),
            format!("Modifier `{name}` is not listed in `resolutionOrder`; it is left out."),
        ));
        false
    });
    document.resolution_order = order;

    // Roles.
    let tokens = &document.tokens;
    document.roles.retain(|role, token| {
        if tokens.contains_key(token) {
            return true;
        }
        if !left_out.contains(token.as_str()) {
            diagnostics.push(role_token_missing(
                &pointer(["roles", role]),
                role,
                token,
                "The document",
            ));
        }
        false
    });
}

fn too_few_contexts(name: &str, count: usize) -> Diagnostic {
    Diagnostic::error(
        code::TOO_FEW_CONTEXTS,
        pointer(["modifiers", name]),
        format!(
            "Modifier `{name}` has {count} context(s), and a modifier needs two or more; it is left out."
        ),
    )
}

pub(crate) fn role_token_missing(at: &str, role: &str, token: &str, whose: &str) -> Diagnostic {
    Diagnostic::error(
        code::ROLE_TOKEN_MISSING,
        at,
        format!(
            "{whose} maps role `{role}` onto `{token}`, which the document doesn't declare; the mapping is left out."
        ),
    )
}
