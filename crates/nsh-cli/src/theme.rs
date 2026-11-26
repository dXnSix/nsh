//! `nsh theme`: converts old `cssVars` themes to the theme format and
//! checks theme documents and patches (`docs/dev/theme-format.md`).
//!
//! Both call the format's one implementation and restate none of its
//! rules: `convert` nsh-core's converter (`theme_convert::convert`, section
//! 12), `check` nsh-theme's reader and resolver, and the core's role table
//! (`Tokens::resolve_with_roles`: the core's roles, the UI sets' and
//! Tailwind's patterns, checked with `nsh_theme::check_roles`). Every
//! diagnostic prints as `<severity> <code> <pointer>: <message>`, and the
//! command exits 1 on any error or `role-unmapped`, else 0.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use clap::Subcommand;
use nsh_core::element::{RoleSpec, Tokens, description::SetDescription, theme_convert};
use nsh_theme::{Diagnostic, Patch, Selection, ThemeDocument, code};

/// What `--over` takes for nsh-base's default as it was in the old format,
/// which the converter keeps ([`theme_convert::OLD_DEFAULT`]).
const OLD_BASE: &str = "nsh/base";

/// `nsh theme`'s subcommands.
#[derive(Subcommand, Clone)]
pub(crate) enum ThemeAction {
    /// Convert an old `cssVars` theme to a theme document.
    ///
    /// Reads OLD (and `--over`'s BASE); writes the document to NEW (`-o`),
    /// else to standard output, and the conversion's diagnostics to
    /// standard error. Exits 1 on any error diagnostic, else 0; the output
    /// is written either way (empty when nothing could be converted).
    Convert {
        /// The old theme (`{ "cssVars": { "theme", "light", "dark" } }`).
        #[arg(value_name = "OLD")]
        old: PathBuf,
        /// The old library default OLD extended, in the old format too: the
        /// document is then complete, the base's tokens and OLD's together
        /// as the old runtime layered them. `nsh/base` is nsh-base's default
        /// as it was in the old format.
        #[arg(long, value_name = "BASE")]
        over: Option<PathBuf>,
        /// The file to write the document to (default: standard output).
        #[arg(short, long = "out", value_name = "NEW")]
        out: Option<PathBuf>,
    },
    /// Check a theme document, or with `--patch` an override patch.
    ///
    /// Reads FILE (and with `--app` the app's `nsh.json`, its theme and its
    /// Rust crates, which it describes); writes nothing but the
    /// diagnostics, to standard output. A document is read, resolved at its
    /// defaults, its references resolved under every combination of one
    /// context per modifier (a cycle is reported with a combination that
    /// makes it), and its roles checked. A patch is checked against the
    /// app's active document: what it refuses of the patch, and what the
    /// patch brings to resolving it and to its roles. Exits 1 on any error
    /// or unmapped required role (`role-unmapped`), else 0.
    Check {
        /// The theme document (or with `--patch`, the patch) to check.
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// FILE is an override patch (`{ "values", "roles" }`), checked
        /// against the active document of `--app`'s app, which it needs.
        #[arg(long, requires = "app")]
        patch: bool,
        /// The app (its directory or entry) whose UI sets' roles the role
        /// checks use, as its describe program lists them (default: the
        /// roles of the core, `nsh/base` and Tailwind's patterns); with
        /// `--patch`, also the app whose active document the patch is
        /// checked against (`nsh.json`'s `"theme"`, else its sets').
        #[arg(long, value_name = "DIR")]
        app: Option<PathBuf>,
    },
}

/// The error of a command whose diagnostics said why it failed: it exits
/// 1 with nothing more printed.
#[derive(Debug)]
pub(crate) struct Reported;

impl std::fmt::Display for Reported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the theme has errors")
    }
}

impl std::error::Error for Reported {}

/// Runs `nsh theme <action>`.
pub(crate) fn run(action: ThemeAction) -> anyhow::Result<()> {
    match action {
        ThemeAction::Convert { old, over, out } => convert(&old, over.as_deref(), out.as_deref()),
        ThemeAction::Check { file, patch, app } => check(&file, patch, app.as_deref()),
    }
}

/// `nsh theme convert`.
fn convert(old: &Path, over: Option<&Path>, out: Option<&Path>) -> anyhow::Result<()> {
    let text = read(old)?;
    let base = match over {
        Some(path) if path == Path::new(OLD_BASE) => Some(theme_convert::OLD_DEFAULT.to_owned()),
        Some(path) => Some(read(path)?),
        None => None,
    };
    let (document, diagnostics) = theme_convert::convert(&text, base.as_deref());
    match out {
        Some(path) => {
            std::fs::write(path, &document).with_context(|| format!("write {}", path.display()))?
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(document.as_bytes())?;
            stdout.flush()?;
        }
    }
    let mut stderr = std::io::stderr().lock();
    for diagnostic in &diagnostics {
        writeln!(stderr, "{}", line(diagnostic))?;
    }
    outcome(&diagnostics)
}

/// `nsh theme check`.
fn check(file: &Path, patch: bool, app: Option<&Path>) -> anyhow::Result<()> {
    let text = read(file)?;
    let (dir, sets) = match app {
        Some(app) => {
            let (dir, sets) = app_sets(app)?;
            (Some(dir), sets)
        }
        None => (None, crate::ui_sets()),
    };
    let declarations: Vec<RoleSpec> = sets
        .iter()
        .flat_map(|set| set.roles.iter().cloned())
        .collect();
    let diagnostics = match dir {
        Some(dir) if patch => check_patch(&text, active_document(&dir, &sets)?, &declarations),
        // `--patch` requires `--app` (clap).
        _ => check_document(&text, &declarations),
    };
    let mut stdout = std::io::stdout().lock();
    for diagnostic in &diagnostics {
        writeln!(stdout, "{}", line(diagnostic))?;
    }
    stdout.flush()?;
    outcome(&diagnostics)
}

/// What `nsh theme check` says of the document `text`: what reading it
/// gives, what resolving it at its defaults and checking its roles against
/// `declarations` give, and the cycles other picks make.
fn check_document(text: &str, declarations: &[RoleSpec]) -> Vec<Diagnostic> {
    let read = nsh_theme::read(text);
    let mut said = read.diagnostics;
    let document = Arc::new(read.document);
    let (_, resolved) =
        Tokens::resolve_with_roles(document.clone(), Selection::new(), Vec::new(), declarations);
    let cycles = cycles(&document, &[], &resolved);
    said.extend(resolved);
    said.extend(cycles);
    unique(said)
}

/// What `nsh theme check --patch` says of the patch `text` over
/// `document`: what reading it gives, what the document refuses of it
/// (`Patch::check`), and what it brings to resolving the document and to
/// checking its roles, at the defaults and under the other picks: the
/// document's own problems are its own check's.
fn check_patch(text: &str, document: ThemeDocument, declarations: &[RoleSpec]) -> Vec<Diagnostic> {
    let (patch, mut said) = nsh_theme::read_patch(text);
    if said.iter().any(|d| d.code == code::NOT_A_DOCUMENT) {
        return said;
    }
    said.extend(patch.check(&document));
    let document = Arc::new(document);
    let resolve = |patches: Vec<Arc<Patch>>| {
        Tokens::resolve_with_roles(document.clone(), Selection::new(), patches, declarations).1
    };
    let before = resolve(Vec::new());
    let after = resolve(vec![Arc::new(patch.clone())]);
    let cycles_before = cycles(&document, &[], &before);
    let cycles_after = cycles(&document, &[&patch], &after);
    said.extend(after.into_iter().filter(|d| !before.contains(d)));
    said.extend(
        cycles_after
            .into_iter()
            .filter(|d| !cycles_before.iter().any(|b| b.pointer == d.pointer)),
    );
    unique(said)
}

/// The `reference-cycle`s that `nsh_theme::resolve` reports under some
/// combination of one context per modifier of `document` (under
/// `patches`) and not at the defaults (`defaults`, what resolving there
/// gave): each once, with the first combination found that makes it,
/// which its message names.
fn cycles(
    document: &ThemeDocument,
    patches: &[&Patch],
    defaults: &[Diagnostic],
) -> Vec<Diagnostic> {
    // Each modifier's contexts, its default first.
    let axes: Vec<(&str, Vec<&str>)> = document
        .modifiers
        .iter()
        .map(|(name, modifier)| {
            let mut contexts: Vec<&str> = modifier.contexts.keys().collect();
            if let Some(at) = contexts.iter().position(|c| *c == modifier.default) {
                let default = contexts.remove(at);
                contexts.insert(0, default);
            }
            (name, contexts)
        })
        .filter(|(_, contexts)| !contexts.is_empty())
        .collect();
    let mut found: Vec<Diagnostic> = Vec::new();
    // An odometer over the combinations; all zeros, the defaults, are
    // `defaults`' own.
    let mut picks = vec![0usize; axes.len()];
    while let Some(turn) = picks
        .iter()
        .zip(&axes)
        .rposition(|(pick, (_, contexts))| pick + 1 < contexts.len())
    {
        picks[turn] += 1;
        for pick in &mut picks[turn + 1..] {
            *pick = 0;
        }
        let selection = axes
            .iter()
            .zip(&picks)
            .fold(Selection::new(), |selection, ((name, contexts), pick)| {
                selection.with(*name, contexts[*pick])
            });
        for diagnostic in nsh_theme::resolve(document, &selection, patches).diagnostics {
            let known =
                |d: &Diagnostic| d.code == code::REFERENCE_CYCLE && d.pointer == diagnostic.pointer;
            if diagnostic.code != code::REFERENCE_CYCLE
                || defaults.iter().any(known)
                || found.iter().any(known)
            {
                continue;
            }
            let picked: Vec<String> = axes
                .iter()
                .zip(&picks)
                .filter(|(_, pick)| **pick > 0)
                .map(|((name, contexts), pick)| format!("`{name}: {}`", contexts[*pick]))
                .collect();
            let mut diagnostic = diagnostic;
            diagnostic.message = format!(
                "{} It forms with {} picked, the other modifiers at their defaults.",
                diagnostic.message,
                picked.join(" and ")
            );
            found.push(diagnostic);
        }
    }
    found
}

/// The UI sets of the app `app` (its directory or entry): `nsh/base` and
/// those its Rust crates (`nsh.json` `"rust"`) register, from their
/// describe program, as the other commands describe them; with the app's
/// directory.
fn app_sets(app: &Path) -> anyhow::Result<(PathBuf, Arc<[SetDescription]>)> {
    let entry = crate::entry::resolve(app).with_context(|| format!("--app {}", app.display()))?;
    let dir = dunce::canonicalize(&entry)?
        .parent()
        .context("the app's entry has no directory")?
        .to_path_buf();
    if let Some(rust) = crate::rust_app::of(&entry, false, None)? {
        crate::rust_app::set_current(rust.clone());
        crate::set_app_sets(rust.describe()?);
    }
    Ok((dir, crate::ui_sets()))
}

/// The active document of the app in `dir` whose UI sets are `sets`: its
/// own (`nsh.json`'s `"theme"`, as the build reads it), else the one its
/// sets register.
fn active_document(dir: &Path, sets: &[SetDescription]) -> anyhow::Result<ThemeDocument> {
    let theme = nsh_bundler::app_theme(dir, sets).map_err(|e| anyhow::anyhow!(e.plain))?;
    Ok(match theme.document {
        Some(text) => nsh_theme::read(&text).document,
        None => nsh_core::element::description::theme_document(sets)
            .0
            .unwrap_or_default(),
    })
}

/// The text of `path`.
fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

/// `<severity> <code> <pointer>: <message>`.
fn line(diagnostic: &Diagnostic) -> String {
    format!("{} {diagnostic}", diagnostic.severity.name())
}

/// `diagnostics` without repeats, in their order.
fn unique(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = std::collections::HashSet::new();
    diagnostics
        .into_iter()
        .filter(|d| seen.insert(d.clone()))
        .collect()
}

/// Fails ([`Reported`]) on an error or an unmapped required role.
fn outcome(diagnostics: &[Diagnostic]) -> anyhow::Result<()> {
    if diagnostics
        .iter()
        .any(|d| d.is_error() || d.code == code::ROLE_UNMAPPED)
    {
        return Err(Reported.into());
    }
    Ok(())
}
