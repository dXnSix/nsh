//! Where the code the macros generate finds `nsh-core`.
//!
//! A crate that depends on `nsh-core` names it `::nsh_core` (`nsh-core`
//! itself too: `extern crate self as nsh_core`), or by its own name when it
//! renames the dependency. A crate that takes nsh through another crate (a
//! framework that re-exports it) cannot: a `::` path reaches only the
//! crate's own dependencies. Its macros' code says `nsh_core` without the
//! `::` instead, the name the framework's prelude brings into scope next to
//! the macros (`pub use nsh_core;`). The crate's manifest tells the two
//! apart; without one to read, the code says `::nsh_core`.

use proc_macro_crate::{Error, FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::Ident;

/// How the generated code names `nsh-core`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Root {
    /// A dependency of the crate (or the crate itself), by this name.
    Dependency(String),
    /// A name in scope where the macro is used (`nsh_core`).
    InScope,
}

fn root() -> Root {
    #[cfg(test)]
    if REEXPORTED.with(std::cell::Cell::get) {
        return Root::InScope;
    }
    root_of(crate_name("nsh-core"))
}

#[cfg(test)]
thread_local! {
    static REEXPORTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// In the macros' unit tests: runs `f` as the macros run in a crate that
/// takes `nsh-core` through a framework's re-export (its manifest has no
/// `nsh-core`), whatever this crate's own manifest says.
#[cfg(test)]
pub(crate) fn as_reexported<T>(f: impl FnOnce() -> T) -> T {
    REEXPORTED.with(|on| on.set(true));
    let out = f();
    REEXPORTED.with(|on| on.set(false));
    out
}

fn root_of(found: Result<FoundCrate, Error>) -> Root {
    match found {
        Ok(FoundCrate::Itself) => Root::Dependency("nsh_core".into()),
        Ok(FoundCrate::Name(name)) => Root::Dependency(name),
        // The manifest has no `nsh-core`: something else brings it.
        Err(Error::CrateNotFound { .. }) => Root::InScope,
        // Nothing to tell from (not built by Cargo, a manifest it cannot
        // read): as a dependency.
        Err(_) => Root::Dependency("nsh_core".into()),
    }
}

/// `nsh-core`'s path in code a macro generates: `::nsh_core`, or
/// `nsh_core` when the crate takes it through another crate.
pub(crate) fn nsh_core() -> TokenStream2 {
    path(&root(), 0)
}

/// [`nsh_core`] in a module the macro generates inside the one it is used
/// in: the name in scope is the enclosing module's (`super::nsh_core`).
pub(crate) fn nsh_core_in_module() -> TokenStream2 {
    path(&root(), 1)
}

/// [`nsh_core`] as doc comments link to it (`::nsh_core`, `nsh_core`).
pub(crate) fn nsh_core_in_docs() -> String {
    match root() {
        Root::Dependency(name) => format!("::{name}"),
        Root::InScope => "nsh_core".into(),
    }
}

fn path(root: &Root, depth: usize) -> TokenStream2 {
    match root {
        Root::Dependency(name) => {
            let name = Ident::new(name, Span::call_site());
            quote!(::#name)
        }
        Root::InScope => {
            let supers = std::iter::repeat_n(quote!(super::), depth);
            quote!(#(#supers)* nsh_core)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dependency_is_named_from_the_root() {
        let itself = root_of(Ok(FoundCrate::Itself));
        assert_eq!(path(&itself, 0).to_string(), ":: nsh_core");
        assert_eq!(path(&itself, 1).to_string(), ":: nsh_core");
        let renamed = root_of(Ok(FoundCrate::Name("nsh".into())));
        assert_eq!(path(&renamed, 0).to_string(), ":: nsh");
    }

    #[test]
    fn a_crate_without_nsh_core_names_the_one_in_scope() {
        let found = root_of(Err(Error::CrateNotFound {
            crate_name: "nsh-core".into(),
            path: "Cargo.toml".into(),
        }));
        assert_eq!(found, Root::InScope);
        assert_eq!(path(&found, 0).to_string(), "nsh_core");
        assert_eq!(path(&found, 1).to_string(), "super :: nsh_core");
    }

    #[test]
    fn without_a_manifest_the_code_names_a_dependency() {
        let found = root_of(Err(Error::CargoManifestDirNotSet));
        assert_eq!(found, Root::Dependency("nsh_core".into()));
    }

    /// Every path to `nsh-core` in the macros' code comes from this module.
    /// A `::nsh_core` written out (a macro body merged from upstream as it
    /// is) still compiles for crates that depend on `nsh-core`, and breaks
    /// a crate that takes nsh through a re-export only when it uses that
    /// macro. Doc comments and the test modules may write it.
    #[test]
    fn a_crate_without_nsh_core_reaches_it_from_every_macro() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let needle = concat!(":", ":nsh_core");
        let mut written = Vec::new();
        let mut files: Vec<_> = std::fs::read_dir(&src)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            let mut in_tests = false;
            for (n, line) in lines.iter().enumerate() {
                if in_tests {
                    in_tests = *line != "}";
                    continue;
                }
                if *line == "#[cfg(test)]"
                    && lines
                        .get(n + 1)
                        .is_some_and(|next| next.starts_with("mod "))
                {
                    in_tests = true;
                    continue;
                }
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for (at, _) in line.match_indices(needle) {
                    let before = &line[..at];
                    if !(before.ends_with("root") || before.ends_with("super")) {
                        let name = file.file_name().unwrap().to_string_lossy();
                        written.push(format!("{name}:{}: {}", n + 1, line.trim()));
                    }
                }
            }
        }
        assert!(
            written.is_empty(),
            "write `#nsh_core` from crate::root::nsh_core() instead:\n{}",
            written.join("\n")
        );
    }
}
