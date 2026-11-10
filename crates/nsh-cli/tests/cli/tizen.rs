//! `--target tizen`: Samsung TVs natively. The targets parse for `nsh run`,
//! `nsh package` and `nsh test`; `nsh.json`'s `tizen` section (shared with
//! `tizen-web`) is checked; `nsh package` stages and signs TPKs; `nsh run`
//! installs, maps and launches the development package. A fake Tizen
//! runtime (`NSH_RUNTIME`: the host and a library per architecture) stands
//! in for the `tizen` component, and fakes of `tz` and `sdb` on `PATH` for
//! the Tizen SDK.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A fresh directory with an app (`App.tsx`, `nsh.json` of `config`), the
/// fake runtime in `runtime/tizen/` and the fake `tz` and `sdb` in `bin/`.
fn app(config: &str) -> Directory {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = Directory(std::env::temp_dir().join(format!(
        "nsh-cli-tizen-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    let root = &dir.0;
    fs::create_dir_all(root.join("app/img")).unwrap();
    fs::write(
        root.join("app/App.tsx"),
        "import { mount, Text } from 'nsh'; mount(() => <Text>Hello</Text>);",
    )
    .unwrap();
    fs::write(root.join("app/nsh.json"), config).unwrap();
    for variant in ["release", "dev"] {
        let runtime = root.join("runtime/tizen").join(variant);
        for arch in ["armv7l", "aarch64", "x86_64"] {
            fs::create_dir_all(runtime.join(arch)).unwrap();
            fs::write(
                runtime.join(arch).join("libnsh.so"),
                format!("{variant} {arch}"),
            )
            .unwrap();
        }
        fs::write(runtime.join("Nsh.Tizen.Host.dll"), "host").unwrap();
    }
    fs::create_dir_all(root.join("bin")).unwrap();
    // `tz`: its arguments, one line a call; `pack` writes a package next
    // to the staged one (its manifest, to read back).
    script(
        &root.join("bin/tz"),
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$TZ_LOG"
if [ "$1" = pack ]; then
  for last; do :; done
  cp "$last/tizen-manifest.xml" "$last/../$(basename "$last")-signed.tpk"
fi
"#,
    );
    // `sdb`: its arguments; one TV, which fails to launch the app (the
    // run then ends, its sequence logged).
    script(
        &root.join("bin/sdb"),
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$SDB_LOG"
case "$*" in
  devices) printf 'List of devices attached\ntv-1\tdevice\tUE43\n' ;;
  *capability) printf 'secure_protocol:enabled\ncpu_arch:armv7\n' ;;
  *execute*) echo 'error: the TV refused'; exit 1 ;;
esac
"#,
    );
    dir
}

fn script(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// `nsh` with the fake runtime, `tz` and `sdb` (and no Tizen SDK).
fn nsh(dir: &Directory) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nsh"));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.0.join("bin")];
    paths.extend(std::env::split_paths(&path));
    command
        .env("NSH_RUNTIME", dir.0.join("runtime"))
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("TZ_LOG", dir.0.join("tz.log"))
        .env("SDB_LOG", dir.0.join("sdb.log"))
        .env("TIZEN_SDK", dir.0.join("no-sdk"))
        .env("NO_COLOR", "1");
    command
}

fn package(dir: &Directory, extra: &[&str]) -> Output {
    nsh(dir)
        .args(["package", "--target", "tizen", "--out"])
        .arg(dir.0.join("out"))
        .args(extra)
        .arg(dir.0.join("app"))
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The files of `dir`, relative to it, sorted.
fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(at) = pending.pop() {
        for entry in fs::read_dir(&at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                out.push(
                    path.strip_prefix(dir)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    out.sort();
    out
}

const SIGNED: &str = r#"{ "id": "com.acme.counter", "name": "Counter", "version": "1.2.0",
    "tizen": { "package": "AbCdE12345", "signing": "tv-profile" } }"#;

#[test]
fn the_tizen_target_parses_for_run_package_and_test() {
    let dir = app(r#"{ "id": "com.acme.counter" }"#);
    for command in [
        &["run", "--help"][..],
        &["package", "--help"],
        &["test", "--help"],
    ] {
        let output = nsh(&dir).args(command).output().unwrap();
        let help = String::from_utf8_lossy(&output.stdout);
        assert!(help.contains("tizen"), "{command:?}: {help}");
    }
    // Arguments of other targets are refused for it, and its own for them.
    let output = package(&dir, &["--abi", "x86_64"]);
    assert!(
        stderr(&output).contains("--abi only applies to --target android"),
        "{}",
        stderr(&output)
    );
    let output = nsh(&dir)
        .args(["package", "--target", "web", "--arch", "armv7l"])
        .arg(dir.0.join("app"))
        .output()
        .unwrap();
    assert!(
        stderr(&output).contains("--arch only applies to --target tizen"),
        "{}",
        stderr(&output)
    );
    let output = package(&dir, &["--arch", "riscv64"]);
    assert!(stderr(&output).contains("riscv64"), "{}", stderr(&output));
    let output = nsh(&dir)
        .args(["test", "--target", "tizen", "--fps"])
        .arg(dir.0.join("app"))
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn values_tizen_refuses_are_refused() {
    for (config, message) in [
        (
            r#""tizen": { "package": "short" }"#,
            "tizen.package `short`",
        ),
        (
            r#""tizen": { "package": "AbCdE-1234" }"#,
            "tizen.package `AbCdE-1234`",
        ),
        (
            r#""tizen": { "package": "AbCdE12345", "appId": "Other12345.App" }"#,
            "tizen.appId `Other12345.App`",
        ),
        (r#""version": "1.0.0-beta""#, "version `1.0.0-beta`"),
        (
            r#""tizen": { "apiVersion": "eight" }"#,
            "tizen.apiVersion `eight`",
        ),
        (
            r#""tizen": { "privileges": ["camera"] }"#,
            "tizen.privileges `camera`",
        ),
    ] {
        let dir = app(&format!(r#"{{ "id": "com.acme.counter", {config} }}"#));
        let output = package(&dir, &["--sign", "tv"]);
        assert!(!output.status.success(), "{config}");
        assert!(
            stderr(&output).contains(message),
            "{config}: {}",
            stderr(&output)
        );
        assert!(!dir.0.join("out/tizen").exists(), "{config}");
    }
}

#[test]
fn a_package_without_a_profile_is_refused_with_how_to_make_one() {
    let dir = app(r#"{ "id": "com.acme.counter" }"#);
    let output = package(&dir, &[]);
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("--sign <profile>"), "{err}");
    assert!(err.contains("tizen.signing"), "{err}");
    assert!(err.contains("Certificate Manager"), "{err}");
    assert!(!dir.0.join("tz.log").exists());
}

#[test]
fn a_package_is_the_host_the_library_and_the_app_signed_with_tz() {
    let dir = app(SIGNED);
    let output = package(&dir, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let staged = dir.0.join("out/tizen/Counter-armv7l");
    let names = files(&staged);
    for file in [
        "bin/Nsh.Tizen.Host.dll",
        "lib/libnsh.so",
        "res/nsh/app.json",
        "res/nsh/bundle.nshc",
        "tizen-manifest.xml",
    ] {
        assert!(names.iter().any(|n| n == file), "{file} in {names:?}");
    }
    assert_eq!(read(staged.join("lib/libnsh.so")), "release armv7l");
    let app_json: serde_json::Value =
        serde_json::from_str(&read(staged.join("res/nsh/app.json"))).unwrap();
    assert_eq!(app_json["id"], "com.acme.counter");
    assert_eq!(app_json["name"], "Counter");
    let manifest = read(staged.join("tizen-manifest.xml"));
    for part in [
        r#"api-version="8.0" package="AbCdE12345" version="1.2.0""#,
        r#"<ui-application appid="AbCdE12345.Counter" exec="Nsh.Tizen.Host.dll" type="dotnet""#,
        "<label>Counter</label>",
        r#"<profile name="tv-samsung" />"#,
        "<privilege>http://tizen.org/privilege/internet</privilege>",
    ] {
        assert!(manifest.contains(part), "{part} in {manifest}");
    }
    assert!(!manifest.contains("<feature"), "{manifest}");
    assert_eq!(
        read(dir.0.join("tz.log")).trim(),
        format!("pack -t tpk -s tv-profile {}", staged.display())
    );
    // What `tz` wrote, as `<out>/<name>.tpk`.
    assert_eq!(read(dir.0.join("out/Counter.tpk")), manifest);
}

#[test]
fn several_architectures_make_the_multi_architecture_upload() {
    let dir = app(SIGNED);
    let output = package(&dir, &["--arch", "aarch64,armv7", "--sign", "other"]);
    assert!(output.status.success(), "{}", stderr(&output));
    for (arch, file, number, feature) in [
        ("armv7l", "armv7", 1, "armv7"),
        ("aarch64", "aarch64", 2, "armv8"),
    ] {
        let staged = dir.0.join("out/tizen").join(format!("Counter-{arch}"));
        assert_eq!(
            read(staged.join("lib/libnsh.so")),
            format!("release {arch}")
        );
        let tpk = dir
            .0
            .join(format!("out/AbCdE12345-1.2.0.{number}-{file}.tpk"));
        let manifest = read(&tpk);
        assert!(
            manifest.contains(&format!(r#"version="1.2.0.{number}""#)),
            "{manifest}"
        );
        assert!(
            manifest.contains(&format!(
                r#"<feature name="http://tizen.org/feature/platform.core.cpu.arch.{feature}">true</feature>"#
            )),
            "{manifest}"
        );
    }
    let log = read(dir.0.join("tz.log"));
    assert_eq!(log.lines().count(), 2, "{log}");
    assert!(
        log.lines().all(|l| l.starts_with("pack -t tpk -s other ")),
        "{log}"
    );
    let zip = fs::read(dir.0.join("out/AbCdE12345-1.2.0.zip")).unwrap();
    for name in [
        "AbCdE12345-1.2.0.1-armv7.tpk",
        "AbCdE12345-1.2.0.2-aarch64.tpk",
    ] {
        assert!(
            zip.windows(name.len()).any(|w| w == name.as_bytes()),
            "{name} in the upload"
        );
    }
}

#[test]
fn run_installs_maps_and_launches_the_development_package() {
    // No profile: refused before anything reaches the TV.
    let dir = app(r#"{ "id": "com.acme.counter" }"#);
    let output = nsh(&dir)
        .args(["run", "--target", "tizen", "--port", "0"])
        .arg(dir.0.join("app"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("tizen.signing"),
        "{}",
        stderr(&output)
    );
    assert!(!dir.0.join("sdb.log").exists());

    let dir = app(SIGNED);
    let output = nsh(&dir)
        .args(["run", "--target", "tizen", "--port", "0"])
        .arg(dir.0.join("app"))
        .output()
        .unwrap();
    // The fake TV refuses the launch: the run ends there.
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("the TV refused"),
        "{}",
        stderr(&output)
    );
    let work = fs::canonicalize(dir.0.join("app"))
        .unwrap()
        .join(".nsh/tizen");
    let tpk = work.join("NshDev-armv7l.tpk");
    let dev = read(&tpk);
    assert!(dev.contains(r#"package="NshDevTizn""#), "{dev}");
    assert_eq!(read(work.join("dev-armv7l/lib/libnsh.so")), "dev armv7l");
    assert!(!work.join("dev-armv7l/res/nsh/bundle.nshc").exists());
    let log = read(dir.0.join("sdb.log"));
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(
        lines[..4],
        [
            "devices",
            "-s tv-1 capability",
            &*format!("-s tv-1 install {}", tpk.display()),
            "-s tv-1 shell 0 kill NshDevTizn.NshDev"
        ],
        "{log}"
    );
    assert!(
        lines[4].starts_with("-s tv-1 reverse tcp:8790 tcp:"),
        "{log}"
    );
    assert_eq!(
        lines[5], "-s tv-1 shell 0 execute NshDevTizn.NshDev",
        "{log}"
    );
    // Undone on the way out.
    assert_eq!(
        lines[6..],
        [
            "-s tv-1 shell 0 kill NshDevTizn.NshDev",
            "-s tv-1 reverse --remove tcp:8790"
        ],
        "{log}"
    );

    // The device has this build: not installed again.
    fs::remove_file(dir.0.join("sdb.log")).unwrap();
    let _ = nsh(&dir)
        .args(["run", "--target", "tizen", "--port", "0"])
        .arg(dir.0.join("app"))
        .output()
        .unwrap();
    let log = read(dir.0.join("sdb.log"));
    assert!(!log.contains(" install "), "{log}");
    assert_eq!(read(dir.0.join("tz.log")).lines().count(), 1);
}
