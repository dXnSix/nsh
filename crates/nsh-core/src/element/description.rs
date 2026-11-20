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
//! ([`Description::from_json`]). It lists every set the entries serve
//! ([`Registry::served_sets`]: their submodules and the libraries' sets
//! they serve), each with the entry that serves it
//! ([`SetDescription::entry`]). The document carries its format
//! ([`FORMAT`]) and the version of nsh-core that wrote it ([`VERSION`]):
//! `nsh` takes only its own.

use serde_json::{Map, Value, json};

use super::{Registry, RoleDeclaration};

/// The version of the JSON format of a [`Description`]: bumped whenever
/// what it holds or means changes.
pub const FORMAT: u32 = 8;

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
    /// The index, in the list of registries [`Description::of`] was given,
    /// of the one that serves it (the first, when two do): an app's crate,
    /// whose entry's own set is the first set of that index, then its
    /// submodules and the sets it serves ([`Registry::serve`]). 0 for a set
    /// described on its own ([`SetDescription::of`]).
    pub entry: usize,
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
    /// The theme documents it registers, as written: its own
    /// ([`Registry::theme_document`]), then its fallbacks
    /// ([`Registry::fallback_theme_document`]), marked.
    pub theme_documents: Vec<ThemeDocumentDescription>,
    /// The roles its components draw with ([`Registry::roles`]): their
    /// declarations, with value type, doc and `required`.
    pub roles: Vec<RoleDeclaration>,
    /// The class extensions it registers ([`Registry::class_extension`]):
    /// the methods `.nsh/types` adds to its classes' other declarations.
    pub class_extensions: Vec<ClassExtensionDescription>,
    /// The Rust type of each class it exports, by the class's JS name:
    /// what tells an extension's class from another of the same name.
    pub class_types: Vec<ClassTypeDescription>,
    /// The Rust type of each component it registers, by its export: what
    /// tells the toolchain two sets register one component
    /// ([`component_clash`]).
    pub component_types: Vec<ComponentTypeDescription>,
}

/// A component a set registers, by its export and its Rust type.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComponentTypeDescription {
    /// The name JS imports it by.
    pub name: String,
    /// Its Rust type, the name the core makes it by
    /// ([`super::ComponentSpec::name`]: `app::chart::Chart`).
    pub rust_type: String,
}

impl ComponentTypeDescription {
    /// Component `name` (its export) of Rust type `rust_type`.
    pub fn new(name: impl Into<String>, rust_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rust_type: rust_type.into(),
        }
    }
}

/// A class a set exports, by its JS name and its Rust type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassTypeDescription {
    /// Its JS name.
    pub name: String,
    /// Its Rust type (`app::AppBuilder`).
    pub rust_type: String,
}

/// A class extension a set registers ([`Registry::class_extension`]), as
/// data: what `.nsh/types` declares of it where another set declares its
/// class (`export interface <class><generics> { <members> }`, which
/// TypeScript merges with the class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassExtensionDescription {
    /// The JS name of the class it adds methods to.
    pub class: String,
    /// That class's Rust type ([`ClassTypeDescription::rust_type`]).
    pub class_type: String,
    /// The class's type parameters as its declaration writes them
    /// (`<T>`), or nothing.
    pub generics: String,
    /// The methods it adds, as lines of a class body with their doc
    /// comments.
    pub members: String,
    /// Its Rust type, telling two extensions apart.
    pub extension: String,
}

impl ClassExtensionDescription {
    /// The class extensions `registry` registers, by their class's name,
    /// then their type's ([`Registry::class_extensions`]).
    pub fn of(registry: &Registry) -> Vec<ClassExtensionDescription> {
        registry
            .class_extensions()
            .into_iter()
            .map(|def| ClassExtensionDescription {
                class: def.class.name.to_owned(),
                class_type: (def.class.type_name)().to_owned(),
                generics: super::component::class_generics(def.class),
                members: super::component::extension_lines(def),
                extension: (def.type_name)().to_owned(),
            })
            .collect()
    }
}

/// A theme document a set registers, as data: its text as written, and
/// whether it is a fallback ([`Registry::fallback_theme_document`]), active
/// only when no set registers a document of its own.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeDocumentDescription {
    /// The document as written.
    pub json: String,
    /// Whether it is a fallback document.
    pub fallback: bool,
}

impl ThemeDocumentDescription {
    /// The document `json`, a fallback or not.
    pub fn new(json: impl Into<String>, fallback: bool) -> Self {
        Self {
            json: json.into(),
            fallback,
        }
    }
}

/// The first component two of `sets` register, by its Rust type
/// ([`SetDescription::component_types`]): the message an engine serving
/// them would panic with as it starts ([`Registry::merged`]), which the
/// toolchain reports before building anything. `None`: no component is
/// registered twice.
pub fn component_clash(sets: &[SetDescription]) -> Option<String> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for (i, set) in sets.iter().enumerate() {
        // A set listed twice is one set.
        if sets[..i].iter().any(|s| s.specifier == set.specifier) {
            continue;
        }
        for component in &set.component_types {
            let rust_type = component.rust_type.as_str();
            if let Some((_, first)) = seen.iter().find(|(t, _)| *t == rust_type) {
                return Some(super::component::in_two_sets(
                    rust_type,
                    true,
                    Some(first),
                    Some(&set.specifier),
                ));
            }
            seen.push((rust_type, &set.specifier));
        }
    }
    None
}

/// The roles `sets` list and the core's own ([`super::role::all`]), each
/// once, by name, as declared: every role TS's `role()` may name. `Err`
/// names a role two sets list reading different value types.
pub fn roles(sets: &[SetDescription]) -> Result<Vec<RoleDeclaration>, String> {
    let core = super::role::all();
    let mut out: Vec<RoleDeclaration> = Vec::new();
    for role in core
        .into_iter()
        .chain(sets.iter().flat_map(|s| s.roles.iter().cloned()))
    {
        match out.iter().find(|r| r.name == role.name) {
            Some(listed) if listed.value_type != role.value_type => {
                return Err(format!(
                    "the role `{}` is listed reading {} and {}",
                    role.name, listed.value_type, role.value_type
                ));
            }
            Some(_) => {}
            None => out.push(role),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The document `sets` make active when the app has none of its own, as
/// the core takes it ([`Registry::theme_document_of_sets`]): the first a set
/// registers, else the first fallback, read, with its diagnostics and a
/// `document-unused` warning of each other set's own. Documents are never
/// merged. How the toolchain reads the theme the core will draw with.
pub fn theme_document(
    sets: &[SetDescription],
) -> (Option<nsh_theme::ThemeDocument>, Vec<nsh_theme::Diagnostic>) {
    let of = |fallback: bool| {
        sets.iter().flat_map(move |set| {
            set.theme_documents
                .iter()
                .filter(move |d| d.fallback == fallback)
                .map(move |d| (set.specifier.as_str(), d.json.as_str()))
        })
    };
    let mut documents = of(false);
    let mut said = Vec::new();
    let mut active = None;
    for (owner, text) in documents.by_ref() {
        let read = nsh_theme::read(text);
        let refused = super::theme::refused(&read.diagnostics);
        said.extend(read.diagnostics);
        if !refused {
            active = Some((owner, read.document));
            break;
        }
    }
    if active.is_none() {
        for (owner, text) in of(true) {
            let read = nsh_theme::read(text);
            let refused = super::theme::refused(&read.diagnostics);
            said.extend(read.diagnostics);
            if !refused {
                active = Some((owner, read.document));
                break;
            }
        }
    }
    for (owner, _) in documents {
        said.push(nsh_theme::Diagnostic::warning(
            nsh_theme::code::DOCUMENT_UNUSED,
            "",
            format!(
                "The theme document of {owner} is unused: {} registers the active one.",
                active.as_ref().map_or("another set", |(a, _)| *a)
            ),
        ));
    }
    (active.map(|(_, document)| document), said)
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
            entry: 0,
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
            theme_documents: registry
                .theme_documents()
                .into_iter()
                .map(|(_, json)| ThemeDocumentDescription::new(json, false))
                .chain(
                    registry
                        .fallback_theme_documents()
                        .into_iter()
                        .map(|(_, json)| ThemeDocumentDescription::new(json, true)),
                )
                .collect(),
            roles: registry.role_specs().to_vec(),
            class_extensions: ClassExtensionDescription::of(registry),
            class_types: registry
                .class_specs()
                .iter()
                .map(|c| ClassTypeDescription {
                    name: c.name.to_owned(),
                    rust_type: (c.def.type_name)().to_owned(),
                })
                .collect(),
            component_types: specs
                .iter()
                .map(|c| ComponentTypeDescription::new(c.export.clone(), c.name))
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
            "entry": self.entry,
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
            "theme_documents": self.theme_documents.iter().map(|d| json!({
                "json": d.json,
                "fallback": d.fallback,
            })).collect::<Vec<_>>(),
            "roles": self.roles.iter().map(|r| json!({
                "name": r.name,
                "kind": r.value_type.name(),
                "doc": r.doc,
                "required": r.required,
            })).collect::<Vec<_>>(),
            "classExtensions": self.class_extensions.iter().map(|e| json!({
                "class": e.class,
                "classType": e.class_type,
                "generics": e.generics,
                "members": e.members,
                "extension": e.extension,
            })).collect::<Vec<_>>(),
            "classTypes": self.class_types.iter().map(|c| json!({
                "name": c.name,
                "type": c.rust_type,
            })).collect::<Vec<_>>(),
            "componentTypes": self.component_types.iter().map(|c| json!({
                "name": c.name,
                "type": c.rust_type,
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
        let entry = set
            .get("entry")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| format!("{at}: no index entry"))?;
        Ok(SetDescription {
            specifier,
            entry,
            exports,
            declarations: string(set, "declarations", &at)?,
            reexported_values: strings(reexports, "values", &format!("{at}.reexports"))?,
            reexported_types: strings(reexports, "types", &format!("{at}.reexports"))?,
            native_values: strings(set, "native_values", &at)?,
            assets,
            undocumented: strings(set, "undocumented", &at)?,
            theme_documents: array(set, "theme_documents", &at)?
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let at = format!("{at}.theme_documents[{i}]");
                    let d = object(d, &at)?;
                    Ok(ThemeDocumentDescription::new(
                        string(d, "json", &at)?,
                        boolean(d, "fallback", &at)?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?,
            roles: array(set, "roles", &at)?
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let at = format!("{at}.roles[{i}]");
                    let r = object(r, &at)?;
                    let kind = string(r, "kind", &at)?;
                    let value_type = super::ValueType::from_name(&kind)
                        .ok_or_else(|| format!("{at}.kind: unknown kind {kind:?}"))?;
                    let doc = match r.get("doc") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(doc)) => Some(doc.clone()),
                        Some(_) => return Err(format!("{at}.doc: not a string")),
                    };
                    let declaration = RoleDeclaration::new(string(r, "name", &at)?, value_type)
                        .with_required(boolean(r, "required", &at)?);
                    Ok(match doc {
                        Some(doc) => declaration.with_doc(doc),
                        None => declaration,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            class_extensions: array(set, "classExtensions", &at)?
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let at = format!("{at}.classExtensions[{i}]");
                    let e = object(e, &at)?;
                    Ok(ClassExtensionDescription {
                        class: string(e, "class", &at)?,
                        class_type: string(e, "classType", &at)?,
                        generics: string(e, "generics", &at)?,
                        members: string(e, "members", &at)?,
                        extension: string(e, "extension", &at)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            class_types: array(set, "classTypes", &at)?
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let at = format!("{at}.classTypes[{i}]");
                    let c = object(c, &at)?;
                    Ok(ClassTypeDescription {
                        name: string(c, "name", &at)?,
                        rust_type: string(c, "type", &at)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            component_types: array(set, "componentTypes", &at)?
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let at = format!("{at}.componentTypes[{i}]");
                    let c = object(c, &at)?;
                    Ok(ComponentTypeDescription::new(
                        string(c, "name", &at)?,
                        string(c, "type", &at)?,
                    ))
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
/// order `nsh.json` `"rust"` lists the crates, each crate's own set, then
/// its submodules and the sets it serves ([`SetDescription::entry`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    /// The version of nsh-core that wrote it ([`VERSION`]).
    pub nsh: String,
    /// The sets.
    pub sets: Vec<SetDescription>,
}

impl Description {
    /// The description of `sets` and every set they serve, each once
    /// ([`Registry::served_sets`]), with the index in `sets` of the one
    /// serving it ([`SetDescription::entry`]); written by this nsh-core.
    ///
    /// # Panics
    ///
    /// When one of those sets has no specifier (it is no UI set), or two
    /// different ones have one specifier.
    #[track_caller]
    pub fn of(sets: &[Registry]) -> Description {
        Description {
            nsh: VERSION.to_owned(),
            sets: Registry::served_sets_by_entry(sets)
                .into_iter()
                .map(|(entry, set)| SetDescription {
                    entry,
                    ..SetDescription::from(set)
                })
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

fn boolean(object: &Map<String, Value>, key: &str, at: &str) -> Result<bool, String> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{at}: no boolean {key}"))
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

/// Theme documents as data: the description carries each set's documents
/// as written, and the toolchain takes the first as the core does.
#[cfg(test)]
mod theme_documents {
    use super::*;

    const DS: &str = r##"{
        "tokens": {
            "surface-overlay": { "kind": "color", "value": "#111111", "doc": "Popovers." },
            "radius-card": { "kind": "number", "value": 14, "doc": "Cards." },
            "font-body": { "kind": "font", "value": "Inter" }
        },
        "roles": { "popover": "surface-overlay" }
    }"##;

    fn set(specifier: &'static str, json: &'static str) -> SetDescription {
        let mut registry = Registry::ui_set(specifier);
        registry.theme_document(json);
        SetDescription::from(&registry)
    }

    #[test]
    fn a_sets_documents_round_trip() {
        let doc = Description {
            nsh: VERSION.into(),
            sets: vec![set("ds/native", DS)],
        };
        let text = doc.to_json();
        let format = format!("\"format\":{FORMAT}");
        assert!(text.contains(&format), "{text}");
        let read = Description::from_json(&text).unwrap();
        assert_eq!(read, doc);
        assert_eq!(
            read.sets[0].theme_documents,
            [ThemeDocumentDescription::new(DS, false)]
        );
        // The format before is refused, as every other.
        let before = format!("\"format\":{}", FORMAT - 1);
        assert!(Description::from_json(&text.replace(&format, &before)).is_err());
    }

    #[test]
    fn the_toolchain_takes_the_first_document_as_the_core_does() {
        let other = r##"{ "tokens": { "panel": { "kind": "color", "value": "#222222" } } }"##;
        let (document, said) = theme_document(&[set("ds/native", DS), set("app/native", other)]);
        let document = document.unwrap();
        assert!(document.token("surface-overlay").is_some());
        assert!(document.token("panel").is_none());
        let codes: Vec<_> = said.iter().map(|d| d.code).collect();
        assert_eq!(codes, [nsh_theme::code::DOCUMENT_UNUSED]);
        let (none, _) = theme_document(&[SetDescription::from(&Registry::ui_set("x/native"))]);
        assert!(none.is_none());
    }
}

/// The roles a UI set lists ([`Registry::roles`]): in its description, read
/// back from JSON, and merged with the core's for the toolchain.
#[cfg(test)]
mod set_roles {
    use super::*;
    use crate::element::{NumberRole, Role, role};

    #[allow(unreachable_pub, reason = "as a UI set's crate writes its roles")]
    mod kit {
        use crate::element::{NumberRole, Role};

        crate::roles! {
            /// A banner's surface,
            /// whatever the app calls it.
            ///
            /// Name `banner`.
            pub BANNER: Role = "banner",
            /// How round a chip is.
            pub CHIP_RADIUS: NumberRole = "chip-radius",
        }
    }

    fn set() -> Registry {
        let mut registry = Registry::ui_set("test/kit");
        registry.roles(kit::all());
        registry
    }

    #[test]
    fn a_sets_roles_are_in_its_description() {
        let described = SetDescription::of(&set()).unwrap();
        assert_eq!(
            described.roles,
            [
                // The doc's lines joined, without the line naming it.
                RoleDeclaration::new("banner", crate::element::ValueType::Color)
                    .with_doc("A banner's surface, whatever the app calls it."),
                RoleDeclaration::new("chip-radius", crate::element::ValueType::Number)
                    .with_doc("How round a chip is."),
            ]
        );
        // The constants are what they were: a role is its name.
        assert_eq!(kit::BANNER, Role::new("banner"));
        assert_eq!(kit::CHIP_RADIUS, NumberRole::new("chip-radius"));
    }

    #[test]
    fn the_roles_survive_the_json() {
        let registry = set();
        let json = Description::of(std::slice::from_ref(&registry)).to_json();
        let read = Description::from_json(&json).unwrap();
        assert_eq!(
            read.sets[0].roles,
            SetDescription::of(&registry).unwrap().roles
        );
        // A kind the reader does not know is refused.
        let wrong = json.replace("\"kind\":\"number\"", "\"kind\":\"nope\"");
        let error = Description::from_json(&wrong).unwrap_err();
        assert!(error.contains("unknown kind"), "{error}");
    }

    #[test]
    fn the_toolchain_has_the_cores_roles_and_every_sets_once() {
        let mut other = Registry::ui_set("test/other");
        // A role two sets list is one role.
        other.roles([
            kit::BANNER.declaration(" Another set's words for it.\n", false),
            role::POPOVER.declaration("", false),
        ]);
        let sets = [
            SetDescription::of(&set()).unwrap(),
            SetDescription::of(&other).unwrap(),
        ];
        let all = roles(&sets).unwrap();
        let names: Vec<&str> = all.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "accent",
                "accent-foreground",
                "banner",
                "border",
                "chip-radius",
                "destructive",
                "font-sans",
                "muted-foreground",
                "popover",
                "popover-foreground",
                "primary",
            ]
        );
        let of = |name: &str| all.iter().find(|r| r.name == name).unwrap();
        assert_eq!(of("font-sans").value_type, crate::element::ValueType::Font);
        // The first to list a role documents it: the core, then the sets
        // in order.
        assert_eq!(
            of("popover").doc.as_deref(),
            Some("Popovers, menus, tooltips, alerts.")
        );
        assert_eq!(
            of("banner").doc.as_deref(),
            Some("A banner's surface, whatever the app calls it.")
        );
    }

    #[test]
    fn a_role_listed_with_two_kinds_is_refused() {
        let mut other = Registry::ui_set("test/other");
        other.roles([NumberRole::new("banner").declaration("", false)]);
        let sets = [
            SetDescription::of(&set()).unwrap(),
            SetDescription::of(&other).unwrap(),
        ];
        let error = roles(&sets).unwrap_err();
        assert_eq!(
            error,
            "the role `banner` is listed reading color and number"
        );
        // Merged for a host, the clash is logged (`role-conflict`) and the
        // first listing kept: a registration never panics.
        let sets = [set(), other];
        let merged = Registry::merged(&sets);
        let banner = merged
            .role_specs()
            .iter()
            .find(|r| r.name == "banner")
            .unwrap();
        assert_eq!(banner.value_type, crate::element::ValueType::Color);
    }
}

/// The sets an app's crates serve (`Registry::serve`) in its description:
/// each once, with the crate that serves it.
#[cfg(test)]
mod served {
    use super::*;
    use crate::prelude::*;

    /// A library's component.
    #[component]
    fn Chip() -> impl IntoElement {
        view()
    }

    /// An app's component.
    #[component]
    fn Card() -> impl IntoElement {
        view()
    }

    fn library() -> Registry {
        let mut set = Registry::ui_set("acme/ui");
        set.register::<Chip>();
        set
    }

    fn app(specifier: &'static str) -> Registry {
        let mut app = Registry::ui_set(specifier);
        app.serve(library());
        app
    }

    #[test]
    fn each_set_is_described_once_with_the_entry_serving_it() {
        let mut first = app("app/a");
        first.register::<Card>();
        let description = Description::of(&[first, app("app/b")]);
        let sets: Vec<_> = description
            .sets
            .iter()
            .map(|s| (s.specifier.as_str(), s.entry))
            .collect();
        assert_eq!(sets, [("app/a", 0), ("acme/ui", 0), ("app/b", 1)]);
        // Each component with its Rust type.
        assert_eq!(
            description.sets[1].component_types,
            [ComponentTypeDescription::new(
                "Chip",
                "nsh_core::element::description::served::Chip"
            )]
        );
        assert_eq!(description.sets[0].component_types[0].name, "Card");
        // In the JSON, as written.
        let json = description.to_json();
        assert!(json.contains("\"entry\":1"), "{json}");
        assert_eq!(Description::from_json(&json).unwrap(), description);
        // A set described on its own is its own entry.
        assert_eq!(SetDescription::from(&library()).entry, 0);
    }

    #[test]
    fn a_set_without_its_entry_or_component_types_is_refused() {
        let json = Description::of(&[app("app/a")]).to_json();
        let mut value: Value = serde_json::from_str(&json).unwrap();
        value["sets"][1]
            .as_object_mut()
            .unwrap()
            .remove("entry")
            .unwrap();
        let error = Description::from_json(&value.to_string()).unwrap_err();
        assert_eq!(error, "sets[1] (acme/ui): no index entry");
        let mut value: Value = serde_json::from_str(&json).unwrap();
        value["sets"][0]
            .as_object_mut()
            .unwrap()
            .remove("componentTypes")
            .unwrap();
        let error = Description::from_json(&value.to_string()).unwrap_err();
        assert_eq!(error, "sets[0] (app/a): no list componentTypes");
        // A description of format 7, before served sets, is refused.
        let old = json.replace(&format!("\"format\":{FORMAT}"), "\"format\":7");
        let error = Description::from_json(&old).unwrap_err();
        assert_eq!(
            error,
            "the description is in format 7, and this nsh reads format 8"
        );
    }
}

/// Two sets registering one component, which the toolchain refuses from
/// the description before an engine starts.
#[cfg(test)]
mod duplicate_components {
    use super::*;
    use crate::prelude::*;

    /// Registered by two sets.
    #[component]
    fn Chip() -> impl IntoElement {
        view()
    }

    fn set(specifier: &'static str) -> SetDescription {
        let mut set = Registry::ui_set(specifier);
        set.register::<Chip>();
        SetDescription::from(&set)
    }

    #[test]
    fn a_component_two_sets_register_is_named_with_both() {
        assert_eq!(
            component_clash(&[set("app/native"), set("acme/ui")]).unwrap(),
            "component `nsh_core::element::description::duplicate_components::Chip` is registered by both `app/native` and `acme/ui`: register it in one set; a library's set is served with `Registry::serve`"
        );
        // One set, or one listed twice, registers it once.
        assert_eq!(component_clash(&[set("app/native")]), None);
        assert_eq!(
            component_clash(&[set("app/native"), set("app/native")]),
            None
        );
        assert_eq!(
            component_clash(&[set("app/native"), Registry::ui_set("acme/ui").into()]),
            None
        );
    }
}
