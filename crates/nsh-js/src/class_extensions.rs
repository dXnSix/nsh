//! Class extensions (`#[class_extension(C)]`), headless: the methods a
//! served set's extension adds are on the class's prototype, from any set
//! that exports the class and on instances members return, and run as the
//! class's own methods do; without such a set there are none; two sets
//! adding one name to one class stop the host from starting. Written in a
//! crate other than nsh-core, through its prelude only.

use nsh_core::prelude::*;

use crate::{Host, HostOptions};

/// The class's crate.
mod app {
    use nsh_core::prelude::*;

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
        pub(crate) fn services(&self) -> String {
            self.services.join(",")
        }

        /// A builder made by a static.
        pub(crate) fn make() -> Builder {
            Builder {
                services: vec!["made".to_owned()],
            }
        }

        /// A builder made by a method.
        pub(crate) fn child(&self) -> Builder {
            Builder {
                services: vec!["child".to_owned()],
            }
        }
    }
}

/// The extending crate.
mod i18n {
    use nsh_core::prelude::*;

    use super::app::Builder;

    /// The i18n service's options.
    #[class]
    pub(crate) struct I18nOptions {
        pub(crate) locale: String,
    }

    #[class]
    impl I18nOptions {
        /// The locale to start with.
        #[getter]
        pub(crate) fn locale(&self) -> String {
            self.locale.clone()
        }

        #[setter]
        pub(crate) fn set_locale(&mut self, locale: String) {
            self.locale = locale;
        }
    }

    /// Methods the i18n crate adds to the app builder.
    pub(crate) struct I18nBuilder;

    #[class_extension(Builder)]
    impl I18nBuilder {
        /// Registers the i18n service, configured on the spot by
        /// `configure`.
        pub(crate) fn add_i18n(
            this: Instance<Builder>,
            #[args(options)] configure: Option<Callback<Instance<I18nOptions>>>,
        ) -> Result<Instance<Builder>, String> {
            let options = Instance::new(I18nOptions {
                locale: "en".to_owned(),
            });
            if let Some(configure) = configure {
                configure.call(options.clone())?;
            }
            let locale = options.with(|o| o.locale.clone());
            this.with_mut(|b| b.services.push(format!("i18n:{locale}")));
            Ok(this)
        }

        /// Adds `by` services named `label` (`extra` when left out); how
        /// many it has.
        pub(crate) fn add_many(this: Instance<Builder>, by: i64, label: Option<String>) -> i64 {
            let label = label.unwrap_or_else(|| "extra".to_owned());
            this.with_mut(|b| {
                for _ in 0..by {
                    b.services.push(label.clone());
                }
                b.services.len() as i64
            })
        }

        /// Refuses with `why`.
        pub(crate) fn refuse(this: Instance<Builder>, why: String) -> Result<(), String> {
            let _ = this;
            Err(why)
        }

        /// Panics.
        pub(crate) fn explode(this: Instance<Builder>) {
            let _ = this;
            panic!("the builder exploded");
        }
    }
}

/// Another crate adding a name the i18n crate adds.
mod other {
    use nsh_core::prelude::*;

    use super::app::Builder;

    /// Another i18n.
    pub(crate) struct OtherI18n;

    #[class_extension(Builder)]
    impl OtherI18n {
        /// Registers another i18n service.
        pub(crate) fn add_i18n(this: Instance<Builder>) -> Instance<Builder> {
            this
        }
    }
}

/// The class's set.
fn app_set() -> Registry {
    let mut r = Registry::ui_set("test/ext-app");
    r.class::<app::Builder>();
    r
}

/// The extending set.
fn i18n_set() -> Registry {
    let mut r = Registry::ui_set("test/ext-i18n");
    r.class_extension::<i18n::I18nBuilder>();
    r
}

fn host(ui_sets: Vec<Registry>) -> Host {
    Host::with_options(HostOptions {
        ui_sets,
        ..HostOptions::default()
    })
    .unwrap()
}

/// Runs `code` (a module importing the class from `test/ext-app`); the
/// texts it shows with `show(...)`.
fn run(host: &Host, code: &str) -> Result<Vec<String>, String> {
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Builder }} from \"test/ext-app\";\n{code}"
    ))
    .map_err(|e| e.message)?;
    let mut ops = Vec::new();
    host.take_ops(&mut ops);
    host.apply_posts(&mut ops);
    Ok(ops
        .into_iter()
        .filter_map(|op| match op {
            nsh_core::Op::CreateText { text, .. } => Some(text.to_string()),
            _ => None,
        })
        .collect())
}

/// Shows the shape of each of `Builder.prototype`'s properties named.
const SHAPES: &str = r#"
for (const name of ["addI18n", "addMany", "refuse", "explode"]) {
    const d = Object.getOwnPropertyDescriptor(Builder.prototype, name);
    show(d === undefined ? `${name} none` : [name, typeof d.value, d.value.name, d.value.length,
        d.writable, d.configurable, d.enumerable].join(" "));
}
"#;

/// What [`SHAPES`] shows with the extension.
const WITH: [&str; 4] = [
    "addI18n function addI18n 1 true true false",
    "addMany function addMany 2 true true false",
    "refuse function refuse 1 true true false",
    "explode function explode 0 true true false",
];

#[test]
fn the_methods_are_on_the_prototype_with_the_extension_in_another_set() {
    // The bundle never imports the extending set.
    let host = host(vec![app_set(), i18n_set()]);
    assert_eq!(run(&host, SHAPES).unwrap(), WITH);
    // The class's own members are as they were.
    let shown = run(
        &host,
        r#"
        const own = Object.getOwnPropertyDescriptor(Builder.prototype, "addStorage");
        show([own.writable, own.configurable, own.enumerable, own.value.length].join(" "));
        show(Object.getOwnPropertyNames(Builder.prototype).sort().join(","));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "true true false 0",
            "addI18n,addMany,addStorage,child,constructor,dispose,explode,refuse,services"
        ]
    );
}

#[test]
fn the_methods_are_on_the_prototype_with_the_extension_in_the_class_s_own_set() {
    let mut set = app_set();
    set.class_extension::<i18n::I18nBuilder>();
    let host = host(vec![set]);
    assert_eq!(run(&host, SHAPES).unwrap(), WITH);
}

#[test]
fn a_submodule_of_a_served_set_brings_them_too() {
    let mut set = app_set();
    let mut sub = Registry::ui_set("test/ext-app/i18n");
    sub.class_extension::<i18n::I18nBuilder>();
    set.submodule(sub);
    let host = host(vec![set]);
    assert_eq!(run(&host, SHAPES).unwrap(), WITH);
}

#[test]
fn without_a_set_registering_the_extension_there_are_none() {
    let host = host(vec![app_set()]);
    assert_eq!(
        run(&host, SHAPES).unwrap(),
        [
            "addI18n none",
            "addMany none",
            "refuse none",
            "explode none"
        ]
    );
    let shown = run(
        &host,
        r#"
        show(String("addI18n" in Builder.prototype));
        show(new Builder().addStorage().services);
        show(Object.getOwnPropertyNames(Builder.prototype).sort().join(","));
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "false",
            "storage",
            "addStorage,child,constructor,dispose,services"
        ]
    );
}

#[test]
fn instances_members_return_have_them() {
    let host = host(vec![app_set(), i18n_set()]);
    let shown = run(
        &host,
        r#"
        show(Builder.make().addI18n().services);
        show(new Builder().child().addI18n().services);
        "#,
    )
    .unwrap();
    assert_eq!(shown, ["made,i18n:en", "child,i18n:en"]);
}

#[test]
fn calls_chain_on_the_same_object_and_the_class_sees_what_they_changed() {
    let host = host(vec![app_set(), i18n_set()]);
    let shown = run(
        &host,
        r#"
        const b = new Builder();
        show(String(b.addI18n() === b));
        const chained = new Builder().addStorage().addI18n((o) => { o.locale = "hr"; }).addStorage();
        show(chained.services);
        show(String(b.addMany(2, "x")));
        // A trailing `Option` may be left out.
        show(String(b.addMany(1)));
        show(b.services);
        "#,
    )
    .unwrap();
    assert_eq!(
        shown,
        [
            "true",
            "storage,i18n:hr,storage",
            "3",
            "4",
            "i18n:en,x,x,extra"
        ]
    );
}

#[test]
fn a_callback_is_called_on_the_spot_with_an_instance_usable_in_the_call() {
    let host = host(vec![app_set(), i18n_set()]);
    let shown = run(
        &host,
        r#"
        const seen = [];
        const b = new Builder().addI18n((o) => {
            seen.push(o.locale);
            o.locale = "de";
            seen.push(o.locale);
        });
        seen.push(b.services);
        show(seen.join(" "));
        "#,
    )
    .unwrap();
    assert_eq!(shown, ["en de i18n:de"]);
}

#[test]
fn a_wrong_this_or_argument_throws_a_type_error_naming_the_method() {
    let host = host(vec![app_set(), i18n_set()]);
    let shown = run(
        &host,
        r#"
        const b = new Builder();
        const tries = [
            () => Builder.prototype.addI18n.call({}),
            () => b.addMany("two"),
            () => b.addI18n(42),
        ];
        for (const t of tries) {
            try { t(); show("no error"); } catch (e) { show(`${e.constructor.name}: ${e.message}`); }
        }
        "#,
    )
    .unwrap();
    assert_eq!(shown.len(), 3, "{shown:?}");
    assert_eq!(
        shown[0],
        "TypeError: Builder.addI18n: Illegal invocation: not a Builder"
    );
    assert!(
        shown[1].starts_with("TypeError: Builder.addMany: `by`: "),
        "{shown:?}"
    );
    assert!(
        shown[2].starts_with("TypeError: Builder.addI18n: `configure`: "),
        "{shown:?}"
    );
}

#[test]
fn an_err_or_a_panic_throws_an_error() {
    let host = host(vec![app_set(), i18n_set()]);
    let shown = run(
        &host,
        r#"
        const b = new Builder();
        for (const t of [() => b.refuse("no i18n here"), () => b.explode(),
                         () => b.addI18n(() => { throw new Error("bad options"); })]) {
            try { t(); show("no error"); } catch (e) { show(`${e.constructor.name}: ${e.message}`); }
        }
        // The object still works.
        show(b.addI18n().services);
        "#,
    )
    .unwrap();
    assert_eq!(shown.len(), 4, "{shown:?}");
    assert_eq!(shown[0], "Error: no i18n here");
    assert!(
        shown[1].starts_with("Error: ") && shown[1].contains("the builder exploded"),
        "{shown:?}"
    );
    assert!(
        shown[2].starts_with("Error: ") && shown[2].contains("bad options"),
        "{shown:?}"
    );
    assert_eq!(shown[3], "i18n:en");
}

#[test]
fn a_host_given_two_sets_adding_one_name_to_a_class_refuses_to_start() {
    let mut clash = Registry::ui_set("test/ext-other");
    clash.class_extension::<other::OtherI18n>();
    let sets = vec![app_set(), i18n_set(), clash];
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _ = Host::with_options(HostOptions {
            ui_sets: sets,
            ..HostOptions::default()
        });
    }))
    .expect_err("the host refuses the sets");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default();
    assert_eq!(
        message,
        format!(
            "class extensions `{}` (in `test/ext-i18n`) and `{}` (in `test/ext-other`) both add `addI18n` to class `Builder` (`{}`), by two UI sets: rename one",
            std::any::type_name::<i18n::I18nBuilder>(),
            std::any::type_name::<other::OtherI18n>(),
            std::any::type_name::<app::Builder>(),
        )
    );
}
