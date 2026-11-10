//! The TV's side: `ui_app_main` through `dxn_gpui_tizen::run`, the package
//! in the app's resources, and the app framework's directories. Compiled
//! only for `target_vendor = "tizen"`.

use std::{
    ffi::{CStr, c_char, c_int},
    path::PathBuf,
};

use dxn_gpui_tizen_sys as sys;
use nsh_platform_gpui::{AppOptions, AssetBase, CacheLocation, CacheOptions, Limits, Root};

use crate::{
    app::{self, Source},
    package::{self, Package},
};

/// Runs the app until it exits: the package's bundle, or the Rust app
/// `root` makes.
///
/// # Safety
///
/// `argv` must point to `argc` NUL-terminated strings that stay valid until
/// this returns (the C# host's).
pub(crate) unsafe fn main(
    argc: c_int,
    argv: *mut *mut c_char,
    root: Option<fn() -> Root>,
) -> c_int {
    let options = dxn_gpui_tizen::RunOptions::default();
    // SAFETY: argv as this function's caller guarantees.
    unsafe { dxn_gpui_tizen::run(argc, argv, options, move |cx| launch(root, cx)) }
}

/// Opens the window with what the package runs.
fn launch(root: Option<fn() -> Root>, cx: &mut gpui::App) {
    let Some(dir) = dxn_gpui_tizen::resource_path(package::DIR) else {
        log::error!("the app framework names no resource directory");
        cx.quit();
        return;
    };
    let package = Package::read(&dir);
    let mut options = options(&package);
    match source(root, &package, &mut options) {
        Some(source) => app::launch(source, options, package.name, cx),
        None => cx.quit(),
    }
}

/// The package's bundle (a newer signed package of it when one was
/// accepted, with its assets), or the Rust app's root; `None` when the
/// bundle cannot be read.
fn source(
    root: Option<fn() -> Root>,
    package: &Package,
    options: &mut AppOptions,
) -> Option<Source> {
    if let Some(root) = root {
        return Some(Source::App(nsh_platform_gpui::App::Rust(root())));
    }
    // SAFETY: the bytecode is a resource of the app's own signed package.
    let bundle = match unsafe { nsh_platform_gpui::Bundle::read_trusted(&package.bundle()) } {
        Ok(bundle) => bundle,
        Err(err) => {
            log::error!(
                "cannot read the app's bundle {}: {err}",
                package.bundle().display()
            );
            return None;
        }
    };
    let launch = nsh_engine::updates::launch(bundle, &package.dir, options.data_dir.as_deref());
    options.asset_base = Some(AssetBase::Directory(launch.assets));
    Some(Source::App(nsh_platform_gpui::App::Js(launch.bundle)))
}

/// The view's options for `package`: its id and appearance, its assets, the
/// app framework's data and cache directories, and no VM limits (the app
/// runs on its own, as Linux's packaged apps do).
fn options(package: &Package) -> AppOptions {
    let mut options = AppOptions {
        asset_base: Some(AssetBase::Directory(package.assets())),
        limits: Limits::standalone(),
        appearance: package.appearance,
        data_dir: app_dir(sys::app_get_data_path).map(|dir| dir.join("nsh")),
        ..AppOptions::default()
    };
    if let Some(cache) = app_dir(sys::app_get_cache_path) {
        options.cache = CacheOptions::at(CacheLocation::Dir(cache.join("nsh")));
    }
    if let Some(id) = &package.id {
        options.app_id = id.clone();
    }
    if let Some(name) = &package.name {
        options.title = name.clone();
    }
    options
}

/// A directory the app framework names (`app_get_data_path`, ...).
fn app_dir(get: unsafe extern "C" fn() -> *mut c_char) -> Option<PathBuf> {
    // SAFETY: the app framework's getters take nothing and return a malloc'd
    // string or NULL (after `ui_app_main` started the app).
    malloced(unsafe { get() }).map(PathBuf::from)
}

/// The text of a malloc'd C string, which is freed.
pub(crate) fn malloced(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated string the callee malloc'd, freed once here.
    unsafe {
        let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        libc::free(ptr.cast());
        Some(text)
    }
}
