//! `role()` from TSX: what a theme role draws with, as `token()` gives a
//! token. Under a design system's theme type it is the token the type maps
//! the role onto; without one, the token of the role's name. A class reads
//! a role as a variable (`bg-(--popover)`).

use nsh_core::{
    Color, MountBatch, MountItem,
    element::{AdaptiveColor, AppTheme, Registry},
};
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

const FILE_ROLES: &str = r##"import { mount, role, setTheme, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
const said = signal("none");
setTimeout(() => {
  updateTheme({ cssVars: { light: { surface: "#eeeeee" } } });
  step.set(1);
}, 30);
setTimeout(() => {
  updateTheme({ cssVars: {}, roles: { warning: "surface" } });
  step.set(2);
}, 60);
setTimeout(() => {
  try {
    setTheme({ cssVars: { light: { surface: "#111111" } }, roles: { popover: "surfce" } });
    said.set("taken");
  } catch (e) {
    said.set(`refused ${(e as Error).name}: ${(e as Error).message}`);
  }
  step.set(3);
}, 90);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`popover ${role("popover")}`}</Text>
    <Text>{`warning ${role("warning") ?? "its own"}`}</Text>
    <Text>{`said ${said()}`}</Text>
  </View>
));
"##;

#[test]
fn a_theme_file_maps_roles_onto_its_tokens() {
    let config = r##"{ "theme": {
        "cssVars": { "light": { "surface": "#fafafa" }, "dark": { "surface": "#171717" } },
        "roles": { "popover": "surface" } } }"##;
    let (_engine, rx) = start_with(FILE_ROLES, Environment::default(), None, Some(config));
    let batches = until(&rx, |b| shows(b, "step 0"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #fafafaff")),
        "{batches:?}"
    );
    // The role follows its token: no copy of the color was taken.
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #eeeeeeff")),
        "{batches:?}"
    );
    // `updateTheme` adds to the roles the theme maps.
    let batches = until(&rx, |b| shows(b, "step 2"));
    assert!(
        batches.iter().any(|b| shows(b, "warning #eeeeeeff")),
        "{batches:?}"
    );
    // A role mapped onto no token: `setTheme` refuses the file whole.
    let batches = until(&rx, |b| shows(b, "step 3"));
    let refused =
        "said refused TypeError: setTheme: `roles.popover`: the theme has no token `surfce`";
    assert!(batches.iter().any(|b| shows(b, refused)), "{batches:?}");
    assert!(
        !batches.iter().any(|b| shows(b, "popover #111111ff")),
        "{batches:?}"
    );
}

const CLASSES: &str = r##"import { mount, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ cssVars: { light: { overlay: "#333333" }, dark: { overlay: "#cccccc" } } });
  step.set(1);
}, 30);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <View class="bg-(--popover) rounded-(--radius) p-2" />
    <View class="bg-(--warning)" />
  </View>
));
"##;

/// The opaque backgrounds and the corner radii the batches gave views, each
/// sorted.
fn drawn(batches: &[MountBatch]) -> (Vec<Color>, Vec<f32>) {
    let styles = || {
        batches
            .iter()
            .flat_map(|b| &b.items)
            .filter_map(|i| match i {
                MountItem::ViewStyle { style, .. } => Some(style),
                _ => None,
            })
    };
    let mut backgrounds: Vec<Color> = styles()
        .map(|s| s.background_color)
        .filter(|c| c.0 & 0xff == 0xff)
        .collect();
    backgrounds.sort_by_key(|c| c.0);
    let mut radii: Vec<f32> = styles()
        .map(|s| s.corner_radius.top_left.unwrap_or(s.border_radius).0)
        .filter(|r| *r > 0.0)
        .collect();
    radii.sort_by(f32::total_cmp);
    (backgrounds, radii)
}

#[test]
fn a_class_reads_a_role_as_a_variable() {
    for (appearance, popover, warning, changed) in [
        ("light", 0x1111_11ff, 0xaa55_00ff_u32, 0x3333_33ff),
        ("dark", 0xeeee_eeff, 0xffcc_00ff, 0xcccc_ccff),
    ] {
        let config = format!(r#"{{ "appearance": "{appearance}" }}"#);
        let (_engine, rx) = start_with_sets(
            CLASSES,
            Environment::default(),
            None,
            Some(&config),
            brand(),
        );
        // The type maps `popover` onto `overlay`, `warning` onto
        // `status-caution` and `radius` onto `corner`.
        let (backgrounds, radii) = drawn(&until(&rx, |b| shows(b, "step 0")));
        let mut wanted = [Color(popover), Color(warning)];
        wanted.sort_by_key(|c| c.0);
        assert_eq!(backgrounds, wanted, "{appearance}");
        assert_eq!(radii, [6.0], "{appearance}");
        // `overlay` changes: the view reading it through the role is
        // restyled, the other is not.
        let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 1")));
        assert_eq!(backgrounds, [Color(changed)], "{appearance}");
    }
}

const MAPPED_LATER: &str = r##"import { mount, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ cssVars: {}, roles: { warning: "surface" } });
  step.set(1);
}, 30);
setTimeout(() => {
  updateTheme({ cssVars: { light: { surface: "#eeeeee" } } });
  step.set(2);
}, 60);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <View class="h-4 bg-(--warning)" />
  </View>
));
"##;

#[test]
fn a_class_takes_a_role_mapped_while_the_app_runs() {
    let config = r##"{ "theme": { "cssVars": { "light": { "surface": "#fafafa" } } } }"##;
    let (_engine, rx) = start_with(MAPPED_LATER, Environment::default(), None, Some(config));
    // Nothing maps `warning` and the theme has no such token: no class yet.
    let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 0")));
    assert_eq!(backgrounds, []);
    // The theme maps it: the class is the token's color, and follows it.
    let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 1")));
    assert_eq!(backgrounds, [Color(0xfafa_faff)]);
    let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 2")));
    assert_eq!(backgrounds, [Color(0xeeee_eeff)]);
}

const RESET: &str = r##"import { mount, resetTheme, role, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ cssVars: { light: { popover: "#333333", "status-caution": "#444444" } } });
  step.set(1);
}, 30);
setTimeout(() => { resetTheme(["popover"]); step.set(2); }, 60);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`popover ${role("popover")}`}</Text>
    <Text>{`warning ${role("warning")}`}</Text>
  </View>
));
"##;

#[test]
fn reset_theme_takes_a_roles_name() {
    let config = r#"{ "appearance": "light" }"#;
    let (_engine, rx) = start_with_sets(RESET, Environment::default(), None, Some(config), brand());
    // Under the type a role's name in a theme file sets `overlay`.
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #333333ff")),
        "{batches:?}"
    );
    assert!(
        batches.iter().any(|b| shows(b, "warning #444444ff")),
        "{batches:?}"
    );
    // And in `resetTheme` it gives `overlay` its default back, and only it.
    let batches = until(&rx, |b| shows(b, "step 2"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #111111ff")),
        "{batches:?}"
    );
    assert!(
        !batches.iter().any(|b| shows(b, "warning #aa5500ff")),
        "{batches:?}"
    );
}
