//! `role()` from TSX: what a theme role draws with, as `token()` gives a
//! token. Under a design system's theme document it is the token the
//! document maps the role onto; without a mapping, the token of the role's
//! name. A class reads a role as a variable (`bg-(--popover)`).

use nsh_core::{Color, MountBatch, MountItem, element::Registry};
use nsh_engine::Environment;

use crate::components::{shows, start_with, start_with_sets, until};

/// A design system that calls the popover's surface `overlay`, with dark
/// values in its `colorMode` modifier.
const BRAND: &str = r##"{
    "tokens": {
        "overlay": { "kind": "color", "value": "#111111" },
        "status-caution": { "kind": "color", "value": "#aa5500" },
        "corner": { "kind": "number", "value": 6 } },
    "modifiers": { "colorMode": { "default": "light", "contexts": {
        "light": {}, "dark": { "overlay": "#eeeeee", "status-caution": "#ffcc00" } } } },
    "resolutionOrder": ["colorMode"],
    "roles": { "popover": "overlay", "warning": "status-caution", "radius": "corner" }
}"##;

fn brand() -> Vec<Registry> {
    let mut set = Registry::ui_set("brand/native");
    set.theme_document(BRAND);
    vec![set]
}

/// `app` with the design system's `colorMode` picked as `mode` first.
fn in_mode(app: &str, mode: &str) -> String {
    app.replace("__MODE__", mode)
}

const READS: &str = r##"import { mount, role, setModifier, signal, Text, token, updateTheme, View } from "nsh";

setModifier("colorMode", "__MODE__");
const step = signal(0);
setTimeout(() => {
  updateTheme({ values: { overlay: "#333333" } });
  step.set(1);
}, 30);

mount(() => (
  <View>
    <Text>{`step ${step()}`}</Text>
    <Text>{`popover ${role("popover")}`}</Text>
    <Text>{`warning ${role("warning")}`}</Text>
    <Text>{`radius ${role("radius")}`}</Text>
    <Text>{`border ${role("border")}`}</Text>
    <Text>{`token ${token("popover")}`}</Text>
  </View>
));
"##;

#[test]
fn a_role_is_the_token_a_theme_document_maps_it_onto() {
    for (mode, popover, warning) in [
        ("light", "#111111", "#aa5500"),
        ("dark", "#eeeeee", "#ffcc00"),
    ] {
        let app = in_mode(READS, mode);
        let (_engine, rx) = start_with_sets(&app, Environment::default(), None, None, brand());
        let batches = until(&rx, |b| shows(b, "step 0"));
        let said = |text: String| batches.iter().any(|b| shows(b, &text));
        assert!(said(format!("popover {popover}")), "{mode}: {batches:?}");
        assert!(said(format!("warning {warning}")), "{mode}: {batches:?}");
        assert!(said("radius 6".into()), "{mode}: {batches:?}");
        // A role nothing maps and no token answers, and a role's name as a
        // token: there is none of that name here.
        assert!(said("border undefined".into()), "{mode}: {batches:?}");
        assert!(said("token undefined".into()), "{mode}: {batches:?}");
        // The token the role reads changes (an override wins over the
        // context): what read the role follows.
        let batches = until(&rx, |b| shows(b, "step 1"));
        assert!(
            batches.iter().any(|b| shows(b, "popover #333333")),
            "{mode}: {batches:?}"
        );
        // And only it.
        assert!(
            !batches
                .iter()
                .any(|b| shows(b, &format!("warning {warning}"))),
            "{mode}: {batches:?}"
        );
    }
}

const NAMED: &str = r##"import { mount, role, setTheme, signal, Text, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  setTheme({
    tokens: {
      popover: { kind: "color", value: "#fafafa" },
      warning: { kind: "color", value: "#b45309" },
    },
  });
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
    // `nsh-base`'s fallback document's `popover`; it has no `warning`.
    assert!(
        batches.iter().any(|b| shows(b, "popover #ffffff")),
        "{batches:?}"
    );
    assert!(
        batches.iter().any(|b| shows(b, "warning its own")),
        "{batches:?}"
    );
    // A document with a `warning` token: the role reads it.
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #fafafa")),
        "{batches:?}"
    );
    assert!(
        batches.iter().any(|b| shows(b, "warning #b45309")),
        "{batches:?}"
    );
}

const FILE_ROLES: &str = r##"import { mount, role, setTheme, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
const said = signal("none");
setTimeout(() => {
  updateTheme({ values: { surface: "#eeeeee" } });
  step.set(1);
}, 30);
setTimeout(() => {
  updateTheme({ roles: { warning: "surface" } });
  step.set(2);
}, 60);
setTimeout(() => {
  const diagnostics = setTheme({
    tokens: { surface: { kind: "color", value: "#111111" } },
    roles: { popover: "surfce" },
  });
  said.set(diagnostics.map((d) => `${d.code} ${d.pointer}`).join(", "));
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
        "tokens": { "surface": { "kind": "color", "value": "#fafafa" } },
        "roles": { "popover": "surface" } } }"##;
    let (_engine, rx) = start_with(FILE_ROLES, Environment::default(), None, Some(config));
    let batches = until(&rx, |b| shows(b, "step 0"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #fafafa")),
        "{batches:?}"
    );
    // The role follows its token: no copy of the color was taken.
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #eeeeee")),
        "{batches:?}"
    );
    // `updateTheme` adds to the roles the document maps.
    let batches = until(&rx, |b| shows(b, "step 2"));
    assert!(
        batches.iter().any(|b| shows(b, "warning #eeeeee")),
        "{batches:?}"
    );
    // A role mapped onto no token: the mapping is left out and returned,
    // and the document is active with the rest. The app's overrides stay:
    // its `surface` value, its `warning` mapping.
    let batches = until(&rx, |b| shows(b, "step 3"));
    assert!(
        batches
            .iter()
            .any(|b| shows(b, "said role-token-missing /roles/popover")),
        "{batches:?}"
    );
    assert!(
        batches.iter().any(|b| shows(b, "popover undefined")),
        "{batches:?}"
    );
    assert!(
        !batches.iter().any(|b| shows(b, "popover #111111")),
        "{batches:?}"
    );
}

const CLASSES: &str = r##"import { mount, setModifier, signal, Text, updateTheme, View } from "nsh";

setModifier("colorMode", "__MODE__");
const step = signal(0);
setTimeout(() => {
  updateTheme({ values: { overlay: "#333333" } });
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
    for (mode, popover, warning) in [
        ("light", 0x1111_11ff, 0xaa55_00ff_u32),
        ("dark", 0xeeee_eeff, 0xffcc_00ff),
    ] {
        let app = in_mode(CLASSES, mode);
        let (_engine, rx) = start_with_sets(&app, Environment::default(), None, None, brand());
        // The document maps `popover` onto `overlay`, `warning` onto
        // `status-caution` and `radius` onto `corner`.
        let (backgrounds, radii) = drawn(&until(&rx, |b| shows(b, "step 0")));
        let mut wanted = [Color(popover), Color(warning)];
        wanted.sort_by_key(|c| c.0);
        assert_eq!(backgrounds, wanted, "{mode}");
        assert_eq!(radii, [6.0], "{mode}");
        // `overlay` changes: the view reading it through the role is
        // restyled, the other is not.
        let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 1")));
        assert_eq!(backgrounds, [Color(0x3333_33ff)], "{mode}");
    }
}

const MAPPED_LATER: &str = r##"import { mount, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ roles: { warning: "surface" } });
  step.set(1);
}, 30);
setTimeout(() => {
  updateTheme({ values: { surface: "#eeeeee" } });
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
    let config =
        r##"{ "theme": { "tokens": { "surface": { "kind": "color", "value": "#fafafa" } } } }"##;
    let (_engine, rx) = start_with(MAPPED_LATER, Environment::default(), None, Some(config));
    // Nothing maps `warning` and the document has no such token: no class
    // yet.
    let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 0")));
    assert_eq!(backgrounds, []);
    // The app maps it: the class is the token's color, and follows it.
    let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 1")));
    assert_eq!(backgrounds, [Color(0xfafa_faff)]);
    let (backgrounds, _) = drawn(&until(&rx, |b| shows(b, "step 2")));
    assert_eq!(backgrounds, [Color(0xeeee_eeff)]);
}

const RESET: &str = r##"import { mount, resetTheme, role, signal, Text, updateTheme, View } from "nsh";

const step = signal(0);
setTimeout(() => {
  updateTheme({ roles: { popover: "status-caution" }, values: { "status-caution": "#444444" } });
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

/// `resetTheme` of a role's name removes the app's mapping of the role:
/// it reads the document's mapping again, and the rest of the patch stays.
#[test]
fn reset_theme_takes_a_roles_name() {
    let (_engine, rx) = start_with_sets(RESET, Environment::default(), None, None, brand());
    let batches = until(&rx, |b| shows(b, "step 1"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #444444")),
        "{batches:?}"
    );
    assert!(
        batches.iter().any(|b| shows(b, "warning #444444")),
        "{batches:?}"
    );
    let batches = until(&rx, |b| shows(b, "step 2"));
    assert!(
        batches.iter().any(|b| shows(b, "popover #111111")),
        "{batches:?}"
    );
    assert!(
        !batches.iter().any(|b| shows(b, "warning #aa5500")),
        "{batches:?}"
    );
}
