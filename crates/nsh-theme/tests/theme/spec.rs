//! The examples of `docs/dev/theme-format.md`, read from the repository and
//! run by their info strings, so an edited example that no longer holds
//! fails.

use std::collections::HashSet;

use nsh_theme::{
    Diagnostic, Patch, RoleDeclaration, Selection, Severity, ThemeDocument, Value, ValueType,
    check_roles, code, read, read_patch, resolve,
};

const PAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/dev/theme-format.md"
);

/// A fenced block whose info string starts with `json theme`.
struct Block {
    /// What follows `json `: `theme`, `theme-invalid <code>`, ….
    tag: String,
    body: String,
    /// The fence's line, 1-based.
    line: usize,
}

fn page() -> String {
    std::fs::read_to_string(PAGE).expect("docs/dev/theme-format.md is readable")
}

/// The page's tagged blocks, found with a line scan.
fn blocks() -> Vec<Block> {
    let text = page();
    let mut blocks = Vec::new();
    let mut open: Option<(Option<String>, usize, String)> = None;
    for (i, line) in text.lines().enumerate() {
        let fence = line.trim_start().strip_prefix("```");
        match (&mut open, fence) {
            (None, Some(info)) => {
                let tag = info
                    .trim()
                    .strip_prefix("json ")
                    .filter(|t| t.starts_with("theme"))
                    .map(str::to_owned);
                open = Some((tag, i + 1, String::new()));
            }
            (Some(_), Some(rest)) if rest.trim().is_empty() => {
                if let Some((Some(tag), line, body)) = open.take() {
                    blocks.push(Block { tag, body, line });
                }
            }
            (Some((_, _, body)), _) => {
                body.push_str(line);
                body.push('\n');
            }
            (None, None) => {}
        }
    }
    assert!(open.is_none(), "an unclosed fence");
    blocks
}

fn declarations(block: &Block) -> Vec<RoleDeclaration> {
    let json: serde_json::Value = serde_json::from_str(&block.body)
        .unwrap_or_else(|e| panic!("line {}: the roles block is not JSON: {e}", block.line));
    let items = json
        .as_array()
        .unwrap_or_else(|| panic!("line {}: the roles block is not an array", block.line));
    items
        .iter()
        .map(|item| {
            let name = item["name"]
                .as_str()
                .unwrap_or_else(|| panic!("line {}: a role without a name", block.line));
            let ty = item["type"].as_str().and_then(ValueType::from_name);
            let ty =
                ty.unwrap_or_else(|| panic!("line {}: role {name} has no known type", block.line));
            let mut declaration = RoleDeclaration::new(name, ty);
            if let Some(doc) = item["doc"].as_str() {
                declaration = declaration.with_doc(doc);
            }
            declaration.with_required(item["required"].as_bool().unwrap_or(false))
        })
        .collect()
}

/// `read`, `resolve` at the defaults, then `check_roles` against `roles`.
fn run(text: &str, roles: &[RoleDeclaration]) -> (ThemeDocument, Vec<Diagnostic>) {
    let read = read(text);
    let resolved = resolve(&read.document, &Selection::new(), &[]);
    let checked = check_roles(&read.document, &[], &resolved, roles, &[]);
    let mut diagnostics = read.diagnostics;
    diagnostics.extend(resolved.diagnostics);
    diagnostics.extend(checked);
    (read.document, diagnostics)
}

/// The complete example and the declarations it is checked against.
fn complete_example(blocks: &[Block]) -> (ThemeDocument, Vec<RoleDeclaration>) {
    let mut roles = Vec::new();
    for block in blocks {
        match block.tag.as_str() {
            "theme-roles" => roles = declarations(block),
            "theme-example" => {
                let (document, diagnostics) = run(&block.body, &roles);
                assert!(
                    diagnostics.is_empty(),
                    "line {}: {diagnostics:#?}",
                    block.line
                );
                return (document, roles);
            }
            _ => {}
        }
    }
    panic!("the page has no `json theme-example` block");
}

#[test]
fn every_tagged_example_holds() {
    let blocks = blocks();
    let (example, example_roles) = complete_example(&blocks);
    let mut roles: Vec<RoleDeclaration> = Vec::new();
    let mut failures = Vec::new();
    for block in &blocks {
        let tag = block.tag.as_str();
        match tag.split_once(' ').unwrap_or((tag, "")) {
            ("theme-roles", "") => roles = declarations(block),
            ("theme" | "theme-example", "") => {
                let (_, diagnostics) = run(&block.body, &roles);
                if !diagnostics.is_empty() {
                    failures.push(format!("line {}: {tag} gives {diagnostics:#?}", block.line));
                }
            }
            ("theme-invalid", expected) => {
                let (_, diagnostics) = run(&block.body, &roles);
                if code::ALL.iter().all(|(c, _)| *c != expected) {
                    failures.push(format!(
                        "line {}: `{expected}` is no diagnostic code",
                        block.line
                    ));
                } else if diagnostics.len() != 1 || diagnostics[0].code != expected {
                    failures.push(format!(
                        "line {}: expected one {expected}, got {diagnostics:#?}",
                        block.line
                    ));
                }
            }
            ("theme-patch", "") => {
                let (patch, mut diagnostics) = read_patch(&block.body);
                diagnostics.extend(patch.check(&example));
                let resolved = resolve(&example, &Selection::new(), &[&patch]);
                diagnostics.extend(resolved.diagnostics.iter().cloned());
                diagnostics.extend(check_roles(
                    &example,
                    &[&patch],
                    &resolved,
                    &example_roles,
                    &[],
                ));
                if !diagnostics.is_empty() {
                    failures.push(format!(
                        "line {}: the patch gives {diagnostics:#?}",
                        block.line
                    ));
                }
            }
            _ => failures.push(format!("line {}: unknown tag `json {tag}`", block.line)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_page_has_a_valid_example_per_value_type() {
    let mut types = HashSet::new();
    for block in blocks()
        .iter()
        .filter(|b| b.tag == "theme" || b.tag == "theme-example")
    {
        types.extend(
            read(&block.body)
                .document
                .tokens
                .values()
                .map(|t| t.value_type),
        );
    }
    let missing: Vec<_> = ValueType::ALL
        .into_iter()
        .filter(|t| !types.contains(t))
        .collect();
    assert!(
        missing.is_empty(),
        "no valid example has a token of {missing:?}"
    );
}

#[test]
fn the_page_has_an_invalid_example_per_error_code() {
    let shown: HashSet<String> = blocks()
        .iter()
        .filter_map(|b| b.tag.strip_prefix("theme-invalid ").map(str::to_owned))
        .collect();
    let reported_here =
        |c: &str| !matches!(c, "role-conflict" | "document-unused" | "unknown-modifier");
    let missing: Vec<_> = code::ALL
        .iter()
        .filter(|(c, severity)| {
            *severity == Severity::Error && reported_here(c) && !shown.contains(*c)
        })
        .map(|(c, _)| *c)
        .collect();
    assert!(
        missing.is_empty(),
        "no `json theme-invalid` example for {missing:?}"
    );
}

#[test]
fn the_complete_example_layers_semantic_tokens_over_primitives() {
    let blocks = blocks();
    assert_eq!(
        blocks.iter().filter(|b| b.tag == "theme-example").count(),
        1
    );
    let (example, roles) = complete_example(&blocks);
    // colorMode sets primitives, accent points intents at them.
    let color_mode = example
        .modifiers
        .get("colorMode")
        .expect("a colorMode modifier");
    for (_, context) in color_mode.contexts.iter() {
        assert!(context.values.values().all(|v| v.as_reference().is_none()));
    }
    let accent = example.modifiers.get("accent").expect("an accent modifier");
    for (_, context) in accent.contexts.iter() {
        for (name, value) in context.values.iter() {
            assert!(name.starts_with("intent-"), "{name}");
            assert!(value.as_reference().is_some(), "{name}");
        }
    }
    // Roles map onto semantic tokens: tokens that are references, never
    // literal palette colors, and never named after the role.
    for (role, token) in example.roles.iter() {
        let value = &example.token(token).expect("a declared token").value;
        assert!(
            value.has_references(),
            "role {role} maps onto {token}, a literal"
        );
        assert!(!token.starts_with("roles-") && token != role, "{token}");
    }
    assert!(roles.iter().any(|r| r.required));
    let dark_cyan = resolve(
        &example,
        &Selection::new()
            .with("colorMode", "dark")
            .with("accent", "cyan"),
        &[],
    );
    assert!(dark_cyan.diagnostics.is_empty());
    assert_eq!(
        dark_cyan.get("intent-primary-bg-hover"),
        Some(&Value::Color(
            nsh_theme::Color::from_hex("#23afd0").unwrap()
        ))
    );
    // A patch changes nothing it doesn't name.
    let empty = resolve(&example, &Selection::new(), &[&Patch::new()]);
    assert!(
        empty
            .changed(&resolve(&example, &Selection::new(), &[]))
            .is_empty()
    );
}

#[test]
fn the_page_has_its_sections_in_order() {
    let text = page();
    let sections = [
        "## 1. Top-level members",
        "## 2. The token entry",
        "## 3. Value types",
        "## 4. References",
        "## 5. Modifiers and contexts",
        "## 6. `resolutionOrder`",
        "## 7. Roles",
        "## 8. The override patch",
        "## 9. The layer table",
        "## 10. The active document",
        "## 11. Diagnostics",
        "## 12. The conversion from `cssVars`", // nsh theme convert
        "## 13. A complete example",
    ];
    let mut at = 0;
    for section in sections {
        let found = text[at..]
            .find(section)
            .unwrap_or_else(|| panic!("`{section}` is missing or out of order"));
        at += found + section.len();
    }
}
