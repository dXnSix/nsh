//! Rust's theme writes inside a synchronous call from JS, headless:
//! `set_modifier` and `set_color_scheme` called from a `#[class]` member
//! go as the root's ops with the call's turn, after what JS sent before
//! the call, as JS's `setModifier` and `setColorScheme` send theirs. The
//! reactive runtime is not entered in the call.

use nsh_core::{
    Op, Prop, ROOT_ID,
    element::{Registry, runtime_entered, set_color_scheme, set_modifier},
    platform::Appearance,
    prelude::*,
};

use crate::{Host, HostOptions};

/// Writes the theme from its calls.
#[class]
pub(crate) struct Picker;

#[class]
impl Picker {
    /// Gives modifier `name` the context `context`; whether the runtime
    /// was entered.
    pub(crate) fn pick(name: String, context: String) -> bool {
        set_modifier(&name, &context);
        runtime_entered()
    }

    /// Shows the surface dark.
    pub(crate) fn dark() {
        set_color_scheme(Appearance::Dark);
    }
}

fn host() -> Host {
    let mut r = Registry::ui_set("test/theme-in-calls");
    r.class::<Picker>();
    Host::with_options(HostOptions {
        ui_sets: vec![r],
        ..HostOptions::default()
    })
    .unwrap()
}

/// What the ops `code` (a module importing the class) sent hold, in order:
/// a text's content, a root prop's debug form.
fn sent(code: &str) -> Vec<String> {
    let host = host();
    host.eval_source(&format!(
        "import {{ text as show }} from \"nsh:rt\";\nimport {{ Picker }} from \"test/theme-in-calls\";\n{code}"
    ))
    .unwrap();
    let mut ops = Vec::new();
    host.take_ops(&mut ops);
    ops.into_iter()
        .filter_map(|op| match op {
            Op::CreateText { text, .. } => Some(text.to_string()),
            Op::SetProp {
                id: ROOT_ID,
                prop: prop @ (Prop::UiModifier(..) | Prop::Appearance(_)),
            } => Some(format!("{prop:?}")),
            _ => None,
        })
        .collect()
}

#[test]
fn a_write_goes_with_the_turn_in_order_with_what_js_sent_before_the_call() {
    let sent = sent(
        r#"
        show("before");
        show(String(Picker.pick("colorMode", "dark")));
        Picker.dark();
        show("after");
        "#,
    );
    assert_eq!(
        sent,
        [
            "before",
            r#"UiModifier("colorMode", "dark")"#,
            "false",
            "Appearance(Dark)",
            "after"
        ]
    );
}
