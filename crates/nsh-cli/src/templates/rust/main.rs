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
    if let Some(theme) = member("theme") {
        root = root.theme(theme);
    }
    if let Some(overrides) = member("themeOverrides") {
        root = root.theme_overrides(overrides);
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

/// `nsh.json`'s `key`, `"theme"` (the theme document) or `"themeOverrides"`
/// (the override patch): a file beside it, or the member inline, which
/// `nsh run` gives the app too.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn member(key: &str) -> Option<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let config: nsh_core::JsonValue = std::fs::read_to_string(dir.join("nsh.json"))
        .ok()?
        .parse()
        .ok()?;
    match config.get(key)? {
        nsh_core::JsonValue::String(file) => std::fs::read_to_string(dir.join(file)).ok(),
        inline => Some(inline.to_string()),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; use the TSX entry with nsh run for iOS and Android."
);
