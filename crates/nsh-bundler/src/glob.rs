//! `import.meta.glob`: the modules whose paths match a pattern, as Vite
//! imports them. The call is replaced, before the module is compiled, by an
//! object of the matched files, keyed by their path as the pattern names
//! them, each a function that imports it (`() => import(path)`), or with
//! `{ eager: true }` the module itself, imported up front. The bundler then
//! sees literal imports: a lazy module is in the bundle (or its chunk) and
//! runs when it is first imported.
//!
//! ```ts
//! const pages = import.meta.glob("./pages/*.tsx");   // { "./pages/Home.tsx": () => import(…) }
//! const all = import.meta.glob("/**/*.preview.tsx"); // from the app's directory
//! const eager = import.meta.glob("./icons/*.ts", { eager: true, import: "default" });
//! ```
//!
//! A pattern is a string literal or an array of them; one starting with `!`
//! leaves out what it matches. `./` and `../` patterns are relative to the
//! importing module, `/` ones to the app's directory. A pattern has `*`
//! (any name part), `**` (any number of directories), `?` (one character)
//! and `{a,b}` (either). Directories named `node_modules`, `.nsh`, `target`
//! and those starting with `.` are searched only when the pattern names
//! them. The options are a literal object of `eager` (a boolean) and
//! `import` (an export's name). Keys are sorted, so the object is the same
//! on every machine.

use std::path::{Component, Path, PathBuf};

use oxc::{
    allocator::Allocator,
    ast::ast::{
        Argument, ArrayExpressionElement, CallExpression, Expression, ObjectPropertyKind,
        PropertyKey,
    },
    ast_visit::{Visit, walk},
    parser::Parser,
    span::{GetSpan, SourceType, Span},
};

/// One call, read from the source.
struct Call {
    span: Span,
    patterns: Vec<String>,
    eager: bool,
    import: Option<String>,
}

/// Replaces the `import.meta.glob` calls of `source` (the module at
/// `importer`, of `source_type`); `root` is the app's directory, which `/`
/// patterns start from. `None`: the module makes none. `Err`: a call this
/// cannot read (its patterns or options are not literals), or a pattern
/// that names no directory it can search.
pub(crate) fn expand(
    source: &str,
    source_type: SourceType,
    importer: &Path,
    root: &Path,
) -> Result<Option<String>, String> {
    if !source.contains("import.meta.glob") {
        return Ok(None);
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    // The compiler reports a syntax error with its code frame.
    if !parsed.diagnostics.is_empty() {
        return Ok(None);
    }
    let mut finder = Finder::default();
    finder.visit_program(&parsed.program);
    if let Some(error) = finder.error {
        return Err(error);
    }
    if finder.calls.is_empty() {
        return Ok(None);
    }
    let dir = importer.parent().unwrap_or(Path::new("."));
    let mut code = String::with_capacity(source.len());
    let mut hoisted = String::new();
    let mut at = 0;
    for (n, call) in finder.calls.iter().enumerate() {
        let files = matches(&call.patterns, dir, root)?;
        code.push_str(&source[at..call.span.start as usize]);
        code.push_str(&object(call, n, &files, &mut hoisted));
        at = call.span.end as usize;
    }
    code.push_str(&source[at..]);
    // Imports are hoisted: at the end, the module's lines keep their numbers.
    if !hoisted.is_empty() {
        code.push('\n');
        code.push_str(&hoisted);
    }
    Ok(Some(code))
}

/// The object a call becomes: `{ key: () => import(path) }`, or with
/// `eager` the namespaces (or `import`'s export) imported in `hoisted`.
fn object(call: &Call, n: usize, files: &[(String, PathBuf)], hoisted: &mut String) -> String {
    let mut out = String::from("({");
    for (i, (key, path)) in files.iter().enumerate() {
        let spec = serde_json::Value::String(path.to_string_lossy().replace('\\', "/")).to_string();
        let key = serde_json::Value::String(key.clone()).to_string();
        if i > 0 {
            out.push_str(", ");
        }
        if call.eager {
            let local = format!("__nsh_glob_{n}_{i}");
            match &call.import {
                Some(name) => {
                    let name = serde_json::Value::String(name.clone()).to_string();
                    hoisted.push_str(&format!("import {{ {name} as {local} }} from {spec};\n"));
                }
                None => hoisted.push_str(&format!("import * as {local} from {spec};\n")),
            }
            out.push_str(&format!("{key}: {local}"));
        } else {
            match &call.import {
                Some(name) => {
                    let name = serde_json::Value::String(name.clone()).to_string();
                    out.push_str(&format!(
                        "{key}: () => import({spec}).then((m) => m[{name}])"
                    ));
                }
                None => out.push_str(&format!("{key}: () => import({spec})")),
            }
        }
    }
    out.push_str("})");
    out
}

/// The files `patterns` match, as (key, path), sorted by key.
fn matches(patterns: &[String], dir: &Path, root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut found = std::collections::BTreeMap::new();
    let (excluded, included): (Vec<&String>, Vec<&String>) =
        patterns.iter().partition(|p| p.starts_with('!'));
    if included.is_empty() {
        return Err("import.meta.glob: a pattern that is not `!…` is needed".to_owned());
    }
    for pattern in included {
        for pattern in braces(pattern) {
            let (base, rest, key_base) = split(&pattern, dir, root)?;
            let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
            let mut files = Vec::new();
            walk_dir(&base, &segments, &mut Vec::new(), &mut files);
            for relative in files {
                let key = format!("{key_base}{}", relative.join("/"));
                let path = base.join(relative.iter().collect::<PathBuf>());
                found.insert(key, path);
            }
        }
    }
    for pattern in excluded {
        let pattern = &pattern[1..];
        for pattern in braces(pattern) {
            let (_, rest, key_base) = split(&pattern, dir, root)?;
            let segments: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
            found.retain(|key, _| {
                let Some(tail) = key.strip_prefix(&key_base) else {
                    return true;
                };
                let parts: Vec<&str> = tail.split('/').collect();
                !path_matches(&segments, &parts)
            });
        }
    }
    Ok(found.into_iter().collect())
}

/// A pattern as (the directory to search, the pattern below it, the prefix
/// of the keys it gives): its leading part with no wildcard is the
/// directory.
fn split(pattern: &str, dir: &Path, root: &Path) -> Result<(PathBuf, String, String), String> {
    let (start, rest, prefix) = if let Some(rest) = pattern.strip_prefix('/') {
        (root.to_path_buf(), rest, "/".to_owned())
    } else if pattern.starts_with("./") || pattern.starts_with("../") {
        (dir.to_path_buf(), pattern, String::new())
    } else {
        return Err(format!(
            "import.meta.glob(\"{pattern}\"): a pattern starts with `./`, `../` or `/` (the app's directory)"
        ));
    };
    let mut base = start;
    let mut key = prefix;
    let mut parts = rest.split('/').peekable();
    let mut below = Vec::new();
    while let Some(part) = parts.next() {
        let wild = part.contains(['*', '?', '{']);
        if wild || parts.peek().is_none() {
            below.push(part);
            below.extend(parts.by_ref());
            break;
        }
        match part {
            "." | "" => key.push_str(if part == "." { "./" } else { "" }),
            ".." => {
                base.pop();
                key.push_str("../");
            }
            name => {
                base.push(name);
                key.push_str(name);
                key.push('/');
            }
        }
    }
    // A file named in full: its directory is searched for it.
    let base = normalize(&base);
    if !base.is_dir() {
        return Ok((base, String::from("\0"), key));
    }
    Ok((base, below.join("/"), key))
}

/// `path` with `.` and `..` worked out.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Directories searched only when a pattern names them.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "target"
}

/// The files below `dir` whose path (`at`, then their name) matches
/// `segments`, into `out`.
fn walk_dir(dir: &Path, segments: &[&str], at: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        // A symbolic link is followed to what it names.
        let is_dir = kind.is_dir() || (kind.is_symlink() && path.is_dir());
        at.push(name.clone());
        if is_dir {
            let named = segments.iter().any(|s| *s == name);
            if !skipped(&name) || named {
                // Only below where the pattern may still match.
                if prefix_may_match(segments, at) {
                    walk_dir(&path, segments, at, out);
                }
            }
        } else if path_matches(segments, &at.iter().map(String::as_str).collect::<Vec<_>>()) {
            out.push(at.clone());
        }
        at.pop();
    }
}

/// Whether some path that starts with `parts` could match `segments`.
fn prefix_may_match(segments: &[&str], parts: &[String]) -> bool {
    fn go(segments: &[&str], parts: &[String]) -> bool {
        match (segments.first(), parts.first()) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(&"**"), Some(_)) => true,
            (Some(s), Some(p)) => segment_matches(s, p) && go(&segments[1..], &parts[1..]),
        }
    }
    go(segments, parts)
}

/// Whether the path `parts` matches `segments` (`**` is any number of
/// directories).
fn path_matches(segments: &[&str], parts: &[&str]) -> bool {
    match (segments.first(), parts.first()) {
        (None, None) => true,
        (Some(&"**"), _) => {
            path_matches(&segments[1..], parts)
                || (!parts.is_empty() && path_matches(segments, &parts[1..]))
        }
        (Some(s), Some(p)) => segment_matches(s, p) && path_matches(&segments[1..], &parts[1..]),
        _ => false,
    }
}

/// Whether the name `name` matches `pattern` (`*`, `?`; braces are
/// expanded before).
fn segment_matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let (mut i, mut j) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

/// `pattern` with its first `{a,b}` group expanded, recursively.
fn braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_owned()];
    };
    let mut depth = 0;
    let mut close = None;
    for (i, c) in pattern[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return vec![pattern.to_owned()];
    };
    let (head, body, tail) = (
        &pattern[..open],
        &pattern[open + 1..close],
        &pattern[close + 1..],
    );
    let mut options = Vec::new();
    let (mut depth, mut from) = (0, 0);
    for (i, c) in body.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                options.push(&body[from..i]);
                from = i + 1;
            }
            _ => {}
        }
    }
    options.push(&body[from..]);
    options
        .into_iter()
        .flat_map(|o| braces(&format!("{head}{o}{tail}")))
        .collect()
}

/// Finds the `import.meta.glob(…)` calls.
#[derive(Default)]
struct Finder {
    calls: Vec<Call>,
    error: Option<String>,
}

impl<'a> Visit<'a> for Finder {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if is_glob(&call.callee) {
            match read(call) {
                Ok(c) => self.calls.push(c),
                Err(e) => {
                    self.error.get_or_insert(e);
                }
            }
            return;
        }
        walk::walk_call_expression(self, call);
    }
}

/// Whether `callee` is `import.meta.glob`.
fn is_glob(callee: &Expression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = callee else {
        return false;
    };
    member.property.name == "glob" && matches!(&member.object, Expression::ImportMeta(_))
}

/// A call's literal patterns and options.
fn read(call: &CallExpression<'_>) -> Result<Call, String> {
    const WHAT: &str = "import.meta.glob takes a string literal or an array of them, and a literal { eager, import }";
    let mut patterns = Vec::new();
    match call.arguments.first() {
        Some(Argument::StringLiteral(s)) => patterns.push(s.value.to_string()),
        Some(Argument::TemplateLiteral(t)) if t.expressions.is_empty() => {
            patterns.push(t.quasis.iter().map(|q| q.value.raw.as_str()).collect())
        }
        Some(Argument::ArrayExpression(array)) => {
            for element in &array.elements {
                match element {
                    ArrayExpressionElement::StringLiteral(s) => patterns.push(s.value.to_string()),
                    _ => return Err(WHAT.to_owned()),
                }
            }
        }
        _ => return Err(WHAT.to_owned()),
    }
    let (mut eager, mut import) = (false, None);
    if let Some(options) = call.arguments.get(1) {
        let Argument::ObjectExpression(object) = options else {
            return Err(WHAT.to_owned());
        };
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return Err(WHAT.to_owned());
            };
            let name = match &property.key {
                PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                PropertyKey::StringLiteral(s) => s.value.as_str(),
                _ => return Err(WHAT.to_owned()),
            };
            match (name, &property.value) {
                ("eager", Expression::BooleanLiteral(b)) => eager = b.value,
                ("import", Expression::StringLiteral(s)) => import = Some(s.value.to_string()),
                (other, _) => {
                    return Err(format!(
                        "import.meta.glob: option `{other}` is not supported (`eager`, `import`)"
                    ));
                }
            }
        }
    }
    if call.arguments.len() > 2 {
        return Err(WHAT.to_owned());
    }
    Ok(Call {
        span: call.span(),
        patterns,
        eager,
        import,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nsh-glob-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for file in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "export default 1;\n").unwrap();
        }
        dunce::canonicalize(&dir).unwrap()
    }

    fn keys(code: &str) -> Vec<String> {
        code.match_indices("\": ")
            .map(|(at, _)| {
                let head = &code[..at];
                head[head.rfind('"').unwrap() + 1..].to_owned()
            })
            .collect()
    }

    #[test]
    fn names_match_by_parts() {
        assert!(segment_matches("*.preview.tsx", "Card.preview.tsx"));
        assert!(!segment_matches("*.preview.tsx", "Card.tsx"));
        assert!(segment_matches("?ard.tsx", "Card.tsx"));
        assert!(path_matches(&["**", "*.tsx"], &["a", "b", "c.tsx"]));
        assert!(path_matches(&["**", "*.tsx"], &["c.tsx"]));
        assert!(!path_matches(&["*", "*.tsx"], &["c.tsx"]));
        assert_eq!(braces("./{a,b}/*.{ts,tsx}").len(), 4);
    }

    #[test]
    fn root_patterns_find_the_app_s_files_and_skip_tool_folders() {
        let root = tree(
            "root",
            &[
                "src/Card.preview.tsx",
                "src/deep/Menu.preview.tsx",
                "src/Card.tsx",
                "node_modules/x/Y.preview.tsx",
                ".nsh/sketch/Z.preview.tsx",
                "target/W.preview.tsx",
            ],
        );
        let out = expand(
            "export const all = import.meta.glob(\"/**/*.preview.tsx\");\n",
            SourceType::ts(),
            Path::new("/elsewhere/Stage.ts"),
            &root,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            keys(&out),
            ["/src/Card.preview.tsx", "/src/deep/Menu.preview.tsx"]
        );
        let card = root
            .join("src/Card.preview.tsx")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            out.contains(&format!("() => import(\"{card}\")")),
            "{}",
            out
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn relative_patterns_negations_eager_and_named_imports() {
        let root = tree(
            "relative",
            &["pages/Home.tsx", "pages/About.tsx", "pages/Draft.tsx"],
        );
        let source = "const a = import.meta.glob([\"./pages/*.tsx\", \"!./pages/Draft.tsx\"]);\n\
                      const b = import.meta.glob(\"./pages/{Home,About}.tsx\", { eager: true, import: \"default\" });\n";
        let out = expand(source, SourceType::ts(), &root.join("App.ts"), &root)
            .unwrap()
            .unwrap();
        let (a, b) = out.split_once('\n').unwrap();
        assert_eq!(keys(a), ["./pages/About.tsx", "./pages/Home.tsx"]);
        assert!(b.contains("\"./pages/About.tsx\": __nsh_glob_1_0"), "{b}");
        assert!(
            out.contains("import { \"default\" as __nsh_glob_1_0 } from"),
            "{}",
            out
        );
        // The module's own lines keep their numbers: imports go at the end.
        assert!(out.starts_with("const a = "));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn what_cannot_be_read_is_an_error_and_other_code_is_untouched() {
        let root = tree("errors", &["a.ts"]);
        let run = |code: &str| expand(code, SourceType::ts(), &root.join("App.ts"), &root);
        assert!(run("import.meta.glob(name);").is_err());
        assert!(run("import.meta.glob(\"./*.ts\", { query: \"?raw\" });").is_err());
        assert!(run("import.meta.glob(\"*.ts\");").is_err());
        assert!(run("const url = import.meta.url;").unwrap().is_none());
        // Nothing matched: an empty object.
        let out = run("const none = import.meta.glob(\"./missing/*.ts\");")
            .unwrap()
            .unwrap();
        assert!(out.contains("const none = ({});"), "{}", out);
        let _ = std::fs::remove_dir_all(root);
    }
}
