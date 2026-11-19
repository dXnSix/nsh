//! Rust's `set_color_scheme` and JS's `colorScheme()`, headless: an
//! appearance a synchronous call from JS sets shows in `colorScheme()` as
//! the call returns, as JS's `setColorScheme` shows its own, and one the
//! core shows (`Host::set_appearance`, which the engine calls when Rust set
//! it from a task or a component) shows there too, with no op back. A
//! reset to `System` shows the system's scheme again.

use nsh_core::{
    Op, Prop, ROOT_ID,
    element::{Registry, set_color_scheme},
    platform::{Appearance, ColorScheme, Environment},
    prelude::*,
};

use crate::{Host, HostOptions};

/// Sets the app's appearance from its calls.
#[module]
pub(crate) struct Scheme;

#[module]
impl Scheme {
    /// Shows the app in `name` (`"light"`, `"dark"` or `"system"`).
    pub(crate) fn set(name: String) {
        set_color_scheme(match name.as_str() {
            "light" => Appearance::Light,
            "dark" => Appearance::Dark,
            _ => Appearance::System,
        });
    }
}

/// A host under a dark system, with `Scheme` in `test/color-scheme`.
fn host() -> Host {
    let mut r = Registry::ui_set("test/color-scheme");
    r.module::<Scheme>();
    let host = Host::with_options(HostOptions {
        ui_sets: vec![r],
        ..HostOptions::default()
    })
    .unwrap();
    host.set_environment(&Environment {
        color_scheme: ColorScheme::Dark,
        ..Environment::default()
    });
    host
}

/// What the ops sent since the last call hold, in order: a text's
/// content, a root appearance's debug form.
fn sent(host: &Host) -> Vec<String> {
    let mut ops = Vec::new();
    host.take_ops(&mut ops);
    ops.into_iter()
        .filter_map(|op| match op {
            Op::CreateText { text, .. } => Some(text.to_string()),
            Op::SetProp {
                id: ROOT_ID,
                prop: prop @ Prop::Appearance(_),
            } => Some(format!("{prop:?}")),
            _ => None,
        })
        .collect()
}

/// The app: an effect that shows `colorScheme()` whenever it changes, then
/// `code`.
fn run(code: &str) -> (Host, Vec<String>) {
    let host = host();
    host.eval_source(&format!(
        r#"import {{ text as show }} from "nsh:rt";
import {{ colorScheme, effect, setColorScheme }} from "nsh";
import {{ Scheme }} from "test/color-scheme";
void setColorScheme;
effect(() => show(`effect ${{colorScheme()}}`));
{code}"#
    ))
    .unwrap();
    let sent = sent(&host);
    (host, sent)
}

#[test]
fn a_call_that_sets_light_under_a_dark_system_shows_light_as_it_returns() {
    let (_host, sent) = run(r#"
        show(`before ${colorScheme()}`);
        Scheme.set("light");
        show(`after ${colorScheme()}`);
        "#);
    assert_eq!(
        sent,
        [
            "effect dark",
            "before dark",
            "Appearance(Light)",
            "effect light",
            "after light"
        ]
    );
}

#[test]
fn the_call_shows_what_js_s_set_color_scheme_shows() {
    let rust = run(r#"Scheme.set("light"); show(colorScheme());"#).1;
    let js = run(r#"void Scheme; setColorScheme("light"); show(colorScheme());"#).1;
    assert_eq!(rust, js);
}

#[test]
fn a_reset_to_the_system_in_a_call_shows_the_system_s_scheme_again() {
    let (_host, sent) = run(r#"
        Scheme.set("light");
        Scheme.set("system");
        show(`reset ${colorScheme()}`);
        "#);
    assert_eq!(
        sent,
        [
            "effect dark",
            "Appearance(Light)",
            "effect light",
            "Appearance(System)",
            "effect dark",
            "reset dark"
        ]
    );
}

#[test]
fn an_appearance_the_core_shows_reaches_color_scheme_with_no_op_back() {
    let (host, _) = run("");
    assert!(host.set_appearance(Appearance::Light));
    assert_eq!(sent(&host), ["effect light"]);
    // Already JS's: nothing runs.
    assert!(!host.set_appearance(Appearance::Light));
    assert_eq!(sent(&host), Vec::<String>::new());
    // Back to the system's.
    assert!(host.set_appearance(Appearance::System));
    assert_eq!(sent(&host), ["effect dark"]);
}

#[test]
fn the_system_s_scheme_changing_after_a_reset_shows() {
    let (host, _) = run(r#"Scheme.set("light"); Scheme.set("system");"#);
    host.set_environment(&Environment {
        color_scheme: ColorScheme::Light,
        ..Environment::default()
    });
    assert_eq!(sent(&host), ["effect light"]);
}
