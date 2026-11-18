//! Modifiers and `resolutionOrder`, references (chains, cycles, member
//! types, mismatches), the layers (MODEL-RESOLVE's cases (a)–(f)), patches
//! and roles.

use nsh_theme::{
    Color, Context, Duration, Easing, EasingKeyword, Font, LineHeight, Member, Patch, Resolved,
    RoleDeclaration, RolePattern, Selection, ThemeDocument, Token, Value, ValueType, check_roles,
    read, read_patch, resolve, resolve_role, write, write_patch,
};

use super::{clean, codes, one, sorted};

/// Primitives under semantic tokens: `colorMode` sets primitives, `accent`
/// points the intents at primitives, `density` sets numbers.
const EXAMPLE: &str = r##"{
    "name": "Example",
    "tokens": {
        "gray-1": { "kind": "color", "value": "#fcfcfc" },
        "gray-12": { "kind": "color", "value": "#202020" },
        "blue-9": { "kind": "color", "value": "#0090ff" },
        "cyan-9": { "kind": "color", "value": "#00a2c7" },
        "white": { "kind": "color", "value": "#ffffff" },
        "space-3": { "kind": "number", "value": 12 },
        "leading-body": { "kind": "number", "value": 20 },
        "surface-app": { "kind": "color", "value": "{gray-1}" },
        "text-primary": { "kind": "color", "value": "{gray-12}" },
        "intent-primary-bg": { "kind": "color", "value": "{blue-9}" },
        "intent-primary-fg": { "kind": "color", "value": "{white}" },
        "text-body": { "kind": "typography", "value": { "fontSize": 14, "lineHeight": "{leading-body}" } },
        "shadow-focus": { "kind": "shadow", "value": [{ "color": "{intent-primary-bg}", "spread": 2 }] }
    },
    "modifiers": {
        "colorMode": { "default": "light", "contexts": {
            "light": {},
            "dark": { "gray-1": "#111111", "gray-12": "#eeeeee", "blue-9": "#3b9eff", "cyan-9": "#00c2d7" }
        } },
        "accent": { "default": "blue", "contexts": {
            "blue": { "intent-primary-bg": "{blue-9}" },
            "cyan": { "intent-primary-bg": "{cyan-9}" }
        } },
        "density": { "default": "regular", "contexts": {
            "regular": {},
            "compact": { "space-3": 8, "leading-body": 16 }
        } }
    },
    "resolutionOrder": ["colorMode", "accent", "density"],
    "roles": {
        "primary": "intent-primary-bg",
        "primary-foreground": "intent-primary-fg",
        "background": "surface-app",
        "foreground": "text-primary"
    }
}"##;

fn example() -> ThemeDocument {
    clean(EXAMPLE).document
}

fn hex(text: &str) -> Value {
    Value::Color(Color::from_hex(text).unwrap())
}

fn pick(pairs: &[(&str, &str)]) -> Selection {
    pairs
        .iter()
        .fold(Selection::new(), |s, (m, c)| s.with(*m, *c))
}

fn patch(text: &str) -> Patch {
    let (patch, diagnostics) = read_patch(text);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    patch
}

fn quiet(document: &ThemeDocument, selection: &Selection, patches: &[&Patch]) -> Resolved {
    let resolved = resolve(document, selection, patches);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:#?}",
        resolved.diagnostics
    );
    resolved
}

// ---------------------------------------------------------------------------
// Modifiers

#[test]
fn a_modifier_with_fewer_than_two_contexts_is_left_out() {
    for contexts in [r#""contexts": {}"#, r#""contexts": { "a": {} }"#, ""] {
        let comma = if contexts.is_empty() { "" } else { "," };
        let text = format!(
            r#"{{ "tokens": {{}}, "modifiers": {{ "m": {{ "default": "a"{comma} {contexts} }} }}, "resolutionOrder": ["m"] }}"#
        );
        let read = read(&text);
        // With no context `a`, the default names none either: both hold.
        let expected: &[&str] = if contexts.contains("\"a\"") {
            &["too-few-contexts"]
        } else {
            &["too-few-contexts", "bad-default"]
        };
        assert_eq!(codes(&read.diagnostics), expected, "{text}");
        assert!(read.diagnostics.iter().all(|d| d.pointer == "/modifiers/m"));
        assert!(read.document.modifiers.is_empty());
        assert!(read.document.resolution_order.is_empty());
    }
    // A missing default with one context: both, in the same order.
    let read = read(
        r#"{ "tokens": {}, "modifiers": { "m": { "contexts": { "a": {} } } }, "resolutionOrder": ["m"] }"#,
    );
    assert_eq!(
        codes(&read.diagnostics),
        ["too-few-contexts", "bad-default"]
    );
    assert!(read.document.modifiers.is_empty());
}

#[test]
fn a_modifier_without_a_default_or_with_a_default_that_names_no_context_is_left_out() {
    let read = one(
        r#"{ "tokens": {}, "modifiers": { "m": { "contexts": { "a": {}, "b": {} } } }, "resolutionOrder": ["m"] }"#,
        "bad-default",
        "/modifiers/m",
    );
    assert!(read.document.modifiers.is_empty());
    let read = one(
        r#"{ "tokens": {}, "modifiers": { "m": { "default": "c", "contexts": { "a": {}, "b": {} } } }, "resolutionOrder": ["m"] }"#,
        "bad-default",
        "/modifiers/m",
    );
    assert!(read.document.modifiers.is_empty());
}

#[test]
fn a_context_entry_for_an_undeclared_token_is_left_out_and_adds_no_name() {
    let read = one(
        r##"{ "tokens": { "t": { "kind": "color", "value": "#000" } },
            "modifiers": { "m": { "default": "a", "contexts": { "a": {}, "b": { "t": "#fff", "new-name": "#fff" } } } },
            "resolutionOrder": ["m"] }"##,
        "undeclared-token",
        "/modifiers/m/contexts/b/new-name",
    );
    let b = &read
        .document
        .modifiers
        .get("m")
        .unwrap()
        .contexts
        .get("b")
        .unwrap()
        .values;
    assert_eq!(b.keys().collect::<Vec<_>>(), ["t"]);
    let resolved = quiet(&read.document, &pick(&[("m", "b")]), &[]);
    assert_eq!(resolved.values.keys().collect::<Vec<_>>(), ["t"]);
}

#[test]
fn a_context_entry_of_the_wrong_type_is_left_out_and_the_context_stays() {
    let read = one(
        r##"{ "tokens": { "t": { "kind": "color", "value": "#000" }, "n": { "kind": "number", "value": 1 } },
            "modifiers": { "m": { "default": "a", "contexts": { "a": {}, "b": { "t": 3, "n": 2 } } } },
            "resolutionOrder": ["m"] }"##,
        "bad-value",
        "/modifiers/m/contexts/b/t",
    );
    let modifier = read.document.modifiers.get("m").unwrap();
    assert_eq!(modifier.contexts.len(), 2);
    let resolved = quiet(&read.document, &pick(&[("m", "b")]), &[]);
    assert_eq!(resolved.get("t"), Some(&hex("#000")));
    assert_eq!(resolved.get("n"), Some(&Value::Number(2.0)));
}

#[test]
fn an_empty_context_is_valid_and_changes_nothing() {
    let document = example();
    let light = quiet(&document, &pick(&[("colorMode", "light")]), &[]);
    let base = quiet(&document, &Selection::new(), &[]);
    assert_eq!(light, base);
    assert_eq!(light.get("gray-1"), Some(&hex("#fcfcfc")));
}

#[test]
fn a_missing_resolution_order_gives_order_missing_once_per_modifier() {
    let read = read(
        r#"{ "tokens": { "n": { "kind": "number", "value": 1 } }, "modifiers": {
            "a": { "default": "x", "contexts": { "x": { "n": 2 }, "y": {} } },
            "b": { "default": "x", "contexts": { "x": { "n": 3 }, "y": {} } }
        } }"#,
    );
    assert_eq!(
        sorted(&read.diagnostics),
        [
            ("order-missing", "/modifiers/a".to_owned()),
            ("order-missing", "/modifiers/b".to_owned())
        ]
    );
    assert!(read.document.modifiers.is_empty());
    assert_eq!(
        quiet(&read.document, &Selection::new(), &[]).get("n"),
        Some(&Value::Number(1.0))
    );
}

#[test]
fn a_modifier_missing_from_resolution_order_is_left_out() {
    let read = one(
        r#"{ "tokens": { "n": { "kind": "number", "value": 1 } }, "modifiers": {
            "a": { "default": "x", "contexts": { "x": { "n": 2 }, "y": {} } },
            "b": { "default": "x", "contexts": { "x": { "n": 3 }, "y": {} } }
        }, "resolutionOrder": ["a"] }"#,
        "order-missing",
        "/modifiers/b",
    );
    assert_eq!(read.document.modifiers.keys().collect::<Vec<_>>(), ["a"]);
    assert_eq!(
        quiet(&read.document, &Selection::new(), &[]).get("n"),
        Some(&Value::Number(2.0))
    );
}

#[test]
fn a_resolution_order_item_that_names_no_modifier_is_ignored() {
    let read = one(
        r#"{ "tokens": {}, "modifiers": { "a": { "default": "x", "contexts": { "x": {}, "y": {} } } },
            "resolutionOrder": ["ghost", "a"] }"#,
        "order-unknown",
        "/resolutionOrder/0",
    );
    assert_eq!(read.document.resolution_order, ["a"]);
}

#[test]
fn a_second_listing_in_resolution_order_is_ignored() {
    let read = one(
        r#"{ "tokens": {}, "modifiers": {
            "a": { "default": "x", "contexts": { "x": {}, "y": {} } },
            "b": { "default": "x", "contexts": { "x": {}, "y": {} } }
        }, "resolutionOrder": ["a", "b", "a"] }"#,
        "order-duplicate",
        "/resolutionOrder/2",
    );
    assert_eq!(read.document.resolution_order, ["a", "b"]);
}

#[test]
fn json_member_order_never_decides() {
    let reordered = r##"{
        "resolutionOrder": ["colorMode", "accent", "density"],
        "roles": { "foreground": "text-primary", "background": "surface-app", "primary-foreground": "intent-primary-fg", "primary": "intent-primary-bg" },
        "modifiers": {
            "density": { "contexts": { "compact": { "leading-body": 16, "space-3": 8 }, "regular": {} }, "default": "regular" },
            "accent": { "contexts": { "cyan": { "intent-primary-bg": "{cyan-9}" }, "blue": { "intent-primary-bg": "{blue-9}" } }, "default": "blue" },
            "colorMode": { "contexts": { "dark": { "cyan-9": "#00c2d7", "blue-9": "#3b9eff", "gray-12": "#eeeeee", "gray-1": "#111111" }, "light": {} }, "default": "light" }
        },
        "tokens": {
            "shadow-focus": { "value": [{ "spread": 2, "color": "{intent-primary-bg}" }], "kind": "shadow" },
            "text-body": { "value": { "lineHeight": "{leading-body}", "fontSize": 14 }, "kind": "typography" },
            "intent-primary-fg": { "value": "{white}", "kind": "color" },
            "intent-primary-bg": { "value": "{blue-9}", "kind": "color" },
            "text-primary": { "value": "{gray-12}", "kind": "color" },
            "surface-app": { "value": "{gray-1}", "kind": "color" },
            "leading-body": { "value": 20, "kind": "number" },
            "space-3": { "value": 12, "kind": "number" },
            "white": { "value": "#ffffff", "kind": "color" },
            "cyan-9": { "value": "#00a2c7", "kind": "color" },
            "blue-9": { "value": "#0090ff", "kind": "color" },
            "gray-12": { "value": "#202020", "kind": "color" },
            "gray-1": { "value": "#fcfcfc", "kind": "color" }
        }
    }"##;
    let a = example();
    let b = clean(reordered).document;
    for selection in [
        Selection::new(),
        pick(&[
            ("colorMode", "dark"),
            ("accent", "cyan"),
            ("density", "compact"),
        ]),
    ] {
        let mut x: Vec<_> = quiet(&a, &selection, &[])
            .values
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect();
        let mut y: Vec<_> = quiet(&b, &selection, &[])
            .values
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect();
        x.sort_by(|p, q| p.0.cmp(&q.0));
        y.sort_by(|p, q| p.0.cmp(&q.0));
        assert_eq!(x, y);
    }
}

#[test]
fn a_selected_context_that_does_not_exist_warns_and_uses_the_default() {
    let document = example();
    let resolved = resolve(&document, &pick(&[("accent", "magenta")]), &[]);
    assert_eq!(codes(&resolved.diagnostics), ["unknown-context"]);
    assert_eq!(resolved.diagnostics[0].pointer, "/modifiers/accent");
    assert_eq!(resolved.get("intent-primary-bg"), Some(&hex("#0090ff")));
    // A modifier the document lacks is the core's to report.
    assert!(
        resolve(&document, &pick(&[("brand", "x")]), &[])
            .diagnostics
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// MODEL-RESOLVE's cases

#[test]
fn a_references_join_modifiers() {
    let document = example();
    let dark = &document
        .modifiers
        .get("colorMode")
        .unwrap()
        .contexts
        .get("dark")
        .unwrap()
        .values;
    assert!(
        dark.values().all(|v| v.as_reference().is_none()),
        "colorMode sets only primitives"
    );
    for (_, context) in document.modifiers.get("accent").unwrap().contexts.iter() {
        assert!(
            context.values.values().all(|v| v.as_reference().is_some()),
            "accent sets only references"
        );
    }
    let blue = quiet(
        &document,
        &pick(&[("colorMode", "dark"), ("accent", "blue")]),
        &[],
    );
    assert_eq!(blue.get("intent-primary-bg"), Some(&hex("#3b9eff")));
    let cyan = quiet(
        &document,
        &pick(&[("colorMode", "dark"), ("accent", "cyan")]),
        &[],
    );
    assert_eq!(cyan.get("intent-primary-bg"), Some(&hex("#00c2d7")));
    let light_cyan = quiet(&document, &pick(&[("accent", "cyan")]), &[]);
    assert_eq!(light_cyan.get("intent-primary-bg"), Some(&hex("#00a2c7")));
}

const TWO_SETTERS: &str = r##"{
    "tokens": { "t": { "kind": "color", "value": "#000000" } },
    "modifiers": {
        "first": { "default": "off", "contexts": { "off": {}, "on": { "t": "#111111" } } },
        "second": { "default": "off", "contexts": { "off": {}, "on": { "t": "#222222" } } }
    },
    "resolutionOrder": ["first", "second"]
}"##;

#[test]
fn b_order_decides() {
    let mut document = clean(TWO_SETTERS).document;
    let both = pick(&[("first", "on"), ("second", "on")]);
    assert_eq!(quiet(&document, &both, &[]).get("t"), Some(&hex("#222222")));
    document.resolution_order.reverse();
    assert!(nsh_theme::check(&document).is_empty());
    assert_eq!(quiet(&document, &both, &[]).get("t"), Some(&hex("#111111")));
    assert_eq!(
        quiet(&document, &pick(&[("second", "on")]), &[]).get("t"),
        Some(&hex("#222222"))
    );
}

#[test]
fn c_a_patch_survives_a_context_switch() {
    let document = example();
    let app = patch(r##"{ "values": { "surface-app": "#ff0000" } }"##);
    for mode in ["light", "dark"] {
        let resolved = quiet(&document, &pick(&[("colorMode", mode)]), &[&app]);
        assert_eq!(resolved.get("surface-app"), Some(&hex("#ff0000")), "{mode}");
    }
    let dark = quiet(&document, &pick(&[("colorMode", "dark")]), &[&app]);
    assert_eq!(dark.get("text-primary"), Some(&hex("#eeeeee")));
}

#[test]
fn d_a_patch_reaches_through_references() {
    let document = example();
    let app = patch(r##"{ "values": { "blue-9": "#abcdef" } }"##);
    for mode in ["light", "dark"] {
        let resolved = quiet(&document, &pick(&[("colorMode", mode)]), &[&app]);
        assert_eq!(
            resolved.get("intent-primary-bg"),
            Some(&hex("#abcdef")),
            "{mode}"
        );
        let Some(Value::Shadow(shadow)) = resolved.get("shadow-focus") else {
            panic!()
        };
        assert_eq!(
            shadow.0[0].color,
            Member::Value(Color::rgb(0xab, 0xcd, 0xef))
        );
    }
}

#[test]
fn e_an_empty_selection_gives_the_defaults() {
    let document = example();
    let empty = quiet(&document, &Selection::new(), &[]);
    let defaults = quiet(
        &document,
        &pick(&[
            ("colorMode", "light"),
            ("accent", "blue"),
            ("density", "regular"),
        ]),
        &[],
    );
    assert_eq!(empty, defaults);
    assert_eq!(empty.get("surface-app"), Some(&hex("#fcfcfc")));
    assert_eq!(empty.get("space-3"), Some(&Value::Number(12.0)));
    assert_eq!(empty.values.len(), document.tokens.len());
    assert!(empty.values.values().all(|v| !v.has_references()));
}

#[test]
fn f_changed_lists_the_tokens_and_roles_that_differ() {
    let document = example();
    let light = quiet(&document, &Selection::new(), &[]);
    let dark = quiet(&document, &pick(&[("colorMode", "dark")]), &[]);
    let changes = light.changed(&dark);
    assert_eq!(
        changes.tokens,
        [
            "gray-1",
            "gray-12",
            "blue-9",
            "cyan-9",
            "surface-app",
            "text-primary",
            "intent-primary-bg",
            "shadow-focus"
        ]
    );
    assert_eq!(changes.roles, ["primary", "background", "foreground"]);
    assert!(light.changed(&light).is_empty());

    let remap = patch(r#"{ "roles": { "primary": "intent-primary-fg" } }"#);
    let remapped = quiet(&document, &Selection::new(), &[&remap]);
    let changes = light.changed(&remapped);
    assert!(changes.tokens.is_empty());
    assert_eq!(changes.roles, ["primary"]);
    assert_eq!(
        remapped.roles.get("primary").map(String::as_str),
        Some("intent-primary-fg")
    );
}

#[test]
fn f_changed_lists_names_one_side_lacks() {
    let mut document = example();
    let before = quiet(&document, &Selection::new(), &[]);
    document.insert_token(
        "ring",
        Token::new(ValueType::Color, Value::reference("blue-9")),
    );
    document.set_role("ring", "ring");
    document.remove_role("foreground");
    let after = quiet(&document, &Selection::new(), &[]);
    let changes = before.changed(&after);
    assert_eq!(changes.tokens, ["ring"]);
    assert_eq!(changes.roles, ["foreground", "ring"]);
}

// ---------------------------------------------------------------------------
// References

#[test]
fn a_chain_three_deep_resolves_through() {
    let read = clean(
        r##"{ "tokens": {
            "a": { "kind": "color", "value": "{b}" },
            "b": { "kind": "color", "value": "{c}" },
            "c": { "kind": "color", "value": "{d}" },
            "d": { "kind": "color", "value": "#123456" }
        } }"##,
    );
    let resolved = quiet(&read.document, &Selection::new(), &[]);
    for name in ["a", "b", "c", "d"] {
        assert_eq!(resolved.get(name), Some(&hex("#123456")));
    }
}

const PICKED_CYCLE: &str = r##"{
    "tokens": {
        "a": { "kind": "color", "value": "#111111" },
        "b": { "kind": "color", "value": "#222222" },
        "c": { "kind": "color", "value": "{b}" },
        "ring": { "kind": "shadow", "value": [{ "color": "{a}", "spread": 2 }] }
    },
    "modifiers": {
        "first": { "default": "off", "contexts": { "off": {}, "on": { "a": "{b}" } } },
        "second": { "default": "off", "contexts": { "off": {}, "on": { "b": "{a}" } } }
    },
    "resolutionOrder": ["first", "second"]
}"##;

#[test]
fn a_cycle_two_picks_make_loses_only_its_highest_entry() {
    let document = clean(PICKED_CYCLE).document;
    let defaults = quiet(&document, &Selection::new(), &[]);
    assert_eq!(defaults.get("a"), Some(&hex("#111111")));
    quiet(&document, &pick(&[("first", "on")]), &[]);
    quiet(&document, &pick(&[("second", "on")]), &[]);

    let both = resolve(&document, &pick(&[("first", "on"), ("second", "on")]), &[]);
    assert_eq!(codes(&both.diagnostics), ["reference-cycle"]);
    assert_eq!(
        both.diagnostics[0].pointer,
        "/modifiers/second/contexts/on/b"
    );
    assert!(both.diagnostics[0].message.contains("`b`"));
    // b falls to its own value; a, a whole value and a member that depend on
    // the cycle resolve through what remains.
    assert_eq!(both.get("b"), Some(&hex("#222222")));
    assert_eq!(both.get("a"), Some(&hex("#222222")));
    assert_eq!(both.get("c"), Some(&hex("#222222")));
    let Some(Value::Shadow(ring)) = both.get("ring") else {
        panic!()
    };
    assert_eq!(ring.0[0].color, Member::Value(Color::rgb(0x22, 0x22, 0x22)));
    assert_eq!(both.values.len(), 4);
}

#[test]
fn within_one_layer_the_last_entry_of_the_cycle_is_left_out() {
    let document = clean(
        r##"{
            "tokens": { "a": { "kind": "number", "value": 1 }, "b": { "kind": "number", "value": 2 } },
            "modifiers": { "m": { "default": "off", "contexts": { "off": {}, "on": { "b": "{a}", "a": "{b}" } } } },
            "resolutionOrder": ["m"]
        }"##,
    )
    .document;
    let resolved = resolve(&document, &pick(&[("m", "on")]), &[]);
    assert_eq!(codes(&resolved.diagnostics), ["reference-cycle"]);
    assert_eq!(
        resolved.diagnostics[0].pointer,
        "/modifiers/m/contexts/on/a"
    );
    assert_eq!(resolved.get("a"), Some(&Value::Number(1.0)));
    assert_eq!(resolved.get("b"), Some(&Value::Number(1.0)));
}

#[test]
fn a_patch_entry_in_a_cycle_ranks_above_a_context() {
    let document = clean(PICKED_CYCLE).document;
    let host = patch(r#"{ "values": { "b": "{a}" } }"#);
    let resolved = resolve(&document, &pick(&[("first", "on")]), &[&host]);
    assert_eq!(codes(&resolved.diagnostics), ["reference-cycle"]);
    assert_eq!(resolved.diagnostics[0].pointer, "/values/b");
    assert!(resolved.diagnostics[0].message.contains("patch 1"));
    assert_eq!(resolved.get("a"), Some(&hex("#222222")));
}

#[test]
fn a_cycle_of_own_values_leaves_its_last_token_without_a_value() {
    let read = clean(
        r#"{ "tokens": {
            "a": { "kind": "number", "value": "{b}" },
            "b": { "kind": "number", "value": "{a}" },
            "c": { "kind": "number", "value": 3 }
        } }"#,
    );
    let resolved = resolve(&read.document, &Selection::new(), &[]);
    assert_eq!(codes(&resolved.diagnostics), ["reference-cycle"]);
    assert_eq!(resolved.diagnostics[0].pointer, "/tokens/b");
    assert_eq!(resolved.values.keys().collect::<Vec<_>>(), ["c"]);
}

#[test]
fn a_member_reference_of_each_member_type_resolves() {
    let read = clean(
        r##"{ "tokens": {
            "c": { "kind": "color", "value": "#112233" },
            "n": { "kind": "number", "value": 4 },
            "f": { "kind": "font", "value": "Inter, sans-serif" },
            "d": { "kind": "duration", "value": "120ms" },
            "e": { "kind": "easing", "value": "ease-out" },
            "type": { "kind": "typography", "value": { "fontSize": "{n}", "fontFamily": "{f}" } },
            "border": { "kind": "border", "value": { "color": "{c}", "width": "{n}", "style": "solid" } },
            "motion": { "kind": "transition", "value": { "duration": "{d}", "easing": "{e}" } }
        } }"##,
    );
    let resolved = quiet(&read.document, &Selection::new(), &[]);
    let Some(Value::Typography(t)) = resolved.get("type") else {
        panic!()
    };
    assert_eq!(t.font_size, Member::Value(4.0));
    assert_eq!(
        t.font_family,
        Some(Member::Value(Font::new("Inter, sans-serif")))
    );
    let Some(Value::Border(b)) = resolved.get("border") else {
        panic!()
    };
    assert_eq!(
        (b.color.clone(), b.width.clone()),
        (
            Member::Value(Color::rgb(0x11, 0x22, 0x33)),
            Member::Value(4.0)
        )
    );
    let Some(Value::Transition(m)) = resolved.get("motion") else {
        panic!()
    };
    assert_eq!(m.duration, Member::Value(Duration::from_millis(120.0)));
    assert_eq!(
        m.easing,
        Member::Value(Easing::Keyword(EasingKeyword::EaseOut))
    );
}

#[test]
fn a_line_height_reference_follows_density_as_points() {
    let document = example();
    for (context, expected) in [("regular", 20.0), ("compact", 16.0)] {
        let resolved = quiet(&document, &pick(&[("density", context)]), &[]);
        let Some(Value::Typography(t)) = resolved.get("text-body") else {
            panic!()
        };
        assert_eq!(
            t.line_height,
            Some(Member::Value(LineHeight::Px(expected))),
            "{context}"
        );
    }
}

#[test]
fn a_type_mismatch_at_read_time_is_reference_type() {
    let read = one(
        r#"{ "tokens": {
            "n": { "kind": "number", "value": 1 },
            "c": { "kind": "color", "value": "{n}" }
        } }"#,
        "reference-type",
        "/tokens/c",
    );
    assert!(read.document.token("c").is_none());
}

#[test]
fn a_type_mismatch_a_patch_introduces_is_refused_and_skipped() {
    let document = example();
    let app = patch(
        r#"{ "values": { "surface-app": "{space-3}", "text-body": { "fontSize": "{gray-1}" } } }"#,
    );
    let diagnostics = app.check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [
            ("reference-type", "/values/surface-app".to_owned()),
            ("reference-type", "/values/text-body".to_owned())
        ]
    );
    let resolved = quiet(&document, &Selection::new(), &[&app]);
    assert_eq!(resolved.get("surface-app"), Some(&hex("#fcfcfc")));
}

#[test]
fn many_independent_cycles_give_one_diagnostic_each() {
    let mut document = ThemeDocument::new();
    let pairs = 5_000;
    for i in 0..pairs {
        document.insert_token(
            format!("a{i}"),
            Token::new(ValueType::Number, Value::reference(format!("b{i}"))),
        );
        document.insert_token(
            format!("b{i}"),
            Token::new(ValueType::Number, Value::reference(format!("a{i}"))),
        );
        document.insert_token(
            format!("c{i}"),
            Token::new(ValueType::Number, Value::Number(i as f64)),
        );
    }
    let started = std::time::Instant::now();
    let resolved = resolve(&document, &Selection::new(), &[]);
    let elapsed = started.elapsed();
    assert_eq!(resolved.diagnostics.len(), pairs);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|d| d.code == "reference-cycle")
    );
    let pointers: std::collections::HashSet<_> = resolved
        .diagnostics
        .iter()
        .map(|d| d.pointer.as_str())
        .collect();
    assert_eq!(pointers.len(), pairs);
    // Each cycle loses the entry of its last token, b{i}.
    assert!(pointers.contains("/tokens/b0") && pointers.contains("/tokens/b4999"));
    assert_eq!(resolved.values.len(), pairs);
    assert_eq!(resolved.get("c4999"), Some(&Value::Number(4999.0)));
    // One walk, not one per cycle (which took minutes here).
    assert!(elapsed < std::time::Duration::from_secs(5), "{elapsed:?}");
}

#[test]
fn a_patch_built_with_integer_json_round_trips() {
    let document = example();
    let mut app = Patch::new();
    app.set_value("space-3", serde_json::json!(10));
    app.set_value(
        "text-body",
        serde_json::json!({ "fontSize": 15, "fontWeight": 500, "lineHeight": "18px" }),
    );
    app.set_role("primary", "intent-primary-fg");
    assert!(app.check(&document).is_empty());
    let written = write_patch(&app);
    let (again, diagnostics) = read_patch(&written);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(again, app);
    assert_eq!(write_patch(&again), written);
    let resolved = quiet(&document, &Selection::new(), &[&again]);
    assert_eq!(resolved.get("space-3"), Some(&Value::Number(10.0)));
}

#[test]
fn a_long_chain_and_a_long_ring_resolve_without_recursion() {
    let mut document = ThemeDocument::new();
    let n = 20_000;
    document.insert_token("t0", Token::new(ValueType::Number, Value::Number(7.0)));
    for i in 1..n {
        document.insert_token(
            format!("t{i}"),
            Token::new(ValueType::Number, Value::reference(format!("t{}", i - 1))),
        );
    }
    assert!(nsh_theme::check(&document).is_empty());
    let resolved = quiet(&document, &Selection::new(), &[]);
    assert_eq!(
        resolved.get(&format!("t{}", n - 1)),
        Some(&Value::Number(7.0))
    );

    let mut ring = ThemeDocument::new();
    for i in 0..2_000 {
        ring.insert_token(
            format!("r{i}"),
            Token::new(
                ValueType::Number,
                Value::reference(format!("r{}", (i + 1) % 2_000)),
            ),
        );
    }
    let resolved = resolve(&ring, &Selection::new(), &[]);
    assert_eq!(codes(&resolved.diagnostics), ["reference-cycle"]);
    assert!(resolved.values.is_empty());
}

// ---------------------------------------------------------------------------
// Patches

#[test]
fn read_patch_reads_values_and_roles() {
    let (patch, diagnostics) = read_patch(
        r##"{ "values": { "surface-app": "#FF0000", "space-3": 10, "space-3": 11 }, "roles": { "primary": "intent-primary-fg", "x": 1 }, "extra": true }"##,
    );
    assert_eq!(
        sorted(&diagnostics),
        [
            ("bad-value", "/roles/x".to_owned()),
            ("duplicate-member", "/values/space-3".to_owned()),
            ("unknown-member", "/extra".to_owned()),
        ]
    );
    assert_eq!(
        patch.values.get("surface-app"),
        Some(&serde_json::json!("#FF0000"))
    );
    assert_eq!(patch.values.get("space-3"), Some(&serde_json::json!(10.0)));
    assert_eq!(
        patch.roles.get("primary").map(String::as_str),
        Some("intent-primary-fg")
    );
    for text in ["", "[]", "{", "null"] {
        let (patch, diagnostics) = read_patch(text);
        assert_eq!(codes(&diagnostics), ["not-a-document"]);
        assert!(patch.is_empty());
    }
    let (_, diagnostics) = read_patch(r#"{ "values": [] }"#);
    assert_eq!(sorted(&diagnostics), [("bad-value", "/values".to_owned())]);
}

#[test]
fn write_patch_round_trips() {
    let text = r##"{ "values": { "surface-app": "#ff0000", "space-3": 10.0, "text-body": { "fontSize": 15, "lineHeight": "18px" } }, "roles": { "primary": "intent-primary-fg" } }"##;
    let first = patch(text);
    let written = write_patch(&first);
    let second = patch(&written);
    assert_eq!(second, first);
    assert_eq!(write_patch(&second), written);
    assert!(written.contains("\"space-3\": 10,"), "{written}");
    assert!(
        written.starts_with("{\n  \"values\": {\n    \"surface-app\": \"#ff0000\""),
        "{written}"
    );
    assert_eq!(write_patch(&Patch::new()), "{}\n");
}

#[test]
fn patch_check_is_the_one_validator() {
    let document = example();
    let app = patch(
        r##"{
            "values": {
                "brand-new": "#000000",
                "surface-app": "not a color",
                "text-primary": "{missing}",
                "space-3": 4,
                "text-body": { "fontSize": 15, "textCase": "upper" }
            },
            "roles": { "accent": "missing-token", "primary": "intent-primary-fg" }
        }"##,
    );
    let diagnostics = app.check(&document);
    assert_eq!(
        sorted(&diagnostics),
        [
            ("bad-value", "/values/surface-app".to_owned()),
            ("reference-missing", "/values/text-primary".to_owned()),
            ("role-token-missing", "/roles/accent".to_owned()),
            ("undeclared-token", "/values/brand-new".to_owned()),
            ("unknown-member", "/values/text-body/textCase".to_owned()),
        ]
    );
    // resolve skips exactly what check refuses.
    let resolved = quiet(&document, &Selection::new(), &[&app]);
    assert!(resolved.get("brand-new").is_none());
    assert_eq!(resolved.get("surface-app"), Some(&hex("#fcfcfc")));
    assert_eq!(resolved.get("text-primary"), Some(&hex("#202020")));
    assert_eq!(resolved.get("space-3"), Some(&Value::Number(4.0)));
    assert_eq!(resolved.roles.get("accent"), None);
    assert_eq!(
        resolved.roles.get("primary").map(String::as_str),
        Some("intent-primary-fg")
    );
}

#[test]
fn an_override_of_a_name_b_lacks_applies_again_when_a_is_active() {
    let a = example();
    let b = clean(r##"{ "tokens": { "surface-app": { "kind": "color", "value": "#000000" } } }"##)
        .document;
    let app = patch(
        r##"{ "values": { "surface-app": "#ff0000", "gray-1": "#00ff00" }, "roles": { "background": "gray-1" } }"##,
    );

    let on_a = quiet(&a, &Selection::new(), &[&app]);
    assert_eq!(on_a.get("gray-1"), Some(&hex("#00ff00")));
    assert_eq!(
        on_a.roles.get("background").map(String::as_str),
        Some("gray-1")
    );

    assert_eq!(
        sorted(&app.check(&b)),
        [
            ("role-token-missing", "/roles/background".to_owned()),
            ("undeclared-token", "/values/gray-1".to_owned())
        ]
    );
    let on_b = quiet(&b, &Selection::new(), &[&app]);
    assert_eq!(on_b.get("surface-app"), Some(&hex("#ff0000")));
    assert!(on_b.get("gray-1").is_none());
    assert!(on_b.roles.is_empty());

    let again = quiet(&a, &Selection::new(), &[&app]);
    assert_eq!(again, on_a);
}

#[test]
fn a_later_patch_wins_and_merge_keeps_one_entry_per_name() {
    let document = example();
    let app = patch(r##"{ "values": { "surface-app": "#111111", "space-3": 1 } }"##);
    let host = patch(r##"{ "values": { "surface-app": "#222222" } }"##);
    let resolved = quiet(&document, &Selection::new(), &[&app, &host]);
    assert_eq!(resolved.get("surface-app"), Some(&hex("#222222")));
    assert_eq!(resolved.get("space-3"), Some(&Value::Number(1.0)));

    let mut merged = app.clone();
    merged.merge(&host);
    merged.merge(&patch(
        r##"{ "values": { "space-3": 2 }, "roles": { "primary": "white" } }"##,
    ));
    assert_eq!(
        merged.values.keys().collect::<Vec<_>>(),
        ["surface-app", "space-3"]
    );
    let resolved = quiet(&document, &Selection::new(), &[&merged]);
    assert_eq!(resolved.get("surface-app"), Some(&hex("#222222")));
    assert_eq!(resolved.get("space-3"), Some(&Value::Number(2.0)));
    merged.remove_value("space-3");
    merged.remove_role("primary");
    assert_eq!(
        quiet(&document, &Selection::new(), &[&merged]).get("space-3"),
        Some(&Value::Number(12.0))
    );
}

#[test]
fn typed_values_set_in_a_patch_resolve() {
    let document = example();
    let mut app = Patch::new();
    app.set("leading-body", &Value::Number(30.0));
    app.set("intent-primary-bg", &Value::reference("cyan-9"));
    app.set_role("primary", "intent-primary-bg");
    assert!(app.check(&document).is_empty());
    let resolved = quiet(&document, &Selection::new(), &[&app]);
    assert_eq!(resolved.get("intent-primary-bg"), Some(&hex("#00a2c7")));
    let Some(Value::Typography(t)) = resolved.get("text-body") else {
        panic!()
    };
    assert_eq!(t.line_height, Some(Member::Value(LineHeight::Px(30.0))));
}

// ---------------------------------------------------------------------------
// Roles

fn library() -> Vec<RoleDeclaration> {
    vec![
        RoleDeclaration::new("primary", ValueType::Color).with_doc("The main action's background."),
        RoleDeclaration::new("background", ValueType::Color).with_required(true),
        RoleDeclaration::new("font-body", ValueType::Font),
        RoleDeclaration::new("radius", ValueType::Number),
    ]
}

fn tailwind() -> Vec<RolePattern> {
    vec![
        RolePattern::new("radius-", &[ValueType::Number]),
        RolePattern::new("shadow-", &[ValueType::Shadow]),
        RolePattern::new(
            "text-",
            &[ValueType::Color, ValueType::Typography, ValueType::Number],
        ),
    ]
}

#[test]
fn a_role_resolves_through_its_layers() {
    let document = example();
    let (roles, patterns) = (library(), tailwind());
    assert_eq!(
        resolve_role("primary", &document, &[], &roles, &patterns).as_deref(),
        Some("intent-primary-bg")
    );
    let first = patch(r#"{ "roles": { "primary": "cyan-9" } }"#);
    let second = patch(r#"{ "roles": { "primary": "blue-9" } }"#);
    assert_eq!(
        resolve_role("primary", &document, &[&first], &roles, &patterns).as_deref(),
        Some("cyan-9")
    );
    assert_eq!(
        resolve_role("primary", &document, &[&first, &second], &roles, &patterns).as_deref(),
        Some("blue-9")
    );
    let resolved = quiet(&document, &Selection::new(), &[&first, &second]);
    assert_eq!(
        resolved.roles.get("primary").map(String::as_str),
        Some("blue-9")
    );
    assert!(check_roles(&document, &[&first, &second], &resolved, &roles, &patterns).is_empty());
}

#[test]
fn a_wrong_typed_patch_mapping_over_a_valid_document_mapping_reads_the_document_s() {
    let document = example();
    let (roles, patterns) = (library(), tailwind());
    let app = patch(r#"{ "roles": { "primary": "space-3" } }"#);
    assert!(app.check(&document).is_empty());
    assert_eq!(
        resolve_role("primary", &document, &[&app], &roles, &patterns).as_deref(),
        Some("intent-primary-bg")
    );
    let resolved = quiet(&document, &Selection::new(), &[&app]);
    let diagnostics = check_roles(&document, &[&app], &resolved, &roles, &patterns);
    assert_eq!(
        sorted(&diagnostics),
        [("role-type", "/roles/primary".to_owned())]
    );
    assert!(diagnostics[0].message.contains("Patch 1"));
}

#[test]
fn a_role_without_a_mapping_reads_the_token_of_its_own_name() {
    let mut document = example();
    document.insert_token("radius", Token::new(ValueType::Number, Value::Number(6.0)));
    let (roles, patterns) = (library(), tailwind());
    assert_eq!(
        resolve_role("radius", &document, &[], &roles, &patterns).as_deref(),
        Some("radius")
    );
    assert_eq!(
        resolve_role("white", &document, &[], &roles, &patterns).as_deref(),
        Some("white")
    );
    assert_eq!(
        resolve_role("nothing", &document, &[], &roles, &patterns),
        None
    );

    document.insert_token(
        "radius",
        Token::new(ValueType::Color, Value::Color(Color::rgb(0, 0, 0))),
    );
    assert_eq!(
        resolve_role("radius", &document, &[], &roles, &patterns),
        None
    );
    let resolved = quiet(&document, &Selection::new(), &[]);
    let diagnostics = check_roles(&document, &[], &resolved, &roles, &patterns);
    assert_eq!(
        sorted(&diagnostics),
        [("role-type", "/tokens/radius".to_owned())]
    );
}

#[test]
fn tailwind_patterns_check_role_families() {
    let document = example();
    let (roles, patterns) = (library(), tailwind());
    let resolved = quiet(&document, &Selection::new(), &[]);

    let mut clean_doc = document.clone();
    clean_doc.set_role("radius-card", "space-3");
    clean_doc.set_role("text-title", "text-body");
    clean_doc.set_role("brand", "blue-9");
    assert!(check_roles(&clean_doc, &[], &resolved, &roles, &patterns).is_empty());
    assert_eq!(
        resolve_role("radius-card", &clean_doc, &[], &roles, &patterns).as_deref(),
        Some("space-3")
    );

    let mut wrong = document.clone();
    wrong.set_role("shadow-card", "gray-1");
    let diagnostics = check_roles(&wrong, &[], &resolved, &roles, &patterns);
    assert_eq!(
        sorted(&diagnostics),
        [("role-type", "/roles/shadow-card".to_owned())]
    );
    assert_eq!(
        resolve_role("shadow-card", &wrong, &[], &roles, &patterns),
        None
    );

    let mut unknown = document.clone();
    unknown.set_role("gutter", "space-3");
    let diagnostics = check_roles(&unknown, &[], &resolved, &roles, &patterns);
    assert_eq!(
        sorted(&diagnostics),
        [("role-unknown", "/roles/gutter".to_owned())]
    );
    assert!(diagnostics[0].severity == nsh_theme::Severity::Warning);
    assert_eq!(
        resolve_role("gutter", &unknown, &[], &roles, &patterns).as_deref(),
        Some("space-3")
    );
}

#[test]
fn a_required_role_that_resolves_to_nothing_is_role_unmapped() {
    let document =
        clean(r##"{ "tokens": { "gray-1": { "kind": "color", "value": "#fff" } } }"##).document;
    let resolved = quiet(&document, &Selection::new(), &[]);
    let diagnostics = check_roles(&document, &[], &resolved, &library(), &[]);
    assert_eq!(
        sorted(&diagnostics),
        [("role-unmapped", "/roles/background".to_owned())]
    );
    let mut mapped = document.clone();
    mapped.set_role("background", "gray-1");
    assert!(check_roles(&mapped, &[], &resolved, &library(), &[]).is_empty());
}

#[test]
fn check_roles_does_not_report_role_token_missing_again() {
    let mut document = example();
    document.set_role("background", "gone");
    document.insert_token(
        "background",
        Token::new(ValueType::Color, Value::reference("gray-1")),
    );
    assert_eq!(codes(&nsh_theme::check(&document)), ["role-token-missing"]);
    let app = patch(r#"{ "roles": { "primary": "also-gone" } }"#);
    let resolved = quiet(&document, &Selection::new(), &[&app]);
    assert!(check_roles(&document, &[&app], &resolved, &library(), &tailwind()).is_empty());
    // The refused mappings fall through: the patch's to the document's, the
    // document's to the token of the role's own name.
    assert_eq!(
        resolve_role("primary", &document, &[&app], &library(), &[]).as_deref(),
        Some("intent-primary-bg")
    );
    assert_eq!(
        resolve_role("background", &document, &[], &library(), &[]).as_deref(),
        Some("background")
    );
    assert_eq!(resolved.roles.get("background"), None);
}

#[test]
fn no_input_panics() {
    let declarations = library();
    let patterns = tailwind();
    let app = patch(r##"{ "values": { "gray-1": "#000" }, "roles": { "primary": "white" } }"##);
    for end in 0..=EXAMPLE.len() {
        let read = read(&EXAMPLE[..end]);
        let resolved = resolve(
            &read.document,
            &pick(&[("colorMode", "dark"), ("accent", "nope")]),
            &[&app],
        );
        check_roles(&read.document, &[&app], &resolved, &declarations, &patterns);
        let _ = write(&read.document);
        let (p, _) = read_patch(&EXAMPLE[..end]);
        let _ = p.check(&read.document);
    }
    // A document in memory with values that fit nothing.
    let mut document = ThemeDocument::new();
    document.insert_token("a", Token::new(ValueType::Color, Value::Number(f64::NAN)));
    document.insert_token("b", Token::new(ValueType::Number, Value::reference("")));
    document.insert_token(
        "c",
        Token::new(ValueType::Typography, Value::reference("a")),
    );
    document.insert_modifier(
        "m",
        nsh_theme::Modifier::new("x")
            .with_context("y", Context::new().with("zz", Value::Boolean(true))),
    );
    document.resolution_order = vec!["m".to_owned(), "m".to_owned(), "q".to_owned()];
    let resolved = resolve(&document, &Selection::new(), &[]);
    check_roles(&document, &[], &resolved, &declarations, &patterns);
    assert!(!nsh_theme::check(&document).is_empty());
    let _ = read(&write(&document));
}
