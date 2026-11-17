//! Class extensions (`#[class_extension(C)]`, `Registry::class_extension`):
//! instance methods a set adds to a class of another crate or set; what
//! registering them does and refuses, and how one set declares them.

use crate::element::{ClassExtensionDef, MemberKind, Registry, declarations, panic_message};

/// The class's crate.
mod app {
    use crate::prelude::*;

    /// Builds the app.
    #[class]
    pub(crate) struct Builder {
        pub(crate) services: Vec<String>,
    }

    #[class]
    impl Builder {
        /// A new builder.
        pub(crate) fn new() -> Self {
            Builder {
                services: Vec::new(),
            }
        }

        /// Adds storage.
        pub(crate) fn add_storage(this: Instance<Self>) -> Instance<Self> {
            this.with_mut(|b| b.services.push("storage".to_owned()));
            this
        }

        /// The services added.
        #[getter]
        pub(crate) fn services(&self) -> Vec<String> {
            self.services.clone()
        }

        #[setter]
        pub(crate) fn set_services(&mut self, services: Vec<String>) {
            self.services = services;
        }

        /// A builder of the defaults.
        pub(crate) fn defaults() -> Builder {
            Builder {
                services: Vec::new(),
            }
        }
    }

    /// A node of a tree, declared as an interface.
    #[class(interface)]
    pub(crate) struct Node;

    #[class]
    impl Node {
        /// Its name.
        #[getter]
        pub(crate) fn name(&self) -> String {
            String::new()
        }
    }

    /// A token naming a service of type `T`.
    #[class(generic = "T")]
    pub(crate) struct Token;

    #[class]
    impl Token {
        /// A token.
        pub(crate) fn new() -> Self {
            Token
        }
    }
}

/// The extending crate: its own module, naming the class by its path.
mod i18n {
    use super::app;
    use crate::prelude::*;

    /// The i18n service's options.
    #[class]
    pub(crate) struct I18nOptions {
        pub(crate) locale: String,
    }

    #[class]
    impl I18nOptions {
        /// The locale to start with.
        #[setter]
        pub(crate) fn set_locale(&mut self, locale: String) {
            self.locale = locale;
        }

        #[getter]
        pub(crate) fn locale(&self) -> String {
            self.locale.clone()
        }
    }

    /// Methods the i18n crate adds to the app builder.
    pub(crate) struct I18nBuilder;

    #[class_extension(app::Builder)]
    impl I18nBuilder {
        /// Registers the i18n service, configured by `configure`.
        pub(crate) fn add_i18n(
            this: Instance<app::Builder>,
            #[args(options)] configure: Option<Callback<Instance<I18nOptions>>>,
        ) -> Result<Instance<app::Builder>, String> {
            let _ = configure;
            this.with_mut(|b| b.services.push(Self::name()));
            Ok(this)
        }

        /// Loads the translations.
        pub(crate) async fn load(this: Instance<app::Builder>, locale: String) -> Vec<String> {
            let _ = locale;
            this.with(|b| b.services.clone())
        }

        /// The translations, later.
        pub(crate) fn translations(this: Instance<app::Builder>) -> Later<String> {
            let _ = this;
            Later::new(async { Ok::<_, String>(String::new()) })
        }

        pub(crate) fn undocumented(this: Instance<app::Builder>) {
            let _ = this;
        }

        /// A helper, Rust's only.
        fn name() -> String {
            "i18n".to_owned()
        }
    }

    /// Methods of a tree's nodes.
    pub(crate) struct NodePaths;

    #[class_extension(app::Node)]
    impl NodePaths {
        /// Its path from the root.
        pub(crate) fn path(this: Instance<app::Node>) -> String {
            let _ = this;
            String::new()
        }
    }

    /// Methods of tokens.
    pub(crate) struct TokenDefaults;

    #[class_extension(app::Token)]
    impl TokenDefaults {
        /// The token's default value, `fallback` when it has none.
        #[ts("(fallback: T): T")]
        pub(crate) fn or_default(this: Instance<app::Token>, fallback: JsHandle) -> JsHandle {
            let _ = this;
            fallback
        }
    }
}

/// A crate whose extension alone names its types.
mod sources {
    use super::app;
    use crate::prelude::*;

    /// Where translations come from.
    #[derive(Debug, Clone, PartialEq, JsObject)]
    pub(crate) struct TranslationSource {
        /// Their address.
        pub(crate) url: String,
    }

    /// How translations load.
    #[derive(Debug, Clone, Copy, PartialEq, Default, PropEnum)]
    pub(crate) enum LoadMode {
        /// All at the start.
        #[default]
        Eager,
        /// Each when first used.
        Lazy,
    }

    /// Methods adding translation sources to the builder.
    pub(crate) struct SourceBuilder;

    #[class_extension(app::Builder)]
    impl SourceBuilder {
        /// Adds `source`; how it loads.
        pub(crate) fn add_source(
            this: Instance<app::Builder>,
            source: TranslationSource,
        ) -> LoadMode {
            let _ = (this, source);
            LoadMode::Lazy
        }
    }
}

/// Another crate, extending the builder with other names.
mod weather {
    use super::app;
    use crate::prelude::*;

    /// Methods the weather crate adds to the app builder.
    pub(crate) struct WeatherBuilder;

    #[class_extension(app::Builder)]
    impl WeatherBuilder {
        /// Registers the weather service.
        pub(crate) fn add_weather(this: Instance<app::Builder>) -> Instance<app::Builder> {
            this
        }
    }

    /// Adds a name the i18n crate adds too.
    pub(crate) struct OtherI18n;

    #[class_extension(app::Builder)]
    impl OtherI18n {
        /// Registers another i18n service.
        pub(crate) fn add_i18n(this: Instance<app::Builder>) -> Instance<app::Builder> {
            this
        }
    }

    /// Named like the class's own method.
    pub(crate) struct Storage;

    #[class_extension(app::Builder)]
    impl Storage {
        /// Registers storage.
        pub(crate) fn add_storage(this: Instance<app::Builder>) -> Instance<app::Builder> {
            this
        }
    }

    /// Named like the class's property.
    pub(crate) struct Services;

    #[class_extension(app::Builder)]
    impl Services {
        /// Lists the services.
        pub(crate) fn services(this: Instance<app::Builder>) -> Vec<String> {
            this.with(|b| b.services.clone())
        }
    }

    /// Named like the class's static.
    pub(crate) struct Defaults;

    #[class_extension(app::Builder)]
    impl Defaults {
        /// Resets to the defaults.
        pub(crate) fn defaults(this: Instance<app::Builder>) -> Instance<app::Builder> {
            this
        }
    }
}

/// What `f` panics with.
fn panic_of(f: impl FnOnce()) -> String {
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("it panics");
    panic_message(payload)
}

/// The names of `defs`' holder types, last segment only.
fn names(defs: &[&'static ClassExtensionDef]) -> Vec<&'static str> {
    defs.iter()
        .map(|d| (d.type_name)().rsplit("::").next().unwrap())
        .collect()
}

#[test]
fn registering_an_extension_registers_what_its_methods_name_but_not_its_class() {
    let mut set = Registry::ui_set("test/i18n");
    set.class_extension::<i18n::I18nBuilder>();
    let classes: Vec<_> = set.class_specs().iter().map(|c| c.name).collect();
    assert_eq!(classes, ["I18nOptions"]);
    assert_eq!(names(&set.class_extensions()), ["I18nBuilder"]);
    let def = set.class_extensions()[0];
    assert_eq!(def.class.name, "Builder");
    let methods: Vec<_> = def.members.iter().map(|m| m.name).collect();
    assert_eq!(
        methods,
        ["addI18n", "load", "translations", "undocumented"],
        "helpers are Rust's"
    );
    assert!(def.members.iter().all(|m| m.kind == MemberKind::Method));
    assert!(!def.members[0].call.is_async());
    assert!(def.members[1].call.is_async(), "async fn");
    assert!(def.members[2].call.is_async(), "Later");
    // Again: nothing more.
    set.class_extension::<i18n::I18nBuilder>();
    assert_eq!(set.class_extensions().len(), 1);
    // With the class, the set exports the class.
    let mut both = Registry::ui_set("test/app");
    both.class::<app::Builder>()
        .class_extension::<i18n::I18nBuilder>();
    let classes: Vec<_> = both.class_specs().iter().map(|c| c.name).collect();
    assert_eq!(classes, ["Builder", "I18nOptions"]);
}

#[test]
fn a_sets_extensions_are_listed_by_class_then_type() {
    let mut set = Registry::ui_set("test/ext");
    set.class_extension::<weather::WeatherBuilder>()
        .class_extension::<i18n::TokenDefaults>()
        .class_extension::<i18n::NodePaths>()
        .class_extension::<i18n::I18nBuilder>();
    assert_eq!(
        names(&set.class_extensions()),
        [
            "I18nBuilder",
            "WeatherBuilder",
            "NodePaths",
            "TokenDefaults"
        ]
    );
}

#[test]
fn the_extensions_of_several_sets_are_listed_once_each() {
    let mut app = Registry::ui_set("test/app");
    app.class::<app::Builder>()
        .class_extension::<i18n::I18nBuilder>();
    let mut sub = Registry::ui_set("test/app/weather");
    sub.class_extension::<weather::WeatherBuilder>();
    app.submodule(sub);
    // One extension registered by two sets is one.
    let mut other = Registry::ui_set("test/other");
    other
        .class_extension::<i18n::I18nBuilder>()
        .class_extension::<i18n::NodePaths>();
    let sets = [app, other];
    assert_eq!(
        names(&Registry::class_extensions_in(&sets)),
        ["I18nBuilder", "WeatherBuilder", "NodePaths"]
    );
    // The core's view of them has every set's.
    let merged = Registry::merged(&sets);
    assert_eq!(
        names(&merged.class_extensions()),
        ["I18nBuilder", "WeatherBuilder", "NodePaths"]
    );
}

#[test]
fn a_method_named_like_an_instance_member_of_the_class_is_refused() {
    let class = std::any::type_name::<app::Builder>();
    for (message, register) in [
        (
            format!(
                "class extension `{}` adds `addStorage` to class `Builder` (`{class}`), which has a member of that name: rename one",
                std::any::type_name::<weather::Storage>()
            ),
            (|set: &mut Registry| {
                set.class_extension::<weather::Storage>();
            }) as fn(&mut Registry),
        ),
        (
            format!(
                "class extension `{}` adds `services` to class `Builder` (`{class}`), which has a member of that name: rename one",
                std::any::type_name::<weather::Services>()
            ),
            |set: &mut Registry| {
                set.class_extension::<weather::Services>();
            },
        ),
    ] {
        let mut set = Registry::ui_set("test/ext");
        assert_eq!(panic_of(|| register(&mut set)), message);
        assert!(set.class_extensions().is_empty());
    }
}

#[test]
fn a_method_named_like_a_static_of_the_class_is_no_clash() {
    // The static is on the constructor, the method on the prototype.
    let mut set = Registry::ui_set("test/ext");
    set.class::<app::Builder>()
        .class_extension::<weather::Defaults>();
    assert_eq!(names(&set.class_extensions()), ["Defaults"]);
}

#[test]
fn two_extensions_adding_one_name_to_a_class_in_one_set_are_refused() {
    let mut set = Registry::ui_set("test/ext");
    set.class_extension::<i18n::I18nBuilder>();
    let message = panic_of(|| {
        set.class_extension::<weather::OtherI18n>();
    });
    assert_eq!(
        message,
        format!(
            "class extensions `{}` and `{}` both add `addI18n` to class `Builder` (`{}`) in `test/ext`: rename one",
            std::any::type_name::<i18n::I18nBuilder>(),
            std::any::type_name::<weather::OtherI18n>(),
            std::any::type_name::<app::Builder>(),
        )
    );
    // Other names may be added.
    let mut set = Registry::ui_set("test/ext");
    set.class_extension::<i18n::I18nBuilder>()
        .class_extension::<weather::WeatherBuilder>();
    assert_eq!(set.class_extensions().len(), 2);
}

#[test]
fn two_sets_adding_one_name_to_a_class_are_refused() {
    let mut i18n = Registry::ui_set("test/i18n");
    i18n.class_extension::<i18n::I18nBuilder>();
    let mut other = Registry::ui_set("test/other");
    other.class_extension::<weather::OtherI18n>();
    let sets = [i18n, other];
    let expected = format!(
        "class extensions `{}` (in `test/i18n`) and `{}` (in `test/other`) both add `addI18n` to class `Builder` (`{}`), by two UI sets: rename one",
        std::any::type_name::<i18n::I18nBuilder>(),
        std::any::type_name::<weather::OtherI18n>(),
        std::any::type_name::<app::Builder>(),
    );
    assert_eq!(
        panic_of(|| {
            Registry::class_extensions_in(&sets);
        }),
        expected
    );
    // The core takes its sets through `merged`: the same.
    assert_eq!(
        panic_of(|| {
            Registry::merged(&sets);
        }),
        expected
    );
}

#[test]
fn a_set_declaring_the_class_lists_the_methods_before_dispose() {
    let mut set = Registry::ui_set("test/app");
    set.class::<app::Builder>()
        .class_extension::<i18n::I18nBuilder>();
    let text = declarations(&set, &[]);
    let class = text
        .split("export declare class Builder {\n")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .unwrap_or_else(|| panic!("{text}"));
    let expected = concat!(
        "  /** A new builder. */\n",
        "  constructor();\n",
        "  /** Adds storage. */\n",
        "  addStorage(): Builder;\n",
        "  /** The services added. */\n",
        "  get services(): (string)[];\n",
        "  set services(services: (string)[]);\n",
        "  /** A builder of the defaults. */\n",
        "  static defaults(): Builder;\n",
        "  /** Registers the i18n service, configured by `configure`. */\n",
        "  addI18n(configure?: ((options: I18nOptions) => void) | null): Builder;\n",
        "  /** Loads the translations. */\n",
        "  load(locale: string): Promise<(string)[]>;\n",
        "  /** The translations, later. */\n",
        "  translations(): Promise<string>;\n",
        "  undocumented(): void;\n",
        "  /** Frees the Rust value now (else when the object is collected); the object throws after. */\n",
        "  dispose(): void;\n",
        "  [Symbol.dispose](): void;",
    );
    assert_eq!(class, expected, "{text}");
    // The types the methods name are the set's.
    assert!(
        text.contains("export declare class I18nOptions {"),
        "{text}"
    );
}

#[test]
fn an_interface_class_and_a_generic_class_list_them_too() {
    let mut set = Registry::ui_set("test/app");
    set.class::<app::Node>()
        .class::<app::Token>()
        .class_extension::<i18n::NodePaths>()
        .class_extension::<i18n::TokenDefaults>();
    let text = declarations(&set, &[]);
    assert!(
        text.contains(concat!(
            "export interface Node {\n",
            "  /** Its name. */\n",
            "  readonly name: string;\n",
            "  /** Its path from the root. */\n",
            "  path(): string;\n",
            "}\n",
        )),
        "{text}"
    );
    // `#[ts]` names the class's type parameter.
    assert!(
        text.contains(concat!(
            "  /** The token's default value, `fallback` when it has none. */\n",
            "  orDefault(fallback: T): T;\n",
            "  /** Frees the Rust value now",
        )),
        "{text}"
    );
}

#[test]
fn without_the_extension_the_declarations_are_the_classes_alone() {
    let mut alone = Registry::ui_set("test/app");
    alone.class::<app::Builder>();
    let before = declarations(&alone, &[]);
    // An extension in another set leaves this one's as they are.
    let mut other = Registry::ui_set("test/i18n");
    other.class_extension::<i18n::I18nBuilder>();
    assert_eq!(declarations(&alone, &[]), before);
    assert!(!declarations(&other, &[]).contains("class Builder"));
    // With it, the methods' lines are all that is added to the class.
    let mut with = Registry::ui_set("test/app");
    with.class::<app::Builder>()
        .class_extension::<weather::WeatherBuilder>();
    let after = declarations(&with, &[]);
    let added = "  /** Registers the weather service. */\n  addWeather(): Builder;\n";
    assert!(after.contains(added), "{after}");
    assert_eq!(after.replacen(added, "", 1), before);
}

#[test]
fn an_undocumented_method_is_listed_under_its_class() {
    let mut set = Registry::ui_set("test/app");
    set.class::<app::Builder>()
        .class_extension::<i18n::I18nBuilder>();
    let undocumented = set.undocumented();
    assert!(
        undocumented.contains(&"Builder.undocumented".to_owned()),
        "{undocumented:?}"
    );
    assert!(!undocumented.contains(&"Builder.addI18n".to_owned()));
}

#[test]
fn the_named_types_only_an_extension_names_are_declared_once() {
    let mut only = Registry::ui_set("test/sources");
    only.class_extension::<sources::SourceBuilder>();
    let mut both = Registry::ui_set("test/app");
    both.class::<app::Builder>()
        .class_extension::<sources::SourceBuilder>();
    for set in [only, both] {
        let text = declarations(&set, &[]);
        assert_eq!(
            text.matches("export type LoadMode = \"eager\" | \"lazy\";")
                .count(),
            1,
            "{text}"
        );
        assert_eq!(
            text.matches(
                "export interface TranslationSource {\n  /** Their address. */\n  url: string;\n}"
            )
            .count(),
            1,
            "{text}"
        );
    }
}
