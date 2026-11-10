//! Samsung TV backend for nsh (Tizen): `libnsh.so`, the library a TPK's C#
//! host calls with its one P/Invoke, `nsh_tizen_main(argc, argv)`.
//!
//! It joins two pieces: NSH's GPUI backend (`nsh-platform-gpui`'s
//! `AppView`, which draws the app, takes its input and runs its engine) and
//! dXn.GpuiTizen's Tizen platform for GPUI (`dxn_gpui_tizen`: `ui_app_main`
//! and Ecore's main loop, the one full-screen window, EGL, the remote's
//! keys, the on-screen keyboard, the app's lifecycle, the TV's settings).
//! What it adds is the TV app around the view:
//!
//! - `app`: the window, the TV's settings given to `AppView` as they
//!   change (`settings`), the app framework's pause and resume as the
//!   app's state (`lifecycle`), the question whether to leave when
//!   nothing takes Back (`back`);
//! - `keys`: the remote's key names as NSH reads them;
//! - `package`: the app in the TPK's resources;
//! - `tv`: the TV's side of `nsh/tizen` (`system_info`'s product
//!   information, the remote's keys grabbed while the app runs);
//! - `tizen` (Tizen only): the entry point and the app framework's
//!   directories and log;
//! - `dev` (feature `dev`): the development runtime, connected through
//!   `sdb reverse` or `sdb forward`.
//!
//! The logic builds on the desktops too (its tests run there); off the
//! desktops and Tizen the crate is empty. `docs/dev/tizen.md` has the
//! builds.

#![cfg(any(target_os = "linux", target_os = "macos", windows))]

mod app;
mod back;
#[cfg(feature = "dev")]
mod dev;
mod keys;
mod lifecycle;
mod package;
mod settings;
#[cfg(target_vendor = "tizen")]
mod tizen;
mod tv;

pub use nsh_platform_gpui::set_ui_sets;

/// Runs the app packaged with the library (or, built with `dev`, the
/// development runtime) until it exits: `nsh_tizen_main`'s body, for the
/// host of an app with Rust components of its own, which calls
/// [`set_ui_sets`] first.
///
/// # Safety
///
/// `argv` must point to `argc` NUL-terminated strings that stay valid until
/// it returns, as the C# host passes them.
#[cfg(target_vendor = "tizen")]
pub unsafe fn main(argc: core::ffi::c_int, argv: *mut *mut core::ffi::c_char) -> core::ffi::c_int {
    // SAFETY: as this function's caller guarantees.
    unsafe { tizen::main(argc, argv, None) }
}

/// Runs the Rust app whose root `root` makes (`nsh.json` `"root"`) until it
/// exits, with the package's assets, id and name.
///
/// # Safety
///
/// As [`main`].
#[cfg(target_vendor = "tizen")]
pub unsafe fn run_rust(
    argc: core::ffi::c_int,
    argv: *mut *mut core::ffi::c_char,
    root: fn() -> nsh_platform_gpui::Root,
) -> core::ffi::c_int {
    // SAFETY: as this function's caller guarantees.
    unsafe { tizen::main(argc, argv, Some(root)) }
}

/// The TV library's one export: the C# host's P/Invoke.
///
/// # Safety
///
/// Called by the host with `argc` NUL-terminated strings in `argv`.
#[cfg(target_vendor = "tizen")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsh_tizen_main(
    argc: core::ffi::c_int,
    argv: *mut *mut core::ffi::c_char,
) -> core::ffi::c_int {
    // SAFETY: argv comes from the host as documented above.
    unsafe { main(argc, argv) }
}
