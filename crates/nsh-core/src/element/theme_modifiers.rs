//! Theme modifiers: the other modes of a theme (a brand, a density, a
//! contrast), precomputed, as a DTCG resolver's modifiers are. A theme file's
//! `"modifiers"` section names each modifier with its `default` and its
//! `contexts`, each context a partial `cssVars` of only what it changes
//! (light and dark stay native inside every context):
//!
//! ```json
//! "modifiers": {
//!   "brand": {
//!     "default": "ocean",
//!     "contexts": {
//!       "ocean": {},
//!       "forest": { "light": { "brand-accent": "#16a34a" }, "dark": { "brand-accent": "#4ade80" } }
//!     }
//!   }
//! }
//! ```
//!
//! The tokens are the file's values with each modifier's selected context
//! over them, in file order (a later modifier wins on a token two change);
//! at first each modifier's default. A context adds no names, so the classes
//! are the same in every context. [`super::set_modifier`] (TS:
//! `setModifier`) picks a context; what reads a token it changes follows.

use std::sync::Arc;

use super::{Tokens, theme_file::LeftOut};

/// A modifier as it is listed ([`ThemeModifiers::axes`], TS: `modifiers()`):
/// its name, its contexts in the file's order and its default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierAxis {
    /// Its name (`brand`).
    pub name: Box<str>,
    /// Its contexts (`ocean`, `forest`).
    pub contexts: Vec<Box<str>>,
    /// The context picked when nothing else is.
    pub default: Box<str>,
}

/// A theme's modifiers, with the context each has now. Tokens read from a
/// theme file that has them carry them ([`Tokens::modifiers`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeModifiers {
    /// The file's own values of every name a context sets.
    base: Tokens,
    axes: Vec<Axis>,
}

#[derive(Debug, Clone, PartialEq)]
struct Axis {
    name: Box<str>,
    default: usize,
    selected: usize,
    /// Each context's name and the values it sets.
    contexts: Vec<(Box<str>, Tokens)>,
}

impl ThemeModifiers {
    /// Each modifier, in the file's order.
    pub fn axes(&self) -> Vec<ModifierAxis> {
        self.axes
            .iter()
            .map(|axis| ModifierAxis {
                name: axis.name.clone(),
                contexts: axis.contexts.iter().map(|(name, _)| name.clone()).collect(),
                default: axis.contexts[axis.default].0.clone(),
            })
            .collect()
    }

    /// The context modifier `name` has now.
    pub fn selected(&self, name: &str) -> Option<&str> {
        let axis = self.axes.iter().find(|axis| *axis.name == *name)?;
        Some(&axis.contexts[axis.selected].0)
    }

    /// These modifiers with `name`'s context `context`. `Err` says there is
    /// no such modifier, or no such context of it.
    pub fn select(&self, name: &str, context: &str) -> Result<ThemeModifiers, String> {
        let mut out = self.clone();
        let Some(axis) = out.axes.iter_mut().find(|axis| *axis.name == *name) else {
            return Err(format!("the theme has no modifier `{name}`"));
        };
        let Some(i) = axis.contexts.iter().position(|(c, _)| **c == *context) else {
            return Err(format!("the modifier `{name}` has no context `{context}`"));
        };
        axis.selected = i;
        Ok(out)
    }

    /// The tokens the contexts set, as the contexts picked now make them:
    /// each name a context sets is the value of the last modifier whose
    /// context picked sets it, else the file's own.
    pub fn tokens(&self) -> Tokens {
        let mut out = Tokens::default();
        let mut names: Vec<&str> = self
            .axes
            .iter()
            .flat_map(|axis| axis.contexts.iter().flat_map(|(_, t)| t.names()))
            .collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            let from = self
                .axes
                .iter()
                .rev()
                .map(|axis| &axis.contexts[axis.selected].1)
                .find(|t| t.kind(name).is_some())
                .unwrap_or(&self.base);
            out.copy_named(from, name);
        }
        out
    }

    /// These modifiers and `other`'s, `other`'s after (a modifier of `other`
    /// wins on a token both change). `Err` names a modifier both have.
    pub(crate) fn merged(mut self, other: ThemeModifiers) -> Result<ThemeModifiers, String> {
        for axis in &other.axes {
            if self.axes.iter().any(|a| a.name == axis.name) {
                return Err(format!("the theme modifier `{}` is given twice", axis.name));
            }
        }
        for name in other.base.names().collect::<Vec<_>>() {
            self.base.copy_named(&other.base, name);
        }
        self.axes.extend(other.axes);
        Ok(self)
    }

    /// These modifiers with `base`'s values as the own values of the
    /// tokens their contexts set: the values under the contexts once a
    /// theme file sets some of them over a document's.
    pub(crate) fn rebased(mut self, base: &Tokens) -> ThemeModifiers {
        for name in self.base.names().map(str::to_owned).collect::<Vec<_>>() {
            if base.kind(&name).is_some() {
                self.base.copy_named(base, &name);
            }
        }
        self
    }

    /// `tokens` with these modifiers' contexts picked now over them, and
    /// carrying them.
    pub(crate) fn apply(self, tokens: &Tokens) -> Tokens {
        let mut out = self.tokens().over(tokens);
        out.set_modifiers(Some(Arc::new(self)));
        out
    }

    /// The modifiers of theme file `value`, whose own values are `base`,
    /// with what of them was left out. `read` reads a context (a theme file
    /// of its `cssVars`) against `base`, as the file's changes are read. A
    /// context adds no names: one `base` lacks is left out.
    pub(crate) fn read(
        json: &str,
        value: &crate::JsonValue,
        base: &Tokens,
        read: impl Fn(&str) -> Result<(Tokens, Vec<LeftOut>), String>,
    ) -> (Option<ThemeModifiers>, Vec<LeftOut>) {
        let mut left_out = Vec::new();
        let Some(section) = value.get("modifiers") else {
            return (None, left_out);
        };
        let Some(modifiers) = section.as_object() else {
            left_out.push(LeftOut::said(
                "modifiers: expected an object of modifier names and their contexts",
            ));
            return (None, left_out);
        };
        let mut axes = Vec::new();
        let mut touched = Tokens::default();
        for (name, modifier) in modifiers {
            let at = format!("modifiers.{name}");
            let contexts = modifier
                .get("contexts")
                .and_then(crate::JsonValue::as_object);
            let Some(contexts) = contexts.filter(|c| c.len() >= 2) else {
                left_out.push(LeftOut::said(format!(
                    "{at}: expected `contexts`, an object of two or more contexts"
                )));
                continue;
            };
            let default = modifier.get("default").and_then(crate::JsonValue::as_str);
            let Some(default) = default.and_then(|d| contexts.keys().position(|c| c == d)) else {
                left_out.push(LeftOut::said(format!(
                    "{at}: expected `default`, the name of one of its contexts"
                )));
                continue;
            };
            let mut read_contexts = Vec::new();
            for (context, vars) in contexts {
                let at = format!("{at}.contexts.{context}");
                if !vars.is_object() {
                    left_out.push(LeftOut::said(format!(
                        "{at}: expected `{{ theme, light, dark }}`, as `cssVars` is"
                    )));
                    read_contexts.push((context.as_str().into(), Tokens::default()));
                    continue;
                }
                let json = serde_json::json!({ "cssVars": vars }).to_string();
                let (part, said) = match read(&json) {
                    Ok(read) => read,
                    Err(e) => (Tokens::default(), vec![LeftOut::said(e)]),
                };
                left_out.extend(said.into_iter().map(|l| l.within(&at)));
                // Only the theme's own names, each of its own kind.
                let mut part = part;
                let unknown: Vec<Box<str>> = part
                    .names()
                    .filter(|name| base.kind(name) != part.kind(name))
                    .map(Into::into)
                    .collect();
                for name in &unknown {
                    part.remove_named(name);
                }
                for name in unknown {
                    left_out.push(LeftOut::said(format!(
                        "{at}: `{name}`: a context sets only the theme's own tokens"
                    )));
                }
                for name in part.names() {
                    touched.copy_named(base, name);
                }
                read_contexts.push((context.as_str().into(), part));
            }
            axes.push(Axis {
                name: name.as_str().into(),
                default,
                selected: default,
                contexts: read_contexts,
            });
        }
        if axes.is_empty() {
            return (None, left_out);
        }
        // In the file's order, which the parsed value does not keep.
        let order = file_order(json);
        let at =
            |list: &[&str], name: &str| list.iter().position(|n| *n == name).unwrap_or(usize::MAX);
        let names: Vec<&str> = order.iter().map(|(m, _)| m.as_str()).collect();
        axes.sort_by_key(|axis| at(&names, &axis.name));
        for axis in &mut axes {
            let Some((_, contexts)) = order.iter().find(|(m, _)| *m == *axis.name) else {
                continue;
            };
            let contexts: Vec<&str> = contexts.iter().map(String::as_str).collect();
            let default = axis.contexts[axis.default].0.clone();
            axis.contexts.sort_by_key(|(name, _)| at(&contexts, name));
            axis.default = axis
                .contexts
                .iter()
                .position(|(n, _)| *n == default)
                .unwrap_or(0);
            axis.selected = axis.default;
        }
        (
            Some(ThemeModifiers {
                base: touched,
                axes,
            }),
            left_out,
        )
    }
}

impl Tokens {
    /// These tokens with the contexts `picks` (each a modifier's name and a
    /// context's) picked over them, as a [`super::ThemeScope`]'s `modifiers`
    /// pick them, with a sentence for each pick the theme has not: no
    /// modifiers, no such modifier, no such context of it.
    pub fn picked<N: AsRef<str>, C: AsRef<str>>(&self, picks: &[(N, C)]) -> (Tokens, Vec<String>) {
        let mut said = Vec::new();
        let Some(mut modifiers) = self.modifiers().cloned() else {
            if !picks.is_empty() {
                said.push("modifiers: the theme has no modifiers".to_owned());
            }
            return (self.clone(), said);
        };
        for (name, context) in picks {
            match modifiers.select(name.as_ref(), context.as_ref()) {
                Ok(picked) => modifiers = picked,
                Err(e) => said.push(format!("modifiers: {e}")),
            }
        }
        (modifiers.apply(self), said)
    }
}

/// The modifiers of theme file `json` and each one's contexts, in the
/// order the file writes them (empty where it is no such file).
fn file_order(json: &str) -> Vec<(String, Vec<String>)> {
    serde_json::from_str::<Top>(json)
        .map(|t| t.0)
        .unwrap_or_default()
}

/// A theme file's modifiers in order.
struct Top(Vec<(String, Vec<String>)>);
/// The modifiers of a `"modifiers"` section, each with its contexts.
struct Modifiers(Vec<(String, Vec<String>)>);
/// A modifier's contexts' names.
struct Contexts(Vec<String>);
/// The keys of an object, in order (none of anything else).
struct Keys(Vec<String>);

/// Implements `Deserialize` for a type read as an object by `$visit_map`,
/// and as empty from anything else.
macro_rules! ordered {
    ($ty:ident, $map:ident => $body:expr) => {
        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = $ty;
                    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str("a JSON value")
                    }
                    fn visit_map<A: serde::de::MapAccess<'de>>(
                        self,
                        mut $map: A,
                    ) -> Result<$ty, A::Error> {
                        $body
                    }
                    fn visit_seq<A: serde::de::SeqAccess<'de>>(
                        self,
                        mut s: A,
                    ) -> Result<$ty, A::Error> {
                        while s.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                        Ok($ty(Vec::new()))
                    }
                    fn visit_bool<E>(self, _: bool) -> Result<$ty, E> {
                        Ok($ty(Vec::new()))
                    }
                    fn visit_i64<E>(self, _: i64) -> Result<$ty, E> {
                        Ok($ty(Vec::new()))
                    }
                    fn visit_u64<E>(self, _: u64) -> Result<$ty, E> {
                        Ok($ty(Vec::new()))
                    }
                    fn visit_f64<E>(self, _: f64) -> Result<$ty, E> {
                        Ok($ty(Vec::new()))
                    }
                    fn visit_str<E>(self, _: &str) -> Result<$ty, E> {
                        Ok($ty(Vec::new()))
                    }
                    fn visit_unit<E>(self) -> Result<$ty, E> {
                        Ok($ty(Vec::new()))
                    }
                }
                d.deserialize_any(V)
            }
        }
    };
}

ordered!(Top, map => {
    let mut out = Vec::new();
    while let Some(key) = map.next_key::<String>()? {
        if key == "modifiers" {
            out = map.next_value::<Modifiers>()?.0;
        } else {
            map.next_value::<serde::de::IgnoredAny>()?;
        }
    }
    Ok(Top(out))
});

ordered!(Modifiers, map => {
    let mut out = Vec::new();
    while let Some(key) = map.next_key::<String>()? {
        out.push((key, map.next_value::<Contexts>()?.0));
    }
    Ok(Modifiers(out))
});

ordered!(Contexts, map => {
    let mut out = Vec::new();
    while let Some(key) = map.next_key::<String>()? {
        if key == "contexts" {
            out = map.next_value::<Keys>()?.0;
        } else {
            map.next_value::<serde::de::IgnoredAny>()?;
        }
    }
    Ok(Contexts(out))
});

ordered!(Keys, map => {
    let mut out = Vec::new();
    while let Some(key) = map.next_key::<String>()? {
        map.next_value::<serde::de::IgnoredAny>()?;
        out.push(key);
    }
    Ok(Keys(out))
});
