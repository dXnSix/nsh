//! The layer stack (CORE-LAYERS, CORE-UPDATE, CORE-SCOPE): picks before the
//! first frame, overrides over contexts, the host over the app, the change
//! set, the app's one patch with its updates and resets, and `ThemeScope`'s
//! picks, patch and appearance.

use nsh_theme::code;

use super::theme_format_store::{bare_core, color, logged, said};
use crate::{
    Color, Command, Core, MountBatch, MountItem, ROOT_ID,
    core::tests::FakeEngine,
    element::{
        Patch, PatchValues, Root, ScopeModifiers, Token, role, set_modifier, theme_scope::scoped,
        token,
    },
    platform::ColorScheme,
    prelude::*,
};

const DOCUMENT: &str = r##"{
  "tokens": {
    "radius-card": { "kind": "number", "value": 8 },
    "gap": { "kind": "number", "value": 4 },
    "blue-9": { "kind": "color", "value": "#0090ff" },
    "cyan-9": { "kind": "color", "value": "#00a2c7" },
    "primary": { "kind": "color", "value": "{blue-9}" },
    "ring": { "kind": "color", "value": "{primary}" },
    "surface": { "kind": "color", "value": "#ffffff" },
    "overlay": { "kind": "color", "value": "#eeeeee" },
    "font-ui": { "kind": "font", "value": "Inter" },
    "label": { "kind": "string", "value": "regular" }
  },
  "modifiers": {
    "density": { "default": "regular",
      "contexts": { "regular": {}, "compact": { "radius-card": 4, "gap": 2 } } },
    "accent": { "default": "blue",
      "contexts": { "blue": {}, "cyan": { "primary": "{cyan-9}" } } },
    "colorMode": { "default": "light",
      "contexts": { "light": {}, "dark": { "surface": "#0a0a0a" } } },
    "voice": { "default": "plain",
      "contexts": { "plain": {}, "loud": { "font-ui": "Impact", "label": "LOUD" } } }
  },
  "resolutionOrder": ["density", "accent", "colorMode", "voice"],
  "roles": { "popover": "surface" }
}"##;

/// A core drawing with [`DOCUMENT`].
fn themed() -> Core<FakeEngine> {
    let mut c = bare_core();
    assert!(c.set_theme(Some(DOCUMENT)).is_empty());
    c.install_theme();
    c
}

fn number(c: &Core<FakeEngine>, name: &str) -> Option<f32> {
    c.tokens().number(name)
}

fn sorted(names: &[String]) -> Vec<&str> {
    let mut names: Vec<&str> = names.iter().map(String::as_str).collect();
    names.sort_unstable();
    names
}

/// The color a binding reads of token `name`, transparent without one.
fn token_color(name: &str) -> crate::Hsla {
    match token(name) {
        Some(Token::Color(c)) => c,
        _ => crate::Hsla::default(),
    }
}

/// A root that picks a context while it is built.
#[component]
fn PicksWhileBuilt() -> impl IntoElement {
    set_modifier("accent", "cyan");
    view().size_10().bg(move || token_color("primary"))
}

/// The last background a batch sets.
fn last_background(batch: &MountBatch) -> Option<Color> {
    batch.items.iter().rev().find_map(|i| match i {
        MountItem::ViewStyle { style, .. } if style.background_color != Color(0) => {
            Some(style.background_color)
        }
        _ => None,
    })
}

#[test]
fn a_pick_while_the_root_is_built_reaches_the_first_mount_batch() {
    let mut c = themed();
    c.commit();
    c.mount_root(&Root::of::<PicksWhileBuilt>());
    let batch = c.commit().unwrap();
    assert_eq!(last_background(&batch), Some(Color(0x00a2_c7ff)));
    assert_eq!(c.tokens().modifier("accent"), Some("cyan"));
}

/// Today's `core/tests.rs` case "an update of a token a context sets"
/// reads 12.0 after `compact` is picked.
#[test]
fn a_picked_context_does_not_override_the_apps_overrides() {
    let mut c = themed();
    c.update_tokens(Patch::new().value("radius-card", 12.0));
    c.install_theme();
    c.set_modifier("density", "compact");
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(12.0));
    // What the context sets and nothing overrides takes its value.
    assert_eq!(number(&c, "gap"), Some(2.0));
}

#[test]
fn an_override_of_a_primitive_reaches_every_token_that_refers_to_it() {
    let mut c = themed();
    c.update_tokens(Patch::new().value("blue-9", crate::Hsla::from(crate::color::rgb(0x3e63dd))));
    c.install_theme();
    assert_eq!(color(c.tokens(), "primary"), Some(Color(0x3e63_ddff)));
    assert_eq!(color(c.tokens(), "ring"), Some(Color(0x3e63_ddff)));
    assert_eq!(
        sorted(&c.theme_changes().tokens),
        ["blue-9", "primary", "ring"]
    );
}

#[test]
fn the_hosts_overrides_win_over_the_apps() {
    let mut c = themed();
    c.update_tokens(Patch::new().value("radius-card", 12.0));
    c.set_host_patch(Some(Patch::new().value("radius-card", 20.0)));
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(20.0));
    c.set_host_patch(None);
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(12.0));
    assert!(
        c.set_host_theme(Some(r#"{ "values": { "gap": 9 } }"#))
            .is_empty()
    );
    c.install_theme();
    assert_eq!(number(&c, "gap"), Some(9.0));
}

#[test]
fn a_modifier_switch_hands_on_exactly_what_resolve_changed() {
    let mut c = themed();
    c.set_modifier("accent", "cyan");
    c.install_theme();
    let changes = c.theme_changes().clone();
    assert_eq!(sorted(&changes.tokens), ["primary", "ring"]);
    assert!(changes.roles.is_empty(), "{changes:?}");
    c.set_modifier("colorMode", "dark");
    c.install_theme();
    let changes = c.theme_changes().clone();
    assert_eq!(sorted(&changes.tokens), ["surface"]);
    assert_eq!(sorted(&changes.roles), ["popover"]);
    // A pick of the context shown changes nothing.
    c.set_modifier("colorMode", "dark");
    assert!(!c.install_theme());
}

#[test]
fn a_patch_that_only_remaps_a_role_is_a_changed_role() {
    let mut c = themed();
    c.update_tokens(Patch::new().role("popover", "overlay"));
    c.install_theme();
    let changes = c.theme_changes().clone();
    assert!(changes.tokens.is_empty(), "{changes:?}");
    assert_eq!(sorted(&changes.roles), ["popover"]);
    assert_eq!(
        c.tokens().role(role::POPOVER).map(Color::from),
        Some(Color(0xeeee_eeff))
    );
}

#[test]
fn a_pick_of_a_modifier_the_document_lacks_logs_and_changes_nothing() {
    let mut c = themed();
    let (installed, lines) = logged(|| {
        c.set_modifier("nope", "x");
        c.install_theme()
    });
    assert!(!installed);
    assert!(said(&lines, code::UNKNOWN_MODIFIER), "{lines:?}");
    assert_eq!(c.tokens().modifier("nope"), None);
}

#[test]
fn a_pick_of_a_context_its_modifier_lacks_logs_and_changes_nothing() {
    let mut c = themed();
    let (installed, lines) = logged(|| {
        c.set_modifier("density", "spacious");
        c.install_theme()
    });
    assert!(!installed);
    assert!(said(&lines, code::UNKNOWN_CONTEXT), "{lines:?}");
    assert_eq!(c.tokens().modifier("density"), Some("regular"));
    assert_eq!(number(&c, "radius-card"), Some(8.0));
}

/// B1: update `radius-card`, reset it, switch the modifier and switch
/// back: no cleared override comes back.
#[test]
fn b1_a_reset_override_stays_gone_across_modifier_switches() {
    let mut c = themed();
    c.update_tokens(Patch::new().value("radius-card", 12.0));
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(12.0));
    c.reset_tokens(Some(&["radius-card".into()]));
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(8.0));
    c.set_modifier("density", "compact");
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(4.0));
    c.set_modifier("density", "regular");
    c.install_theme();
    assert_eq!(number(&c, "radius-card"), Some(8.0));
    assert!(c.app_patch().is_empty());
}

/// B9: a patch's entry for a name the document doesn't declare, or of a
/// value that doesn't fit its type, is reported and skipped.
#[test]
fn b9_a_patch_refuses_undeclared_names_and_values_of_another_type() {
    let mut c = themed();
    let mut patch = Patch::new().value("nope", 1.0).value("gap", 6.0);
    patch.set_value("radius-card", serde_json::json!("#ffffff"));
    let ((), lines) = logged(|| {
        c.update_tokens(patch);
        c.install_theme();
    });
    assert!(said(&lines, code::UNDECLARED_TOKEN), "{lines:?}");
    assert!(said(&lines, code::BAD_VALUE), "{lines:?}");
    assert_eq!(number(&c, "radius-card"), Some(8.0));
    assert_eq!(number(&c, "gap"), Some(6.0));
    assert_eq!(c.tokens().get("nope"), None);
    // Kept as given: a document that declares the name takes it.
    c.set_theme(Some(
        r#"{ "tokens": { "nope": { "kind": "number", "value": 0 } } }"#,
    ));
    c.install_theme();
    assert_eq!(number(&c, "nope"), Some(1.0));
}

#[test]
fn ten_thousand_updates_of_one_name_leave_one_entry() {
    let mut c = themed();
    for i in 0..10_000 {
        c.update_tokens(Patch::new().value("radius-card", i as f32));
    }
    c.install_theme();
    assert_eq!(c.app_patch().values.len(), 1);
    assert_eq!(number(&c, "radius-card"), Some(9_999.0));
}

#[test]
fn a_reset_of_a_role_name_clears_its_mapping() {
    let mut c = themed();
    c.update_tokens(
        Patch::new()
            .role("popover", "overlay")
            .value("surface", crate::Hsla::from(crate::color::rgb(0x111111))),
    );
    c.install_theme();
    assert_eq!(c.tokens().role_name(role::POPOVER), "overlay");
    // The name as a role: its mapping goes, the token `surface` stays.
    c.reset_tokens(Some(&["popover".into()]));
    c.install_theme();
    assert_eq!(c.tokens().role_name(role::POPOVER), "surface");
    assert!(c.app_patch().roles.is_empty());
    assert_eq!(color(c.tokens(), "surface"), Some(Color(0x1111_11ff)));
    // The name as a token.
    c.reset_tokens(Some(&["surface".into()]));
    c.install_theme();
    assert_eq!(color(c.tokens(), "surface"), Some(Color(0xffff_ffff)));
    assert!(c.app_patch().is_empty());
}

#[test]
fn a_full_reset_empties_the_patch_with_the_theme_overrides_entries() {
    let mut c = bare_core();
    assert!(
        c.set_app_theme(
            Some(DOCUMENT),
            Some(r#"{ "values": { "gap": 6 }, "roles": { "popover": "overlay" } }"#),
        )
        .is_empty()
    );
    c.install_theme();
    assert_eq!(number(&c, "gap"), Some(6.0));
    assert_eq!(c.tokens().role_name(role::POPOVER), "overlay");
    // Updates merge after `themeOverrides`' entries.
    c.update_tokens(Patch::new().value("gap", 7.0).value("radius-card", 12.0));
    c.install_theme();
    assert_eq!(number(&c, "gap"), Some(7.0));
    assert_eq!(c.app_patch().values.len(), 2);
    c.reset_tokens(None);
    c.install_theme();
    assert!(c.app_patch().is_empty());
    assert_eq!(number(&c, "gap"), Some(4.0));
    assert_eq!(number(&c, "radius-card"), Some(8.0));
    assert_eq!(c.tokens().role_name(role::POPOVER), "surface");
}

/// B7: a context may set any declared token, of any type.
#[test]
fn b7_a_context_may_set_any_declared_token() {
    let mut c = themed();
    c.set_modifier("voice", "loud");
    c.install_theme();
    assert_eq!(c.tokens().font("font-ui").map(|f| &**f), Some("Impact"));
    assert_eq!(c.tokens().string("label"), Some("LOUD"));
}

/// A root whose build changes the theme three times.
#[component]
fn ThreeActions() -> impl IntoElement {
    update_tokens(Patch::new().value("radius-card", 12.0));
    update_tokens(Patch::new().value("gap", 9.0));
    set_modifier("accent", "cyan");
    view()
}

#[test]
fn every_theme_action_of_a_turn_resolves_once() {
    let mut c = themed();
    while c.commit().is_some() {}
    c.mount_root(&Root::of::<ThreeActions>());
    let batch = c.commit().unwrap();
    let sent = batch
        .items
        .iter()
        .filter(|i| {
            matches!(
                i,
                MountItem::Command {
                    id: ROOT_ID,
                    command: Command::Tokens(_)
                }
            )
        })
        .count();
    assert_eq!(sent, 1);
    assert_eq!(
        sorted(&c.theme_changes().tokens),
        ["gap", "primary", "radius-card", "ring"]
    );
    assert_eq!(number(&c, "radius-card"), Some(12.0));
}

/// B2: an outer scope's patch and the host's overrides survive an inner
/// scope's pick.
#[test]
fn b2_nested_scopes_keep_the_outer_patch_and_the_hosts_under_an_inner_pick() {
    let mut c = themed();
    c.set_host_patch(Some(Patch::new().value("radius-card", 20.0)));
    c.install_theme();
    let (outer, _) = scoped(c.tokens(), None, Some(Patch::new().value("gap", 10.0)));
    let (inner, _) = scoped(
        &outer,
        Some(&ScopeModifiers::new([("accent", "cyan")])),
        None,
    );
    assert_eq!(outer.number("radius-card"), Some(20.0));
    assert_eq!(outer.number("gap"), Some(10.0));
    assert_eq!(color(&outer, "primary"), Some(Color(0x0090_ffff)));
    assert_eq!(inner.number("radius-card"), Some(20.0));
    assert_eq!(inner.number("gap"), Some(10.0));
    assert_eq!(color(&inner, "primary"), Some(Color(0x00a2_c7ff)));
    // The surface is as it was.
    assert_eq!(number(&c, "gap"), Some(4.0));
}

/// B2, with a reference whose target the inner scope picks: references
/// resolve once for the subtree, after every layer.
#[test]
fn b2_a_reference_whose_target_an_inner_scope_picks_resolves_for_its_subtree() {
    let c = themed();
    let (outer, _) = scoped(
        c.tokens(),
        None,
        Some(Patch::new().value("cyan-9", crate::Hsla::from(crate::color::rgb(0x123456)))),
    );
    let (inner, _) = scoped(
        &outer,
        Some(&ScopeModifiers::new([("accent", "cyan")])),
        None,
    );
    assert_eq!(color(&outer, "ring"), Some(Color(0x0090_ffff)));
    assert_eq!(color(&inner, "primary"), Some(Color(0x1234_56ff)));
    assert_eq!(color(&inner, "ring"), Some(Color(0x1234_56ff)));
    assert_eq!(inner.modifier("accent"), Some("cyan"));
    assert_eq!(outer.modifier("accent"), Some("blue"));
}

#[test]
fn a_scopes_bad_patch_entries_and_picks_are_reported_and_left_out() {
    let c = themed();
    let (scope, said) = scoped(
        c.tokens(),
        Some(&ScopeModifiers::new([
            ("nope", "x"),
            ("density", "compact"),
        ])),
        Some(Patch::new().value("missing", 1.0).value("gap", 3.0)),
    );
    let codes: Vec<&str> = said.iter().map(|d| d.code).collect();
    assert!(codes.contains(&code::UNDECLARED_TOKEN), "{codes:?}");
    assert!(codes.contains(&code::UNKNOWN_MODIFIER), "{codes:?}");
    assert_eq!(scope.number("gap"), Some(3.0));
    assert_eq!(scope.number("radius-card"), Some(4.0));
    assert_eq!(scope.get("missing"), None);
}

/// The hex of a color token where it is read.
fn hex(name: &str) -> String {
    match token(name) {
        Some(Token::Color(c)) => format!("{:06x}", Color::from(c).0 >> 8),
        _ => "none".into(),
    }
}

#[component]
fn AloneReader() -> impl IntoElement {
    text(|| format!("alone {} {}", hex("surface"), dark()))
}

#[component]
fn PickedReader() -> impl IntoElement {
    text(|| format!("picked {} {}", hex("surface"), dark()))
}

/// A scope with `appearance` alone, and one that also picks the theme's
/// light/dark modifier.
#[component]
fn Appearances() -> impl IntoElement {
    view()
        .child(
            theme_scope()
                .appearance(Some(ColorScheme::Dark))
                .child(alone_reader()),
        )
        .child(
            theme_scope()
                .appearance(Some(ColorScheme::Dark))
                .modifiers(Some(ScopeModifiers::new([("colorMode", "dark")])))
                .child(picked_reader()),
        )
}

/// The texts of a batch, sorted.
fn texts(batch: &MountBatch) -> Vec<String> {
    let mut texts: Vec<String> = batch
        .items
        .iter()
        .filter_map(|i| match i {
            MountItem::Text { text, .. } => Some(text.text.clone()),
            _ => None,
        })
        .collect();
    texts.sort();
    texts
}

#[test]
fn a_scopes_appearance_alone_sets_its_color_scheme_and_no_token_value() {
    let mut c = themed();
    c.mount_root(&Root::of::<Appearances>());
    let texts = texts(&c.commit().unwrap());
    assert!(texts.contains(&"alone ffffff true".to_owned()), "{texts:?}");
}

#[test]
fn a_scopes_appearance_with_the_light_dark_pick_shows_the_dark_values() {
    let mut c = themed();
    c.mount_root(&Root::of::<Appearances>());
    let texts = texts(&c.commit().unwrap());
    assert!(
        texts.contains(&"picked 0a0a0a true".to_owned()),
        "{texts:?}"
    );
    // The surface around them keeps its own.
    assert_eq!(color(c.tokens(), "surface"), Some(Color(0xffff_ffff)));
}
