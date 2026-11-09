//! UI sets as data: what the toolchain needs to know of a set without
//! linking it. The bundler, the bytecode compiler and `.nsh/types` see a
//! set through its [`SetDescription`]: its specifier, its exports and their
//! kinds, its TypeScript declarations, the runtime values it re-exports
//! and the files it ships. Nothing in it runs a component.
//!
//! The prebuilt `nsh` describes the sets it links itself (the mobile UI).
//! An app's own crates (`nsh.json` `"rust"`) are described by a small
//! program `nsh` generates for the app (`nsh-cli` `rust_app.rs`): it links
//! only the app's crates and nsh-core, calls each crate's
//! `#[native_module]` entry and prints a [`Description`] as JSON
//! ([`Description::to_json`]), which `nsh` reads back
//! ([`Description::from_json`]). The document carries its format
//! ([`FORMAT`]) and the version of nsh-core that wrote it ([`VERSION`]):
//! `nsh` takes only its own.

use serde_json::{Map, Value, json};

use super::Registry;

/// The version of the JSON format of a [`Description`]: bumped whenever
/// what it holds or means changes.
pub const FORMAT: u32 = 3;

/// The version of nsh-core that writes descriptions: the version of nsh
/// the app's crates were built with.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What kind of value an export of a set is: how JS reaches it, as the
/// bundler (hints about JSX tags) and the bytecode compiler (the names a
/// module exports) see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// A Rust component (`registry.register::<C>()`), with the functions of
    /// a module of the same name if the set has one.
    Component,
    /// A module object (`registry.module::<M>()`).
    Module,
    /// One function of a flat module.
    Function,
    /// A Rust class (`#[class]`).
    Class,
    /// A runtime value the set exports as it is
    /// ([`Registry::reexport`], [`Registry::native_values`]).
    Value,
}

impl ExportKind {
    /// Its name in the JSON format.
    pub fn as_str(self) -> &'static str {
        match self {
            ExportKind::Component => "component",
            ExportKind::Module => "module",
            ExportKind::Function => "function",
            ExportKind::Class => "class",
            ExportKind::Value => "value",
        }
    }

    /// The kind named `name` in the JSON format.
    pub fn parse(name: &str) -> Option<ExportKind> {
        Some(match name {
            "component" => ExportKind::Component,
            "module" => ExportKind::Module,
            "function" => ExportKind::Function,
            "class" => ExportKind::Class,
            "value" => ExportKind::Value,
            _ => return None,
        })
    }
}

/// One export of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDescription {
    /// The name JS imports it by.
    pub name: String,
    /// What it is.
    pub kind: ExportKind,
}

/// A file a set ships ([`Registry::assets`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDescription {
    /// Its path among the app's assets (`@gallery-native/logo@2x.png`).
    pub path: String,
    /// The file, where the set's crate was built.
    pub source: String,
}

/// A UI set as data: everything the toolchain reads from a [`Registry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetDescription {
    /// The specifier apps import it by (`nsh/base`, `gallery/native`).
    pub specifier: String,
    /// Its exports, each name once, in the order a module declares them.
    pub exports: Vec<ExportDescription>,
    /// The TypeScript declarations of its components, modules and classes
    /// ([`super::declarations`]); the values it re-exports are declared by
    /// the runtime that makes them.
    pub declarations: String,
    /// The runtime values it exports as they are ([`Registry::reexport`]).
    pub reexported_values: Vec<String>,
    /// The runtime types it exports as they are ([`Registry::reexport`]).
    pub reexported_types: Vec<String>,
    /// The values the runtime makes for it ([`Registry::native_values`]).
    pub native_values: Vec<String>,
    /// The files it ships.
    pub assets: Vec<AssetDescription>,
    /// The exports TS sees without a doc comment ([`Registry::undocumented`]),
    /// which the toolchain warns of.
    pub undocumented: Vec<String>,
    /// The theme types it registers ([`Registry::theme`]).
    pub themes: Vec<ThemeDescription>,
}

/// A theme type as data ([`Registry::theme`]): what the toolchain reads of
/// it, as the app's crate is never linked into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDescription {
    /// The type's name (`dxn_ds::DsTheme`).
    pub type_name: String,
    /// Its tokens.
    pub tokens: Vec<ThemeTokenDescription>,
    /// Its default values, as a theme file ([`super::theme_file::write`]).
    pub defaults: String,
}

/// One token of a [`ThemeDescription`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeTokenDescription {
    /// Its name (`surface-raised`).
    pub name: String,
    /// Its kind ([`super::TokenKind::name`]).
    pub kind: String,
    /// Its doc comment.
    pub doc: String,
    /// The roles it plays ([`super::Role::name`]), if any.
    pub roles: Vec<String>,
}

/// The theme types `sets` describe, merged as [`Registry::merged`] merges
/// them: `Ok(None)` when they describe none, `Err` naming two types that
/// claim one token name or one role.
pub fn theme_types(sets: &[SetDescription]) -> Result<Option<ThemeTypes>, String> {
    let mut set = super::theme::ThemeSet::empty();
    let mut tokens = Vec::new();
    for theme in sets.iter().flat_map(|s| &s.themes) {
        let mut kinds = Vec::new();
        for t in &theme.tokens {
            let kind = super::TokenKind::from_name(&t.kind).ok_or_else(|| {
                format!(
                    "{}: `{}`: unknown kind {:?}",
                    theme.type_name, t.name, t.kind
                )
            })?;
            // A role is its name; it reads what the token playing it
            // holds.
            let roles = match kind.role_kind() {
                Some(reads) => t
                    .roles
                    .iter()
                    .map(|r| super::AnyRole::new(r.clone(), reads))
                    .collect::<Vec<_>>(),
                None if t.roles.is_empty() => Vec::new(),
                None => {
                    return Err(format!(
                        "{}: `{}`: a {} token plays no role",
                        theme.type_name, t.name, t.kind
                    ));
                }
            };
            kinds.push((t.name.as_str(), kind, roles));
        }
        let (defaults, _) = super::theme_file::parse(&theme.defaults, &super::Tokens::default())?;
        set.try_add(
            &theme.type_name,
            kinds.iter().map(|(n, k, r)| (*n, *k, r.as_slice())),
            defaults,
        )?;
        for (t, (_, kind, _)) in theme.tokens.iter().zip(&kinds) {
            if !tokens
                .iter()
                .any(|(n, ..): &(String, super::TokenKind, String)| *n == t.name)
            {
                tokens.push((t.name.clone(), *kind, t.doc.clone()));
            }
        }
    }
    if tokens.is_empty() {
        return Ok(None);
    }
    tokens.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Some(ThemeTypes {
        set: std::sync::Arc::new(set),
        tokens,
    }))
}

/// The theme types an app's sets describe, merged ([`theme_types`]): how
/// the toolchain reads `nsh.json`'s theme and types `token()`.
#[derive(Debug, Clone)]
pub struct ThemeTypes {
    set: std::sync::Arc<super::theme::ThemeSet>,
    tokens: Vec<(String, super::TokenKind, String)>,
}

impl ThemeTypes {
    /// The tokens of theme file `json` over the types' defaults (`None`:
    /// the defaults), with what of it was left out (a name no type
    /// declares, a value of another kind), as the core reads it.
    pub fn read(&self, json: Option<&str>) -> Result<(super::Tokens, Vec<String>), String> {
        super::theme::TokenProvider::Themes(self.set.clone()).read(json)
    }

    /// Each declared token, sorted by name: its name, kind and doc.
    pub fn tokens(&self) -> impl Iterator<Item = (&str, super::TokenKind, &str)> {
        self.tokens
            .iter()
            .map(|(n, k, d)| (n.as_str(), *k, d.as_str()))
    }
}

impl SetDescription {
    /// The description of `registry`, if it is a set apps import (it has a
    /// specifier).
    pub fn of(registry: &Registry) -> Option<SetDescription> {
        let specifier = registry.specifier()?;
        let specs = registry.specs();
        let modules = registry.module_specs();
        let mut exports: Vec<ExportDescription> = Vec::new();
        let mut add = |name: &str, kind: ExportKind| {
            if !exports.iter().any(|e| e.name == name) {
                exports.push(ExportDescription {
                    name: name.to_owned(),
                    kind,
                });
            }
        };
        for spec in &specs {
            add(&spec.export, ExportKind::Component);
        }
        for m in &modules {
            if specs.iter().any(|s| s.export == m.name)
                || (!m.flat && registry.function_members(m.name).is_some())
            {
                continue;
            }
            if m.flat {
                for f in &m.functions {
                    add(f.name, ExportKind::Function);
                }
            } else {
                add(m.name, ExportKind::Module);
            }
        }
        for class in registry.class_specs() {
            add(class.name, ExportKind::Class);
        }
        let (values, types) = registry.reexports();
        for value in values.iter().chain(registry.native_value_names()) {
            add(value, ExportKind::Value);
        }
        let strings = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect();
        Some(SetDescription {
            specifier: specifier.to_owned(),
            exports,
            declarations: super::declarations(registry, &[]),
            reexported_values: strings(values),
            reexported_types: strings(types),
            native_values: strings(registry.native_value_names()),
            assets: registry
                .asset_files()
                .map(|file| AssetDescription {
                    path: file.path().to_owned(),
                    source: file.source.to_owned(),
                })
                .collect(),
            undocumented: registry.undocumented(),
            themes: registry
                .theme_specs()
                .iter()
                .map(|spec| ThemeDescription {
                    type_name: spec.type_name().to_owned(),
                    tokens: spec
                        .token_specs()
                        .iter()
                        .map(|t| ThemeTokenDescription {
                            name: t.name.to_string(),
                            kind: t.kind.name().to_owned(),
                            doc: t.doc.to_owned(),
                            roles: t.roles.iter().map(|r| r.name().to_owned()).collect(),
                        })
                        .collect(),
                    defaults: super::theme_file::write(&spec.defaults()),
                })
                .collect(),
        })
    }

    /// The names of the exports of `kind`.
    pub fn exports_of(&self, kind: ExportKind) -> impl Iterator<Item = &str> {
        self.exports
            .iter()
            .filter(move |e| e.kind == kind)
            .map(|e| e.name.as_str())
    }

    fn to_value(&self) -> Value {
        json!({
            "specifier": self.specifier,
            "exports": self.exports.iter().map(|e| json!({
                "name": e.name,
                "kind": e.kind.as_str(),
            })).collect::<Vec<_>>(),
            "declarations": self.declarations,
            "reexports": {
                "values": self.reexported_values,
                "types": self.reexported_types,
            },
            "native_values": self.native_values,
            "assets": self.assets.iter().map(|a| json!({
                "path": a.path,
                "source": a.source,
            })).collect::<Vec<_>>(),
            "undocumented": self.undocumented,
            "themes": self.themes.iter().map(|t| json!({
                "type": t.type_name,
                "tokens": t.tokens.iter().map(|k| json!({
                    "name": k.name,
                    "kind": k.kind,
                    "doc": k.doc,
                    "roles": k.roles,
                })).collect::<Vec<_>>(),
                "defaults": t.defaults,
            })).collect::<Vec<_>>(),
        })
    }

    fn from_value(value: &Value, at: &str) -> Result<SetDescription, String> {
        let set = object(value, at)?;
        let specifier = string(set, "specifier", at)?;
        if specifier.is_empty() {
            return Err(format!("{at}.specifier is empty"));
        }
        let at = format!("{at} ({specifier})");
        let exports = array(set, "exports", &at)?
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let at = format!("{at}.exports[{i}]");
                let e = object(e, &at)?;
                let kind = string(e, "kind", &at)?;
                Ok(ExportDescription {
                    name: string(e, "name", &at)?,
                    kind: ExportKind::parse(&kind)
                        .ok_or_else(|| format!("{at}.kind: unknown kind {kind:?}"))?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let reexports = object(
            set.get("reexports")
                .ok_or_else(|| format!("{at}: no reexports"))?,
            &format!("{at}.reexports"),
        )?;
        let assets = array(set, "assets", &at)?
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let at = format!("{at}.assets[{i}]");
                let a = object(a, &at)?;
                Ok(AssetDescription {
                    path: string(a, "path", &at)?,
                    source: string(a, "source", &at)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(SetDescription {
            specifier,
            exports,
            declarations: string(set, "declarations", &at)?,
            reexported_values: strings(reexports, "values", &format!("{at}.reexports"))?,
            reexported_types: strings(reexports, "types", &format!("{at}.reexports"))?,
            native_values: strings(set, "native_values", &at)?,
            assets,
            undocumented: strings(set, "undocumented", &at)?,
            themes: array(set, "themes", &at)?
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let at = format!("{at}.themes[{i}]");
                    let t = object(t, &at)?;
                    let tokens = array(t, "tokens", &at)?
                        .iter()
                        .enumerate()
                        .map(|(j, k)| {
                            let at = format!("{at}.tokens[{j}]");
                            let k = object(k, &at)?;
                            Ok(ThemeTokenDescription {
                                name: string(k, "name", &at)?,
                                kind: string(k, "kind", &at)?,
                                doc: string(k, "doc", &at)?,
                                roles: k
                                    .get("roles")
                                    .and_then(Value::as_array)
                                    .map(|roles| {
                                        roles
                                            .iter()
                                            .filter_map(Value::as_str)
                                            .map(str::to_owned)
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    Ok(ThemeDescription {
                        type_name: string(t, "type", &at)?,
                        tokens,
                        defaults: string(t, "defaults", &at)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        })
    }
}

impl From<&Registry> for SetDescription {
    /// [`SetDescription::of`] a UI set.
    ///
    /// # Panics
    ///
    /// When `registry` has no specifier: it is no set apps import.
    #[track_caller]
    fn from(registry: &Registry) -> Self {
        SetDescription::of(registry).expect("a UI set: a registry with a specifier")
    }
}

impl From<Registry> for SetDescription {
    /// [`SetDescription::of`] a UI set, as [`From<&Registry>`].
    #[track_caller]
    fn from(registry: Registry) -> Self {
        SetDescription::from(&registry)
    }
}

/// The document an app's describe program prints: its crates' sets, in the
/// order `nsh.json` `"rust"` lists the crates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// The version of nsh-core that wrote it ([`VERSION`]).
    pub nsh: String,
    /// The sets.
    pub sets: Vec<SetDescription>,
}

impl Description {
    /// The description of `sets`, written by this nsh-core.
    ///
    /// # Panics
    ///
    /// When one of `sets` has no specifier (it is no UI set).
    #[track_caller]
    pub fn of(sets: &[Registry]) -> Description {
        Description {
            nsh: VERSION.to_owned(),
            sets: sets
                .iter()
                .flat_map(Registry::sets)
                .map(SetDescription::from)
                .collect(),
        }
    }

    /// The document as JSON.
    pub fn to_json(&self) -> String {
        json!({
            "format": FORMAT,
            "nsh": self.nsh,
            "sets": self.sets.iter().map(SetDescription::to_value).collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// Reads a document [`Description::to_json`] wrote. A document of
    /// another [`FORMAT`] is refused, saying which; whether its
    /// [`Description::nsh`] is the reader's own is the reader's call.
    pub fn from_json(text: &str) -> Result<Description, String> {
        let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        let doc = object(&value, "the description")?;
        let format = doc
            .get("format")
            .and_then(Value::as_u64)
            .ok_or("the description has no format")?;
        if format != u64::from(FORMAT) {
            return Err(format!(
                "the description is in format {format}, and this nsh reads format {FORMAT}"
            ));
        }
        let sets = array(doc, "sets", "the description")?
            .iter()
            .enumerate()
            .map(|(i, set)| SetDescription::from_value(set, &format!("sets[{i}]")))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Description {
            nsh: string(doc, "nsh", "the description")?,
            sets,
        })
    }
}

fn object<'v>(value: &'v Value, at: &str) -> Result<&'v Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{at} is not an object"))
}

fn string(object: &Map<String, Value>, key: &str, at: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{at}: no string {key}"))
}

fn array<'v>(object: &'v Map<String, Value>, key: &str, at: &str) -> Result<&'v [Value], String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{at}: no list {key}"))
}

fn strings(object: &Map<String, Value>, key: &str, at: &str) -> Result<Vec<String>, String> {
    array(object, key, at)?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{at}.{key}: not a list of strings"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    /// A component of the test set.
    #[component]
    fn Badge() -> impl IntoElement {
        view()
    }

    #[component]
    fn Plain() -> impl IntoElement {
        view()
    }

    fn set() -> Registry {
        let mut r = Registry::ui_set("test/description");
        r.register::<Badge>();
        r.reexport(&["safeArea"], &["Insets"]);
        r.native_values(&["Stack"]);
        r
    }

    #[test]
    fn a_set_is_described_by_its_exports_and_declarations() {
        let set = SetDescription::of(&set()).unwrap();
        assert_eq!(set.specifier, "test/description");
        let exports: Vec<(&str, ExportKind)> = set
            .exports
            .iter()
            .map(|e| (e.name.as_str(), e.kind))
            .collect();
        assert_eq!(
            exports,
            [
                ("Badge", ExportKind::Component),
                ("safeArea", ExportKind::Value),
                ("Stack", ExportKind::Value),
            ]
        );
        assert_eq!(
            set.exports_of(ExportKind::Component).collect::<Vec<_>>(),
            ["Badge"]
        );
        assert_eq!(
            set.declarations,
            crate::element::declarations(&self::set(), &[])
        );
        assert_eq!(set.reexported_values, ["safeArea"]);
        assert_eq!(set.reexported_types, ["Insets"]);
        assert_eq!(set.native_values, ["Stack"]);
        assert!(set.undocumented.is_empty());
        // What TS sees without a doc comment, which `nsh` warns of.
        let mut plain = Registry::ui_set("test/plain");
        plain.register::<Plain>();
        assert_eq!(SetDescription::of(&plain).unwrap().undocumented, ["Plain"]);
        // A registry of no set has no description.
        assert!(SetDescription::of(&Registry::new()).is_none());
    }

    #[test]
    fn a_description_survives_json() {
        let mut description = Description::of(&[set()]);
        description.sets[0].assets.push(AssetDescription {
            path: "@app/logo@2x.png".into(),
            source: "C:\\app\\assets\\logo@2x.png".into(),
        });
        let json = description.to_json();
        assert_eq!(Description::from_json(&json).unwrap(), description);
        assert_eq!(description.nsh, VERSION);
    }

    #[test]
    fn another_format_or_a_broken_document_is_refused() {
        let json = Description::of(&[set()]).to_json();
        let mut value: Value = serde_json::from_str(&json).unwrap();
        value["format"] = (FORMAT + 1).into();
        let error = Description::from_json(&value.to_string()).unwrap_err();
        assert!(error.contains(&format!("format {}", FORMAT + 1)), "{error}");
        value["format"] = FORMAT.into();
        value["sets"][0]["exports"][0]["kind"] = "widget".into();
        let error = Description::from_json(&value.to_string()).unwrap_err();
        assert!(
            error.contains("sets[0] (test/description).exports[0].kind: unknown kind \"widget\""),
            "{error}"
        );
        assert!(
            Description::from_json("{")
                .unwrap_err()
                .starts_with("not JSON")
        );
        assert!(
            Description::from_json("{}")
                .unwrap_err()
                .contains("no format")
        );
    }
}

/// Theme types as data: format 3 carries each set's types, their tokens and
/// defaults, and the toolchain merges them as the registry does.
#[cfg(test)]
mod theme_description {
    use super::*;
    use crate::element::{
        AdaptiveColor, AdaptiveShadow, AppTheme, TokenKind, TokenSpec, Tokens, role, theme_file,
    };

    #[derive(Debug, Clone, PartialEq, Default)]
    struct Ds;

    impl AppTheme for Ds {
        type Patch = ();

        fn token_specs() -> &'static [TokenSpec] {
            static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
                vec![
                    TokenSpec::new("surface-overlay", TokenKind::Color)
                        .doc("Popovers.")
                        .role(role::POPOVER),
                    TokenSpec::new("radius-card", TokenKind::Radius).doc("Cards."),
                    TokenSpec::new("font-body", TokenKind::Font),
                    TokenSpec::new("shadow-raised", TokenKind::Shadow),
                ]
            });
            &SPECS
        }

        fn write_tokens(&self, _: &str, out: &mut Tokens) {
            out.set_color("surface-overlay", AdaptiveColor::rgb(0x111111, 0x222222));
            out.set_number("radius-card", 14.0);
            out.set_dark_number("radius-card", 10.0);
            out.set_font("font-body", "Inter");
            out.set_shadow(
                "shadow-raised",
                AdaptiveShadow::new(
                    [crate::BoxShadow {
                        offset_y: crate::geometry::px(1.0),
                        ..Default::default()
                    }],
                    [],
                ),
            );
        }

        fn read_tokens(_: &Tokens, _: &str) -> Self {
            Ds
        }

        fn write_patch(_: &(), _: &str, _: &mut Tokens) {}
    }

    /// Declares `surface-overlay` too.
    #[derive(Debug, Clone, PartialEq, Default)]
    struct Clash;

    impl AppTheme for Clash {
        type Patch = ();

        fn token_specs() -> &'static [TokenSpec] {
            static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
                vec![TokenSpec::new("surface-overlay", TokenKind::Color)]
            });
            &SPECS
        }

        fn write_tokens(&self, _: &str, _: &mut Tokens) {}

        fn read_tokens(_: &Tokens, _: &str) -> Self {
            Clash
        }

        fn write_patch(_: &(), _: &str, _: &mut Tokens) {}
    }

    fn set<T: AppTheme>(specifier: &'static str) -> SetDescription {
        let mut registry = Registry::ui_set(specifier);
        registry.theme::<T>();
        SetDescription::from(&registry)
    }

    #[test]
    fn a_sets_types_round_trip_in_format_3() {
        let doc = Description {
            nsh: VERSION.into(),
            sets: vec![set::<Ds>("ds/native")],
        };
        let text = doc.to_json();
        assert!(text.contains("\"format\":3"), "{text}");
        let read = Description::from_json(&text).unwrap();
        assert_eq!(read, doc);
        let theme = &read.sets[0].themes[0];
        assert!(theme.type_name.ends_with("Ds"));
        assert_eq!(theme.tokens[0].roles, ["popover"]);
        assert_eq!(theme.tokens[1].kind, "radius");
        // Format 2 is refused, as every other.
        assert!(Description::from_json(&text.replace("\"format\":3", "\"format\":2")).is_err());
    }

    #[test]
    fn defaults_written_as_a_theme_file_read_back() {
        let tokens = Ds.to_tokens();
        let (read, ignored) =
            theme_file::parse(&theme_file::write(&tokens), &Tokens::default()).unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!(
            read.color("surface-overlay"),
            tokens.color("surface-overlay")
        );
        assert_eq!(read.number_in("radius-card", true), Some(10.0));
        assert_eq!(read.number_in("radius-card", false), Some(14.0));
        assert_eq!(read.font("font-body").map(|f| &**f), Some("Inter"));
        assert_eq!(read.shadow("shadow-raised"), tokens.shadow("shadow-raised"));
    }

    #[test]
    fn the_toolchain_merges_the_types_as_the_registry_does() {
        let types = theme_types(&[set::<Ds>("ds/native")]).unwrap().unwrap();
        let (tokens, ignored) = types
            .read(Some(
                r##"{ "cssVars": { "light": { "popover": "#333333", "brand": "#000000" } } }"##,
            ))
            .unwrap();
        assert_eq!(ignored, ["`brand`: no theme type declares it"]);
        assert_eq!(
            tokens
                .color("surface-overlay")
                .map(|c| crate::Color::from(c.light)),
            Some(crate::Color::from(crate::color::rgb(0x333333)))
        );
        assert_eq!(tokens.role_name(role::POPOVER), "surface-overlay");
        let names: Vec<_> = types.tokens().map(|(n, k, _)| (n, k)).collect();
        assert_eq!(names[0], ("font-body", TokenKind::Font));
        // No types: none; two declaring one name: an error naming both.
        assert!(
            theme_types(&[SetDescription::from(&Registry::ui_set("x/native"))])
                .unwrap()
                .is_none()
        );
        let err = theme_types(&[set::<Ds>("ds/native"), set::<Clash>("app/native")]).unwrap_err();
        assert!(err.contains("declared by both"), "{err}");
    }
}

/// A token playing several roles, as data: the description lists each, the
/// document carries them, and the toolchain's merge maps every one.
#[cfg(test)]
mod token_roles {
    use super::*;
    use crate::element::{AdaptiveColor, AppTheme, TokenKind, TokenSpec, Tokens};

    /// Roles of a UI set's own, beside the core's.
    mod role {
        use crate::element::Role;
        pub(super) use crate::element::role::*;

        pub(super) const CARD: Role = Role::new("card");
    }

    /// One surface for cards and popovers.
    #[derive(Debug, Clone, PartialEq, Default)]
    struct Flat;

    impl AppTheme for Flat {
        type Patch = ();

        fn token_specs() -> &'static [TokenSpec] {
            static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
                vec![
                    TokenSpec::new("surface", TokenKind::Color)
                        .role(role::CARD)
                        .role(role::POPOVER),
                    TokenSpec::new("ink", TokenKind::Color),
                ]
            });
            &SPECS
        }

        fn write_tokens(&self, _: &str, out: &mut Tokens) {
            out.set_color("surface", AdaptiveColor::rgb(0xf5f5f5, 0x171717));
            out.set_color("ink", AdaptiveColor::rgb(0x111111, 0xeeeeee));
        }

        fn read_tokens(_: &Tokens, _: &str) -> Self {
            Flat
        }

        fn write_patch(_: &(), _: &str, _: &mut Tokens) {}
    }

    /// Plays `popover` too.
    #[derive(Debug, Clone, PartialEq, Default)]
    struct Overlay;

    impl AppTheme for Overlay {
        type Patch = ();

        fn token_specs() -> &'static [TokenSpec] {
            static SPECS: std::sync::LazyLock<Vec<TokenSpec>> = std::sync::LazyLock::new(|| {
                vec![TokenSpec::new("overlay", TokenKind::Color).role(role::POPOVER)]
            });
            &SPECS
        }

        fn write_tokens(&self, _: &str, _: &mut Tokens) {}

        fn read_tokens(_: &Tokens, _: &str) -> Self {
            Overlay
        }

        fn write_patch(_: &(), _: &str, _: &mut Tokens) {}
    }

    fn set<T: AppTheme>(specifier: &'static str) -> SetDescription {
        let mut registry = Registry::ui_set(specifier);
        registry.theme::<T>();
        SetDescription::from(&registry)
    }

    #[test]
    fn a_tokens_roles_are_listed_and_survive_the_document() {
        let doc = Description {
            nsh: VERSION.into(),
            sets: vec![set::<Flat>("ds/native")],
        };
        let tokens = &doc.sets[0].themes[0].tokens;
        assert_eq!(tokens[0].roles, ["card", "popover"]);
        assert!(tokens[1].roles.is_empty());
        let text = doc.to_json();
        assert!(text.contains(r#""roles":["card","popover"]"#), "{text}");
        assert_eq!(Description::from_json(&text).unwrap(), doc);
    }

    #[test]
    fn the_toolchain_maps_each_role_and_refuses_one_played_twice() {
        let types = theme_types(&[set::<Flat>("ds/native")]).unwrap().unwrap();
        let (tokens, left_out) = types
            .read(Some(
                r##"{ "cssVars": { "light": { "popover": "#123456" } } }"##,
            ))
            .unwrap();
        assert!(left_out.is_empty(), "{left_out:?}");
        assert_eq!(tokens.role_name(role::CARD), "surface");
        assert_eq!(tokens.role_name(role::POPOVER), "surface");
        assert_eq!(
            tokens.color("surface").map(|c| crate::Color::from(c.light)),
            Some(crate::Color::from(crate::color::rgb(0x123456)))
        );
        let err =
            theme_types(&[set::<Flat>("ds/native"), set::<Overlay>("app/native")]).unwrap_err();
        assert!(
            err.contains("the role `popover` is mapped by both"),
            "{err}"
        );
        // A role is its name: one only the app's crates declare is mapped
        // as the core's are.
        let mut own = set::<Flat>("ds/native");
        own.themes[0].tokens[0].roles.push("banner".into());
        let types = theme_types(&[own]).unwrap().unwrap();
        let (tokens, left_out) = types
            .read(Some(
                r##"{ "cssVars": { "light": { "banner": "#654321" } } }"##,
            ))
            .unwrap();
        assert!(left_out.is_empty(), "{left_out:?}");
        assert_eq!(
            tokens.color("surface").map(|c| crate::Color::from(c.light)),
            Some(crate::Color::from(crate::color::rgb(0x654321)))
        );
    }
}

/// Roles as data: the toolchain knows a role by its name and by the kind
/// of the token that plays it, and no role reads box shadows.
#[cfg(test)]
mod named_roles {
    use super::*;

    fn theme(tokens: &[(&str, &str, &[&str])]) -> SetDescription {
        let mut set = SetDescription::from(&Registry::ui_set("ds/native"));
        set.themes.push(ThemeDescription {
            type_name: "ds::Theme".into(),
            tokens: tokens
                .iter()
                .map(|(name, kind, roles)| ThemeTokenDescription {
                    name: (*name).into(),
                    kind: (*kind).into(),
                    doc: String::new(),
                    roles: roles.iter().map(|r| (*r).into()).collect(),
                })
                .collect(),
            defaults: r##"{ "cssVars": { "theme": { "notice": "#111111", "radius-chip": "4px", "font-title": "Inter", "shadow-raised": "0 1px 2px #0000001a" } } }"##.into(),
        });
        set
    }

    #[test]
    fn a_role_reads_what_the_token_playing_it_holds() {
        let types = theme_types(&[theme(&[
            ("notice", "color", &["banner"]),
            ("radius-chip", "radius", &["chip-radius"]),
            ("font-title", "font", &["title-font"]),
        ])])
        .unwrap()
        .unwrap();
        let (tokens, left_out) = types.read(None).unwrap();
        assert!(left_out.is_empty(), "{left_out:?}");
        // Each role reads its token, as its kind.
        let banner: crate::element::Role = crate::element::Role::new("banner");
        let chip: crate::element::NumberRole = crate::element::Role::new("chip-radius");
        let title: crate::element::FontRole = crate::element::Role::new("title-font");
        assert_eq!(tokens.role(banner), tokens.color("notice"));
        assert_eq!(tokens.role(chip), Some(4.0));
        assert_eq!(tokens.role(title).as_deref(), Some("Inter"));
    }

    #[test]
    fn a_shadow_token_plays_no_role() {
        let err =
            theme_types(&[theme(&[("shadow-raised", "shadow", &["elevation"])])]).unwrap_err();
        assert!(
            err.contains("`shadow-raised`: a shadow token plays no role"),
            "{err}"
        );
        // Without one it is described as it was.
        assert!(
            theme_types(&[theme(&[("shadow-raised", "shadow", &[])])])
                .unwrap()
                .is_some()
        );
    }
}
