//! Reading, every value type and compound member, diagnostics' codes and
//! pointers, duplicates, deep nesting, writing, round trips and editing.

use nsh_theme::{
    Blur, BlurKind, Border, BorderStyle, Color, Context, Duration, Easing, EasingKeyword, Font,
    Gradient, GradientKind, GradientStop, LineHeight, Member, Modifier, Selection, Shadow,
    ShadowLayer, ThemeDocument, Token, Transition, Typography, Value, ValueType, check, resolve,
    write,
};

use super::{clean, codes, one, sorted, token_doc};

/// The value of the token `t` of a clean one-token document.
fn accepts(kind: &str, value: &str) -> Value {
    let read = clean(&token_doc(kind, value));
    read.document
        .token("t")
        .expect("the token is kept")
        .value
        .clone()
}

/// Asserts the one-token document gives one `bad-value` at the token and
/// leaves it out.
fn refuses(kind: &str, value: &str) {
    let read = one(&token_doc(kind, value), "bad-value", "/tokens/t");
    assert!(
        read.document.token("t").is_none(),
        "{kind} {value} was kept"
    );
}

/// Asserts the one-token document keeps the token with one `unknown-member`
/// warning at `pointer`.
fn warns(kind: &str, value: &str, pointer: &str) -> Value {
    let read = one(&token_doc(kind, value), "unknown-member", pointer);
    read.document
        .token("t")
        .expect("the token is kept")
        .value
        .clone()
}

// ---------------------------------------------------------------------------
// The document's members

#[test]
fn a_document_reads_its_name_and_tokens_and_ignores_schema() {
    let read = clean(
        r##"{
            "$schema": "https://example.com/theme.schema.json",
            "name": "Example",
            "tokens": {
                "surface-app": { "kind": "color", "value": "#ffffff", "path": "surface.app", "doc": "The app's background." }
            }
        }"##,
    );
    assert_eq!(read.document.name.as_deref(), Some("Example"));
    let token = read.document.token("surface-app").unwrap();
    assert_eq!(token.value_type, ValueType::Color);
    assert_eq!(token.value, Value::Color(Color::rgb(255, 255, 255)));
    assert_eq!(token.path.as_deref(), Some("surface.app"));
    assert_eq!(token.doc.as_deref(), Some("The app's background."));
}

#[test]
fn text_that_is_not_json_is_not_a_document() {
    for text in ["", "{", "nope", "{\"tokens\": {}} trailing", "{\"a\": 1,}"] {
        let read = one(text, "not-a-document", "");
        assert_eq!(read.document, ThemeDocument::new());
    }
}

#[test]
fn json_that_is_not_an_object_is_not_a_document() {
    for text in ["[]", "[{\"tokens\": {}}]", "3", "\"theme\"", "null", "true"] {
        let read = one(text, "not-a-document", "");
        assert_eq!(read.document, ThemeDocument::new());
    }
}

#[test]
fn css_vars_without_tokens_is_the_legacy_format_and_its_content_is_not_read() {
    let read = one(
        r#"{ "cssVars": { "light": { "background": "oklch(1 0 0)", "background": 3, "x": [[[]]] } }, "name": 3 }"#,
        "legacy-format",
        "",
    );
    assert!(read.diagnostics[0].message.contains("nsh theme convert"));
    assert_eq!(read.document, ThemeDocument::new());
}

#[test]
fn css_vars_beside_tokens_is_an_unknown_member() {
    let read = one(
        r#"{ "tokens": {}, "cssVars": { "light": { "a": 1, "a": 2 } } }"#,
        "unknown-member",
        "/cssVars",
    );
    assert!(read.document.tokens.is_empty());
}

#[test]
fn an_unknown_top_level_member_is_a_warning_and_ignored() {
    let read = one(
        r#"{ "tokens": {}, "colors": { "a": 1 } }"#,
        "unknown-member",
        "/colors",
    );
    assert!(read.document.tokens.is_empty());
}

#[test]
fn name_must_be_a_string() {
    let read = one(r#"{ "name": 3, "tokens": {} }"#, "bad-value", "/name");
    assert_eq!(read.document.name, None);
}

#[test]
fn a_section_of_the_wrong_json_type_is_bad_value() {
    one(r#"{ "tokens": [] }"#, "bad-value", "/tokens");
    one(r#"{ "tokens": {}, "roles": [] }"#, "bad-value", "/roles");
    one(
        r#"{ "tokens": {}, "modifiers": 1 }"#,
        "bad-value",
        "/modifiers",
    );
    one(
        r#"{ "tokens": {}, "resolutionOrder": {} }"#,
        "bad-value",
        "/resolutionOrder",
    );
}

// ---------------------------------------------------------------------------
// Token entries

#[test]
fn token_names_are_non_empty_and_without_braces() {
    for name in ["", "a{b", "c}", "{d}"] {
        let text =
            format!(r##"{{ "tokens": {{ "{name}": {{ "kind": "color", "value": "#fff" }} }} }}"##);
        let pointer = format!("/tokens/{name}");
        let read = one(&text, "bad-name", &pointer);
        assert!(read.document.tokens.is_empty());
    }
    let read = clean(
        r##"{ "tokens": { "surface/app~1": { "kind": "color", "value": "#fff" }, "a.b c": { "kind": "number", "value": 1 } } }"##,
    );
    assert_eq!(
        read.document.tokens.keys().collect::<Vec<_>>(),
        ["surface/app~1", "a.b c"]
    );
}

#[test]
fn pointers_escape_tilde_and_slash() {
    one(
        r##"{ "tokens": { "surface/app~1": { "kind": "color", "value": "red" } } }"##,
        "bad-value",
        "/tokens/surface~1app~01",
    );
}

#[test]
fn a_missing_or_unknown_kind_is_bad_kind() {
    one(
        r#"{ "tokens": { "t": { "value": 1 } } }"#,
        "bad-kind",
        "/tokens/t",
    );
    one(
        r#"{ "tokens": { "t": { "kind": "dimension", "value": 1 } } }"#,
        "bad-kind",
        "/tokens/t",
    );
    one(
        r#"{ "tokens": { "t": { "kind": 3, "value": 1 } } }"#,
        "bad-kind",
        "/tokens/t",
    );
    one(
        r##"{ "tokens": { "t": { "kind": "Color", "value": "#fff" } } }"##,
        "bad-kind",
        "/tokens/t",
    );
}

#[test]
fn a_token_without_value_is_missing_value() {
    let read = one(
        r#"{ "tokens": { "t": { "kind": "color" } } }"#,
        "missing-value",
        "/tokens/t",
    );
    assert!(read.document.tokens.is_empty());
}

#[test]
fn an_entry_that_is_not_an_object_is_bad_value() {
    one(
        r##"{ "tokens": { "t": "#fff" } }"##,
        "bad-value",
        "/tokens/t",
    );
}

#[test]
fn path_and_doc_are_strings() {
    one(
        r#"{ "tokens": { "t": { "kind": "number", "value": 1, "path": 3 } } }"#,
        "bad-value",
        "/tokens/t",
    );
    one(
        r#"{ "tokens": { "t": { "kind": "number", "value": 1, "doc": ["x"] } } }"#,
        "bad-value",
        "/tokens/t",
    );
}

#[test]
fn a_duplicate_path_leaves_the_second_token_out() {
    let read = one(
        r#"{ "tokens": {
            "a": { "kind": "number", "value": 1, "path": "space.base" },
            "b": { "kind": "number", "value": 2, "path": "space.base" },
            "c": { "kind": "number", "value": 3, "path": "space.large" }
        } }"#,
        "duplicate-path",
        "/tokens/b",
    );
    assert_eq!(read.document.tokens.keys().collect::<Vec<_>>(), ["a", "c"]);
}

#[test]
fn an_unknown_entry_member_is_a_warning_and_the_token_is_kept() {
    let read = one(
        r##"{ "tokens": { "t": { "kind": "color", "value": "#fff", "$type": "color" } } }"##,
        "unknown-member",
        "/tokens/t/$type",
    );
    assert!(read.document.token("t").is_some());
}

// ---------------------------------------------------------------------------
// Duplicates and nesting

#[test]
fn a_duplicate_token_keeps_the_first() {
    let read = one(
        r##"{ "tokens": {
            "t": { "kind": "color", "value": "#ffffff" },
            "t": { "kind": "number", "value": 1 }
        } }"##,
        "duplicate-member",
        "/tokens/t",
    );
    assert_eq!(
        read.document.token("t").unwrap().value_type,
        ValueType::Color
    );
}

#[test]
fn a_duplicate_member_inside_a_context_keeps_the_first() {
    let read = one(
        r##"{
            "tokens": { "t": { "kind": "color", "value": "#ffffff" } },
            "modifiers": { "colorMode": { "default": "light", "contexts": {
                "light": {},
                "dark": { "t": "#000000", "t": "#111111" }
            } } },
            "resolutionOrder": ["colorMode"]
        }"##,
        "duplicate-member",
        "/modifiers/colorMode/contexts/dark/t",
    );
    let dark = &read
        .document
        .modifiers
        .get("colorMode")
        .unwrap()
        .contexts
        .get("dark")
        .unwrap()
        .values;
    assert_eq!(dark.get("t"), Some(&Value::Color(Color::rgb(0, 0, 0))));
}

#[test]
fn a_duplicate_member_anywhere_is_reported_where_it_is_written() {
    let read = one(
        r#"{ "tokens": { "t": { "kind": "typography", "value": { "fontSize": 14, "fontSize": 16 } } } }"#,
        "duplicate-member",
        "/tokens/t/value/fontSize",
    );
    let Value::Typography(t) = &read.document.token("t").unwrap().value else {
        panic!()
    };
    assert_eq!(t.font_size, Member::Value(14.0));
    one(
        r#"{ "tokens": {}, "tokens": [] }"#,
        "duplicate-member",
        "/tokens",
    );
}

#[test]
fn a_duplicate_in_an_ignored_member_is_still_reported() {
    let read = nsh_theme::read(r#"{ "tokens": {}, "x": [{ "a": 1, "a": 1 }] }"#);
    assert_eq!(
        sorted(&read.diagnostics),
        [
            ("duplicate-member", "/x/0/a".to_owned()),
            ("unknown-member", "/x".to_owned())
        ]
    );
}

fn nested(depth: usize) -> String {
    format!("{}1{}", "[".repeat(depth), "]".repeat(depth))
}

#[test]
fn input_nested_64_levels_deep_is_read_without_panicking() {
    let deep = nested(64);
    let read = one(
        &format!(r#"{{ "tokens": {{}}, "extra": {deep} }}"#),
        "unknown-member",
        "/extra",
    );
    assert!(read.document.tokens.is_empty());
    refuses("shadow", &deep);
    refuses("easing", &deep);
    let objects = format!("{}1{}", r#"{"a":"#.repeat(64), "}".repeat(64));
    refuses("typography", &objects);
}

#[test]
fn input_nested_deeper_than_the_parser_allows_is_not_a_document() {
    for depth in [200, 10_000, 100_000] {
        one(
            &format!(r#"{{ "tokens": {{}}, "extra": {} }}"#, nested(depth)),
            "not-a-document",
            "",
        );
        let (_, diagnostics) =
            nsh_theme::read_patch(&format!(r#"{{ "values": {{ "t": {} }} }}"#, nested(depth)));
        assert_eq!(codes(&diagnostics), ["not-a-document"]);
    }
}

// ---------------------------------------------------------------------------
// Value types: an accepted and a refused case for each, and for each
// compound member.

#[test]
fn color_takes_four_hex_forms_in_any_case() {
    assert_eq!(
        accepts("color", r##""#fff""##),
        Value::Color(Color::rgb(255, 255, 255))
    );
    assert_eq!(
        accepts("color", r##""#FfF8""##),
        Value::Color(Color::rgba(255, 255, 255, 0x88))
    );
    assert_eq!(
        accepts("color", r##""#1E293B""##),
        Value::Color(Color::rgb(0x1e, 0x29, 0x3b))
    );
    assert_eq!(
        accepts("color", r##""#1e293b80""##),
        Value::Color(Color::rgba(0x1e, 0x29, 0x3b, 0x80))
    );
    for bad in [
        r#""red""#,
        r##""#12""##,
        r##""#12345""##,
        r##""#ggg""##,
        r#""fff""#,
        "12",
        "null",
        r##""#1e293b8""##,
    ] {
        refuses("color", bad);
    }
}

#[test]
fn number_is_a_json_number() {
    assert_eq!(accepts("number", "1.5"), Value::Number(1.5));
    assert_eq!(accepts("number", "-3"), Value::Number(-3.0));
    assert_eq!(accepts("number", "1e3"), Value::Number(1000.0));
    refuses("number", r#""12""#);
    refuses("number", "true");
}

#[test]
fn font_is_a_non_empty_family_list_kept_whole() {
    assert_eq!(
        accepts("font", r#""Inter, system-ui, sans-serif""#),
        Value::Font(Font::new("Inter, system-ui, sans-serif"))
    );
    refuses("font", r#""""#);
    refuses("font", r#"["Inter"]"#);
}

#[test]
fn string_and_boolean() {
    assert_eq!(
        accepts("string", r#""Hello""#),
        Value::String("Hello".to_owned())
    );
    assert_eq!(
        accepts("string", r#""{not a ref""#),
        Value::String("{not a ref".to_owned())
    );
    refuses("string", "12");
    assert_eq!(accepts("boolean", "true"), Value::Boolean(true));
    assert_eq!(accepts("boolean", "false"), Value::Boolean(false));
    refuses("boolean", r#""true""#);
    refuses("boolean", "0");
}

#[test]
fn duration_is_ms_or_s_held_as_milliseconds() {
    assert_eq!(
        accepts("duration", r#""150ms""#),
        Value::Duration(Duration::from_millis(150.0))
    );
    assert_eq!(
        accepts("duration", r#""1.5s""#),
        Value::Duration(Duration::from_millis(1500.0))
    );
    assert_eq!(
        accepts("duration", r#""0s""#),
        Value::Duration(Duration::from_millis(0.0))
    );
    for bad in [
        r#""-5ms""#,
        r#""5""#,
        r#""ms""#,
        r#""1.ms""#,
        r#""5 ms""#,
        r#""1e3ms""#,
        "150",
    ] {
        refuses("duration", bad);
    }
}

#[test]
fn easing_is_a_bezier_or_a_keyword() {
    assert_eq!(
        accepts("easing", "[0.4, 0, 0.2, 1]"),
        Value::Easing(Easing::Bezier([0.4, 0.0, 0.2, 1.0]))
    );
    assert_eq!(
        accepts("easing", "[0, -2, 1, 3]"),
        Value::Easing(Easing::Bezier([0.0, -2.0, 1.0, 3.0]))
    );
    for keyword in EasingKeyword::ALL {
        assert_eq!(
            accepts("easing", &format!("\"{}\"", keyword.name())),
            Value::Easing(Easing::Keyword(keyword))
        );
    }
    for bad in [
        "[1.5, 0, 0.2, 1]",
        "[0, 0, -0.1, 1]",
        "[0, 0, 1]",
        "[0, 0, 1, 1, 1]",
        r#"[0, "0", 1, 1]"#,
        r#""bounce""#,
        "1",
    ] {
        refuses("easing", bad);
    }
}

#[test]
fn typography_reads_each_member() {
    let value = accepts(
        "typography",
        r#"{ "fontSize": 14, "fontFamily": "Inter, sans-serif", "fontWeight": 500, "lineHeight": 1.5, "letterSpacing": -0.2 }"#,
    );
    assert_eq!(
        value,
        Value::Typography(
            Typography::new(14.0)
                .with_font_family(Font::new("Inter, sans-serif"))
                .with_font_weight(500.0)
                .with_line_height(LineHeight::Multiple(1.5))
                .with_letter_spacing(-0.2)
        )
    );
    assert_eq!(
        accepts("typography", r#"{ "fontSize": 14 }"#),
        Value::Typography(Typography::new(14.0))
    );
}

#[test]
fn typography_takes_three_line_height_forms() {
    let line_height = |value: &str| {
        let doc = format!(
            r#"{{ "tokens": {{
                "leading": {{ "kind": "number", "value": 22 }},
                "t": {{ "kind": "typography", "value": {{ "fontSize": 14, "lineHeight": {value} }} }}
            }} }}"#
        );
        let read = clean(&doc);
        let Value::Typography(t) = &read.document.token("t").unwrap().value else {
            panic!()
        };
        t.line_height.clone().unwrap()
    };
    assert_eq!(line_height("1.5"), Member::Value(LineHeight::Multiple(1.5)));
    assert_eq!(
        line_height(r#""20px""#),
        Member::Value(LineHeight::Px(20.0))
    );
    assert_eq!(line_height(r#""{leading}""#), Member::reference("leading"));
}

#[test]
fn typography_refuses_each_bad_member() {
    for bad in [
        r#"{}"#,
        r#"{ "fontSize": "14" }"#,
        r#"{ "fontSize": 14, "fontFamily": "" }"#,
        r#"{ "fontSize": 14, "fontFamily": 3 }"#,
        r#"{ "fontSize": 14, "fontWeight": 0 }"#,
        r#"{ "fontSize": 14, "fontWeight": 1001 }"#,
        r#"{ "fontSize": 14, "fontWeight": "bold" }"#,
        r#"{ "fontSize": 14, "lineHeight": "1.5em" }"#,
        r#"{ "fontSize": 14, "lineHeight": true }"#,
        r#"{ "fontSize": 14, "letterSpacing": "1px" }"#,
        r#"[14]"#,
    ] {
        refuses("typography", bad);
    }
    assert_eq!(
        warns(
            "typography",
            r#"{ "fontSize": 14, "textCase": "upper" }"#,
            "/tokens/t/value/textCase"
        ),
        Value::Typography(Typography::new(14.0))
    );
}

#[test]
fn shadow_reads_layers_with_defaults() {
    let value = accepts(
        "shadow",
        r##"[{ "color": "#0000001a", "y": 1, "blur": 3 }, { "color": "#000", "x": 1, "y": 2, "blur": 4, "spread": -1, "inset": true }]"##,
    );
    assert_eq!(
        value,
        Value::Shadow(Shadow::new(vec![
            ShadowLayer::new(Color::rgba(0, 0, 0, 0x1a))
                .with_offset(0.0, 1.0)
                .with_blur(3.0),
            ShadowLayer::new(Color::rgb(0, 0, 0))
                .with_offset(1.0, 2.0)
                .with_blur(4.0)
                .with_spread(-1.0)
                .with_inset(true),
        ]))
    );
}

#[test]
fn shadow_refuses_each_bad_member() {
    for bad in [
        r##"{ "color": "#000" }"##,
        "[]",
        r#"[{ "y": 1 }]"#,
        r#"[{ "color": "black" }]"#,
        r##"[{ "color": "#000", "x": "1" }]"##,
        r##"[{ "color": "#000", "y": null }]"##,
        r##"[{ "color": "#000", "blur": "2px" }]"##,
        r##"[{ "color": "#000", "spread": [] }]"##,
        r##"[{ "color": "#000", "inset": "yes" }]"##,
        r##"["#000"]"##,
    ] {
        refuses("shadow", bad);
    }
    warns(
        "shadow",
        r##"[{ "color": "#000", "opacity": 0.5 }]"##,
        "/tokens/t/value/0/opacity",
    );
}

#[test]
fn gradient_reads_its_three_types_and_default_angles() {
    let stops = r##"[{ "color": "#fff", "position": 0 }, { "color": "#000" }]"##;
    let expected_stops = vec![
        GradientStop::new(Color::rgb(255, 255, 255), Some(Member::Value(0.0))),
        GradientStop::new(Color::rgb(0, 0, 0), None),
    ];
    let linear = accepts(
        "gradient",
        &format!(r#"{{ "type": "linear", "stops": {stops} }}"#),
    );
    assert_eq!(
        linear,
        Value::Gradient(Gradient::new(
            GradientKind::Linear {
                angle: Member::Value(180.0)
            },
            expected_stops.clone()
        ))
    );
    let turned = accepts(
        "gradient",
        &format!(r#"{{ "type": "linear", "angle": 90, "stops": {stops} }}"#),
    );
    assert_eq!(
        turned,
        Value::Gradient(Gradient::new(
            GradientKind::Linear {
                angle: Member::Value(90.0)
            },
            expected_stops.clone()
        ))
    );
    let radial = accepts(
        "gradient",
        &format!(r#"{{ "type": "radial", "stops": {stops} }}"#),
    );
    assert_eq!(
        radial,
        Value::Gradient(Gradient::new(GradientKind::Radial, expected_stops.clone()))
    );
    let conic = accepts(
        "gradient",
        &format!(r#"{{ "type": "conic", "stops": {stops} }}"#),
    );
    assert_eq!(
        conic,
        Value::Gradient(Gradient::new(
            GradientKind::Conic {
                from: Member::Value(0.0)
            },
            expected_stops
        ))
    );
}

#[test]
fn gradient_refuses_each_bad_member() {
    let two = r##"[{ "color": "#fff" }, { "color": "#000" }]"##;
    for bad in [
        format!(r#"{{ "stops": {two} }}"#),
        format!(r#"{{ "type": "diamond", "stops": {two} }}"#),
        format!(r#"{{ "type": 1, "stops": {two} }}"#),
        format!(r#"{{ "type": "linear", "angle": "90deg", "stops": {two} }}"#),
        r#"{ "type": "linear" }"#.to_owned(),
        r##"{ "type": "linear", "stops": [{ "color": "#fff" }] }"##.to_owned(),
        r##"{ "type": "linear", "stops": { "color": "#fff" } }"##.to_owned(),
        r##"{ "type": "linear", "stops": [{ "color": "#fff" }, { "position": 1 }] }"##.to_owned(),
        r##"{ "type": "linear", "stops": [{ "color": "#fff" }, { "color": "white" }] }"##.to_owned(),
        r##"{ "type": "linear", "stops": [{ "color": "#fff", "position": 1.5 }, { "color": "#000" }] }"##.to_owned(),
        r##"{ "type": "linear", "stops": [{ "color": "#fff", "position": "50%" }, { "color": "#000" }] }"##.to_owned(),
    ] {
        refuses("gradient", &bad);
    }
    warns(
        "gradient",
        &format!(r#"{{ "type": "linear", "stops": {two}, "space": "oklch" }}"#),
        "/tokens/t/value/space",
    );
    warns(
        "gradient",
        r##"{ "type": "linear", "stops": [{ "color": "#fff", "hint": 0.5 }, { "color": "#000" }] }"##,
        "/tokens/t/value/stops/0/hint",
    );
    warns(
        "gradient",
        &format!(r#"{{ "type": "radial", "angle": 45, "stops": {two} }}"#),
        "/tokens/t/value/angle",
    );
}

#[test]
fn blur_reads_its_type_and_radius() {
    assert_eq!(
        accepts("blur", r#"{ "type": "layer", "radius": 8 }"#),
        Value::Blur(Blur::new(BlurKind::Layer, 8.0))
    );
    assert_eq!(
        accepts("blur", r#"{ "type": "background", "radius": 0 }"#),
        Value::Blur(Blur::new(BlurKind::Background, 0.0))
    );
    for bad in [
        r#"{ "radius": 8 }"#,
        r#"{ "type": "glass", "radius": 8 }"#,
        r#"{ "type": "layer" }"#,
        r#"{ "type": "layer", "radius": -1 }"#,
        r#"{ "type": "layer", "radius": "8px" }"#,
    ] {
        refuses("blur", bad);
    }
    warns(
        "blur",
        r#"{ "type": "layer", "radius": 8, "saturate": 2 }"#,
        "/tokens/t/value/saturate",
    );
}

#[test]
fn border_reads_its_color_width_and_style() {
    for (style, expected) in [
        ("solid", BorderStyle::Solid),
        ("dashed", BorderStyle::Dashed),
        ("dotted", BorderStyle::Dotted),
    ] {
        assert_eq!(
            accepts(
                "border",
                &format!(r##"{{ "color": "#e5e7eb", "width": 1, "style": "{style}" }}"##)
            ),
            Value::Border(Border::new(Color::rgb(0xe5, 0xe7, 0xeb), 1.0, expected))
        );
    }
    for bad in [
        r#"{ "width": 1, "style": "solid" }"#,
        r##"{ "color": "#000", "style": "solid" }"##,
        r##"{ "color": "#000", "width": 1 }"##,
        r##"{ "color": "#000", "width": -1, "style": "solid" }"##,
        r##"{ "color": "#000", "width": 1, "style": "groove" }"##,
        r#"{ "color": "gray", "width": 1, "style": "solid" }"#,
    ] {
        refuses("border", bad);
    }
    warns(
        "border",
        r##"{ "color": "#000", "width": 1, "style": "solid", "radius": 4 }"##,
        "/tokens/t/value/radius",
    );
}

#[test]
fn transition_reads_its_members_with_defaults() {
    assert_eq!(
        accepts("transition", r#"{ "duration": "200ms" }"#),
        Value::Transition(Transition::new(Duration::from_millis(200.0)))
    );
    assert_eq!(
        accepts(
            "transition",
            r#"{ "duration": "0.2s", "delay": "50ms", "easing": [0.4, 0, 0.2, 1] }"#
        ),
        Value::Transition(
            Transition::new(Duration::from_millis(200.0))
                .with_delay(Duration::from_millis(50.0))
                .with_easing(Easing::Bezier([0.4, 0.0, 0.2, 1.0]))
        )
    );
    for bad in [
        r#"{ "delay": "50ms" }"#,
        r#"{ "duration": 200 }"#,
        r#"{ "duration": "200ms", "delay": "soon" }"#,
        r#"{ "duration": "200ms", "easing": "bounce" }"#,
        r#"{ "duration": "200ms", "easing": [2, 0, 0, 1] }"#,
    ] {
        refuses("transition", bad);
    }
    warns(
        "transition",
        r#"{ "duration": "200ms", "property": "opacity" }"#,
        "/tokens/t/value/property",
    );
}

// ---------------------------------------------------------------------------
// References at read time

const PRIMITIVES: &str = r##"
    "c": { "kind": "color", "value": "#112233" },
    "n": { "kind": "number", "value": 4 },
    "f": { "kind": "font", "value": "Inter" },
    "d": { "kind": "duration", "value": "120ms" },
    "e": { "kind": "easing", "value": "ease-out" }
"##;

#[test]
fn a_member_reference_of_each_member_type_is_read() {
    let read = clean(&format!(
        r#"{{ "tokens": {{ {PRIMITIVES},
            "ty": {{ "kind": "typography", "value": {{ "fontSize": "{{n}}", "fontFamily": "{{f}}", "fontWeight": "{{n}}", "lineHeight": "{{n}}", "letterSpacing": "{{n}}" }} }},
            "sh": {{ "kind": "shadow", "value": [{{ "color": "{{c}}", "x": "{{n}}", "y": "{{n}}", "blur": "{{n}}", "spread": "{{n}}" }}] }},
            "gr": {{ "kind": "gradient", "value": {{ "type": "conic", "angle": "{{n}}", "stops": [{{ "color": "{{c}}", "position": "{{n}}" }}, {{ "color": "{{c}}" }}] }} }},
            "bl": {{ "kind": "blur", "value": {{ "type": "layer", "radius": "{{n}}" }} }},
            "bo": {{ "kind": "border", "value": {{ "color": "{{c}}", "width": "{{n}}", "style": "solid" }} }},
            "tr": {{ "kind": "transition", "value": {{ "duration": "{{d}}", "delay": "{{d}}", "easing": "{{e}}" }} }}
        }} }}"#
    ));
    assert_eq!(read.document.tokens.len(), 11);
    let Value::Transition(t) = &read.document.token("tr").unwrap().value else {
        panic!()
    };
    assert_eq!(t.easing, Member::reference("e"));
}

#[test]
fn a_member_reference_to_the_wrong_type_is_reference_type() {
    for (kind, value) in [
        ("typography", r#"{ "fontSize": "{c}" }"#),
        ("typography", r#"{ "fontSize": 14, "fontFamily": "{n}" }"#),
        ("typography", r#"{ "fontSize": 14, "lineHeight": "{d}" }"#),
        ("shadow", r#"[{ "color": "{n}" }]"#),
        (
            "gradient",
            r#"{ "type": "linear", "angle": "{c}", "stops": [{ "color": "{c}" }, { "color": "{c}" }] }"#,
        ),
        ("blur", r#"{ "type": "layer", "radius": "{d}" }"#),
        (
            "border",
            r#"{ "color": "{c}", "width": "{c}", "style": "solid" }"#,
        ),
        ("transition", r#"{ "duration": "{e}" }"#),
        ("transition", r#"{ "duration": "{d}", "easing": "{d}" }"#),
    ] {
        let text = format!(
            r#"{{ "tokens": {{ {PRIMITIVES}, "t": {{ "kind": "{kind}", "value": {value} }} }} }}"#
        );
        let read = one(&text, "reference-type", "/tokens/t");
        assert!(read.document.token("t").is_none());
    }
}

#[test]
fn a_whole_value_reference_needs_a_declared_token_of_the_same_type() {
    let read = one(
        r#"{ "tokens": { "a": { "kind": "color", "value": "{missing}" } } }"#,
        "reference-missing",
        "/tokens/a",
    );
    assert!(read.document.tokens.is_empty());
    one(
        r#"{ "tokens": { "n": { "kind": "number", "value": 1 }, "a": { "kind": "color", "value": "{n}" } } }"#,
        "reference-type",
        "/tokens/a",
    );
    one(
        r#"{ "tokens": { "t": { "kind": "typography", "value": { "fontSize": "{missing}" } } } }"#,
        "reference-missing",
        "/tokens/t",
    );
}

#[test]
fn an_entry_that_depends_on_a_refused_token_goes_with_it_silently() {
    let read = one(
        r##"{
            "tokens": {
                "a": { "kind": "color", "value": "{b}" },
                "b": { "kind": "color", "value": "{c}" },
                "c": { "kind": "color", "value": "red" },
                "d": { "kind": "color", "value": "#fff" }
            },
            "roles": { "primary": "b" }
        }"##,
        "bad-value",
        "/tokens/c",
    );
    assert_eq!(read.document.tokens.keys().collect::<Vec<_>>(), ["d"]);
    assert!(read.document.roles.is_empty());
}

// ---------------------------------------------------------------------------
// Pointers

#[test]
fn pointers_name_a_token_a_context_entry_and_a_role() {
    let read = nsh_theme::read(
        r##"{
            "tokens": { "a": { "kind": "color", "value": 1 }, "b": { "kind": "number", "value": 1 } },
            "modifiers": { "density": { "default": "regular", "contexts": {
                "regular": {}, "compact": { "b": "small" }
            } } },
            "resolutionOrder": ["density", "nope"],
            "roles": { "primary": "missing" }
        }"##,
    );
    assert_eq!(
        sorted(&read.diagnostics),
        [
            (
                "bad-value",
                "/modifiers/density/contexts/compact/b".to_owned()
            ),
            ("bad-value", "/tokens/a".to_owned()),
            ("order-unknown", "/resolutionOrder/1".to_owned()),
            ("role-token-missing", "/roles/primary".to_owned()),
        ]
    );
    for diagnostic in &read.diagnostics {
        assert!(diagnostic.is_error());
        assert!(diagnostic.message.ends_with('.'), "{}", diagnostic.message);
    }
    assert!(
        read.diagnostics
            .iter()
            .any(|d| d.message.contains("`primary`") && d.message.contains("`missing`"))
    );
}

// ---------------------------------------------------------------------------
// Writing

#[test]
fn write_has_two_space_indentation_and_keeps_order() {
    let read = clean(
        r##"{"name":"N","tokens":{"z":{"kind":"color","value":"#FFF"},"a":{"kind":"easing","value":[0.4,0,0.2,1.0],"doc":"D","path":"p"}},
            "roles":{"zz":"z","aa":"a"}}"##,
    );
    assert_eq!(
        write(&read.document),
        r##"{
  "name": "N",
  "tokens": {
    "z": {
      "kind": "color",
      "value": "#ffffff"
    },
    "a": {
      "kind": "easing",
      "value": [0.4, 0, 0.2, 1],
      "path": "p",
      "doc": "D"
    }
  },
  "roles": {
    "zz": "z",
    "aa": "a"
  }
}
"##
    );
}

#[test]
fn write_uses_canonical_forms() {
    let written = |kind: &str, value: &str| {
        let read = clean(&token_doc(kind, value));
        let json: serde_json::Value = serde_json::from_str(&write(&read.document)).unwrap();
        json["tokens"]["t"]["value"].to_string()
    };
    assert_eq!(written("color", r##""#FFF""##), r##""#ffffff""##);
    assert_eq!(written("color", r##""#AbCdEf""##), r##""#abcdef""##);
    assert_eq!(written("color", r##""#11223380""##), r##""#11223380""##);
    assert_eq!(written("color", r##""#112233FF""##), r##""#112233""##);
    assert_eq!(written("color", r##""#0008""##), r##""#00000088""##);
    assert_eq!(written("duration", r#""1.5s""#), r#""1500ms""#);
    assert_eq!(written("duration", r#""0.25ms""#), r#""0.25ms""#);
    assert_eq!(written("easing", r#""ease-in""#), r#""ease-in""#);
    assert_eq!(written("easing", "[0.42, 0, 1, 1]"), "[0.42,0,1,1]");
    assert_eq!(written("string", r#""{x""#), r#""{x""#);
    let text = write(&clean(&token_doc("number", "12.0")).document);
    assert!(text.contains("\"value\": 12\n"), "{text}");
    let text = write(&clean(&token_doc("number", "0.5")).document);
    assert!(text.contains("\"value\": 0.5\n"), "{text}");
    let text = write(&clean(&token_doc("number", "-0.0")).document);
    assert!(text.contains("\"value\": 0\n"), "{text}");
}

#[test]
fn write_keeps_the_line_height_form_and_writes_references_as_braces() {
    let read = clean(
        r#"{ "tokens": {
            "n": { "kind": "number", "value": 20 },
            "a": { "kind": "typography", "value": { "fontSize": 14.0, "lineHeight": 1.5 } },
            "b": { "kind": "typography", "value": { "fontSize": 14, "lineHeight": "20.0px" } },
            "c": { "kind": "typography", "value": { "fontSize": "{n}", "lineHeight": "{n}" } },
            "d": { "kind": "number", "value": "{n}" }
        } }"#,
    );
    let json: serde_json::Value = serde_json::from_str(&write(&read.document)).unwrap();
    assert_eq!(
        json["tokens"]["a"]["value"],
        serde_json::json!({ "fontSize": 14, "lineHeight": 1.5 })
    );
    assert_eq!(
        json["tokens"]["b"]["value"],
        serde_json::json!({ "fontSize": 14, "lineHeight": "20px" })
    );
    assert_eq!(
        json["tokens"]["c"]["value"],
        serde_json::json!({ "fontSize": "{n}", "lineHeight": "{n}" })
    );
    assert_eq!(json["tokens"]["d"]["value"], serde_json::json!("{n}"));
}

/// A document with every value type, modifiers, references and roles.
pub(crate) const EVERY_TYPE: &str = r##"{
    "name": "Every type",
    "tokens": {
        "gray-1": { "kind": "color", "value": "#FCFCFC" },
        "gray-12": { "kind": "color", "value": "#202020cc" },
        "space-2": { "kind": "number", "value": 8.0 },
        "leading-body": { "kind": "number", "value": 20 },
        "font-sans": { "kind": "font", "value": "Inter, system-ui, sans-serif" },
        "label-ok": { "kind": "string", "value": "OK" },
        "motion-reduced": { "kind": "boolean", "value": false },
        "duration-fast": { "kind": "duration", "value": "0.15s" },
        "ease-standard": { "kind": "easing", "value": [0.2, 0, 0, 1] },
        "ease-exit": { "kind": "easing", "value": "ease-in" },
        "text-body": { "kind": "typography", "value": { "fontFamily": "{font-sans}", "fontSize": 14, "fontWeight": 400, "lineHeight": "{leading-body}", "letterSpacing": 0 } },
        "text-title": { "kind": "typography", "value": { "fontSize": 20, "lineHeight": 1.2 } },
        "shadow-card": { "kind": "shadow", "value": [{ "color": "#0000001a", "y": 1, "blur": 3 }, { "color": "{gray-12}", "y": "{space-2}", "blur": 16, "spread": -4, "inset": true }] },
        "gradient-hero": { "kind": "gradient", "value": { "type": "linear", "angle": 135, "stops": [{ "color": "{gray-1}", "position": 0 }, { "color": "#0090ff", "position": 1 }] } },
        "gradient-glow": { "kind": "gradient", "value": { "type": "radial", "stops": [{ "color": "#fff" }, { "color": "#fff0" }] } },
        "gradient-spin": { "kind": "gradient", "value": { "type": "conic", "angle": 90, "stops": [{ "color": "#f00" }, { "color": "#00f" }] } },
        "blur-glass": { "kind": "blur", "value": { "type": "background", "radius": "{space-2}" } },
        "border-subtle": { "kind": "border", "value": { "color": "{gray-12}", "width": 1, "style": "dashed" } },
        "transition-press": { "kind": "transition", "value": { "duration": "{duration-fast}", "delay": "10ms", "easing": "{ease-standard}" } },
        "surface-app": { "kind": "color", "value": "{gray-1}", "path": "surface.app", "doc": "The app's background." },
        "text-primary": { "kind": "color", "value": "{gray-12}", "path": "text.primary" }
    },
    "modifiers": {
        "colorMode": { "default": "light", "contexts": {
            "light": {},
            "dark": { "gray-1": "#111111", "gray-12": "#EEEEEE" }
        } },
        "density": { "default": "regular", "contexts": {
            "regular": {},
            "compact": { "space-2": 6, "leading-body": 16, "text-title": { "fontSize": 18, "lineHeight": "22px" } }
        } }
    },
    "resolutionOrder": ["colorMode", "density"],
    "roles": { "background": "surface-app", "foreground": "text-primary", "font-body": "font-sans" }
}"##;

#[test]
fn a_document_with_every_type_round_trips() {
    let document = clean(EVERY_TYPE).document;
    let types: std::collections::HashSet<_> =
        document.tokens.values().map(|t| t.value_type).collect();
    assert_eq!(types.len(), ValueType::ALL.len());
    let text = write(&document);
    let again = clean(&text);
    assert_eq!(again.document, document);
    assert_eq!(write(&again.document), text);
    assert!(check(&document).is_empty());
}

#[test]
fn writing_a_read_document_twice_gives_the_same_bytes() {
    let first = write(&nsh_theme::read(EVERY_TYPE).document);
    let second = write(&nsh_theme::read(EVERY_TYPE).document);
    assert_eq!(first, second);
    assert_eq!(write(&clean(&first).document), first);
}

#[test]
fn values_built_in_memory_round_trip() {
    let mut document = ThemeDocument::new();
    document.insert_token("n", Token::new(ValueType::Number, Value::Number(3.0)));
    document.insert_token(
        "shadow",
        Token::new(
            ValueType::Shadow,
            Value::Shadow(Shadow::new(vec![
                ShadowLayer::new(Color::rgb(1, 2, 3)).with_blur(Member::reference("n")),
            ])),
        ),
    );
    document.insert_token(
        "transition",
        Token::new(
            ValueType::Transition,
            Value::Transition(
                Transition::new(Duration::from_millis(1.5))
                    .with_easing(Easing::Keyword(EasingKeyword::Linear)),
            ),
        ),
    );
    document.insert_token(
        "border",
        Token::new(
            ValueType::Border,
            Value::Border(Border::new(
                Color::rgba(0, 0, 0, 0),
                0.5,
                BorderStyle::Dotted,
            )),
        )
        .with_doc("hairline"),
    );
    document.insert_token(
        "blur",
        Token::new(
            ValueType::Blur,
            Value::Blur(Blur::new(BlurKind::Layer, Member::reference("n"))),
        ),
    );
    document.insert_token(
        "gradient",
        Token::new(
            ValueType::Gradient,
            Value::Gradient(Gradient::new(
                GradientKind::Conic {
                    from: Member::Value(45.0),
                },
                vec![
                    GradientStop::new(Color::rgb(0, 0, 0), None),
                    GradientStop::new(Member::reference("c"), None),
                ],
            )),
        ),
    );
    document.insert_token(
        "c",
        Token::new(ValueType::Color, Value::Color(Color::rgb(9, 9, 9))),
    );
    assert!(check(&document).is_empty());
    let read = clean(&write(&document));
    assert_eq!(read.document, document);
    assert_eq!(
        Value::Color(Color::rgba(1, 2, 3, 4)).to_json(),
        serde_json::json!("#01020304")
    );
    assert_eq!(
        Value::from_json(ValueType::Duration, &serde_json::json!("2s")),
        Ok(Value::Duration(Duration::from_millis(2000.0)))
    );
    assert!(
        Value::from_json(
            ValueType::Blur,
            &serde_json::json!({ "type": "layer", "radius": -2 })
        )
        .is_err()
    );
}

// ---------------------------------------------------------------------------
// Editing

fn base() -> ThemeDocument {
    clean(
        r##"{
            "tokens": {
                "gray-1": { "kind": "color", "value": "#fcfcfc" },
                "gray-12": { "kind": "color", "value": "#202020" },
                "surface-app": { "kind": "color", "value": "{gray-1}" },
                "text-primary": { "kind": "color", "value": "{gray-12}" }
            },
            "modifiers": {
                "colorMode": { "default": "light", "contexts": {
                    "light": {},
                    "dark": { "gray-1": "#111111", "gray-12": "#eeeeee" }
                } },
                "contrast": { "default": "normal", "contexts": {
                    "normal": {},
                    "high": { "gray-12": "#000000" }
                } }
            },
            "resolutionOrder": ["colorMode", "contrast"],
            "roles": { "background": "surface-app", "foreground": "text-primary" }
        }"##,
    )
    .document
}

#[test]
fn an_inserted_token_goes_last_and_a_replaced_one_keeps_its_place() {
    let mut document = base();
    let old = document.insert_token(
        "gray-1",
        Token::new(ValueType::Color, Value::Color(Color::rgb(1, 1, 1))),
    );
    assert_eq!(
        old.unwrap().value,
        Value::Color(Color::rgb(0xfc, 0xfc, 0xfc))
    );
    document.insert_token(
        "accent",
        Token::new(ValueType::Color, Value::reference("gray-12")).with_path("accent"),
    );
    assert_eq!(
        document.tokens.keys().collect::<Vec<_>>(),
        ["gray-1", "gray-12", "surface-app", "text-primary", "accent"]
    );
    assert!(check(&document).is_empty());
    let text = write(&document);
    assert!(text.find("\"gray-1\"").unwrap() < text.find("\"gray-12\"").unwrap());
    assert!(text.contains("\"value\": \"#010101\""));
    assert_eq!(clean(&text).document, document);
}

#[test]
fn removing_a_token_removes_its_context_entries_and_keeps_its_role_mappings() {
    let mut document = base();
    document.insert_token(
        "muted",
        Token::new(ValueType::Color, Value::Color(Color::rgb(5, 5, 5))),
    );
    document.set_role("muted-foreground", "muted");
    document
        .modifiers
        .get_mut("colorMode")
        .unwrap()
        .contexts
        .get_mut("dark")
        .unwrap()
        .set("muted", Value::Color(Color::rgb(6, 6, 6)));
    assert!(check(&document).is_empty());

    assert!(document.remove_token("muted").is_some());
    let dark = &document
        .modifiers
        .get("colorMode")
        .unwrap()
        .contexts
        .get("dark")
        .unwrap()
        .values;
    assert!(!dark.contains_key("muted"));
    assert_eq!(
        document.roles.get("muted-foreground").map(String::as_str),
        Some("muted")
    );
    let diagnostics = check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [("role-token-missing", "/roles/muted-foreground".to_owned())]
    );
    let text = write(&document);
    assert!(!text.contains("\"muted\": \"#060606\""));
    assert_eq!(
        sorted(&nsh_theme::read(&text).diagnostics),
        sorted(&diagnostics)
    );
}

#[test]
fn removing_a_modifier_removes_its_resolution_order_item() {
    let mut document = base();
    assert!(document.remove_modifier("colorMode").is_some());
    assert_eq!(document.resolution_order, ["contrast"]);
    assert!(check(&document).is_empty());
    assert!(!write(&document).contains("colorMode"));
}

#[test]
fn removing_the_default_context_is_allowed_and_check_reports_bad_default() {
    let mut document = base();
    document
        .modifiers
        .get_mut("contrast")
        .unwrap()
        .remove_context("normal");
    document
        .modifiers
        .get_mut("contrast")
        .unwrap()
        .insert_context("low", Context::new());
    let diagnostics = check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [("bad-default", "/modifiers/contrast".to_owned())]
    );
    assert_eq!(
        sorted(&nsh_theme::read(&write(&document)).diagnostics),
        sorted(&diagnostics)
    );

    // A two-context modifier losing its default's context: both conditions
    // hold, both are reported, and the modifier is left out once.
    let mut document = base();
    document
        .modifiers
        .get_mut("colorMode")
        .unwrap()
        .remove_context("light");
    let diagnostics = check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [
            ("bad-default", "/modifiers/colorMode".to_owned()),
            ("too-few-contexts", "/modifiers/colorMode".to_owned()),
        ]
    );
    let read = nsh_theme::read(&write(&document));
    assert_eq!(sorted(&read.diagnostics), sorted(&diagnostics));
    assert_eq!(
        read.document.modifiers.keys().collect::<Vec<_>>(),
        ["contrast"]
    );
}

#[test]
fn tiny_and_huge_durations_and_px_line_heights_round_trip() {
    for value in [
        r#""0.000001ms""#,
        r#""0.0000001s""#,
        r#""10000000000000000ms""#,
        r#""123456789012345678901234567890s""#,
    ] {
        let read = clean(&token_doc("duration", value));
        let text = write(&read.document);
        assert!(!text.contains("e-") && !text.contains("e+"), "{text}");
        let again = clean(&text);
        assert_eq!(again.document, read.document, "{value}: {text}");
        assert_eq!(write(&again.document), text);
    }
    for line_height in ["0.000001px", "10000000000000000px", "1000000000000000px"] {
        let doc = token_doc(
            "typography",
            &format!(r#"{{ "fontSize": 14, "lineHeight": "{line_height}" }}"#),
        );
        let read = clean(&doc);
        let text = write(&read.document);
        assert!(text.contains(&format!("\"{line_height}\"")), "{text}");
        assert_eq!(clean(&text).document, read.document);
    }
}

#[test]
fn numbers_are_written_without_an_exponent_or_a_zero_fraction() {
    for (value, written) in [
        ("1e15", "1000000000000000"),
        ("1e16", "10000000000000000"),
        ("0.000001", "0.000001"),
        ("-2.5e-7", "-0.00000025"),
        ("12.0", "12"),
    ] {
        let read = clean(&token_doc("number", value));
        let text = write(&read.document);
        assert!(
            text.contains(&format!("\"value\": {written}\n")),
            "{value}: {text}"
        );
        assert_eq!(clean(&text).document, read.document);
    }
}

#[test]
fn a_reference_to_a_removed_token_is_reference_missing_on_check_read_and_resolve() {
    let mut document = base();
    document.remove_token("gray-1");
    let diagnostics = check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [("reference-missing", "/tokens/surface-app".to_owned())]
    );
    assert_eq!(
        sorted(&nsh_theme::read(&write(&document)).diagnostics),
        sorted(&diagnostics)
    );
    let resolved = resolve(&document, &Selection::new(), &[]);
    assert_eq!(sorted(&resolved.diagnostics), sorted(&diagnostics));
    assert!(resolved.get("surface-app").is_none());
}

#[test]
fn context_entries_order_items_and_roles_are_editable_in_place() {
    let mut document = base();
    let contrast = document.modifiers.get_mut("contrast").unwrap();
    contrast
        .contexts
        .get_mut("high")
        .unwrap()
        .set("gray-1", Value::Color(Color::rgb(255, 255, 255)));
    contrast
        .contexts
        .get_mut("high")
        .unwrap()
        .set("gray-12", Value::reference("gray-1"));
    contrast.insert_context(
        "low",
        Context::new().with("gray-12", Value::Color(Color::rgb(80, 80, 80))),
    );
    document.resolution_order.reverse();
    document.set_role("background", "gray-1");
    document.set_role("border", "gray-12");
    document.remove_role("foreground");
    assert!(check(&document).is_empty());
    let text = write(&document);
    let read = clean(&text);
    assert_eq!(read.document, document);
    let high = &read
        .document
        .modifiers
        .get("contrast")
        .unwrap()
        .contexts
        .get("high")
        .unwrap()
        .values;
    assert_eq!(high.keys().collect::<Vec<_>>(), ["gray-12", "gray-1"]);
    assert_eq!(read.document.resolution_order, ["contrast", "colorMode"]);
    assert_eq!(
        read.document.roles.iter().collect::<Vec<_>>(),
        [
            ("background", &"gray-1".to_owned()),
            ("border", &"gray-12".to_owned())
        ]
    );
}

#[test]
fn check_gives_read_s_codes_and_pointers_for_edited_documents() {
    let mut document = base();
    document.insert_token(
        "bad{name",
        Token::new(ValueType::Number, Value::Number(1.0)),
    );
    document.insert_token(
        "weight",
        Token::new(ValueType::Number, Value::Color(Color::rgb(0, 0, 0))),
    );
    document.insert_token(
        "p1",
        Token::new(ValueType::Number, Value::Number(1.0)).with_path("same"),
    );
    document.insert_token(
        "p2",
        Token::new(ValueType::Number, Value::Number(2.0)).with_path("same"),
    );
    document.insert_token(
        "typo",
        Token::new(
            ValueType::Typography,
            Value::Typography(Typography::new(14.0).with_font_weight(2000.0)),
        ),
    );
    document.insert_modifier(
        "lonely",
        Modifier::new("only").with_context("only", Context::new()),
    );
    document.resolution_order.push("lonely".to_owned());
    document.resolution_order.push("ghost".to_owned());
    document.resolution_order.push("contrast".to_owned());
    document
        .modifiers
        .get_mut("colorMode")
        .unwrap()
        .contexts
        .get_mut("dark")
        .unwrap()
        .set("ghost-token", Value::Number(1.0));
    let diagnostics = check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [
            ("bad-name", "/tokens/bad{name".to_owned()),
            ("bad-value", "/tokens/typo".to_owned()),
            ("bad-value", "/tokens/weight".to_owned()),
            ("duplicate-path", "/tokens/p2".to_owned()),
            ("order-duplicate", "/resolutionOrder/4".to_owned()),
            ("order-unknown", "/resolutionOrder/3".to_owned()),
            ("too-few-contexts", "/modifiers/lonely".to_owned()),
            (
                "undeclared-token",
                "/modifiers/colorMode/contexts/dark/ghost-token".to_owned()
            ),
        ]
    );
    assert_eq!(
        sorted(&nsh_theme::read(&write(&document)).diagnostics),
        sorted(&diagnostics)
    );
    // check never changes the document.
    assert!(document.token("weight").is_some());
}

#[test]
fn map_keeps_order_through_inserts_and_removals() {
    let mut map: nsh_theme::Map<i32> = [("a", 1), ("b", 2), ("c", 3)].into_iter().collect();
    map.insert("b", 20);
    map.insert("d", 4);
    map.remove("a");
    assert_eq!(
        map.iter()
            .map(|(k, v)| (k.to_owned(), *v))
            .collect::<Vec<_>>(),
        [
            ("b".to_owned(), 20),
            ("c".to_owned(), 3),
            ("d".to_owned(), 4)
        ]
    );
    assert_eq!(map.position("d"), Some(2));
    assert_eq!(map.get("c"), Some(&3));
    map.retain(|k, _| k != "c");
    assert_eq!(map.position("d"), Some(1));
    assert_eq!(map.get_index(0), Some(("b", &20)));
}

#[test]
fn json_nested_128_levels_deep_is_not_a_document() {
    let objects = |depth: usize| {
        format!(
            "{}{{}}{}",
            r#"{"a":"#.repeat(depth - 1),
            "}".repeat(depth - 1)
        )
    };
    one(&objects(127), "unknown-member", "/a");
    one(&objects(128), "not-a-document", "");
}
