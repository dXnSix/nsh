//! Resolving a document: the layers, then references, once.
//!
//! [`resolve`] is the one implementation of the stack:
//!
//! 1. every token's `value`;
//! 2. for each modifier in `resolutionOrder`, the entries of its selected
//!    context (the [`Selection`]'s pick, or else its `default`);
//! 3. the patches, in the order given, a later one winning (the app's patch,
//!    then the host's, then a scope's);
//! 4. references, resolved once against the final values.
//!
//! A cycle the layers form loses its highest entry (`reference-cycle`), and
//! its token takes the next layer down's value; this repeats until no cycle
//! remains. It costs O(tokens + references + patch entries) per run, and
//! runs once per change, never per read.

use std::collections::HashSet;

use crate::{
    diagnostic::{Diagnostic, code, pointer},
    document::{Map, ThemeDocument},
    json::Json,
    read::{
        RefProblem, bad_value, check_references, reference_diagnostic, role_token_missing,
        undeclared,
    },
    value::{self, Value, ValueType},
};

/// Which context each modifier shows. A modifier it doesn't name shows its
/// `default`.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    /// Modifier name → context name.
    pub picks: Map<String>,
}

impl Selection {
    /// No pick: every modifier shows its default.
    pub fn new() -> Self {
        Self::default()
    }

    /// The selection with `modifier` showing `context`.
    pub fn with(mut self, modifier: impl Into<String>, context: impl Into<String>) -> Self {
        self.picks.insert(modifier, context.into());
        self
    }

    /// Picks `context` for `modifier`, returning the previous pick.
    pub fn set(
        &mut self,
        modifier: impl Into<String>,
        context: impl Into<String>,
    ) -> Option<String> {
        self.picks.insert(modifier, context.into())
    }

    /// Removes the pick of `modifier`: it shows its default again.
    pub fn remove(&mut self, modifier: &str) -> Option<String> {
        self.picks.remove(modifier)
    }

    /// The context picked for `modifier`.
    pub fn get(&self, modifier: &str) -> Option<&str> {
        self.picks.get(modifier).map(String::as_str)
    }
}

/// An override patch: values for declared tokens and role mappings.
///
/// A patch is stored as given, its values in their JSON form: they are read
/// against the active document's types. [`Patch::check`] is its one
/// validator, and [`resolve`] skips the entries it refuses, so an override of
/// a name one document lacks applies again when a document that declares it
/// is active.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    /// Token name → value, in its JSON form, numbers as `f64` (as
    /// [`Patch::set_value`] and `read_patch` store them).
    pub values: Map<serde_json::Value>,
    /// Role name → token name.
    pub roles: Map<String>,
}

impl Patch {
    /// An empty patch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the value of `token` (in its JSON form); a replaced entry keeps
    /// its place. Numbers are stored as `f64`, as `read_patch` stores them,
    /// so a patch equals itself after `write_patch` and `read_patch`.
    pub fn set_value(
        &mut self,
        token: impl Into<String>,
        value: serde_json::Value,
    ) -> Option<serde_json::Value> {
        self.values
            .insert(token, Json::from_serde(&value).to_serde())
    }

    /// Sets the value of `token` from a typed value.
    pub fn set(&mut self, token: impl Into<String>, value: &Value) -> Option<serde_json::Value> {
        self.values.insert(token, value.to_json())
    }

    /// Maps `role` onto `token`; a replaced mapping keeps its place.
    pub fn set_role(
        &mut self,
        role: impl Into<String>,
        token: impl Into<String>,
    ) -> Option<String> {
        self.roles.insert(role, token.into())
    }

    /// Removes the value of `token`.
    pub fn remove_value(&mut self, token: &str) -> Option<serde_json::Value> {
        self.values.remove(token)
    }

    /// Removes the mapping of `role`.
    pub fn remove_role(&mut self, role: &str) -> Option<String> {
        self.roles.remove(role)
    }

    /// Merges `later` into this patch: its entries replace this one's for
    /// the same token or role, new ones go last.
    pub fn merge(&mut self, later: &Patch) {
        for (name, value) in later.values.iter() {
            self.values.insert(name, value.clone());
        }
        for (role, token) in later.roles.iter() {
            self.roles.insert(role, token.clone());
        }
    }

    /// Whether the patch has no entry.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.roles.is_empty()
    }

    /// Checks the patch against `document`: `undeclared-token` for a value of
    /// a name the document doesn't declare (a patch never adds a name),
    /// `bad-value` for a value that doesn't fit the token's type,
    /// `reference-missing` / `reference-type` for its references,
    /// `role-token-missing` for a role mapped onto an undeclared token, and
    /// `unknown-member` warnings for unknown compound members. [`resolve`]
    /// skips exactly the entries this refuses.
    pub fn check(&self, document: &ThemeDocument) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (name, json) in self.values.iter() {
            if let Err(Some(diagnostic)) = patch_value(document, name, json, &mut diagnostics) {
                diagnostics.push(diagnostic);
            }
        }
        for (role, token) in self.roles.iter() {
            if !document.tokens.contains_key(token) {
                diagnostics.push(role_token_missing(
                    &pointer(["roles", role]),
                    role,
                    token,
                    "The patch",
                ));
            }
        }
        diagnostics
    }
}

/// A patch's value for `name`, read against the document: the value, or
/// the diagnostic that refuses it (`None` never: a refusal always says why).
fn patch_value(
    document: &ThemeDocument,
    name: &str,
    json: &serde_json::Value,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Value, Option<Diagnostic>> {
    let at = pointer(["values", name]);
    let Some(token) = document.tokens.get(name) else {
        return Err(Some(undeclared(&at, name, "the patch")));
    };
    let what = format!("token `{name}` in the patch");
    let mut unknown = Vec::new();
    let value = value::parse(token.value_type, &Json::from_serde(json), &mut unknown)
        .and_then(|value| value::validate(token.value_type, &value).map(|()| value))
        .map_err(|reason| Some(bad_value(&at, &what, token.value_type, &reason)))?;
    check_references(token.value_type, &value, &document.tokens, &HashSet::new())
        .map_err(|problem| reference_diagnostic(problem, &at, &what))?;
    for path in unknown {
        let full: Vec<&str> = ["values", name]
            .into_iter()
            .chain(path.iter().map(String::as_str))
            .collect();
        let member = full.last().copied().unwrap_or_default();
        warnings.push(Diagnostic::warning(
            code::UNKNOWN_MEMBER,
            pointer(full.iter().copied()),
            format!(
                "`{member}` is not a member of the {} of {what}; it is ignored.",
                token.value_type
            ),
        ));
    }
    Ok(value)
}

/// What [`resolve`] gives: every token's final value, with no reference
/// left, the role mapping, and the diagnostics.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    /// Token name → its value, in the document's order. A token whose every
    /// entry a cycle refused, or that refers to such a token, has none.
    pub values: Map<Value>,
    /// Role name → the token its highest mapping onto a declared token names:
    /// the patches' `roles` (a later patch first), then the document's.
    /// Role types are checked by [`check_roles`](crate::check_roles) and
    /// [`resolve_role`](crate::resolve_role), which take the declarations.
    pub roles: Map<String>,
    /// `unknown-context`, `reference-missing` and `reference-cycle`.
    pub diagnostics: Vec<Diagnostic>,
}

/// What differs between two [`Resolved`]s.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The token names whose values differ (or that one of the two lacks).
    pub tokens: Vec<String>,
    /// The role names whose resolved token, or that token's value, differs.
    pub roles: Vec<String>,
}

impl Changes {
    /// Whether nothing differs.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty() && self.roles.is_empty()
    }
}

impl Resolved {
    /// The resolved value of the token `name`.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// The token names and role names that differ from `other`: exactly the
    /// tokens whose values differ, and the roles whose resolved token or
    /// value differs. This one's order first, then names only `other` has.
    pub fn changed(&self, other: &Resolved) -> Changes {
        let mut changes = Changes::default();
        for (name, value) in self.values.iter() {
            if other.values.get(name) != Some(value) {
                changes.tokens.push(name.to_owned());
            }
        }
        for name in other.values.keys() {
            if !self.values.contains_key(name) {
                changes.tokens.push(name.to_owned());
            }
        }
        let role_differs = |role: &str| {
            let mine = self.roles.get(role);
            let theirs = other.roles.get(role);
            mine != theirs
                || mine.is_some_and(|token| self.values.get(token) != other.values.get(token))
        };
        for role in self.roles.keys() {
            if role_differs(role) {
                changes.roles.push(role.to_owned());
            }
        }
        for role in other.roles.keys() {
            if !self.roles.contains_key(role) {
                changes.roles.push(role.to_owned());
            }
        }
        changes
    }
}

/// One entry of a token's stack.
struct Entry {
    /// (layer, position in that layer): higher wins, and loses first in a
    /// cycle.
    rank: (usize, usize),
    value: Value,
    pointer: String,
    /// Where the entry is, for messages.
    place: String,
}

/// Resolves `document` under `selection` and `patches` (a later patch
/// winning). Never panics.
pub fn resolve(document: &ThemeDocument, selection: &Selection, patches: &[&Patch]) -> Resolved {
    let tokens = &document.tokens;
    let mut diagnostics = Vec::new();
    let mut stacks: Vec<Vec<Entry>> = (0..tokens.len()).map(|_| Vec::new()).collect();
    let none = HashSet::new();

    // A document's entry: a missing reference is reported, a wrong type
    // (an unchecked document) skipped, as `check` reports it.
    let mut push_document_entry =
        |index: usize,
         entry: Entry,
         ty: ValueType,
         what: &str,
         diagnostics: &mut Vec<Diagnostic>| {
            if value::validate(ty, &entry.value).is_err() {
                return;
            }
            match check_references(ty, &entry.value, tokens, &none) {
                Ok(()) => stacks[index].push(entry),
                Err(problem @ RefProblem::Missing(_)) => {
                    diagnostics.extend(reference_diagnostic(problem, &entry.pointer, what));
                }
                Err(_) => {}
            }
        };

    // 1. The tokens' own values.
    for (index, (name, token)) in tokens.iter().enumerate() {
        let entry = Entry {
            rank: (0, index),
            value: token.value.clone(),
            pointer: pointer(["tokens", name]),
            place: "its own value".to_owned(),
        };
        push_document_entry(
            index,
            entry,
            token.value_type,
            &format!("token `{name}`"),
            &mut diagnostics,
        );
    }

    // 2. The selected contexts, in resolutionOrder.
    let mut layer = 1;
    let mut seen = HashSet::new();
    for modifier_name in &document.resolution_order {
        if !seen.insert(modifier_name.as_str()) {
            continue;
        }
        let Some(modifier) = document.modifiers.get(modifier_name) else {
            continue;
        };
        let picked = selection.get(modifier_name);
        let context_name = match picked {
            Some(pick) if modifier.contexts.contains_key(pick) => pick,
            Some(pick) => {
                diagnostics.push(Diagnostic::warning(
                    code::UNKNOWN_CONTEXT,
                    pointer(["modifiers", modifier_name]),
                    format!(
                        "Modifier `{modifier_name}` has no context `{pick}`; its default `{}` is used.",
                        modifier.default
                    ),
                ));
                modifier.default.as_str()
            }
            None => modifier.default.as_str(),
        };
        let Some(context) = modifier.contexts.get(context_name) else {
            continue;
        };
        for (position, (token_name, value)) in context.values.iter().enumerate() {
            let Some(index) = tokens.position(token_name) else {
                continue;
            };
            let Some((_, token)) = tokens.get_index(index) else {
                continue;
            };
            let place = format!("context `{context_name}` of modifier `{modifier_name}`");
            let entry = Entry {
                rank: (layer, position),
                value: value.clone(),
                pointer: pointer([
                    "modifiers",
                    modifier_name,
                    "contexts",
                    context_name,
                    token_name,
                ]),
                place: place.clone(),
            };
            let what = format!("token `{token_name}` in {place}");
            push_document_entry(index, entry, token.value_type, &what, &mut diagnostics);
        }
        layer += 1;
    }

    // 3. The patches, each entry as `Patch::check` takes it.
    for (number, patch) in patches.iter().enumerate() {
        for (position, (token_name, json)) in patch.values.iter().enumerate() {
            let Ok(value) = patch_value(document, token_name, json, &mut Vec::new()) else {
                continue;
            };
            let Some(index) = tokens.position(token_name) else {
                continue;
            };
            stacks[index].push(Entry {
                rank: (layer, position),
                value,
                pointer: pointer(["values", token_name]),
                place: format!("patch {}", number + 1),
            });
        }
        layer += 1;
    }

    // 4. References, once, against the final values; a cycle loses its
    // highest entry until none remains. The walk's state survives a cycle's
    // break: a token already done never depends on one still on the walk,
    // so only the walk's tokens are walked again.
    let mut visits = vec![Visit::New; tokens.len()];
    let mut walk: Vec<Step> = Vec::new();
    for start in 0..tokens.len() {
        if !matches!(visits[start], Visit::New) {
            continue;
        }
        loop {
            visits[start] = Visit::Open(0);
            walk.push((start, targets(document, &stacks, start), 0));
            let Err(cycle) = advance(document, &stacks, &mut visits, &mut walk) else {
                break;
            };
            for (index, _, _) in walk.drain(..) {
                visits[index] = Visit::New;
            }
            let loser = cycle
                .iter()
                .copied()
                .max_by_key(|&i| stacks[i].last().map_or((0, 0), |entry| entry.rank));
            let Some(entry) = loser.and_then(|i| stacks[i].pop()) else {
                // Every token of a cycle has an entry; never reached.
                for index in cycle {
                    visits[index] = Visit::Done(None);
                }
                break;
            };
            let names: Vec<&str> = cycle
                .iter()
                .chain(cycle.first())
                .filter_map(|&i| tokens.get_index(i).map(|(name, _)| name))
                .collect();
            let loser_name = loser
                .and_then(|i| tokens.get_index(i))
                .map_or("", |(name, _)| name);
            diagnostics.push(Diagnostic::error(
                code::REFERENCE_CYCLE,
                entry.pointer,
                format!(
                    "The references {} form a cycle; the entry of `{loser_name}` in {} is left out.",
                    names.join(" → "),
                    entry.place
                ),
            ));
        }
    }
    let values = visits.into_iter().map(|visit| match visit {
        Visit::Done(value) => value,
        _ => None,
    });

    let mut resolved_values = Map::new();
    for (index, value) in values.into_iter().enumerate() {
        if let (Some(value), Some((name, _))) = (value, tokens.get_index(index)) {
            resolved_values.insert(name, value);
        }
    }

    // The role mapping: the highest mapping onto a declared token.
    let mut roles = Map::new();
    let names = document
        .roles
        .keys()
        .chain(patches.iter().flat_map(|patch| patch.roles.keys()));
    for role in names {
        if roles.contains_key(role) {
            continue;
        }
        let layers = patches
            .iter()
            .rev()
            .filter_map(|patch| patch.roles.get(role))
            .chain(document.roles.get(role));
        if let Some(token) = layers.into_iter().find(|token| tokens.contains_key(token)) {
            roles.insert(role, token.clone());
        }
    }

    Resolved {
        values: resolved_values,
        roles,
        diagnostics,
    }
}

#[derive(Clone)]
enum Visit {
    New,
    /// On the walk's stack at this depth.
    Open(usize),
    Done(Option<Value>),
}

/// A token on the walk: its index, its references' targets, the next one
/// to visit.
type Step = (usize, Vec<Option<usize>>, usize);

/// The tokens the top entry of `index`'s stack refers to.
fn targets(document: &ThemeDocument, stacks: &[Vec<Entry>], index: usize) -> Vec<Option<usize>> {
    let tokens = &document.tokens;
    let (Some(entry), Some((_, token))) = (stacks[index].last(), tokens.get_index(index)) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    value::references(token.value_type, &entry.value, &mut found);
    found
        .into_iter()
        .map(|(name, _)| tokens.position(name))
        .collect()
}

/// Walks on until `walk` is empty, each token's value from the top of its
/// stack with references replaced; a cycle's tokens (in reference order)
/// when the top entries form one, the walk left as it is. Iterative: no
/// input recurses.
fn advance(
    document: &ThemeDocument,
    stacks: &[Vec<Entry>],
    visits: &mut [Visit],
    walk: &mut Vec<Step>,
) -> Result<(), Vec<usize>> {
    let tokens = &document.tokens;
    while let Some((index, deps, next)) = walk.last_mut() {
        if let Some(dep) = deps.get(*next).copied() {
            *next += 1;
            let Some(dep) = dep else { continue };
            match visits[dep] {
                Visit::Done(_) => {}
                Visit::Open(depth) => {
                    return Err(walk[depth..].iter().map(|(i, _, _)| *i).collect());
                }
                Visit::New => {
                    visits[dep] = Visit::Open(walk.len());
                    let dep_targets = targets(document, stacks, dep);
                    walk.push((dep, dep_targets, 0));
                }
            }
            continue;
        }
        let index = *index;
        let value = stacks[index].last().and_then(|entry| {
            let lookup = |name: &str| match tokens.position(name).map(|i| &visits[i]) {
                Some(Visit::Done(value)) => value.clone(),
                _ => None,
            };
            value::substitute(&entry.value, &lookup)
        });
        visits[index] = Visit::Done(value);
        walk.pop();
    }
    Ok(())
}
