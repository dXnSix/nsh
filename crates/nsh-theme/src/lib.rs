//! The theme document: its model, reading with diagnostics, writing,
//! editing, layering, references and role checks.
//!
//! A theme document declares tokens (each with a value type and a value),
//! modifiers whose contexts change token values, the order those apply in,
//! and roles that map the names a library reads onto the theme's tokens.
//! The format is specified in `docs/dev/theme-format.md`; this crate is its
//! one implementation: nsh-core, the toolchain and the CLI call it and never
//! re-implement its rules.
//!
//! ```
//! use nsh_theme::{Selection, Value, read, resolve};
//!
//! let text = r##"{
//!   "tokens": {
//!     "gray-900": { "kind": "color", "value": "#111827" },
//!     "gray-50": { "kind": "color", "value": "#f9fafb" },
//!     "surface-app": { "kind": "color", "value": "{gray-50}" }
//!   },
//!   "modifiers": {
//!     "colorMode": {
//!       "default": "light",
//!       "contexts": { "light": {}, "dark": { "surface-app": "{gray-900}" } }
//!     }
//!   },
//!   "resolutionOrder": ["colorMode"]
//! }"##;
//! let read = read(text);
//! assert!(read.diagnostics.is_empty());
//! let dark = resolve(
//!     &read.document,
//!     &Selection::new().with("colorMode", "dark"),
//!     &[],
//! );
//! assert_eq!(
//!     dark.get("surface-app"),
//!     Some(&Value::Color(nsh_theme::Color::rgb(0x11, 0x18, 0x27)))
//! );
//! ```
//!
//! It depends on serde and serde_json only, so an exporter or a tool can use
//! the format with no NSH, UI, platform or JS crate. No input to [`read()`],
//! [`read_patch`], [`resolve()`] or [`check_roles`] panics: bad entries are
//! left out and reported as [`Diagnostic`]s.

pub mod diagnostic;
pub mod document;
mod json;
pub mod read;
pub mod resolve;
pub mod roles;
pub mod value;
pub mod write;

pub use diagnostic::{Diagnostic, Severity, code};
pub use document::{Context, Map, Modifier, ThemeDocument, Token};
pub use read::{Read, check, object_members, read, read_patch};
pub use resolve::{Changes, Patch, Resolved, Selection, resolve};
pub use roles::{RoleDeclaration, RolePattern, check_roles, resolve_role};
pub use value::{
    Blur, BlurKind, Border, BorderStyle, Color, Duration, Easing, EasingKeyword, Font, Gradient,
    GradientKind, GradientStop, LineHeight, Member, Shadow, ShadowLayer, Transition, Typography,
    Value, ValueType,
};
pub use write::{write, write_patch};
