//! Rust's `set_color_scheme` and JS's `colorScheme()` through an engine,
//! under a dark system: an appearance a synchronous call from JS sets (a
//! module function before `mount`) and one a core task sets later (the
//! reactive runtime entered) both reach `colorScheme()` as JS's
//! `setColorScheme` does, and a reset to `System` shows the system's
//! scheme again.

use std::time::Duration;

use crossbeam_channel::Receiver;
use nsh_core::{
    MountBatch, MountItem,
    element::{Registry, runtime_entered, set_color_scheme},
    platform::{Appearance, ColorScheme},
    prelude::*,
};
use nsh_engine::Environment;

use crate::components::start_with_sets;

/// `"light"`, `"dark"`, else `System`.
fn appearance_named(name: &str) -> Appearance {
    match name {
        "light" => Appearance::Light,
        "dark" => Appearance::Dark,
        _ => Appearance::System,
    }
}

/// Sets the app's appearance from Rust.
#[module]
pub(crate) struct Scheme;

#[module]
impl Scheme {
    /// Shows the app in `name` now, in the call.
    pub(crate) fn set(name: String) {
        set_color_scheme(appearance_named(&name));
    }

    /// Starts a task that shows the app in `name` after `ms`
    /// milliseconds, from the reactive runtime; whether it started.
    pub(crate) fn later(name: String, ms: u32) -> bool {
        start_task(async move {
            timer(Duration::from_millis(u64::from(ms))).await;
            assert!(runtime_entered());
            set_color_scheme(appearance_named(&name));
        })
        .is_ok()
    }
}

fn sets() -> Vec<Registry> {
    let mut scheme = Registry::ui_set("test/scheme");
    scheme.module::<Scheme>();
    vec![scheme]
}

/// The app: `setup` (the entry's top level, before `mount`), then a text
/// of `colorScheme()` and one of what it read at the top level.
fn app(setup: &str) -> String {
    format!(
        r#"import {{ colorScheme, mount, setColorScheme, Text, View }} from "nsh";
import {{ Scheme }} from "test/scheme";

void Scheme;
void setColorScheme;
const before = colorScheme();
{setup}
const after = colorScheme();
mount(() => (
  <View>
    <Text>{{`top ${{before}} ${{after}}`}}</Text>
    <Text>{{`scheme ${{colorScheme()}}`}}</Text>
  </View>
));
"#
    )
}

/// The app `setup` starts, under a dark system.
fn start(setup: &str) -> (nsh_engine::Engine, Receiver<MountBatch>) {
    let environment = Environment {
        color_scheme: ColorScheme::Dark,
        ..Environment::default()
    };
    start_with_sets(&app(setup), environment, None, None, sets())
}

/// The texts `batch` shows.
fn texts(batch: &MountBatch) -> Vec<String> {
    batch
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::Text { text, .. } => Some(text.text.to_string()),
            _ => None,
        })
        .collect()
}

/// The appearances `batch` sets.
fn appearances(batch: &MountBatch) -> Vec<Appearance> {
    batch
        .items
        .iter()
        .filter_map(|item| match item {
            MountItem::Appearance { appearance } => Some(*appearance),
            _ => None,
        })
        .collect()
}

/// The texts and appearances of the batches within a moment.
fn collect(rx: &Receiver<MountBatch>) -> (Vec<String>, Vec<Appearance>) {
    let (mut shown, mut set) = (Vec::new(), Vec::new());
    while let Ok(batch) = rx.recv_timeout(Duration::from_millis(500)) {
        shown.extend(texts(&batch));
        set.extend(appearances(&batch));
    }
    (shown, set)
}

#[test]
fn a_call_that_sets_light_under_a_dark_system_shows_light_in_color_scheme() {
    let (_engine, rx) = start(r#"Scheme.set("light");"#);
    let (shown, set) = collect(&rx);
    assert_eq!(shown, ["top dark light", "scheme light"]);
    assert_eq!(set, [Appearance::Light]);
}

#[test]
fn the_call_shows_what_js_s_set_color_scheme_shows() {
    let (_rust, rust) = start(r#"Scheme.set("light");"#);
    let (_js, js) = start(r#"setColorScheme("light");"#);
    assert_eq!(collect(&rust), collect(&js));
}

#[test]
fn a_reset_to_the_system_in_a_call_shows_the_system_s_scheme() {
    let (_engine, rx) = start(r#"Scheme.set("light"); Scheme.set("system");"#);
    let (shown, _) = collect(&rx);
    assert_eq!(shown, ["top dark dark", "scheme dark"]);
}

#[test]
fn a_task_that_sets_light_reaches_color_scheme_and_a_reset_the_system_s() {
    let (_engine, rx) = start(r#"Scheme.later("light", 50); Scheme.later("system", 250);"#);
    let (shown, set) = collect(&rx);
    assert_eq!(
        shown,
        [
            "top dark dark",
            "scheme dark",
            "scheme light",
            "scheme dark"
        ]
    );
    assert_eq!(set, [Appearance::Light, Appearance::System]);
}
