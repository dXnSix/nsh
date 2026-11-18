//! A UI set that registers one required role, `banner`: a theme that maps
//! it nowhere, and has no token of its name, leaves it unmapped
//! (`role-unmapped`).

use nsh_core::prelude::*;

/// The set's roles.
pub mod role {
    nsh_core::roles! {
        /// The banner's surface.
        pub BANNER: nsh_core::element::Role = "banner" required,
    }
}

/// The crate's UI set (`nsh.json` `"rust"`): its roles.
#[native_module("theme-format-role-app/native")]
fn native(registry: &mut Registry) {
    registry.roles(role::all());
}
