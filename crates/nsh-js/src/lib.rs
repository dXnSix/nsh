//! QuickJS host for nsh.
//!
//! [`Host`] owns one QuickJS runtime and context, installs the globals
//! apps use (`console`, timers, web APIs such as `fetch`), serves nsh's
//! native modules (`nsh`, `nsh:rt`, `nsh/*`: the reactive graph, the
//! renderer, components), queues what the app changes as
//! [`nsh_core::Op`]s and dispatches native events back into JS.
//!
//! A host is single-threaded (`!Send`): create and use it on the engine
//! thread.

mod animated;
#[cfg(test)]
mod between_turns;
mod bytecode;
#[cfg(test)]
mod can_call_now;
mod canvas;
mod chunks;
#[cfg(test)]
mod class_extension_types;
#[cfg(test)]
mod class_extensions;
mod classes;
mod clock;
#[cfg(test)]
mod color_scheme_from_rust;
mod components;
mod console;
#[cfg(test)]
mod constructors;
mod declarations;
mod desktop;
mod dev_source;
mod dom;
mod env;
mod error;
mod host;
mod host_channel;
mod hot;
mod i18n;
#[cfg(test)]
mod interface_signatures;
#[cfg(test)]
mod interfaces;
#[cfg(test)]
mod js_handles;
mod keymap;
mod list;
mod modules;
mod navigation;
mod net;
mod platform_modules;
mod protocol;
mod reactive;
mod rejections;
mod resource;
mod sandbox;
mod standard;
#[cfg(test)]
mod start_task;
mod state;
#[cfg(test)]
mod theme_in_calls;
mod ties;
mod timers;
mod touch;
mod web;
#[cfg(test)]
mod web_evaluation;
mod web_page;
mod window;

// The typed API check (docs/dev/geometry.md, "Typed API"): the scan is
// nsh-core's; this crate's lists are `unitless.txt` and `untyped.txt`.
#[cfg(test)]
#[path = "../../nsh-core/src/geometry/unit_scan.rs"]
mod unit_scan;

#[cfg(test)]
#[test]
fn public_api_is_typed() {
    unit_scan::check(std::path::Path::new(env!("CARGO_MANIFEST_DIR")), &[]);
}

pub use nsh_core::{
    js::Limits,
    platform::{ColorScheme, Environment, Idiom, Os},
};

pub use crate::{
    bytecode::{
        BUNDLE_NAME, BYTECODE_VERSION, Strip, compile_bytecode, compile_bytecode_with,
        compile_chunk_bytecode,
    },
    declarations::type_definitions,
    dev_source::{InstanceRef, NodeSource, PropValue},
    error::JsError,
    host::{ErrorHandler, Host, HostOptions, LineExtentMeasurer, TextMeasurer},
    net::{FetchEvent, FetchOp, HttpBody, HttpHead, HttpRequest, NetworkQueue, RequestId},
};

/// Marks an engine context to mount plain JS roots on the host's surface
/// instead of opening an implicit desktop window. Explicit windows and
/// the platform environment are unchanged.
#[doc(hidden)]
#[must_use]
pub fn with_inline_root(
    context: nsh_core::element::EngineContext,
) -> nsh_core::element::EngineContext {
    context.with(dom::InlineRoot)
}

/// The names native module `specifier` exports, if it is one: nsh's own
/// modules, the runtime's and those of `ui_sets` (a bundler checks named
/// imports against them).
pub fn native_module_exports(
    specifier: &str,
    ui_sets: &[nsh_core::element::description::SetDescription],
) -> Option<Vec<String>> {
    modules::ModuleTable::compiling(ui_sets).export_names(specifier)
}
