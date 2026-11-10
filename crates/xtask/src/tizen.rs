//! The Tizen runtime (Samsung TVs): `libnsh.so` (`nsh-platform-tizen`) per
//! architecture, release and development builds, cross-built by
//! dXn.GpuiTizen's driver (`cargo xtask cross` in its checkout, with its
//! target specs, the Tizen 8.0 rootstrap and the Tizen SDK's GCC
//! toolchain), and the TPKs' C# host (`dotnet build`), into
//! `target/tizen`, where `nsh run` / `nsh package` find them:
//!
//! ```text
//! target/tizen/{release,dev}/Nsh.Tizen.Host.dll       the host (and its .deps.json)
//! target/tizen/{release,dev}/<arch>/libnsh.so          armv7l, aarch64, x86_64; stripped
//! target/tizen/{release,dev}/symbols/<arch>/libnsh.so  the same with its symbols
//! target/tizen/targets/<spec>.json, toolchain          the target specs and the nightly they
//!                                                      build with, for apps' Rust crates
//! ```
//!
//! dXn.GpuiTizen's checkout is `DXN_GPUI_TIZEN`, else `../dXn.GpuiTizen`
//! next to this checkout. Its TV builds take gpui-fast and wgpu from their
//! forks: until they are published, a local Cargo config of
//! `[patch.crates-io]` lines (`--config`, `NSH_TIZEN_CARGO_CONFIG`),
//! passed on to Cargo (`docs/dev/tizen.md`).

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, bail};

use crate::{android, term};

/// A Tizen architecture of the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arch {
    /// 32-bit ARM: the TVs.
    Armv7l,
    /// 64-bit ARM.
    Aarch64,
    /// The emulator.
    X86_64,
}

/// Every architecture, in the order they are built.
pub(crate) const ARCHS: [Arch; 3] = [Arch::Armv7l, Arch::Aarch64, Arch::X86_64];

impl Arch {
    /// Its directory in the runtime (Tizen's name for it).
    pub(crate) fn name(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7l",
            Arch::Aarch64 => "aarch64",
            Arch::X86_64 => "x86_64",
        }
    }

    /// `armv7l` (or `armv7`, `arm`), `aarch64` (`arm64`), `x86_64`.
    pub(crate) fn parse(name: &str) -> anyhow::Result<Arch> {
        Ok(match name {
            "armv7l" | "armv7" | "arm" => Arch::Armv7l,
            "aarch64" | "arm64" => Arch::Aarch64,
            "x86_64" | "x86-64" => Arch::X86_64,
            other => bail!("unknown Tizen architecture {other} (armv7l, aarch64, x86_64)"),
        })
    }

    /// The `--target` of dXn.GpuiTizen's cross driver.
    fn cross_target(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7",
            Arch::Aarch64 => "aarch64",
            Arch::X86_64 => "x86_64",
        }
    }

    /// Its target spec's name (`targets/<spec>.json` in dXn.GpuiTizen),
    /// which names its directory in the target directory.
    pub(crate) fn spec(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7-tizen-linux-gnueabi",
            Arch::Aarch64 => "aarch64-tizen-linux-gnu",
            Arch::X86_64 => "x86_64-tizen-linux-gnu",
        }
    }
}

/// Which builds of the library to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Variant {
    /// The packaged apps' library.
    Release,
    /// The development runtime (feature `dev`): `nsh run`'s.
    Dev,
    /// Both.
    Both,
}

impl Variant {
    /// The runtime's variants it makes: (`dev`, directory).
    fn dirs(self) -> &'static [(bool, &'static str)] {
        match self {
            Variant::Release => &[(false, "release")],
            Variant::Dev => &[(true, "dev")],
            Variant::Both => &[(false, "release"), (true, "dev")],
        }
    }
}

/// The C# host's assembly, and what ships beside it.
pub(crate) const HOST_FILES: [&str; 2] = ["Nsh.Tizen.Host.dll", "Nsh.Tizen.Host.deps.json"];
/// The host's project, relative to this checkout.
const HOST_PROJECT: &str = "crates/gpui/nsh-platform-tizen/host/Nsh.Tizen.Host.csproj";
/// Where `dotnet build -c Release` writes it, relative to the project.
const HOST_OUTPUT: &str = "bin/Release/tizen80";

/// dXn.GpuiTizen's checkout: `dxn_gpui_tizen` (`DXN_GPUI_TIZEN`), else the
/// one next to this checkout (`root`).
pub(crate) fn checkout(root: &Path, dxn_gpui_tizen: Option<OsString>) -> PathBuf {
    match dxn_gpui_tizen.filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => root.join("../dXn.GpuiTizen"),
    }
}

/// The target directory the cross builds write to: `CARGO_TARGET_DIR`
/// (relative to this checkout, as a Cargo command here takes it), else
/// this checkout's `target`. It is given to the driver, which runs in
/// dXn.GpuiTizen's checkout, as an absolute path.
pub(crate) fn target_dir(root: &Path, cargo_target_dir: Option<OsString>) -> PathBuf {
    match cargo_target_dir.filter(|dir| !dir.is_empty()) {
        Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
        Some(dir) => root.join(dir),
        None => root.join("target"),
    }
}

/// The arguments of the cross driver (`cargo <these>`, in dXn.GpuiTizen's
/// checkout) that build `variant` of `libnsh.so` for `arch` with the
/// symbol check (one export, `nsh_tizen_main`), `configs` passed on to
/// Cargo. Both variants are one build (`--with-dev-variant`, the dev one in
/// `<target>/dev-variant`); the dev one alone goes there too.
pub(crate) fn cross_args(
    root: &Path,
    arch: Arch,
    variant: Variant,
    target: &Path,
    configs: &[PathBuf],
) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "xtask",
        "cross",
        "--target",
        arch.cross_target(),
        "--release",
        "--manifest-path",
    ]
    .map(OsString::from)
    .into();
    args.push(root.join("crates/gpui/Cargo.toml").into());
    args.extend(
        [
            "-p",
            "nsh-platform-tizen",
            "--check-symbols",
            "--entry",
            "nsh_tizen_main",
        ]
        .map(OsString::from),
    );
    match variant {
        Variant::Release => {}
        Variant::Both => args.push("--with-dev-variant".into()),
        Variant::Dev => {
            args.extend(["--features", "dev", "--target-dir"].map(OsString::from));
            args.push(target.join("dev-variant").into());
        }
    }
    for config in configs {
        args.push("--config".into());
        args.push(config.into());
    }
    args
}

/// Where the cross build put `arch`'s library (`dev`: the development one).
pub(crate) fn built_library(target: &Path, arch: Arch, dev: bool) -> PathBuf {
    let dir = if dev {
        target.join("dev-variant")
    } else {
        target.to_owned()
    };
    dir.join(arch.spec()).join("release/libnsh.so")
}

/// A file and where it goes.
pub(crate) type Pair = (PathBuf, PathBuf);

/// The runtime's files for `variant` ([`Variant::dirs`]) and `archs` in
/// `out` (`target/tizen`): (from, to) for each copy, the libraries to
/// strip apart (`(symbols, stripped)`), from the cross build in `target`
/// and the host's output in `host`.
pub(crate) fn layout(
    out: &Path,
    target: &Path,
    host: &Path,
    variant: Variant,
    archs: &[Arch],
) -> (Vec<Pair>, Vec<Pair>) {
    let mut copies = Vec::new();
    let mut strips = Vec::new();
    for &(dev, name) in variant.dirs() {
        let dir = out.join(name);
        for file in HOST_FILES {
            copies.push((host.join(file), dir.join(file)));
        }
        for &arch in archs {
            let symbols = dir.join("symbols").join(arch.name()).join("libnsh.so");
            copies.push((built_library(target, arch, dev), symbols.clone()));
            strips.push((symbols, dir.join(arch.name()).join("libnsh.so")));
        }
    }
    (copies, strips)
}

/// The nightly dXn.GpuiTizen builds its TV libraries with (`channel` in
/// its `rust-toolchain.toml`).
pub(crate) fn toolchain_channel(rust_toolchain: &str) -> anyhow::Result<String> {
    rust_toolchain
        .lines()
        .filter_map(|line| line.trim().strip_prefix("channel"))
        .filter_map(|rest| rest.trim_start().strip_prefix('='))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .find(|value| !value.is_empty())
        .context("rust-toolchain.toml names no channel")
}

/// The distribution's Tizen runtime (`runtime/tizen` in `out`) from the
/// build in `built` (`target/tizen`): both variants' host and stripped
/// libraries for `archs`, and the target specs with their toolchain; the
/// symbols stay in the build.
pub(crate) fn dist_files(built: &Path, out: &Path, archs: &[Arch]) -> Vec<(PathBuf, PathBuf)> {
    let mut files = Vec::new();
    for (_, name) in Variant::Both.dirs() {
        for file in HOST_FILES {
            files.push((built.join(name).join(file), out.join(name).join(file)));
        }
        for arch in archs {
            let lib = Path::new(name).join(arch.name()).join("libnsh.so");
            files.push((built.join(&lib), out.join(&lib)));
        }
    }
    for file in ARCHS
        .iter()
        .map(|arch| format!("{}.json", arch.spec()))
        .chain(["toolchain".to_owned()])
    {
        let file = Path::new("targets").join(file);
        files.push((built.join(&file), out.join(&file)));
    }
    files
}

/// Why the distribution is made without the Tizen runtime, or `None` to
/// build it: not when `NSH_DIST_NO_TIZEN` opts out, not without
/// dXn.GpuiTizen's checkout (`checkout`), and not without the .NET SDK
/// (`dotnet`, asked last because it runs it).
pub(crate) fn dist_skip(
    opted_out: bool,
    checkout: bool,
    dotnet: impl FnOnce() -> bool,
) -> Option<&'static str> {
    if opted_out {
        Some("NSH_DIST_NO_TIZEN is set")
    } else if !checkout {
        Some("no dXn.GpuiTizen checkout (DXN_GPUI_TIZEN)")
    } else if !dotnet() {
        Some("no .NET SDK (dotnet)")
    } else {
        None
    }
}

/// Builds the Tizen runtime for `archs` into `target/tizen`, returning
/// that directory.
pub(crate) fn build(
    variant: Variant,
    archs: &[Arch],
    configs: &[PathBuf],
) -> anyhow::Result<PathBuf> {
    let root = android::root();
    let root = root.canonicalize().unwrap_or(root);
    let dxn = checkout(&root, env::var_os("DXN_GPUI_TIZEN"));
    if !dxn.join("xtask").is_dir() {
        bail!(
            "dXn.GpuiTizen is not at {}: set DXN_GPUI_TIZEN to its checkout (docs/dev/tizen.md)",
            dxn.display()
        );
    }
    let target = target_dir(&root, env::var_os("CARGO_TARGET_DIR"));
    let configs: Vec<PathBuf> = configs
        .iter()
        .map(|config| {
            if config.is_absolute() {
                config.clone()
            } else {
                root.join(config)
            }
        })
        .collect();
    for &arch in archs {
        // Rustup's `cargo`, not this one's (`CARGO`): dXn.GpuiTizen's
        // `rust-toolchain.toml` picks the toolchain there.
        let mut cross = Command::new("cargo");
        cross
            .current_dir(&dxn)
            .args(cross_args(&root, arch, variant, &target, &configs))
            .env("CARGO_TARGET_DIR", &target)
            // The driver runs dXn.GpuiTizen's pinned nightly; xtask's own
            // toolchain must not be what it inherits.
            .env_remove("RUSTUP_TOOLCHAIN")
            .env_remove("RUSTC")
            .env_remove("CARGO");
        term::cargo(&mut cross, &format!("libnsh.so for {}", arch.name()))?;
    }
    let host = host(&root)?;
    let toolchain = toolchain_channel(
        &fs::read_to_string(dxn.join("rust-toolchain.toml"))
            .context("read dXn.GpuiTizen's rust-toolchain.toml")?,
    )?;
    let out = root.join("target/tizen");
    let (copies, strips) = layout(&out, &target, &host, variant, archs);
    for (from, to) in copies {
        crate::dist::copy(&from, &to)?;
    }
    let strip = llvm_strip(&toolchain)?;
    for (from, to) in strips {
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir)?;
        }
        // The exports (`nsh_tizen_main`) and what the loader needs stay.
        term::run(
            Command::new(&strip)
                .arg("--strip-unneeded")
                .arg("-o")
                .arg(&to)
                .arg(&from),
        )?;
    }
    let targets = out.join("targets");
    fs::create_dir_all(&targets)?;
    for arch in ARCHS {
        let spec = format!("{}.json", arch.spec());
        crate::dist::copy(&dxn.join("targets").join(&spec), &targets.join(&spec))?;
    }
    fs::write(targets.join("toolchain"), format!("{toolchain}\n"))?;
    term::note(out.strip_prefix(&root).unwrap_or(&out).display());
    Ok(out)
}

/// Builds the C# host (`dotnet build -c Release`), returning its output.
fn host(root: &Path) -> anyhow::Result<PathBuf> {
    let project = root.join(HOST_PROJECT);
    let step = term::Step::start("building the C# host");
    let output = Command::new("dotnet")
        .args(["build", "-c", "Release", "-nologo", "-v", "quiet"])
        .arg(&project)
        .output()
        .context("run dotnet (install the .NET SDK to build the TPKs' host)")?;
    if !output.status.success() {
        bail!(
            "dotnet build of {} failed:\n{}{}",
            project.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    step.done("built the C# host");
    Ok(project
        .parent()
        .context("the host's project has no directory")?
        .join(HOST_OUTPUT))
}

/// `llvm-strip` of the nightly `toolchain` (its `llvm-tools`).
fn llvm_strip(toolchain: &str) -> anyhow::Result<PathBuf> {
    let output = Command::new("rustc")
        .env("RUSTUP_TOOLCHAIN", toolchain)
        .args(["--print", "target-libdir"])
        .output()
        .with_context(|| format!("run rustc of {toolchain}"))?;
    if !output.status.success() {
        bail!(
            "rustc of {toolchain}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    // `<sysroot>/lib/rustlib/<host>/lib`: the tools are in `../bin`.
    let libdir = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let strip = libdir
        .parent()
        .context("rustc's target libdir")?
        .join("bin")
        .join(format!("llvm-strip{}", env::consts::EXE_SUFFIX));
    if !strip.is_file() {
        bail!(
            "no llvm-strip in {toolchain}: rustup component add llvm-tools --toolchain {toolchain}"
        );
    }
    Ok(strip)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn architectures_have_tizens_names_and_the_drivers() {
        assert_eq!(ARCHS.map(Arch::name), ["armv7l", "aarch64", "x86_64"]);
        for (name, arch) in [
            ("armv7l", Arch::Armv7l),
            ("armv7", Arch::Armv7l),
            ("arm64", Arch::Aarch64),
            ("x86_64", Arch::X86_64),
        ] {
            assert_eq!(Arch::parse(name).unwrap(), arch, "{name}");
        }
        assert!(Arch::parse("i586").is_err());
        assert_eq!(Arch::Armv7l.cross_target(), "armv7");
        assert_eq!(Arch::X86_64.spec(), "x86_64-tizen-linux-gnu");
    }

    #[test]
    fn the_checkout_and_target_directory_come_from_the_environment() {
        let root = Path::new("/src/nsh");
        assert_eq!(checkout(root, None), Path::new("/src/nsh/../dXn.GpuiTizen"));
        assert_eq!(
            checkout(root, Some("/src/dxn".into())),
            Path::new("/src/dxn")
        );
        assert_eq!(checkout(root, Some("".into())), checkout(root, None));
        assert_eq!(target_dir(root, None), Path::new("/src/nsh/target"));
        assert_eq!(
            target_dir(root, Some("out".into())),
            Path::new("/src/nsh/out")
        );
        assert_eq!(target_dir(root, Some("/t".into())), Path::new("/t"));
    }

    #[test]
    fn the_cross_driver_builds_and_checks_each_variant() {
        let root = Path::new("/src/nsh");
        let target = Path::new("/t");
        let configs = [PathBuf::from("/p.toml")];
        let head = [
            "xtask",
            "cross",
            "--target",
            "armv7",
            "--release",
            "--manifest-path",
            "/src/nsh/crates/gpui/Cargo.toml",
            "-p",
            "nsh-platform-tizen",
            "--check-symbols",
            "--entry",
            "nsh_tizen_main",
        ];
        let with = |tail: &[&str]| -> Vec<String> {
            head.iter().chain(tail).map(|s| (*s).to_owned()).collect()
        };
        assert_eq!(
            strings(&cross_args(
                root,
                Arch::Armv7l,
                Variant::Both,
                target,
                &configs
            )),
            with(&["--with-dev-variant", "--config", "/p.toml"])
        );
        assert_eq!(
            strings(&cross_args(
                root,
                Arch::Armv7l,
                Variant::Release,
                target,
                &[]
            )),
            with(&[])
        );
        assert_eq!(
            strings(&cross_args(root, Arch::Armv7l, Variant::Dev, target, &[])),
            with(&["--features", "dev", "--target-dir", "/t/dev-variant"])
        );
        assert_eq!(
            built_library(target, Arch::Aarch64, false),
            Path::new("/t/aarch64-tizen-linux-gnu/release/libnsh.so")
        );
        assert_eq!(
            built_library(target, Arch::Armv7l, true),
            Path::new("/t/dev-variant/armv7-tizen-linux-gnueabi/release/libnsh.so")
        );
    }

    #[test]
    fn the_runtime_has_the_host_and_a_library_per_architecture() {
        let (copies, strips) = layout(
            Path::new("/o"),
            Path::new("/t"),
            Path::new("/h"),
            Variant::Both,
            &[Arch::Armv7l, Arch::X86_64],
        );
        let pair = |a: &str, b: &str| (PathBuf::from(a), PathBuf::from(b));
        assert_eq!(
            copies,
            [
                pair("/h/Nsh.Tizen.Host.dll", "/o/release/Nsh.Tizen.Host.dll"),
                pair(
                    "/h/Nsh.Tizen.Host.deps.json",
                    "/o/release/Nsh.Tizen.Host.deps.json"
                ),
                pair(
                    "/t/armv7-tizen-linux-gnueabi/release/libnsh.so",
                    "/o/release/symbols/armv7l/libnsh.so"
                ),
                pair(
                    "/t/x86_64-tizen-linux-gnu/release/libnsh.so",
                    "/o/release/symbols/x86_64/libnsh.so"
                ),
                pair("/h/Nsh.Tizen.Host.dll", "/o/dev/Nsh.Tizen.Host.dll"),
                pair(
                    "/h/Nsh.Tizen.Host.deps.json",
                    "/o/dev/Nsh.Tizen.Host.deps.json"
                ),
                pair(
                    "/t/dev-variant/armv7-tizen-linux-gnueabi/release/libnsh.so",
                    "/o/dev/symbols/armv7l/libnsh.so"
                ),
                pair(
                    "/t/dev-variant/x86_64-tizen-linux-gnu/release/libnsh.so",
                    "/o/dev/symbols/x86_64/libnsh.so"
                ),
            ]
        );
        assert_eq!(
            strips[0],
            pair(
                "/o/release/symbols/armv7l/libnsh.so",
                "/o/release/armv7l/libnsh.so"
            )
        );
        assert_eq!(strips.len(), 4);
        let (copies, _) = layout(
            Path::new("/o"),
            Path::new("/t"),
            Path::new("/h"),
            Variant::Dev,
            &[Arch::Aarch64],
        );
        assert!(copies.iter().all(|(_, to)| to.starts_with("/o/dev")));
    }

    #[test]
    fn the_distribution_takes_the_stripped_libraries_and_the_specs() {
        let files = dist_files(Path::new("/b"), Path::new("/d"), &[Arch::Armv7l]);
        let to: Vec<String> = files
            .iter()
            .map(|(_, to)| to.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            to,
            [
                "/d/release/Nsh.Tizen.Host.dll",
                "/d/release/Nsh.Tizen.Host.deps.json",
                "/d/release/armv7l/libnsh.so",
                "/d/dev/Nsh.Tizen.Host.dll",
                "/d/dev/Nsh.Tizen.Host.deps.json",
                "/d/dev/armv7l/libnsh.so",
                "/d/targets/armv7-tizen-linux-gnueabi.json",
                "/d/targets/aarch64-tizen-linux-gnu.json",
                "/d/targets/x86_64-tizen-linux-gnu.json",
                "/d/targets/toolchain",
            ]
        );
        assert!(
            files
                .iter()
                .all(|(from, _)| !from.starts_with("/b/release/symbols"))
        );
        assert_eq!(files[2].0, Path::new("/b/release/armv7l/libnsh.so"));
    }

    #[test]
    fn tizen_is_left_out_when_opted_out_or_unbuildable() {
        assert_eq!(dist_skip(false, true, || true), None);
        assert_eq!(
            dist_skip(true, true, || true),
            Some("NSH_DIST_NO_TIZEN is set")
        );
        assert_eq!(
            dist_skip(false, false, || true),
            Some("no dXn.GpuiTizen checkout (DXN_GPUI_TIZEN)")
        );
        assert_eq!(
            dist_skip(false, true, || false),
            Some("no .NET SDK (dotnet)")
        );
        assert!(dist_skip(true, true, || panic!("ran dotnet")).is_some());
    }

    #[test]
    fn the_nightly_is_dxn_gpui_tizens() {
        let toml = "# One nightly.\n[toolchain]\nchannel = \"nightly-2026-10-18\"\ncomponents = [\"rust-src\"]\n";
        assert_eq!(toolchain_channel(toml).unwrap(), "nightly-2026-10-18");
        assert!(toolchain_channel("[toolchain]\n").is_err());
    }
}
