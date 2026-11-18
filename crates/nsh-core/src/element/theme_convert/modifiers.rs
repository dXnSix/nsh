//! The old theme modifiers' application, moved from `theme_modifiers.rs`: a
//! theme file's `"modifiers"` section names each modifier with its
//! `default` and its `contexts`, each context a partial `cssVars` of only
//! what it changes (light and dark stay native inside every context). The
//! tokens are the file's values with each modifier's selected context over
//! them, in file order (a later modifier wins on a token two change); at
//! first each modifier's default. A context adds no names.

use super::{order::Ordered, reader::LeftOut, store::Store};

/// A theme's modifiers, with the context each has now.
#[derive(Debug, Clone)]
pub(super) struct Modifiers {
    /// The file's own values of every name a context sets.
    base: Store,
    pub(super) axes: Vec<Axis>,
}

/// One modifier.
#[derive(Debug, Clone)]
pub(super) struct Axis {
    pub(super) name: Box<str>,
    /// Its default context's index.
    pub(super) default: usize,
    selected: usize,
    /// Each context's name and the values it sets.
    pub(super) contexts: Vec<(Box<str>, Store)>,
}

impl Modifiers {
    /// These modifiers with `name`'s context `context`. `Err` says there is
    /// no such modifier, or no such context of it.
    #[cfg(test)]
    pub(super) fn select(&self, name: &str, context: &str) -> Result<Modifiers, String> {
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
    #[cfg(test)]
    fn tokens(&self) -> Store {
        let mut out = Store::default();
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

    /// These modifiers and `other`'s, `other`'s after. `Err` names a
    /// modifier both have.
    pub(super) fn merged(mut self, other: Modifiers) -> Result<Modifiers, String> {
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
    /// tokens their contexts set.
    pub(super) fn rebased(mut self, base: &Store) -> Modifiers {
        for name in self.base.names().map(str::to_owned).collect::<Vec<_>>() {
            if base.kind(&name).is_some() {
                self.base.copy_named(base, &name);
            }
        }
        self
    }

    /// `tokens` with these modifiers' contexts picked now over them.
    #[cfg(test)]
    pub(super) fn apply(&self, tokens: &Store) -> Store {
        self.tokens().over(tokens)
    }

    /// The modifiers of theme file `value`, whose own values are `base`,
    /// with what of them was left out. `read` reads a context (a theme file
    /// of its `cssVars`) against `base`. A context adds no names: one
    /// `base` lacks is left out.
    pub(super) fn read(
        json: &str,
        value: &crate::JsonValue,
        base: &Store,
        read: impl Fn(&str) -> Result<(Store, Vec<LeftOut>), String>,
    ) -> (Option<Modifiers>, Vec<LeftOut>) {
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
        let mut touched = Store::default();
        for (name, modifier) in modifiers {
            let at = format!("modifiers.{name}");
            left_out.extend(unknown_members(&at, modifier, &["default", "contexts"]));
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
                let part = read_context(&at, vars, base, &read, &mut left_out);
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
            Some(Modifiers {
                base: touched,
                axes,
            }),
            left_out,
        )
    }
}

/// Context `vars` (a partial `cssVars`, at `at`) read by `read` against
/// `base`: only the theme's own names, each of its own kind; what was left
/// out joins `left_out`.
fn read_context(
    at: &str,
    vars: &crate::JsonValue,
    base: &Store,
    read: &impl Fn(&str) -> Result<(Store, Vec<LeftOut>), String>,
    left_out: &mut Vec<LeftOut>,
) -> Store {
    if !vars.is_object() {
        left_out.push(LeftOut::said(format!(
            "{at}: expected `{{ theme, light, dark }}`, as `cssVars` is"
        )));
        return Store::default();
    }
    left_out.extend(unknown_members(at, vars, &["theme", "light", "dark"]));
    let json = serde_json::json!({ "cssVars": vars }).to_string();
    let (mut part, said) = match read(&json) {
        Ok(read) => read,
        Err(e) => (Store::default(), vec![LeftOut::said(e)]),
    };
    left_out.extend(said.into_iter().map(|l| l.within(at)));
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
    part
}

/// A sentence for each member of object `value` (at `at`) that is none of
/// `known`.
fn unknown_members(at: &str, value: &crate::JsonValue, known: &[&str]) -> Vec<LeftOut> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    object
        .keys()
        .filter(|k| !known.contains(&k.as_str()))
        .map(|k| {
            let expected: Vec<String> = known.iter().map(|k| format!("`{k}`")).collect();
            LeftOut::said(format!(
                "{at}.{k}: no such member (expected {})",
                expected.join(", ")
            ))
        })
        .collect()
}

/// The modifiers of theme file `json` and each one's contexts, in the
/// order the file writes them (empty where it is no such file).
fn file_order(json: &str) -> Vec<(String, Vec<String>)> {
    let ordered = Ordered::parse(json);
    let Some(Ordered::Object(modifiers)) = ordered.get("modifiers") else {
        return Vec::new();
    };
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (name, modifier) in modifiers {
        if out.iter().any(|(n, _)| n == name) {
            continue;
        }
        let contexts = modifier
            .get("contexts")
            .map(|c| c.keys().into_iter().map(str::to_owned).collect())
            .unwrap_or_default();
        out.push((name.clone(), contexts));
    }
    out
}
