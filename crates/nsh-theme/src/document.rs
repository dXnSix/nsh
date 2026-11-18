//! The theme document as data: tokens, modifiers with their contexts,
//! `resolutionOrder` and role mappings, in the order they were read or
//! inserted.
//!
//! Edits are unchecked: [`check`](crate::check) applies `read`'s rules to a
//! document in memory. The editing methods here keep order (a replaced member
//! keeps its place, a new one goes last) and cascade as the spec says:
//! removing a token removes the context entries that name it, removing a
//! modifier removes its `resolutionOrder` item.

use std::collections::HashMap;

use crate::value::{Value, ValueType};

/// A map that keeps its members in the order they were inserted.
///
/// Replacing a member keeps its place; a new member goes last; removing one
/// keeps the others' order. Lookups by name are hashed.
#[derive(Clone)]
pub struct Map<V> {
    entries: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> Map<V> {
    /// An empty map.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// The number of members.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map has no member.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The value of `name`.
    pub fn get(&self, name: &str) -> Option<&V> {
        self.index.get(name).map(|&i| &self.entries[i].1)
    }

    /// The value of `name`, to change in place.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut V> {
        self.index.get(name).map(|&i| &mut self.entries[i].1)
    }

    /// Whether `name` is a member.
    pub fn contains_key(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    /// The position of `name` in the map's order.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    /// The member at `position` in the map's order.
    pub fn get_index(&self, position: usize) -> Option<(&str, &V)> {
        self.entries.get(position).map(|(k, v)| (k.as_str(), v))
    }

    /// Sets `name` to `value`. A member that exists keeps its place and its
    /// old value is returned; a new one goes last.
    pub fn insert(&mut self, name: impl Into<String>, value: V) -> Option<V> {
        let name = name.into();
        if let Some(&i) = self.index.get(&name) {
            return Some(std::mem::replace(&mut self.entries[i].1, value));
        }
        self.index.insert(name.clone(), self.entries.len());
        self.entries.push((name, value));
        None
    }

    /// Removes `name`, keeping the others' order, and returns its value.
    pub fn remove(&mut self, name: &str) -> Option<V> {
        let i = self.index.remove(name)?;
        let (_, value) = self.entries.remove(i);
        for at in self.index.values_mut() {
            if *at > i {
                *at -= 1;
            }
        }
        Some(value)
    }

    /// Keeps only the members `keep` returns true for, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&str, &mut V) -> bool) {
        let before = self.entries.len();
        self.entries.retain_mut(|(k, v)| keep(k, v));
        if self.entries.len() != before {
            self.reindex();
        }
    }

    /// The members in order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// The members in order, their values to change in place.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (&str, &mut V)> {
        self.entries.iter_mut().map(|(k, v)| (k.as_str(), v))
    }

    /// The names in order.
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    /// The values in order.
    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }

    fn reindex(&mut self) {
        self.index.clear();
        for (i, (k, _)) in self.entries.iter().enumerate() {
            self.index.insert(k.clone(), i);
        }
    }
}

impl<V> Default for Map<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Equal when the members and their order are.
impl<V: PartialEq> PartialEq for Map<V> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<V: std::fmt::Debug> std::fmt::Debug for Map<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

impl<K: Into<String>, V> FromIterator<(K, V)> for Map<V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Map::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl<'a, V> IntoIterator for &'a Map<V> {
    type Item = (&'a str, &'a V);
    type IntoIter =
        std::iter::Map<std::slice::Iter<'a, (String, V)>, fn(&'a (String, V)) -> (&'a str, &'a V)>;

    fn into_iter(self) -> Self::IntoIter {
        fn pair<V>((k, v): &(String, V)) -> (&str, &V) {
            (k.as_str(), v)
        }
        self.entries
            .iter()
            .map(pair as fn(&'a (String, V)) -> (&'a str, &'a V))
    }
}

/// A theme document: tokens, modifiers, `resolutionOrder` and roles.
///
/// Read one with [`read`](crate::read()), write it with
/// [`write`](crate::write()), check one edited in memory with
/// [`check`](crate::check).
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeDocument {
    /// The document's `name`.
    pub name: Option<String>,
    /// The tokens by name, in order.
    pub tokens: Map<Token>,
    /// The modifiers by name, in order.
    pub modifiers: Map<Modifier>,
    /// The modifiers' names in the order their contexts apply, a later one
    /// winning.
    pub resolution_order: Vec<String>,
    /// Role name → the name of the token the role reads.
    pub roles: Map<String>,
}

impl ThemeDocument {
    /// An empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// The token `name`.
    pub fn token(&self, name: &str) -> Option<&Token> {
        self.tokens.get(name)
    }

    /// Inserts or replaces the token `name`; a replaced token keeps its place
    /// and is returned.
    pub fn insert_token(&mut self, name: impl Into<String>, token: Token) -> Option<Token> {
        self.tokens.insert(name, token)
    }

    /// Removes the token `name` and every context entry that names it. Role
    /// mappings that name it are kept: `check` reports them as
    /// `role-token-missing`. References to it are reported as
    /// `reference-missing` by the next `check`, `read` or `resolve`.
    pub fn remove_token(&mut self, name: &str) -> Option<Token> {
        let token = self.tokens.remove(name)?;
        for (_, modifier) in self.modifiers.iter_mut() {
            for (_, context) in modifier.contexts.iter_mut() {
                context.values.remove(name);
            }
        }
        Some(token)
    }

    /// Inserts or replaces the modifier `name`; a replaced modifier keeps its
    /// place and is returned. A new modifier is not added to
    /// `resolution_order`: insert its item there too.
    pub fn insert_modifier(
        &mut self,
        name: impl Into<String>,
        modifier: Modifier,
    ) -> Option<Modifier> {
        self.modifiers.insert(name, modifier)
    }

    /// Removes the modifier `name` and its `resolution_order` items.
    pub fn remove_modifier(&mut self, name: &str) -> Option<Modifier> {
        let modifier = self.modifiers.remove(name)?;
        self.resolution_order.retain(|item| item != name);
        Some(modifier)
    }

    /// Maps the role `role` onto the token `token`; a replaced mapping keeps
    /// its place and is returned.
    pub fn set_role(
        &mut self,
        role: impl Into<String>,
        token: impl Into<String>,
    ) -> Option<String> {
        self.roles.insert(role, token.into())
    }

    /// Removes the mapping of `role`.
    pub fn remove_role(&mut self, role: &str) -> Option<String> {
        self.roles.remove(role)
    }
}

/// A token: its value type, its value, and an optional path and doc.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// The `kind`: the type every value of this token has.
    pub value_type: ValueType,
    /// The `value`: a literal of `value_type`, or a reference.
    pub value: Value,
    /// The optional `path`, unique across the document's tokens.
    pub path: Option<String>,
    /// The optional `doc`.
    pub doc: Option<String>,
}

impl Token {
    /// A token of `value_type` with `value`, no path and no doc.
    pub fn new(value_type: ValueType, value: Value) -> Self {
        Self {
            value_type,
            value,
            path: None,
            doc: None,
        }
    }

    /// The token with this `path`.
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// The token with this `doc`.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }
}

/// A modifier: named contexts, one of them the default.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Modifier {
    /// The context used when the selection picks none.
    pub default: String,
    /// The contexts by name, in order.
    pub contexts: Map<Context>,
}

impl Modifier {
    /// A modifier whose default is `default`, with no context yet.
    pub fn new(default: impl Into<String>) -> Self {
        Self {
            default: default.into(),
            contexts: Map::new(),
        }
    }

    /// The modifier with the context `name` added or replaced.
    pub fn with_context(mut self, name: impl Into<String>, context: Context) -> Self {
        self.contexts.insert(name, context);
        self
    }

    /// Inserts or replaces the context `name`; a replaced context keeps its
    /// place and is returned.
    pub fn insert_context(&mut self, name: impl Into<String>, context: Context) -> Option<Context> {
        self.contexts.insert(name, context)
    }

    /// Removes the context `name`. Removing the default's context is allowed:
    /// `check` reports `bad-default`.
    pub fn remove_context(&mut self, name: &str) -> Option<Context> {
        self.contexts.remove(name)
    }
}

/// A context: token name → value. It adds no names; an empty context changes
/// nothing.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    /// The entries, in order.
    pub values: Map<Value>,
}

impl Context {
    /// An empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// The context with the entry for `token` set to `value`.
    pub fn with(mut self, token: impl Into<String>, value: Value) -> Self {
        self.values.insert(token, value);
        self
    }

    /// Sets the entry for `token`; a replaced entry keeps its place and is
    /// returned.
    pub fn set(&mut self, token: impl Into<String>, value: Value) -> Option<Value> {
        self.values.insert(token, value)
    }

    /// Removes the entry for `token`.
    pub fn remove(&mut self, token: &str) -> Option<Value> {
        self.values.remove(token)
    }
}
