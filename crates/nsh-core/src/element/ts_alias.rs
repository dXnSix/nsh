//! Type aliases a set exports for its TypeScript signatures to share
//! ([`super::Registry::ts_alias`]).

/// A TypeScript type alias a set exports, so the signatures it writes in
/// TypeScript (`#[ts("…")]`) can share a type:
///
/// ```
/// # use nsh_core::element::{Registry, TsAlias};
/// # let mut registry = Registry::ui_set("app/native");
/// registry.ts_alias(TsAlias {
///     name: "ServiceKey<T>",
///     ts: "{ readonly prototype: T } | ServiceToken<T>",
///     doc: "What names a service of type `T`: its class, or a token.",
/// });
/// ```
///
/// declares
///
/// ```ts
/// /** What names a service of type `T`: its class, or a token. */
/// export type ServiceKey<T> = { readonly prototype: T } | ServiceToken<T>;
/// ```
///
/// TS imports it as a type (`import type { ServiceKey } from "app/native"`):
/// JS has no value of it. The text is TypeScript's: the classes it names
/// are the set's to register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TsAlias {
    /// Its name, with its type parameters when it is generic
    /// (`ServiceKey<T>`, `Maybe<T = unknown>`).
    pub name: &'static str,
    /// The type it names, as TypeScript writes it.
    pub ts: &'static str,
    /// Documentation.
    pub doc: &'static str,
}

impl TsAlias {
    /// Its name without its type parameters (`ServiceKey`).
    pub fn base_name(&self) -> &'static str {
        let name = self.name.trim();
        name.split_once('<')
            .map_or(name, |(base, _)| base)
            .trim_end()
    }

    /// What is wrong with it, if anything: a name that is no identifier
    /// (with `<..>` after it), no type.
    pub(crate) fn mistake(&self) -> Option<String> {
        let base = self.base_name();
        let identifier = base.chars().enumerate().all(|(i, c)| {
            c == '_' || c == '$' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
        });
        if base.is_empty() || !identifier {
            return Some(format!(
                "type alias `{}`: its name is an identifier, then its type parameters \
                 (`ServiceKey<T>`)",
                self.name
            ));
        }
        let params = self.name.trim()[base.len()..].trim();
        let generic = params
            .strip_prefix('<')
            .and_then(|p| p.strip_suffix('>'))
            .is_some_and(|p| !p.trim().is_empty());
        if !params.is_empty() && !generic {
            return Some(format!(
                "type alias `{}`: its type parameters go between `<` and `>` after its name",
                self.name
            ));
        }
        if self.type_text().is_empty() {
            return Some(format!("type alias `{}` names no type", self.name));
        }
        None
    }

    /// The type, without a trailing `;`.
    fn type_text(&self) -> &'static str {
        let ts = self.ts.trim();
        ts.strip_suffix(';').unwrap_or(ts).trim_end()
    }

    /// Its declaration: `export type ServiceKey<T> = …;`, documented.
    pub(crate) fn declaration(&self) -> String {
        let name = self.name.trim();
        let ts = self.type_text();
        if self.doc.is_empty() {
            format!("export type {name} = {ts};")
        } else {
            format!("/** {} */\nexport type {name} = {ts};", self.doc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TsAlias;
    use crate::{
        element::{Registry, declarations},
        prelude::*,
    };

    const SERVICE_KEY: TsAlias = TsAlias {
        name: "ServiceKey<T>",
        ts: "{ readonly prototype: T } | Token<T>",
        doc: "What names a service of type `T`: its class, or a token.",
    };

    /// Names a service of type `T`.
    #[class(generic = "T")]
    struct Token;

    #[class]
    impl Token {
        /// A token of `T`.
        #[ts("<T>(): Token<T>")]
        pub(crate) fn create() -> Token {
            Token
        }
    }

    /// Keeps services.
    #[class]
    struct Container;

    #[class]
    impl Container {
        /// The service `key` names.
        #[ts("<T>(key: ServiceKey<T>): T")]
        pub(crate) fn get(&self, key: JsHandle) -> JsHandle {
            key
        }
    }

    #[test]
    fn a_set_exports_its_type_aliases_with_its_named_types() {
        let mut registry = Registry::ui_set("test/alias");
        registry
            .class::<Container>()
            .class::<Token>()
            .ts_alias(SERVICE_KEY)
            .ts_alias(TsAlias {
                name: " Millis ",
                ts: "number; ",
                doc: "",
            })
            // Again: nothing.
            .ts_alias(SERVICE_KEY);
        assert_eq!(
            registry
                .ts_aliases()
                .iter()
                .map(TsAlias::base_name)
                .collect::<Vec<_>>(),
            ["Millis", "ServiceKey"]
        );
        assert_eq!(
            declarations(&registry, &[]),
            r#"export type Millis = number;

/** What names a service of type `T`: its class, or a token. */
export type ServiceKey<T> = { readonly prototype: T } | Token<T>;

/** Keeps services. */
export declare class Container {
  /** Made by the class's members only. */
  private constructor();
  /** The service `key` names. */
  get<T>(key: ServiceKey<T>): T;
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}

/** Names a service of type `T`. */
export declare class Token<T> {
  /** Its type parameters, kept in its type (no value at run time). */
  #types: [T];
  /** Made by the class's members only. */
  private constructor();
  /** A token of `T`. */
  static create<T>(): Token<T>;
  /** Frees the Rust value now (else when the object is collected); the object throws after. */
  dispose(): void;
  [Symbol.dispose](): void;
}

"#
        );
        // TS sees it without a doc comment.
        assert_eq!(registry.undocumented(), ["Millis"]);
    }

    fn panics(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let payload = std::panic::catch_unwind(f).expect_err("it panics");
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default()
    }

    #[test]
    fn a_type_alias_is_named_and_names_a_type() {
        for (name, ts, message) in [
            ("", "number", "its name is an identifier"),
            ("1Key", "number", "its name is an identifier"),
            ("Service Key", "number", "its name is an identifier"),
            ("Key<>", "number", "between `<` and `>`"),
            ("Key<T", "number", "between `<` and `>`"),
            ("Key", " ; ", "names no type"),
        ] {
            let error = panics(move || {
                Registry::ui_set("test/alias").ts_alias(TsAlias { name, ts, doc: "" });
            });
            assert!(error.contains(message), "{name}: {error}");
        }
    }

    #[test]
    fn a_type_alias_has_a_name_of_its_own_in_the_set() {
        let error = panics(|| {
            Registry::ui_set("test/alias")
                .ts_alias(SERVICE_KEY)
                .ts_alias(TsAlias {
                    name: "ServiceKey",
                    ts: "object",
                    doc: "",
                });
        });
        assert!(
            error.contains("type aliases `ServiceKey<T>` and `ServiceKey` are both named `ServiceKey` in `test/alias`"),
            "{error}"
        );
        let error = panics(|| {
            Registry::ui_set("test/alias")
                .class::<Token>()
                .ts_alias(TsAlias {
                    name: "Token<T>",
                    ts: "object",
                    doc: "",
                });
        });
        assert!(
            error.contains("type alias `Token<T>` is named `Token` like a class"),
            "{error}"
        );
        let error = panics(|| {
            Registry::ui_set("test/alias")
                .ts_alias(TsAlias {
                    name: "Token",
                    ts: "object",
                    doc: "",
                })
                .class::<Token>();
        });
        assert!(
            error.contains("like a module, a component, an interface or a type alias"),
            "{error}"
        );
    }
}
