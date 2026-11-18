//! Theme documents for the tests: an app's own theme, `nsh/base`'s default
//! with tokens of the app's added, as an app writes one in the theme format
//! (a patch never adds a token: the document declares it).

use nsh_core::{Color, element::AdaptiveColor};
use serde_json::{Value, json};

/// A token an app's theme adds.
pub(crate) enum Add {
    /// A color, its dark value in `colorMode`'s `dark` context when it
    /// differs.
    Color(AdaptiveColor),
    /// A number.
    Number(f32),
    /// A font family list.
    Font(&'static str),
}

/// The color's `#rrggbbaa`.
fn hex(color: nsh_core::Hsla) -> String {
    format!("#{:08x}", Color::from(color).0)
}

/// `nsh/base`'s default theme document with the tokens `add` declared and
/// the roles `roles` mapped (role, token).
pub(crate) fn default_with(add: &[(&str, Add)], roles: &[(&str, &str)]) -> String {
    let mut document: Value =
        serde_json::from_str(nsh_base::DEFAULT_THEME).expect("the default theme is JSON");
    for (name, token) in add {
        let (kind, value, dark) = match token {
            Add::Color(c) => (
                "color",
                json!(hex(c.light)),
                (c.dark != c.light).then(|| json!(hex(c.dark))),
            ),
            Add::Number(n) => ("number", json!(n), None),
            Add::Font(family) => ("font", json!(family), None),
        };
        document["tokens"][*name] = json!({ "kind": kind, "value": value });
        if let Some(dark) = dark {
            document["modifiers"]["colorMode"]["contexts"]["dark"][*name] = dark;
        }
    }
    for (role, token) in roles {
        document["roles"][*role] = json!(token);
    }
    document.to_string()
}
