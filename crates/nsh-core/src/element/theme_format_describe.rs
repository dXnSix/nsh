//! Describe in the theme format (TOOL-TYPES): `FORMAT` 7 on (8 now, with
//! served sets) carries each set's theme documents with the fallback marked
//! and its role declarations with value type, doc and `required`; the
//! toolchain takes the active document as the core does; a `FORMAT` 6
//! description is refused, naming both formats.

use serde_json::Value;

use super::description::{Description, FORMAT, SetDescription, ThemeDocumentDescription, VERSION};
use crate::element::{Registry, RoleDeclaration, ValueType};

const OWN: &str = r##"{ "tokens": { "panel": { "kind": "color", "value": "#222222" } } }"##;

const FALLBACK: &str = r##"{
    "tokens": { "surface": { "kind": "color", "value": "#ffffff" } },
    "roles": { "popover": "surface" }
}"##;

#[allow(unreachable_pub, reason = "as a UI set's crate writes its roles")]
mod kit {
    use crate::element::{NumberRole, Role};

    crate::roles! {
        /// A banner's surface.
        pub BANNER: Role = "banner" required,
        pub CHIP_RADIUS: NumberRole = "chip-radius",
    }
}

fn set(
    specifier: &'static str,
    own: Option<&'static str>,
    fallback: Option<&'static str>,
) -> Registry {
    let mut registry = Registry::ui_set(specifier);
    if let Some(own) = own {
        registry.theme_document(own);
    }
    if let Some(fallback) = fallback {
        registry.fallback_theme_document(fallback);
    }
    registry
}

#[test]
fn the_format_is_8() {
    assert_eq!(FORMAT, 8);
    let text = Description::of(&[set("ds/native", Some(OWN), None)]).to_json();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["format"], 8);
}

#[test]
fn a_format_6_description_is_refused_naming_both_formats() {
    let text = Description::of(&[set("ds/native", Some(OWN), None)]).to_json();
    let mut value: Value = serde_json::from_str(&text).unwrap();
    value["format"] = 6.into();
    let error = Description::from_json(&value.to_string()).unwrap_err();
    assert!(error.contains("format 6"), "{error}");
    assert!(error.contains(&format!("format {FORMAT}")), "{error}");
    // A format 6 set's documents were plain strings: no reader of a later
    // format takes them for documents.
    value["format"] = FORMAT.into();
    value["sets"][0]["theme_documents"] = serde_json::json!([OWN]);
    let error = Description::from_json(&value.to_string()).unwrap_err();
    assert!(
        error.contains("theme_documents[0] is not an object"),
        "{error}"
    );
}

#[test]
fn a_sets_documents_are_described_with_the_fallback_marked() {
    let registry = set("ds/native", Some(OWN), Some(FALLBACK));
    let described = SetDescription::of(&registry).unwrap();
    assert_eq!(
        described.theme_documents,
        [
            ThemeDocumentDescription::new(OWN, false),
            ThemeDocumentDescription::new(FALLBACK, true),
        ]
    );
    let description = Description {
        nsh: VERSION.into(),
        sets: vec![described],
    };
    let text = description.to_json();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["sets"][0]["theme_documents"][1]["fallback"], true);
    assert_eq!(Description::from_json(&text).unwrap(), description);
    // A set with a fallback alone, as `nsh/base` is, describes it so.
    let base = SetDescription::of(&nsh_base_like()).unwrap();
    assert!(base.theme_documents.iter().all(|d| d.fallback));
}

/// A set that registers only a fallback, as `nsh/base` does.
fn nsh_base_like() -> Registry {
    set("base/native", None, Some(FALLBACK))
}

#[test]
fn role_declarations_carry_value_type_doc_and_required() {
    let mut registry = Registry::ui_set("ds/native");
    registry.roles(kit::all());
    let described = SetDescription::of(&registry).unwrap();
    assert_eq!(
        described.roles,
        [
            RoleDeclaration::new("banner", ValueType::Color)
                .with_doc("A banner's surface.")
                .with_required(true),
            RoleDeclaration::new("chip-radius", ValueType::Number),
        ]
    );
    let text = Description::of(std::slice::from_ref(&registry)).to_json();
    let value: Value = serde_json::from_str(&text).unwrap();
    let roles = &value["sets"][0]["roles"];
    assert_eq!(roles[0]["kind"], "color");
    assert_eq!(roles[0]["required"], true);
    assert_eq!(roles[1]["required"], false);
    assert_eq!(roles[1]["doc"], Value::Null);
    let read = Description::from_json(&text).unwrap();
    assert_eq!(read.sets[0].roles, described.roles);
    // A role without `required` is refused, with its place.
    let mut broken = value.clone();
    broken["sets"][0]["roles"][0]
        .as_object_mut()
        .unwrap()
        .remove("required");
    let error = Description::from_json(&broken.to_string()).unwrap_err();
    assert!(
        error.contains("sets[0] (ds/native).roles[0]: no boolean required"),
        "{error}"
    );
}

#[test]
fn the_toolchain_takes_a_sets_own_document_before_any_fallback() {
    let described = |sets: &[Registry]| -> Vec<SetDescription> {
        sets.iter().map(SetDescription::from).collect()
    };
    // The fallback set comes first in the host's order; a set's own
    // document is active all the same.
    let sets = described(&[nsh_base_like(), set("ds/native", Some(OWN), None)]);
    let (document, said) = super::description::theme_document(&sets);
    let document = document.unwrap();
    assert!(document.token("panel").is_some());
    assert!(document.token("surface").is_none());
    assert!(said.is_empty(), "{said:?}");
    // Without a set's own, the fallback.
    let (document, _) = super::description::theme_document(&described(&[nsh_base_like()]));
    assert!(document.unwrap().token("surface").is_some());
    // Without either, none.
    let (none, _) = super::description::theme_document(&described(&[set("x/native", None, None)]));
    assert!(none.is_none());
}
