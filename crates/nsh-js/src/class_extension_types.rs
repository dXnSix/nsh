//! Class extensions in `.nsh/types` (`type_definitions`): the methods a
//! set's extensions add are declared in every other file that exports
//! their class, as an interface TypeScript merges with the class, with the
//! types they name imported; a set's description carries its extensions
//! through JSON.

use nsh_core::{
    element::description::{ClassExtensionDescription, Description, FORMAT, SetDescription},
    prelude::*,
};

use crate::type_definitions;

/// The class's crate.
mod app {
    use nsh_core::prelude::*;

    /// Builds the app.
    #[class]
    pub(crate) struct Builder;

    #[class]
    impl Builder {
        /// A new builder.
        pub(crate) fn new() -> Self {
            Builder
        }
    }

    /// A token naming a service of type `T`.
    #[class(generic = "T extends object = object")]
    pub(crate) struct Token;

    #[class]
    impl Token {
        /// A token.
        pub(crate) fn new() -> Self {
            Token
        }
    }
}

/// The extending crate.
mod i18n {
    use nsh_core::prelude::*;

    use super::app::{Builder, Token};

    /// The i18n service's options.
    #[class]
    pub(crate) struct I18nOptions;

    #[class]
    impl I18nOptions {
        /// The locale to start with.
        #[getter]
        pub(crate) fn locale(&self) -> String {
            String::new()
        }
    }

    /// Where translations come from.
    #[derive(Debug, Clone, PartialEq, JsObject)]
    pub(crate) struct Source {
        /// Their address.
        pub(crate) url: String,
    }

    /// Methods the i18n crate adds to the app builder.
    pub(crate) struct I18nBuilder;

    #[class_extension(Builder)]
    impl I18nBuilder {
        /// Registers the i18n service, configured by `configure`.
        pub(crate) fn add_i18n(
            this: Instance<Builder>,
            #[args(options)] configure: Option<Callback<Instance<I18nOptions>>>,
        ) -> Instance<Builder> {
            let _ = configure;
            this
        }

        /// Loads translations from `source`.
        pub(crate) async fn load(this: Instance<Builder>, source: Source) {
            let _ = (this, source);
        }
    }

    /// Methods handing tokens to the builder.
    pub(crate) struct TokenUse;

    #[class_extension(Builder)]
    impl TokenUse {
        /// Provides the service `token` names; the token.
        pub(crate) fn provide(this: Instance<Builder>, token: Instance<Token>) -> Instance<Token> {
            let _ = this;
            token
        }
    }

    /// Methods of tokens.
    pub(crate) struct TokenDefaults;

    #[class_extension(Token)]
    impl TokenDefaults {
        /// The token's default value, `fallback` when it has none.
        #[ts("(fallback: T): T")]
        pub(crate) fn or_default(this: Instance<Token>, fallback: JsHandle) -> JsHandle {
            let _ = this;
            fallback
        }
    }
}

/// Another crate with a class of the same JS name, and its own `Options`.
mod other {
    use nsh_core::prelude::*;

    /// Another builder.
    #[class]
    pub(crate) struct Builder;

    #[class]
    impl Builder {
        /// A new one.
        pub(crate) fn new() -> Self {
            Builder
        }
    }

    /// The other crate's options.
    #[derive(Debug, Clone, PartialEq, JsObject)]
    pub(crate) struct Options {
        /// A flag.
        pub(crate) flag: bool,
    }

    /// Reads options.
    #[module]
    pub(crate) struct Settings;

    #[module]
    impl Settings {
        /// The options.
        pub(crate) fn read() -> Options {
            Options { flag: false }
        }
    }
}

/// Two service crates, each naming a type `Options` of its own.
mod weather {
    use nsh_core::prelude::*;

    use super::app::Builder;

    /// The weather service's options.
    #[derive(Debug, Clone, PartialEq, JsObject)]
    pub(crate) struct Options {
        /// Its city.
        pub(crate) city: String,
    }

    /// Methods the weather crate adds.
    pub(crate) struct WeatherBuilder;

    #[class_extension(Builder)]
    impl WeatherBuilder {
        /// Registers the weather service.
        pub(crate) fn add_weather(this: Instance<Builder>, options: Options) -> Instance<Builder> {
            let _ = options;
            this
        }
    }
}

mod storage {
    use nsh_core::prelude::*;

    use super::app::Builder;

    /// The storage service's options.
    #[derive(Debug, Clone, PartialEq, JsObject)]
    pub(crate) struct Options {
        /// Its quota.
        pub(crate) quota: f64,
    }

    /// Methods the storage crate adds.
    pub(crate) struct StorageBuilder;

    #[class_extension(Builder)]
    impl StorageBuilder {
        /// Registers the storage service.
        pub(crate) fn add_files(this: Instance<Builder>, options: Options) -> Instance<Builder> {
            let _ = options;
            this
        }
    }
}

/// The class's set.
fn app_set() -> Registry {
    let mut r = Registry::ui_set("test/types-app");
    r.class::<app::Builder>().class::<app::Token>();
    r
}

/// The extending set.
fn i18n_set() -> Registry {
    let mut r = Registry::ui_set("test/types-i18n");
    r.class_extension::<i18n::I18nBuilder>()
        .class_extension::<i18n::TokenDefaults>();
    r
}

/// The file `path` of the `.nsh/types` of `sets`.
fn file(sets: &[Registry], path: &str) -> String {
    let described: Vec<SetDescription> = sets.iter().map(SetDescription::from).collect();
    type_definitions(&described)
        .into_iter()
        .find(|(p, _)| p == path)
        .map(|(_, text)| text)
        .unwrap_or_else(|| panic!("no {path}"))
}

#[test]
fn the_class_s_file_declares_the_methods_with_the_types_they_name() {
    let text = file(&[app_set(), i18n_set()], "test/types-app.d.ts");
    assert!(
        text.contains(concat!(
            "export interface Builder {\n",
            "  /** Registers the i18n service, configured by `configure`. */\n",
            "  addI18n(configure?: ((options: I18nOptions) => void) | null): Builder;\n",
            "  /** Loads translations from `source`. */\n",
            "  load(source: Source): Promise<void>;\n",
            "}\n",
        )),
        "{text}"
    );
    // The class itself is still the set's declaration of it.
    assert_eq!(
        text.matches("export declare class Builder {").count(),
        1,
        "{text}"
    );
    // The types the methods name come from the extending set's file.
    assert!(
        text.contains("import type { I18nOptions, Source } from \"./types-i18n\";"),
        "{text}"
    );
    // That file declares them, and nothing of the class.
    let i18n = file(&[app_set(), i18n_set()], "test/types-i18n.d.ts");
    assert!(
        i18n.contains("export declare class I18nOptions {"),
        "{i18n}"
    );
    assert!(i18n.contains("export interface Source {"), "{i18n}");
    assert!(!i18n.contains("Builder {"), "{i18n}");
}

#[test]
fn a_generic_class_s_interface_repeats_its_type_parameters() {
    let text = file(&[app_set(), i18n_set()], "test/types-app.d.ts");
    assert!(
        text.contains("export declare class Token<T extends object = object> {"),
        "{text}"
    );
    assert!(
        text.contains(concat!(
            "export interface Token<T extends object = object> {\n",
            "  /** The token's default value, `fallback` when it has none. */\n",
            "  orDefault(fallback: T): T;\n",
            "}\n",
        )),
        "{text}"
    );
}

#[test]
fn without_a_set_registering_an_extension_nothing_is_added() {
    let alone = file(&[app_set()], "test/types-app.d.ts");
    assert!(!alone.contains("export interface Builder"), "{alone}");
    assert!(!alone.contains("export interface Token"), "{alone}");
    assert!(!alone.contains("import type"), "{alone}");
    // The extending set's file adds to no file of the class without it.
    let both = file(&[app_set(), i18n_set()], "test/types-app.d.ts");
    let added = both
        .find("export interface Builder {")
        .expect("the methods are declared");
    assert!(both.len() > alone.len() && added > 0);
}

#[test]
fn a_set_declaring_the_class_and_the_extension_lists_them_once() {
    let mut own = Registry::ui_set("test/types-own");
    own.class::<app::Builder>()
        .class_extension::<i18n::I18nBuilder>();
    let sets = [app_set(), own];
    // In the set's own declaration of the class, not again as an interface.
    let text = file(&sets, "test/types-own.d.ts");
    assert!(!text.contains("export interface Builder"), "{text}");
    assert_eq!(text.matches("addI18n(").count(), 1, "{text}");
    let class = text
        .split("export declare class Builder {")
        .nth(1)
        .expect("the class");
    assert!(class.contains("addI18n("), "{text}");
    // The other set's declaration of the class gets them.
    let app = file(&sets, "test/types-app.d.ts");
    assert!(app.contains("export interface Builder {"), "{app}");
    assert!(
        app.contains("import type { I18nOptions, Source } from \"./types-own\";"),
        "{app}"
    );
}

#[test]
fn a_description_carries_its_extensions_through_json() {
    assert_eq!(FORMAT, 8);
    let description = Description::of(&[app_set(), i18n_set()]);
    assert!(description.sets[0].class_extensions.is_empty());
    let extensions = &description.sets[1].class_extensions;
    assert_eq!(
        extensions
            .iter()
            .map(|e| (e.class.as_str(), e.generics.as_str()))
            .collect::<Vec<_>>(),
        [("Builder", ""), ("Token", "<T extends object = object>")]
    );
    assert_eq!(
        extensions[0],
        ClassExtensionDescription {
            class: "Builder".into(),
            class_type: std::any::type_name::<app::Builder>().into(),
            generics: String::new(),
            members: concat!(
                "  /** Registers the i18n service, configured by `configure`. */\n",
                "  addI18n(configure?: ((options: I18nOptions) => void) | null): Builder;\n",
                "  /** Loads translations from `source`. */\n",
                "  load(source: Source): Promise<void>;\n",
            )
            .into(),
            extension: std::any::type_name::<i18n::I18nBuilder>().into(),
        }
    );
    // Each exported class's Rust type, telling it from another of its name.
    assert_eq!(
        description.sets[0]
            .class_types
            .iter()
            .map(|c| (c.name.as_str(), c.rust_type.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Builder", std::any::type_name::<app::Builder>()),
            ("Token", std::any::type_name::<app::Token>()),
        ]
    );
    let json = description.to_json();
    assert!(json.contains("\"classExtensions\":[{"), "{json}");
    assert!(json.contains("\"classTypes\":[{"), "{json}");
    assert_eq!(Description::from_json(&json).unwrap(), description);
    // `.nsh/types` of the read description is that of the sets.
    let read = Description::from_json(&json).unwrap();
    assert_eq!(
        type_definitions(&read.sets),
        type_definitions(&description.sets)
    );
}

#[test]
fn a_class_of_the_same_name_in_another_set_gets_nothing() {
    let mut other = Registry::ui_set("test/types-other");
    other.class::<other::Builder>();
    let sets = [app_set(), other, i18n_set()];
    let app = file(&sets, "test/types-app.d.ts");
    assert!(app.contains("export interface Builder {"), "{app}");
    // Another Rust class named `Builder` is not the extended one.
    let text = file(&sets, "test/types-other.d.ts");
    assert!(text.contains("export declare class Builder {"), "{text}");
    assert!(!text.contains("export interface Builder"), "{text}");
    assert!(!text.contains("addI18n"), "{text}");
}

#[test]
fn types_of_one_name_from_two_sets_are_each_their_own() {
    let mut weather = Registry::ui_set("test/types-weather");
    weather.class_extension::<weather::WeatherBuilder>();
    let mut storage = Registry::ui_set("test/types-storage");
    storage.class_extension::<storage::StorageBuilder>();
    let sets = [app_set(), weather, storage];
    let text = file(&sets, "test/types-app.d.ts");
    // `Options` resolves to the weather set's, imported as usual; the
    // storage set's is named through its own file.
    assert!(
        text.contains("import type { Options } from \"./types-weather\";"),
        "{text}"
    );
    assert!(
        text.contains("addWeather(options: Options): Builder;"),
        "{text}"
    );
    assert!(
        text.contains("addFiles(options: import(\"./types-storage\").Options): Builder;"),
        "{text}"
    );
}

#[test]
fn a_type_the_class_s_file_declares_does_not_capture_an_extension_s() {
    // The class's set declares an `Options` of its own.
    let mut app = app_set();
    app.module::<other::Settings>();
    let mut weather = Registry::ui_set("test/types-weather");
    weather.class_extension::<weather::WeatherBuilder>();
    let text = file(&[app, weather], "test/types-app.d.ts");
    assert!(
        text.contains("export interface Options {\n  /** A flag. */"),
        "{text}"
    );
    assert!(
        text.contains("addWeather(options: import(\"./types-weather\").Options): Builder;"),
        "{text}"
    );
    assert!(!text.contains("import type { Options }"), "{text}");
}

#[test]
fn a_class_both_files_declare_is_named_as_the_class_s_file_has_it() {
    // The extension names the class's crate's generic `Token`, so its set
    // declares `Token` too: the same Rust class, written bare where it is
    // declared (a generic class's declarations are not interchangeable).
    let mut uses = Registry::ui_set("test/types-uses");
    uses.class_extension::<i18n::TokenUse>();
    let sets = [app_set(), uses];
    let theirs = file(&sets, "test/types-uses.d.ts");
    assert!(
        theirs.contains("export declare class Token<T extends object = object> {"),
        "{theirs}"
    );
    let text = file(&sets, "test/types-app.d.ts");
    assert!(
        text.contains("provide(token: Token<any>): Token<any>;"),
        "{text}"
    );
    assert!(!text.contains("import(\""), "{text}");
}
