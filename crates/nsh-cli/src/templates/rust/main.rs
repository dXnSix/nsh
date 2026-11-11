//! Runs the Rust application directly on the machine's native backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    env_logger::init();
    let mut root = platform::Root::of::<__APP_CRATE__::App>();
    if let Some(theme) = theme() {
        root = root.theme(theme);
    }
    platform::run_app(
        platform::AppSource::Rust(root),
        platform::AppOptions {
            title: __APP_TITLE__.to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: __APP_ID__.to_owned(),
            size: (480, 640),
            ..Default::default()
        },
    )
}

/// `nsh.json`'s `"theme"` (a file beside it, or the theme inline), which
/// `nsh run` gives the app too.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn theme() -> Option<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let config: nsh_core::JsonValue = std::fs::read_to_string(dir.join("nsh.json"))
        .ok()?
        .parse()
        .ok()?;
    match config.get("theme")? {
        nsh_core::JsonValue::String(file) => std::fs::read_to_string(dir.join(file)).ok(),
        inline => Some(inline.to_string()),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; use the TSX entry with nsh run for iOS and Android."
);
