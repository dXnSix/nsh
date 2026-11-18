//! What a script runtime returns for a theme write before the core applies
//! it (TS's `setTheme` and `updateTheme`, on nsh-js and the web player).
//!
//! A write is an op the core applies with the turn's other ops, resolving
//! the theme once (`Core::install_theme`); the runtime resolves nothing.
//! It returns the write's diagnostics at once, though: what reading the
//! text gives (`super::theme::read_document`, `super::theme::read_patch`),
//! what the active document refuses of a patch (`Patch::check`), and the
//! role checks of the mappings the write brings, against the roles the
//! served sets declare and Tailwind's patterns (`nsh_theme::check_roles`).
//! None of these needs a value resolved. The core logs them when it
//! applies the write.

use nsh_theme::{Diagnostic, Patch, Resolved, ThemeDocument, code};

use super::Tokens;

impl Tokens {
    /// The role checks of `document`'s mappings against the roles these
    /// tokens were resolved with (the core's and the served sets'
    /// declarations, Tailwind's patterns): a mapping onto a token of a
    /// type the role doesn't read (`role-type`, left out), one for a role
    /// nothing declares onto a token that is not a color (`role-unknown`),
    /// a declared role whose own token has the wrong type. What TS's
    /// `setTheme(document)` returns besides the read's diagnostics. A
    /// required role left unmapped is said by the core, which resolves.
    pub fn check_document(&self, document: &ThemeDocument) -> Vec<Diagnostic> {
        self.role_checks(document, &[])
    }

    /// What `document` refuses of `patch` (`Patch::check`: a name it
    /// doesn't declare, a value that doesn't fit, a reference or role
    /// mapping onto no token) and the role checks of the patch's own
    /// mappings ([`Tokens::check_document`]'s): what TS's
    /// `updateTheme(patch)` returns besides the read's diagnostics.
    pub fn check_patch(&self, document: &ThemeDocument, patch: &Patch) -> Vec<Diagnostic> {
        let mut said = patch.check(document);
        let before = self.role_checks(document, &[]);
        said.extend(
            self.role_checks(document, &[patch])
                .into_iter()
                .filter(|d| !before.contains(d)),
        );
        said
    }

    /// The active document these were resolved with, shared: the one TS's
    /// writes are checked against until the core applies them. `None`:
    /// made of values alone.
    pub fn active_document(&self) -> Option<&std::sync::Arc<ThemeDocument>> {
        self.layers().map(|layers| &layers.document)
    }

    /// `nsh_theme::check_roles` of `document` and `patches` against these
    /// tokens' roles, with no value resolved: what holds of the mappings'
    /// types alone (`role-unmapped` needs values, and is left to the core).
    fn role_checks(&self, document: &ThemeDocument, patches: &[&Patch]) -> Vec<Diagnostic> {
        let core = super::tokens::CORE_ROLES.clone();
        let roles = self.layers().map_or(&core, |layers| &layers.roles);
        nsh_theme::check_roles(
            document,
            patches,
            &Resolved::default(),
            &roles.declarations,
            &roles.patterns,
        )
        .into_iter()
        .filter(|d| d.code != code::ROLE_UNMAPPED)
        .collect()
    }
}

impl Tokens {
    /// The tokens the core installs when it takes the app's built-in
    /// document `document` and overrides `overrides` (`Prop::UiAppTheme`)
    /// over the layers these were resolved from, with no `setTheme`
    /// document active: the same stack and the same resolution as the
    /// core's install (`Tokens::resolve_layers`), the document in place of
    /// these' (the picks it lacks dropped), the overrides as the app's
    /// patch, the host's patch and the roles kept. What a script runtime
    /// reads from the bundle's setup on, before the core applies the op,
    /// so that the entry's code reads the app's theme (TS's `setup`): the
    /// core's own install gives equal tokens, and nothing reads again.
    /// The diagnostics are the core's to log when it installs. These as
    /// they are when they were made of values alone, or neither is given.
    pub fn with_app_theme(
        self: &std::sync::Arc<Self>,
        document: Option<&std::sync::Arc<ThemeDocument>>,
        overrides: Option<&std::sync::Arc<Patch>>,
    ) -> std::sync::Arc<Tokens> {
        let Some(layers) = self.layers() else {
            return self.clone();
        };
        if document.is_none() && overrides.is_none() {
            return self.clone();
        }
        // As the core's `ThemeState::themed` decides it.
        let themed = self.is_themed()
            || document.is_some()
            || overrides.is_some_and(|overrides| !overrides.is_empty());
        let document = document.cloned().unwrap_or_else(|| layers.document.clone());
        let mut selection = layers.selection.clone();
        super::theme::keep_picks(&mut selection, &document);
        let mut patches = layers.patches.clone();
        if let Some(overrides) = overrides {
            // The app's patch is the first, as the core layers them.
            match patches.first_mut() {
                Some(app) => *app = overrides.clone(),
                None => patches.push(overrides.clone()),
            }
        }
        let (mut tokens, _) =
            Tokens::resolve_layers(std::sync::Arc::new(super::tokens::ThemeLayers {
                document,
                selection,
                patches,
                roles: layers.roles.clone(),
            }));
        tokens.set_themed(themed);
        std::sync::Arc::new(tokens)
    }
}
