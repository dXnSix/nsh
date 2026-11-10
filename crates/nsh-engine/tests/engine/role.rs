//! `role()` from TSX: what a theme role draws with, as `token()` gives a
//! token. Under a design system's theme type it is the token the type maps
//! the role onto; without one, the token of the role's name.

use nsh_core::element::{AdaptiveColor, AppTheme, Registry};
use nsh_engine::Environment;

use crate::components::{shows, start_with, start_with_sets, until};

/// A design system that calls the popover's surface `overlay`.
#[derive(Debug, Clone, PartialEq, AppTheme)]
struct Brand {
    #[token(color, role = nsh_base::role::POPOVER)]
    overlay: AdaptiveColor,
    #[token(color, role = nsh_base::role::WARNING)]
    status_caution: AdaptiveColor,
    #[token(radius, role = nsh_base::role::RADIUS)]
    corner: f32,
}

impl Default for Brand {
    fn default() -> Self {
        Brand {
            overlay: AdaptiveColor::rgb(0x111111, 0xeeeeee),
            status_caution: AdaptiveColor::rgb(0xaa5500, 0xffcc00),
            corner: 6.0,
        }
    }
}

fn brand() -> Vec<Registry> {
    let mut set = Registry::ui_set("brand/native");
    set.theme::<Brand>();
    vec![set]
}

const READS: &str = r##"import { mount, role, signal, Text, token, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ cssVars: { light: { overlay: "#333333" }, dark: { overlay: "#cccccc" } } });
  step.set(1);
}, 30);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`popover ${role("popover")}`}</Text>
    <Text>{`warning ${role("warning")}`}</Text>
    <Text>{`radius ${role("radius")}`}</Text>
    <Text>{`border ${role("border")}`}</Text>
    <Text>{`token ${token("popover" as never)}`}</Text>
  </View>
));
"##;

#[test]
fn a_role_is_the_token_a_theme_type_maps_it_onto() {
    for (appearance, popover, warning, changed) in [
        ("light", "#111111ff", "#aa5500ff", "#333333ff"),
        ("dark", "#eeeeeeff", "#ffcc00ff", "#ccccccff"),
    ] {
        let config = format!(r#"{{ "appearance": "{appearance}" }}"#);
        let (_engine, rx) =
            start_with_sets(READS, Environment::default(), None, Some(&config), brand());
        let batches = until(&rx, |b| shows(b, "step 0"));
        let said = |text: String| batches.iter().any(|b| shows(b, &text));
        assert!(
            said(format!("popover {popover}")),
            "{appearance}: {batches:?}"
        );
        assert!(
            said(format!("warning {warning}")),
            "{appearance}: {batches:?}"
        );
        assert!(said("radius 6".into()), "{appearance}: {batches:?}");
        // A role nothing maps and no token answers, and a role's name as a
        // token: there is none of that name here.
        assert!(said("border undefined".into()), "{appearance}: {batches:?}");
        assert!(said("token undefined".into()), "{appearance}: {batches:?}");
        // The token the role reads changes: what read the role follows.
        let batches = until(&rx, |b| shows(b, "step 1"));
        assert!(
            batches
                .iter()
                .any(|b| shows(b, &format!("popover {changed}"))),
            "{appearance}: {batches:?}"
        );
        // And only it.
        assert!(
            !batches
                .iter()
                .any(|b| shows(b, &format!("warning {warning}"))),
            "{appearance}: {batches:?}"
        );
    }
}

const NAMED: &str = r##"import { mount, role, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ cssVars: { light: { warning: "#b45309" } } });
  step.set(1);
}, 30);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`popover ${role("popover")}`}</Text>
    <Text>{`warning ${role("warning") ?? "its own"}`}</Text>
  </View>
));
"##;

#[test]
fn without_a_mapping_a_role_is_the_token_of_its_name() {
    let (_engine, rx) = start_with(NAMED, Environment::default(), None, None);
    let batches = until(&rx, |b| shows(b, "step 0"));
    // shadcn's `popover`; no `warning` in the default theme.
    assert!(
        batches.iter().any(|b| shows(b, "popover #ffffffff")),
        "{batches:?}"
    );
    assert!(
        batches.iter().any(|b| shows(b, "warning its own")),
        "{batches:?}"
    );
    // The theme gets a `warning` token: the role reads it.
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches.iter().any(|b| shows(b, "warning #b45309ff")),
        "{batches:?}"
    );
}
