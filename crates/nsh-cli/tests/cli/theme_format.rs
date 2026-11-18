//! `nsh theme convert` and `nsh theme check` (TOOL-CLI): what each writes
//! where, the diagnostics' lines (`<severity> <code> <pointer>: <message>`)
//! and the exit codes, run on the fixtures in
//! `tests/fixtures/theme-format/`.
use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

/// A fixture's path.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/theme-format")
        .join(name)
}

/// The copy of nsh-base's old default the converter keeps, as a file.
fn old_default() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nsh-core/src/element/theme_convert/fixtures/default-theme.json")
}

/// `nsh theme <args>`.
fn theme(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nsh"));
    command
        .arg("theme")
        .args(args.iter().map(|a| a.as_ref()))
        .env("NO_COLOR", "1")
        .stdin(Stdio::null());
    command
}

fn run(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    theme(args).output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// The diagnostics' lines of `text`, each checked to be one:
/// `<severity> <code> <pointer>: <message>`.
fn diagnostics(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines {
        let (severity, rest) = line.split_once(' ').unwrap_or_default();
        let (code, rest) = rest.split_once(' ').unwrap_or_default();
        let (pointer, message) = rest.split_once(": ").unwrap_or_default();
        assert!(
            ["error", "warning"].contains(&severity)
                && !code.is_empty()
                && code.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                && (pointer.is_empty() || pointer.starts_with('/'))
                && !message.is_empty(),
            "not a diagnostic: {line:?}"
        );
    }
    lines
}

/// Whether one of `lines` starts with `prefix`.
fn has(lines: &[&str], prefix: &str) -> bool {
    lines.iter().any(|line| line.starts_with(prefix))
}

/// A path for a file the test writes, its own.
fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nsh-cli-theme-format-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The JSON of `text`.
fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn convert_writes_the_document_to_standard_output() {
    let output = run(&[&"convert", &fixture("old-theme.json")]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let read = nsh_theme::read(&stdout(&output));
    assert!(read.diagnostics.is_empty(), "{:#?}", read.diagnostics);
    let document = read.document;
    for name in ["surface", "ink", "radius"] {
        assert!(document.tokens.contains_key(name), "{name}");
    }
    // The values that differ by appearance: `colorMode`'s `dark`.
    let mode = document.modifiers.get("colorMode").unwrap();
    assert_eq!(mode.default, "light");
    assert!(
        mode.contexts
            .get("dark")
            .unwrap()
            .values
            .contains_key("ink")
    );
}

#[test]
fn convert_writes_the_file_o_names() {
    let out = scratch("new.json");
    let output = run(&[&"convert", &fixture("old-theme.json"), &"-o", &out]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let written = std::fs::read_to_string(&out).unwrap();
    let printed = stdout(&run(&[&"convert", &fixture("old-theme.json")]));
    assert_eq!(written, printed);
}

#[test]
fn convert_over_a_base_keeps_the_bases_tokens() {
    let own = json(&stdout(&run(&[&"convert", &fixture("old-app-theme.json")])));
    assert!(own["tokens"].get("background").is_none(), "{own}");

    let output = run(&[
        &"convert",
        &fixture("old-app-theme.json"),
        &"--over",
        &"nsh/base",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let over = json(&stdout(&output));
    // The base's tokens, and the app's winning by name.
    assert!(over["tokens"].get("background").is_some(), "{over}");
    assert!(
        over["tokens"]["primary"]["value"]
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case("#e11d48")),
        "{over}"
    );
    // `nsh/base` is the converter's copy of nsh-base's old default.
    let file = run(&[
        &"convert",
        &fixture("old-app-theme.json"),
        &"--over",
        &old_default(),
    ]);
    assert!(file.status.success(), "{}", stderr(&file));
    assert_eq!(stdout(&file), stdout(&output));
}

#[test]
fn convert_prints_its_errors_to_standard_error_exits_1_and_still_writes() {
    let out = scratch("new.json");
    let output = run(&[&"convert", &fixture("old-color-mode.json"), &"-o", &out]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let text = stderr(&output);
    let lines = diagnostics(&text);
    assert!(
        has(&lines, "error color-mode-taken /modifiers/colorMode: "),
        "{text}"
    );
    // Nothing could be converted: the output is written, empty.
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "");
}

#[test]
fn convert_of_a_file_it_cannot_read_fails() {
    let output = run(&[&"convert", &fixture("missing.json")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("missing.json"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn check_of_a_clean_document_prints_nothing_and_exits_0() {
    let output = run(&[&"check", &fixture("valid.json")]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(stdout(&output), "");
}

#[test]
fn check_prints_each_diagnostic_and_exits_1_on_an_error() {
    let output = run(&[&"check", &fixture("errors.json")]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stdout(&output);
    let lines = diagnostics(&text);
    for prefix in [
        "error bad-value /tokens/corner: ",
        "error reference-missing /tokens/link: ",
        // The role checks, against `nsh/base`'s roles: `radius` reads a
        // number.
        "error role-type /roles/radius: ",
    ] {
        assert!(has(&lines, prefix), "{prefix}\n{text}");
    }
}

#[test]
fn check_of_warnings_alone_exits_0() {
    let output = run(&[&"check", &fixture("warnings.json")]);
    assert!(output.status.success(), "{}", stdout(&output));
    let text = stdout(&output);
    assert_eq!(diagnostics(&text).len(), 1, "{text}");
    assert!(
        text.starts_with("warning unknown-member /colors: "),
        "{text}"
    );
}

#[test]
fn check_reports_a_cycle_two_picks_make_with_those_picks() {
    let output = run(&[&"check", &fixture("picked-cycle.json")]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stdout(&output);
    let lines = diagnostics(&text);
    // No cycle at the defaults, nor under one pick alone: the pair's,
    // once, losing its highest entry.
    assert_eq!(lines.len(), 1, "{text}");
    assert!(
        lines[0].starts_with("error reference-cycle /modifiers/second/contexts/on/b: "),
        "{text}"
    );
    assert!(
        lines[0].contains("`first: on`") && lines[0].contains("`second: on`"),
        "{text}"
    );
}

#[test]
fn check_of_an_old_theme_names_the_conversion() {
    let output = run(&[&"check", &fixture("old-theme.json")]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    let lines = diagnostics(&text);
    assert!(has(&lines, "error legacy-format "), "{text}");
    assert!(text.contains("nsh theme convert"), "{text}");
}

#[test]
fn check_patch_without_app_is_a_usage_error() {
    let output = run(&[&"check", &"--patch", &fixture("patch.json")]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("--app"), "{}", stderr(&output));
}

#[test]
fn check_patch_checks_it_against_the_apps_active_document() {
    let app = fixture("patch-app");
    let output = run(&[&"check", &"--patch", &fixture("patch.json"), &"--app", &app]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stdout(&output);
    let lines = diagnostics(&text);
    for prefix in [
        // A patch never adds a token.
        "error undeclared-token /values/nope: ",
        "error bad-value /values/corner: ",
        "error role-type /roles/radius: ",
    ] {
        assert!(has(&lines, prefix), "{prefix}\n{text}");
    }
    // The document's own entries are the document's check's.
    assert!(!text.contains("/tokens/"), "{text}");

    let good = run(&[
        &"check",
        &"--patch",
        &fixture("good-patch.json"),
        &"--app",
        &app,
    ]);
    assert!(good.status.success(), "{}", stdout(&good));
    assert_eq!(stdout(&good), "");

    // A cycle the patch closes with the document's references.
    let cycle = run(&[
        &"check",
        &"--patch",
        &fixture("cycle-patch.json"),
        &"--app",
        &app,
    ]);
    assert_eq!(cycle.status.code(), Some(1), "{}", stderr(&cycle));
    let text = stdout(&cycle);
    assert!(has(&diagnostics(&text), "error reference-cycle "), "{text}");
}

#[test]
fn check_with_app_checks_the_roles_its_sets_register() {
    // The app's crate registers the required role `banner`; its describe
    // program is built into a target directory of these tests' own.
    let target = Path::new(env!("CARGO_BIN_EXE_nsh"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("nsh-cli-theme-format");
    let app = fixture("role-app");
    let check = |document: &str| {
        theme(&[&"check", &fixture(document), &"--app", &app])
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap()
    };
    let unmapped = check("role-unmapped.json");
    assert_eq!(unmapped.status.code(), Some(1), "{}", stderr(&unmapped));
    let text = stdout(&unmapped);
    let lines = diagnostics(&text);
    assert_eq!(lines.len(), 1, "{text}");
    assert!(
        lines[0].starts_with("warning role-unmapped /roles/banner: "),
        "{text}"
    );

    let mapped = check("role-mapped.json");
    assert!(
        mapped.status.success(),
        "{}{}",
        stdout(&mapped),
        stderr(&mapped)
    );
    assert_eq!(stdout(&mapped), "");

    // Without the app, the command's own sets require no role.
    let own = run(&[&"check", &fixture("role-unmapped.json")]);
    assert!(own.status.success(), "{}", stdout(&own));
    assert_eq!(stdout(&own), "");
}

#[test]
fn the_help_says_what_each_subcommand_reads_and_writes() {
    let help = stdout(&run(&[&"--help"]));
    assert!(help.contains("convert") && help.contains("check"), "{help}");
    let convert = stdout(&run(&[&"convert", &"--help"]));
    for part in [
        "Reads OLD",
        "--over",
        "standard output",
        "standard error",
        "Exits 1",
    ] {
        assert!(convert.contains(part), "{part}\n{convert}");
    }
    let check = stdout(&run(&[&"check", &"--help"]));
    for part in [
        "Reads FILE",
        "--patch",
        "--app",
        "every combination",
        "Exits 1",
    ] {
        assert!(check.contains(part), "{part}\n{check}");
    }
}
