//! The crate of the rust domain of the API's test apps
//! (docs/dev/compatibility.md; `nsh.json` `"rust": "native"`): a UI set
//! that ships the files of its `assets/` directory, which the domain's
//! cases import by their URLs (`asset:/@compat-rust-native/<path>`).

use nsh_core::prelude::*;

/// The crate's UI set: the files it ships.
#[native_module("compat-rust/native")]
fn native(registry: &mut Registry) {
    registry.assets(assets!());
}
