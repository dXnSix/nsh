//! What reading, checking and resolving a theme report.
//!
//! One condition has one code, reported by one function (the table in
//! `docs/dev/theme-format.md`, "Diagnostics"). An error leaves its entry out
//! and a warning keeps it; nothing else happens on a diagnostic: no panic, no
//! refusal of the whole document.

use std::fmt;

/// How serious a [`Diagnostic`] is.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    /// The entry is left out.
    Error,
    /// The entry is kept (an unknown member is ignored).
    Warning,
}

impl Severity {
    /// `"error"` or `"warning"`.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One problem found in a theme document, a patch or a role check.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// A stable kebab-case code, one of [`code`]'s constants.
    pub code: &'static str,
    /// An RFC 6901 JSON pointer to the entry, into the text as written
    /// (empty for the whole document).
    pub pointer: String,
    /// An English sentence naming the entry.
    pub message: String,
}

impl Diagnostic {
    /// An error with this code, pointer and message.
    pub fn error(
        code: &'static str,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Error,
            code,
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    /// A warning with this code, pointer and message.
    pub fn warning(
        code: &'static str,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    /// Whether this is an error.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// `{code} {pointer}: {message}`, the form the runtime logs.
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.code, self.pointer, self.message)
    }
}

/// The diagnostic codes. They are stable strings: tools match on them.
pub mod code {
    use super::Severity;

    /// The text is not JSON, or not a JSON object.
    pub const NOT_A_DOCUMENT: &str = "not-a-document";
    /// A top-level `cssVars` and no `tokens` (`nsh theme convert`): the old
    /// format, which that command converts.
    pub const LEGACY_FORMAT: &str = "legacy-format";
    /// A JSON object with a member name twice; the first is kept.
    pub const DUPLICATE_MEMBER: &str = "duplicate-member";
    /// A member the format doesn't know; it is ignored.
    pub const UNKNOWN_MEMBER: &str = "unknown-member";
    /// A token name that is empty or holds `{` or `}`.
    pub const BAD_NAME: &str = "bad-name";
    /// A token with a missing or unknown `kind`.
    pub const BAD_KIND: &str = "bad-kind";
    /// A token without `value`.
    pub const MISSING_VALUE: &str = "missing-value";
    /// A section, entry or compound member of the wrong JSON type or shape, a
    /// compound missing a required member, or a value that doesn't fit its
    /// type.
    pub const BAD_VALUE: &str = "bad-value";
    /// A `path` another token already has.
    pub const DUPLICATE_PATH: &str = "duplicate-path";
    /// A context entry or a patch value for a token the document doesn't
    /// declare.
    pub const UNDECLARED_TOKEN: &str = "undeclared-token";
    /// A modifier with fewer than two contexts.
    pub const TOO_FEW_CONTEXTS: &str = "too-few-contexts";
    /// A modifier without `default`, or whose `default` names none of its
    /// contexts.
    pub const BAD_DEFAULT: &str = "bad-default";
    /// A modifier left out of `resolutionOrder`, or `resolutionOrder` missing
    /// (once per modifier).
    pub const ORDER_MISSING: &str = "order-missing";
    /// A `resolutionOrder` item that names no modifier.
    pub const ORDER_UNKNOWN: &str = "order-unknown";
    /// A second listing of a name in `resolutionOrder`.
    pub const ORDER_DUPLICATE: &str = "order-duplicate";
    /// A selected context that doesn't exist; the default is used.
    pub const UNKNOWN_CONTEXT: &str = "unknown-context";
    /// A selected modifier that doesn't exist (the core's `set_modifier`).
    pub const UNKNOWN_MODIFIER: &str = "unknown-modifier";
    /// A reference to an undeclared token.
    pub const REFERENCE_MISSING: &str = "reference-missing";
    /// A reference whose target has the wrong type.
    pub const REFERENCE_TYPE: &str = "reference-type";
    /// A cycle the layers form; its highest entry is left out.
    pub const REFERENCE_CYCLE: &str = "reference-cycle";
    /// A role mapping onto an undeclared token.
    pub const ROLE_TOKEN_MISSING: &str = "role-token-missing";
    /// A role whose token has the wrong type.
    pub const ROLE_TYPE: &str = "role-type";
    /// A required role that resolves to no token.
    pub const ROLE_UNMAPPED: &str = "role-unmapped";
    /// A mapping for a role nothing declares or matches; it is kept.
    pub const ROLE_UNKNOWN: &str = "role-unknown";
    /// A role two sets register with different value types (the core's
    /// `Registry::roles`).
    pub const ROLE_CONFLICT: &str = "role-conflict";
    /// A set's document that another document's precedence leaves unused
    /// (the core's activation).
    pub const DOCUMENT_UNUSED: &str = "document-unused";

    /// Every code with its severity, in the order of the spec's table.
    pub const ALL: &[(&str, Severity)] = &[
        (NOT_A_DOCUMENT, Severity::Error),
        (LEGACY_FORMAT, Severity::Error),
        (DUPLICATE_MEMBER, Severity::Error),
        (UNKNOWN_MEMBER, Severity::Warning),
        (BAD_NAME, Severity::Error),
        (BAD_KIND, Severity::Error),
        (MISSING_VALUE, Severity::Error),
        (BAD_VALUE, Severity::Error),
        (DUPLICATE_PATH, Severity::Error),
        (UNDECLARED_TOKEN, Severity::Error),
        (TOO_FEW_CONTEXTS, Severity::Error),
        (BAD_DEFAULT, Severity::Error),
        (ORDER_MISSING, Severity::Error),
        (ORDER_UNKNOWN, Severity::Error),
        (ORDER_DUPLICATE, Severity::Error),
        (UNKNOWN_CONTEXT, Severity::Warning),
        (UNKNOWN_MODIFIER, Severity::Warning),
        (REFERENCE_MISSING, Severity::Error),
        (REFERENCE_TYPE, Severity::Error),
        (REFERENCE_CYCLE, Severity::Error),
        (ROLE_TOKEN_MISSING, Severity::Error),
        (ROLE_TYPE, Severity::Error),
        (ROLE_UNMAPPED, Severity::Warning),
        (ROLE_UNKNOWN, Severity::Warning),
        (ROLE_CONFLICT, Severity::Warning),
        (DOCUMENT_UNUSED, Severity::Warning),
    ];
}

/// An RFC 6901 JSON pointer from its reference tokens (`~` → `~0`, `/` → `~1`).
pub(crate) fn pointer<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for part in parts {
        out.push('/');
        for c in part.chars() {
            match c {
                '~' => out.push_str("~0"),
                '/' => out.push_str("~1"),
                c => out.push(c),
            }
        }
    }
    out
}
