//! The bundler in the theme format (TOOL-BUNDLE, TOOL-TYPES): `nsh.json`'s
//! `"theme"` and `"themeOverrides"` as a path and inline, their
//! diagnostics as warnings naming the file and the pointer, an old
//! `cssVars` theme or no document shipping nothing, the class check through
//! the active document and the sets' roles, `setup.ui`'s `{ document,
//! overrides }`, and the three `.nsh/types` files.

use std::path::PathBuf;

use nsh_core::element::{Registry, description::SetDescription};
use oxc::span::SourceType;

use crate::bundle::{AppInfo, AppTheme, app_theme, read_app_theme, tailwind_setup};

const DOCUMENT: &str = r##"{
    "tokens": {
        "brand": { "kind": "color", "value": "#123456", "doc": "The brand's color." },
        "corner": { "kind": "number", "value": 6 }
    },
    "modifiers": { "colorMode": { "default": "light", "contexts": {
        "light": {}, "dark": { "brand": "#654321" } } } },
    "resolutionOrder": ["colorMode"],
    "roles": { "banner": "brand" }
}"##;

const OVERRIDES: &str = r##"{ "values": { "corner": 10 }, "roles": { "popover": "brand" } }"##;

/// A design system's roles: what its components read.
#[allow(unreachable_pub, reason = "as a UI set's crate writes its roles")]
mod kit {
    use nsh_core::element::{Role, Typography};

    nsh_core::roles! {
        /// A banner's surface.
        pub BANNER: Role = "banner" required,
        /// A title's text style.
        pub TITLE: Role<Typography> = "title",
        pub MOTION: Role<std::time::Duration> = "motion",
        pub BETA: Role<bool> = "beta",
    }
}

/// The design system's set: its roles, and a document of its own when
/// `document` is given.
fn kit_set(document: Option<&'static str>) -> SetDescription {
    let mut registry = Registry::ui_set("kit/native");
    registry.roles(kit::all());
    if let Some(document) = document {
        registry.theme_document(document);
    }
    SetDescription::from(&registry)
}

/// An app directory with `files` (`nsh.json` among them), each its own.
fn app(files: &[(&str, &str)]) -> PathBuf {
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("nsh-theme-format-{}-{call}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    dir
}

/// The app's theme of an app with `files`, under `sets`.
fn theme_of(files: &[(&str, &str)], sets: &[SetDescription]) -> AppTheme {
    let dir = app(files);
    let theme = app_theme(&dir, sets).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    theme
}

/// `setup.ui` of an app with `files`, under `sets`, read as JSON.
fn shipped(files: &[(&str, &str)], sets: &[SetDescription]) -> Option<serde_json::Value> {
    let dir = app(files);
    let setup = tailwind_setup(&dir, false, &AppInfo::default(), sets).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    setup
        .ui
        .as_deref()
        .map(|ui| serde_json::from_str(ui).unwrap())
}

/// The classes of `classes` the class check doesn't know, in an app with
/// `files`, under `sets`.
fn unknown(files: &[(&str, &str)], sets: &[SetDescription], classes: &str) -> Vec<String> {
    let dir = app(files);
    let setup = tailwind_setup(&dir, false, &AppInfo::default(), sets).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    crate::classes::check(
        &format!(r#"const a = <View class="{classes}" />;"#),
        SourceType::tsx(),
        &setup.theme,
    )
    .into_iter()
    .map(|u| u.class)
    .collect()
}

/// The color token `name` of `theme`, as `rgb` compares with it.
fn color(theme: &AppTheme, name: &str) -> Option<nsh_core::Color> {
    theme.tokens.color(name).map(nsh_core::Color::from)
}

fn rgb(hex: u32) -> Option<nsh_core::Color> {
    Some(nsh_core::Color::from(nsh_core::color::rgb(hex)))
}

#[test]
fn a_theme_file_is_the_apps_document() {
    let theme = theme_of(
        &[
            ("nsh.json", r#"{ "theme": "theme.json" }"#),
            ("theme.json", DOCUMENT),
        ],
        &[],
    );
    assert_eq!(theme.document.as_deref(), Some(DOCUMENT));
    assert_eq!(theme.overrides, None);
    assert!(theme.warnings.is_empty(), "{:?}", theme.warnings);
    assert_eq!(color(&theme, "brand"), rgb(0x123456));
    assert_eq!(theme.tokens.number("corner"), Some(6.0));
}

#[test]
fn an_inline_theme_is_the_apps_document() {
    let config = format!(r#"{{ "theme": {DOCUMENT} }}"#);
    let theme = theme_of(&[("nsh.json", &config)], &[]);
    let document: serde_json::Value =
        serde_json::from_str(theme.document.as_deref().unwrap()).unwrap();
    assert_eq!(
        document["tokens"]["brand"]["value"],
        serde_json::json!("#123456")
    );
    assert_eq!(color(&theme, "brand"), rgb(0x123456));
    // The app's document replaces a set's: no name of the set's is left.
    let set = r##"{ "tokens": { "panel": { "kind": "color", "value": "#222222" } } }"##;
    let theme = theme_of(&[("nsh.json", &config)], &[kit_set(Some(set))]);
    assert!(theme.tokens.color("panel").is_none());
    assert_eq!(color(&theme, "brand"), rgb(0x123456));
}

#[test]
fn overrides_as_a_file_or_inline_layer_over_the_active_document() {
    let set = || kit_set(Some(DOCUMENT));
    let file = theme_of(
        &[
            ("nsh.json", r#"{ "themeOverrides": "overrides.json" }"#),
            ("overrides.json", OVERRIDES),
        ],
        &[set()],
    );
    let inline = theme_of(
        &[(
            "nsh.json",
            &format!(r#"{{ "themeOverrides": {OVERRIDES} }}"#),
        )],
        &[set()],
    );
    for theme in [&file, &inline] {
        // No document of the app's: the set's is active, under the patch.
        assert_eq!(theme.document, None);
        assert!(theme.overrides.is_some());
        assert!(theme.warnings.is_empty(), "{:?}", theme.warnings);
        assert_eq!(theme.tokens.number("corner"), Some(10.0));
        assert_eq!(theme.tokens.mapped("popover"), Some("brand"));
    }
    assert_eq!(file.overrides.as_deref(), Some(OVERRIDES));
}

#[test]
fn diagnostics_name_the_file_and_the_pointer() {
    let document = r##"{ "tokens": {
        "brand": { "kind": "color", "value": "blue" },
        "corner": { "kind": "number", "value": 6 }
    } }"##;
    let overrides = r##"{ "values": { "nope": "#000000", "corner": "wide" } }"##;
    let theme = theme_of(
        &[
            (
                "nsh.json",
                r#"{ "theme": "theme.json", "themeOverrides": "overrides.json" }"#,
            ),
            ("theme.json", document),
            ("overrides.json", overrides),
        ],
        &[],
    );
    let starts = |prefix: &str| theme.warnings.iter().any(|w| w.starts_with(prefix));
    assert!(
        starts("theme.json#/tokens/brand: bad-value: "),
        "{:?}",
        theme.warnings
    );
    assert!(
        starts("overrides.json#/values/nope: undeclared-token: "),
        "{:?}",
        theme.warnings
    );
    assert!(
        starts("overrides.json#/values/corner: bad-value: "),
        "{:?}",
        theme.warnings
    );
    // The bad entries are left out, the good ones kept.
    assert!(theme.tokens.color("brand").is_none());
    assert_eq!(theme.tokens.number("corner"), Some(6.0));
    // Inline, the pointer is into `nsh.json`.
    let config = format!(r#"{{ "theme": {document} }}"#);
    let theme = theme_of(&[("nsh.json", &config)], &[]);
    assert!(
        theme
            .warnings
            .iter()
            .any(|w| w.starts_with("nsh.json#/theme/tokens/brand: bad-value: ")),
        "{:?}",
        theme.warnings
    );
}

#[test]
fn diagnostics_are_the_builds_warnings() {
    let dir = app(&[(
        "nsh.json",
        r#"{ "themeOverrides": { "values": { "nope": 1 } } }"#,
    )]);
    let setup =
        tailwind_setup(&dir, false, &AppInfo::default(), &[kit_set(Some(DOCUMENT))]).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(setup.theme_warnings.len(), 1, "{:?}", setup.theme_warnings);
    assert!(
        setup.theme_warnings[0]
            .starts_with("nsh.json#/themeOverrides/values/nope: undeclared-token: "),
        "{:?}",
        setup.theme_warnings
    );
}

#[test]
fn an_old_cssvars_theme_ships_no_document() {
    let files = [
        ("nsh.json", r#"{ "theme": "theme.json" }"#),
        (
            "theme.json",
            r##"{ "cssVars": { "light": { "primary": "#e11d48" } } }"##,
        ),
    ];
    let theme = theme_of(&files, &[kit_set(Some(DOCUMENT))]);
    assert_eq!(theme.document, None);
    assert_eq!(theme.warnings.len(), 1, "{:?}", theme.warnings);
    assert!(
        theme.warnings[0].starts_with("theme.json: legacy-format: "),
        "{:?}",
        theme.warnings
    );
    assert!(theme.warnings[0].contains("nsh theme convert"));
    // Nothing is converted: the set's document is active.
    assert!(theme.tokens.color("primary").is_none());
    assert_eq!(color(&theme, "brand"), rgb(0x123456));
    assert_eq!(shipped(&files, &[kit_set(Some(DOCUMENT))]), None);
}

#[test]
fn a_theme_that_is_no_document_ships_none() {
    for files in [
        &[
            ("nsh.json", r#"{ "theme": "theme.json" }"#),
            ("theme.json", "[1, 2]"),
        ][..],
        &[("nsh.json", r#"{ "theme": 5 }"#)][..],
        &[
            ("nsh.json", r#"{ "theme": "theme.json" }"#),
            ("theme.json", "{ not json"),
        ][..],
    ] {
        let theme = theme_of(files, &[]);
        assert_eq!(theme.document, None);
        assert!(
            theme
                .warnings
                .iter()
                .any(|w| w.contains(": not-a-document: ")),
            "{:?}",
            theme.warnings
        );
        assert_eq!(shipped(files, &[]), None);
    }
    // Overrides that are no JSON object ship none either.
    let theme = theme_of(&[("nsh.json", r#"{ "themeOverrides": [] }"#)], &[]);
    assert_eq!(theme.overrides, None);
    assert!(
        theme.warnings[0].starts_with("nsh.json#/themeOverrides: not-a-document: "),
        "{:?}",
        theme.warnings
    );
    // A member naming a file that isn't there fails the build.
    let dir = app(&[("nsh.json", r#"{ "themeOverrides": "missing.json" }"#)]);
    let error = app_theme(&dir, &[]).unwrap_err();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        error
            .plain
            .starts_with("nsh.json: themeOverrides: missing.json: "),
        "{error}"
    );
}

#[test]
fn class_checks_read_the_active_document_and_the_sets_roles() {
    const CLASSES: &str = "bg-brand bg-banner bg-(--banner) rounded-(--corner) bg-popover";
    // The set's document maps its role `banner`: a bare color class and a
    // variable read it.
    assert_eq!(
        unknown(&[], &[kit_set(Some(DOCUMENT))], CLASSES),
        ["bg-popover"]
    );
    // The app's overrides map `popover` too.
    let config = format!(r#"{{ "themeOverrides": {OVERRIDES} }}"#);
    assert!(
        unknown(
            &[("nsh.json", &config)],
            &[kit_set(Some(DOCUMENT))],
            CLASSES
        )
        .is_empty()
    );
    // A mapping onto a token of a type the set's role doesn't read is
    // left out and warned of; the class reads nothing.
    let wrong = r##"{ "tokens": {
        "brand": { "kind": "color", "value": "#123456" },
        "corner": { "kind": "number", "value": 6 }
    }, "roles": { "banner": "corner" } }"##;
    let files = [
        ("nsh.json", r#"{ "theme": "theme.json" }"#),
        ("theme.json", wrong),
    ];
    assert_eq!(
        unknown(&files, &[kit_set(None)], "bg-banner bg-(--banner)"),
        ["bg-banner", "bg-(--banner)"]
    );
    let theme = theme_of(&files, &[kit_set(None)]);
    assert!(
        theme
            .warnings
            .iter()
            .any(|w| w.starts_with("theme.json#/roles/banner: role-type: ")),
        "{:?}",
        theme.warnings
    );
}

#[test]
fn a_required_role_the_apps_document_leaves_unmapped_is_warned_of() {
    let document = r##"{ "tokens": { "page": { "kind": "color", "value": "#ffffff" } } }"##;
    let theme = ("theme.json".to_owned(), document.to_owned());
    let warnings = read_app_theme(Some(&theme), None, &[kit_set(None)]).warnings;
    assert!(
        warnings
            .iter()
            .any(|w| w.starts_with("theme.json#/roles/banner: role-unmapped: ")),
        "{warnings:?}"
    );
    // Mapped by the overrides, it is mapped.
    let overrides = (
        "nsh.json#/themeOverrides".to_owned(),
        r#"{ "roles": { "banner": "page" } }"#.to_owned(),
    );
    let warnings = read_app_theme(Some(&theme), Some(&overrides), &[kit_set(None)]).warnings;
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn setup_ui_carries_the_document_and_the_overrides() {
    let config = format!(r#"{{ "theme": "theme.json", "themeOverrides": {OVERRIDES} }}"#);
    let files = [("nsh.json", config.as_str()), ("theme.json", DOCUMENT)];
    let ui = shipped(&files, &[]).unwrap();
    assert_eq!(
        ui["document"],
        serde_json::from_str::<serde_json::Value>(DOCUMENT).unwrap()
    );
    assert_eq!(
        ui["overrides"],
        serde_json::from_str::<serde_json::Value>(OVERRIDES).unwrap()
    );
    // Either alone, the other `null`.
    let ui = shipped(
        &[
            ("nsh.json", r#"{ "theme": "theme.json" }"#),
            ("theme.json", DOCUMENT),
        ],
        &[],
    )
    .unwrap();
    assert!(ui["document"].is_object());
    assert!(ui["overrides"].is_null());
    let ui = shipped(
        &[(
            "nsh.json",
            &format!(r#"{{ "themeOverrides": {OVERRIDES} }}"#),
        )],
        &[],
    )
    .unwrap();
    assert!(ui["document"].is_null());
    assert!(ui["overrides"].is_object());
    // Neither: nothing.
    assert_eq!(shipped(&[("nsh.json", "{}")], &[]), None);
}

#[test]
fn the_bundle_hands_the_runtime_its_theme() {
    let dir = app(&[
        (
            "nsh.json",
            r#"{ "theme": "theme.json", "themeOverrides": "overrides.json" }"#,
        ),
        ("theme.json", DOCUMENT),
        ("overrides.json", OVERRIDES),
        (
            "App.tsx",
            "import { mount, View } from \"nsh\";\nmount(() => <View class=\"bg-brand\" />);\n",
        ),
    ]);
    let bundle = crate::bundle(&crate::BundleOptions {
        entry: dir.join("App.tsx"),
        ..Default::default()
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    // The setup module the entry imports first hands `{ document, overrides }`
    // as a JSON string (its quotes escaped, or not, as the printer quotes
    // it).
    let code = &bundle.code;
    let has = |key: &str| {
        code.contains(&format!(r#"\"{key}\":"#)) || code.contains(&format!(r#""{key}":"#))
    };
    assert!(has("document") && has("overrides"), "{code}");
    assert!(code.contains("#123456"), "{code}");
}

#[test]
fn tokens_d_ts_is_the_active_documents() {
    // The app's document.
    let theme = theme_of(
        &[
            ("nsh.json", r#"{ "theme": "theme.json" }"#),
            ("theme.json", DOCUMENT),
        ],
        &[],
    );
    let declaration = crate::tokens_declaration(&theme.tokens);
    assert!(
        declaration.contains("    /** The brand's color. */\n    \"brand\": string;\n"),
        "{declaration}"
    );
    assert!(
        declaration.contains("    \"corner\": number;\n"),
        "{declaration}"
    );
    assert!(
        declaration.contains("    \"brand\": \"color\";\n"),
        "{declaration}"
    );
    // Else a set's, else the fallback `nsh/base` registers.
    let theme = theme_of(&[], &[kit_set(Some(DOCUMENT))]);
    assert!(crate::tokens_declaration(&theme.tokens).contains("\"brand\": string;"));
    let base: SetDescription = nsh_base::registry().into();
    let theme = theme_of(&[], &[base]);
    let declaration = crate::tokens_declaration(&theme.tokens);
    assert!(
        declaration.contains("\"primary\": string;"),
        "{declaration}"
    );
    assert!(declaration.contains("\"radius\": number;"), "{declaration}");
}

/// Every value type is typed with the canonical JSON form `token()` gives
/// (`theme-format.md`): durations in `ms`, easings as written, compounds as
/// objects of their members (a shadow an array of layers).
#[test]
fn tokens_d_ts_types_each_value_type_as_ts_reads_it() {
    let tokens = crate::types::tests_support::tokens_of(
        r##"{ "tokens": {
            "duration-fast": { "kind": "duration", "value": "0.15s" },
            "ease-standard": { "kind": "easing", "value": [0.2, 0, 0, 1] },
            "font-body": { "kind": "font", "value": "Inter" },
            "label": { "kind": "string", "value": "OK" },
            "text-body": { "kind": "typography", "value": { "fontSize": 14, "lineHeight": "20px" } },
            "shadow-card": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 3 }] },
            "gradient-hero": { "kind": "gradient", "value": { "type": "conic", "stops": [{ "color": "#000" }, { "color": "#fff" }] } },
            "blur-glass": { "kind": "blur", "value": { "type": "background", "radius": 16 } },
            "border-card": { "kind": "border", "value": { "color": "#e8e8e8", "width": 1, "style": "solid" } },
            "motion-press": { "kind": "transition", "value": { "duration": "100ms", "easing": "ease-out" } }
        } }"##,
    );
    let declaration = crate::tokens_declaration(&tokens);
    const EASING: &str = "[number, number, number, number] | \"linear\" | \"ease\" | \"ease-in\" | \"ease-out\" | \"ease-in-out\"";
    for (name, ty) in [
        ("duration-fast", "`${number}ms`".to_owned()),
        ("ease-standard", EASING.to_owned()),
        ("font-body", "string".to_owned()),
        ("label", "string".to_owned()),
        (
            "text-body",
            "{ fontFamily?: string; fontSize: number; fontWeight?: number; lineHeight?: number | `${number}px`; letterSpacing?: number }".to_owned(),
        ),
        (
            "shadow-card",
            "{ color: string; x?: number; y?: number; blur?: number; spread?: number; inset?: boolean }[]".to_owned(),
        ),
        (
            "gradient-hero",
            "{ type: \"linear\" | \"radial\" | \"conic\"; angle?: number; stops: { color: string; position?: number }[] }".to_owned(),
        ),
        (
            "blur-glass",
            "{ type: \"layer\" | \"background\"; radius: number }".to_owned(),
        ),
        (
            "border-card",
            "{ color: string; width: number; style: \"solid\" | \"dashed\" | \"dotted\" }".to_owned(),
        ),
        (
            "motion-press",
            format!("{{ duration: `${{number}}ms`; delay?: `${{number}}ms`; easing?: {EASING} }}"),
        ),
    ] {
        let line = format!("    {name:?}: {ty};\n");
        assert!(declaration.contains(&line), "{line}\n{declaration}");
    }
    // The one mapping, which nsh-js's declarations share.
    assert_eq!(
        nsh_core::element::token_ts_type(nsh_core::element::ValueType::Shadow),
        "{ color: string; x?: number; y?: number; blur?: number; spread?: number; inset?: boolean }[]"
    );
}

/// The bundler's build has serde_json's `arbitrary_precision` (rolldown
/// enables it), which hands nsh-theme's reader a fractional number as a map:
/// it is read as the number all the same, in a document and in a patch.
#[test]
fn fractional_numbers_are_read_under_arbitrary_precision() {
    let theme = theme_of(
        &[
            (
                "nsh.json",
                r#"{ "theme": "theme.json", "themeOverrides": { "values": { "opacity": 0.25 } } }"#,
            ),
            (
                "theme.json",
                r##"{ "tokens": {
                    "opacity": { "kind": "number", "value": 0.5 },
                    "big": { "kind": "number", "value": 1e3 },
                    "ease-standard": { "kind": "easing", "value": [0.2, 0, 0, 1] },
                    "text-title": { "kind": "typography", "value": { "fontSize": 20.5, "lineHeight": 1.2 } }
                } }"##,
            ),
        ],
        &[],
    );
    assert!(theme.warnings.is_empty(), "{:?}", theme.warnings);
    assert_eq!(theme.tokens.number("opacity"), Some(0.25));
    assert_eq!(theme.tokens.number("big"), Some(1000.0));
    assert!(theme.tokens.easing("ease-standard").is_some());
    assert!(theme.tokens.typography("text-title").is_some());
}

#[test]
fn modifiers_d_ts_lists_each_modifiers_contexts() {
    let theme = theme_of(&[], &[kit_set(Some(DOCUMENT))]);
    let declaration = crate::modifiers_declaration(&theme.tokens);
    assert!(
        declaration.contains("declare module \"nsh\" {\n  interface ThemeModifiers {\n    \"colorMode\": \"light\" | \"dark\";\n  }\n}"),
        "{declaration}"
    );
}

#[test]
fn roles_d_ts_types_each_role_and_says_whether_it_is_required() {
    let declaration = crate::roles_declaration(&[kit_set(None)]).unwrap();
    for line in [
        "    /** A banner's surface. Required: every theme maps it. */\n    \"banner\": string;\n",
        // A compound role: the object `role()` gives.
        "    /** A title's text style. Optional: unmapped, it reads the token of its own name. */\n    \"title\": { fontFamily?: string; fontSize: number; fontWeight?: number; lineHeight?: number | `${number}px`; letterSpacing?: number };\n",
        "    /** Optional: unmapped, it reads the token of its own name. */\n    \"motion\": `${number}ms`;\n",
        "    /** Optional: unmapped, it reads the token of its own name. */\n    \"beta\": boolean;\n",
        // The core's own are there too.
        "    \"font-sans\": string;\n",
    ] {
        assert!(declaration.contains(line), "{line}\n{declaration}");
    }
}
