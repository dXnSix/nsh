//! A design tool's optimized run of an app with Rust crates (`nsh run --release` or `nsh dev
//! --release` with `NSH_DESIGN_SOCKET`, `docs/dev/design-mode.md`): the describing program and the
//! development host build together, in one `cargo build` with the generated workspace's `design`
//! profile; every other run builds them as before. Cargo is a stand-in (`CARGO`) that records each
//! build's arguments and makes the programs it names: the describing one prints the app's set, the
//! host exits at once.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

/// An app with a Rust crate of its own (`"rust": "."`), and a stand-in cargo.
struct App(PathBuf);

impl Drop for App {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The stand-in cargo: `metadata --no-deps` (the app's crate) is the real one's; the app crate's
/// metadata with its dependencies (the nshm it links through Cargo) is the crate alone, linking
/// none; the workspace's metadata and tree, which feature unification asks for at its best effort,
/// fail; a build is written to `$BUILDS` and makes each program it names in the profile's folder
/// of `$CARGO_TARGET_DIR`.
const CARGO: &str = r#"#!/bin/sh
set -eu
case "$1" in
metadata)
    case " $* " in *' --no-deps '*) exec "$REAL_CARGO" "$@";; esac
    manifest=; prev=
    for a in "$@"; do
        if [ "$prev" = --manifest-path ]; then manifest=$a; fi
        prev=$a
    done
    if [ -n "$manifest" ] && [ "$manifest" -ef "$(dirname "$BUILDS")/Cargo.toml" ]; then
        printf '{"packages":[{"id":"app","name":"app-native","manifest_path":"%s","dependencies":[]}],"resolve":{"nodes":[{"id":"app","deps":[]}]}}\n' "$manifest"
        exit 0
    fi
    exit 1;;
tree) exit 1;;
build) ;;
*) exit 2;;
esac
printf '%s\n' "$*" >> "$BUILDS"
profile=debug; packages=; prev=
for a in "$@"; do
    case "$prev" in --profile) profile=$a;; -p) packages="$packages $a";; esac
    if [ "$a" = --release ]; then profile=release; fi
    prev=$a
done
dir="$CARGO_TARGET_DIR/$profile"
mkdir -p "$dir"
for pkg in $packages; do
    case "$pkg" in
    *-describe) printf '#!/bin/sh\ncat "%s"\n' "$DESCRIPTION" > "$dir/$pkg";;
    *) printf '#!/bin/sh\nexit 0\n' > "$dir/$pkg";;
    esac
    chmod +x "$dir/$pkg"
done
"#;

impl App {
    fn new() -> App {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let app = App(std::env::temp_dir().join(format!(
            "nsh-cli-design-builds-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        let dir = &app.0;
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("nsh.json"), r#"{ "rust": "." }"#).unwrap();
        fs::write(
            dir.join("App.tsx"),
            "import { mount, Text } from 'nsh'; mount(() => <Text>Hello</Text>);",
        )
        .unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"app-native\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n\
             [workspace]\n\n[dependencies]\nnsh-core = \"*\"\n",
        )
        .unwrap();
        fs::write(dir.join("src/lib.rs"), "").unwrap();
        // What the crate's entry registers: its set, as the describing program prints it.
        let description = nsh_core::element::description::Description::of(&[
            nsh_core::element::Registry::ui_set("app/native"),
        ])
        .to_json();
        fs::write(dir.join("description.json"), description).unwrap();
        let cargo = dir.join("cargo");
        fs::write(&cargo, CARGO).unwrap();
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
        app
    }

    /// Runs `nsh <args> --port 0 <app>` until its host exits, with the design socket `socket`.
    fn run(&self, args: &[&str], socket: Option<&str>) -> Output {
        let _ = fs::remove_file(self.0.join("builds.txt"));
        let real = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(env!("CARGO_BIN_EXE_nsh"));
        command
            .args(args)
            .args(["--port", "0"])
            .arg(&self.0)
            .current_dir(&self.0)
            .env("CARGO", self.0.join("cargo"))
            .env("REAL_CARGO", real)
            .env("BUILDS", self.0.join("builds.txt"))
            .env("DESCRIPTION", self.0.join("description.json"))
            .env("CARGO_TARGET_DIR", self.0.join("target"))
            .env("NO_COLOR", "1")
            .env_remove("NSH_DESIGN_SOCKET")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(socket) = socket {
            command.env("NSH_DESIGN_SOCKET", socket);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "nsh {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    /// Each build of the last run, in order: the programs it named (`describe`, `host`, joined
    /// with `+` in the order given) and its profile.
    fn builds(&self) -> Vec<(String, String)> {
        let builds = fs::read_to_string(self.0.join("builds.txt")).unwrap_or_default();
        builds
            .lines()
            .map(|line| {
                let words: Vec<&str> = line.split(' ').collect();
                let programs: Vec<&str> = words
                    .windows(2)
                    .filter(|w| w[0] == "-p")
                    .map(|w| {
                        if w[1].ends_with("-describe") {
                            "describe"
                        } else if w[1].ends_with("-host") {
                            "host"
                        } else {
                            "other"
                        }
                    })
                    .collect();
                let profile = match words.windows(2).find(|w| w[0] == "--profile") {
                    Some(w) => w[1].to_owned(),
                    None if words.contains(&"--release") => "release".to_owned(),
                    None => "dev".to_owned(),
                };
                (programs.join("+"), profile)
            })
            .collect()
    }

    /// The workspace's `[profile.design]` table.
    fn design_profile(&self) -> Vec<String> {
        let manifest = fs::read_to_string(self.0.join(".nsh/rust/Cargo.toml")).unwrap();
        manifest
            .lines()
            .skip_while(|line| *line != "[profile.design]")
            .skip(1)
            .take_while(|line| !line.is_empty() && !line.starts_with('['))
            .map(str::to_owned)
            .collect()
    }
}

fn built(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(programs, profile)| ((*programs).to_owned(), (*profile).to_owned()))
        .collect()
}

/// A design tool's `--release` run builds the describing program with the host, in one build with
/// `design`, which the workspace defines as release without LTO, incremental, and says so; the
/// host's own build names both again, so Cargo resolves the same features and has nothing left to
/// do. `nsh dev --release` too.
#[test]
fn a_design_tools_release_run_builds_describe_and_the_host_with_the_design_profile() {
    let app = App::new();
    let output = app.run(&["run", "--release"], Some("tcp:127.0.0.1:9"));
    assert_eq!(
        app.builds(),
        built(&[("describe+host", "design"), ("host+describe", "design")])
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("with the development host for ") && stderr.contains("(design build)"),
        "{stderr}"
    );
    assert_eq!(
        app.design_profile(),
        [
            "inherits = \"release\"",
            "lto = \"off\"",
            "codegen-units = 16",
            "incremental = true",
        ]
    );
    app.run(&["dev", "--release"], Some("/tmp/design.sock"));
    assert_eq!(
        app.builds(),
        built(&[("describe+host", "design"), ("host+describe", "design")])
    );
}

/// Without a design socket (or with an empty one) `--release` builds as before: the describing
/// program in debug, the host in release; a design tool's run without `--release` is a debug
/// run, both in debug.
#[test]
fn other_runs_build_describe_in_debug_and_the_host_as_asked() {
    let app = App::new();
    app.run(&["run", "--release"], None);
    assert_eq!(
        app.builds(),
        built(&[("describe", "dev"), ("host", "release")])
    );
    app.run(&["run", "--release"], Some(""));
    assert_eq!(
        app.builds(),
        built(&[("describe", "dev"), ("host", "release")])
    );
    app.run(&["run"], Some("tcp:127.0.0.1:9"));
    assert_eq!(app.builds(), built(&[("describe", "dev"), ("host", "dev")]));
}
