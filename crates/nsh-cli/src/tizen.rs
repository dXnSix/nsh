//! Samsung TVs natively: `--target tizen`. Apps run in `nsh-platform-tizen`'s
//! `libnsh.so` (NSH's GPUI backend on dXn.GpuiTizen's Tizen platform),
//! which a TPK's C# host starts; the runtime is the `tizen` component
//! (`runtime/tizen`, [`crate::runtime::tizen`]):
//!
//! ```text
//! runtime/tizen/{release,dev}/Nsh.Tizen.Host.dll   the host (and its .deps.json)
//! runtime/tizen/{release,dev}/<arch>/libnsh.so     armv7l, aarch64, x86_64
//! runtime/tizen/targets/<spec>.json, toolchain     the target specs and their nightly (apps' Rust crates)
//! runtime/tizen/targets/symbols.allow             what an app's library may import ([`symbols`])
//! ```
//!
//! `nsh package --target tizen` writes a TPK ([`package`]):
//!
//! ```text
//! <out>/tizen/<name>-<arch>/        the staged package ([`stage`]):
//!   bin/Nsh.Tizen.Host.dll          the host
//!   lib/libnsh.so                   the release library for the architecture
//!   res/nsh/                        bundle.nshc, capabilities.json, app.json, assets/
//!   shared/res/icon.png             the icon, 512×423
//!   tizen-manifest.xml              from nsh.json ([`Manifest`])
//! <out>/<name>.tpk                  the staged package zipped, then re-signed by
//!                                   `tz pack -t tpk -s <profile> -b <unsigned> -o <out>`
//! ```
//!
//! With several architectures (`--arch armv7l,aarch64`) each package's
//! version gets a fourth number (armv7l 1, aarch64 2, x86_64 3) and the CPU
//! feature its TVs match, `<out>/<package>-<version>.<n>-<arch>.tpk`, and the
//! TVs' packages (armv7l, aarch64; not the emulator's x86_64) go into the
//! multi-architecture `.zip` Samsung's Seller Office takes,
//! `<out>/<package>-<version>.zip` (dXn.GpuiTizen's conventions).
//!
//! `nsh run --target tizen` installs the development TPK (the `dev`
//! library, one package for every app: [`DEV_PACKAGE`]) on the TV with
//! `tz install` when the device does not have this build, maps the
//! library's dev port to the dev server with `sdb reverse` (else listens
//! for it through `sdb forward` and relays), and launches it ([`Sdb`]).
//! The `nsh.json` `tizen` section is `tizen-web`'s ([`crate::package::TizenConfig`]).

use std::{
    fmt::Write as _,
    fs,
    io::{Read as _, Write as _},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use anyhow::{Context as _, bail};

use crate::{
    build::Built,
    package::{AppConfig, PackageOptions, PackageTarget},
    tizen_web, ui,
};

pub(crate) mod symbols;

/// A TV's architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub(crate) enum Arch {
    /// 32-bit ARM: the TVs (`armv7` too).
    #[value(name = "armv7l", alias = "armv7")]
    Armv7l,
    /// 64-bit ARM (`arm64` too).
    #[value(alias = "arm64")]
    Aarch64,
    /// The emulator.
    #[value(name = "x86_64")]
    X86_64,
}

impl Arch {
    /// Its directory in the runtime (Tizen's name for it).
    pub(crate) fn name(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7l",
            Arch::Aarch64 => "aarch64",
            Arch::X86_64 => "x86_64",
        }
    }

    /// Its name in a package's file name.
    fn file_name(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7",
            Arch::Aarch64 => "aarch64",
            Arch::X86_64 => "x86_64",
        }
    }

    /// The fourth version number of its package in a multi-architecture
    /// upload, unique per architecture as Seller Office asks.
    fn number(self) -> u32 {
        match self {
            Arch::Armv7l => 1,
            Arch::Aarch64 => 2,
            Arch::X86_64 => 3,
        }
    }

    /// Whether its package goes into the multi-architecture upload: the
    /// TVs' (armv7l, aarch64), not the emulator's (dXn.GpuiTizen's
    /// `STORE_TARGETS`).
    pub(crate) fn in_store(self) -> bool {
        matches!(self, Arch::Armv7l | Arch::Aarch64)
    }

    /// The CPU feature its package requires there (`<feature>`).
    fn feature(self) -> &'static str {
        match self {
            Arch::Armv7l => "http://tizen.org/feature/platform.core.cpu.arch.armv7",
            Arch::Aarch64 => "http://tizen.org/feature/platform.core.cpu.arch.armv8",
            Arch::X86_64 => "http://tizen.org/feature/platform.core.cpu.arch.x86_64",
        }
    }

    /// Its target spec (`runtime/tizen/targets/<spec>.json`).
    pub(crate) fn spec(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7-tizen-linux-gnueabi",
            Arch::Aarch64 => "aarch64-tizen-linux-gnu",
            Arch::X86_64 => "x86_64-tizen-linux-gnu",
        }
    }

    /// The architecture `sdb capability` reports as `cpu_arch`.
    fn of_device(cpu_arch: &str) -> Option<Arch> {
        match cpu_arch {
            "armv7" | "armv7l" | "arm" => Some(Arch::Armv7l),
            "armv8" | "aarch64" | "arm64" => Some(Arch::Aarch64),
            "x86_64" | "x86-64" | "amd64" => Some(Arch::X86_64),
            _ => None,
        }
    }
}

/// `api-version` without `nsh.json` `tizen.apiVersion`: Tizen 8.0, the TVs
/// of 2024 on (dXn.GpuiTizen's floor).
const API_VERSION: &str = "8.0";
/// What every package asks for: the network (the dev server's, the app's).
const PRIVILEGES: &[&str] = &["http://tizen.org/privilege/internet"];
/// The host's assembly (the manifest's `exec`) and what ships beside it.
pub(crate) const HOST: &str = "Nsh.Tizen.Host.dll";
pub(crate) const HOST_DEPS: &str = "Nsh.Tizen.Host.deps.json";
/// The development TPK's package id: one for every app (the library runs
/// whichever app's dev server it reaches), apart from `tizen-web`'s.
pub(crate) const DEV_PACKAGE: &str = "NshDevTizn";
/// The port on the TV the development library connects to, which `sdb
/// reverse` maps to the dev server (`nsh-platform-tizen`'s `dev::DEV_PORT`).
pub(crate) const DEV_PORT: u16 = 8790;
/// The port the development library listens on when the reverse port
/// keeps refusing (`dev::LISTEN_PORT`), which `sdb forward` reaches.
pub(crate) const LISTEN_PORT: u16 = 8791;
/// What a command that signs says when there is no profile.
const NO_PROFILE: &str = "a TPK installs on a TV signed: pass --sign <profile> or set nsh.json tizen.signing \
     to a certificate profile. Make one with Samsung's Certificate Manager (the Tizen extension for VS Code, \
     or `tz` from the Tizen SDK: an author certificate and a distributor certificate with the TV's DUID), \
     then name it";

/// A TPK's manifest, `tizen-manifest.xml` ([`Manifest::xml`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Manifest {
    /// The package id: 10 letters and digits.
    pub(crate) package: String,
    /// The application id, `<package>.<name>`.
    pub(crate) app: String,
    /// The name the TV shows.
    pub(crate) label: String,
    /// `x.y.z`.
    pub(crate) version: String,
    /// The lowest Tizen that installs it.
    pub(crate) api_version: String,
    pub(crate) privileges: Vec<String>,
    /// Whether the package has an icon (`shared/res/icon.png`).
    pub(crate) icon: bool,
    /// For one package of a multi-architecture upload: its architecture
    /// (the version's fourth number, the CPU feature).
    pub(crate) arch: Option<Arch>,
}

impl Manifest {
    /// The app's manifest from `nsh.json` (`tizen`, `version`, `icon`), its
    /// display `name` and its `id`, checked as Tizen checks it: the same
    /// keys and defaults as a Tizen web app's (`tizen_web`).
    pub(crate) fn of(config: &AppConfig, name: &str, id: &str) -> anyhow::Result<Manifest> {
        let tizen = &config.tizen;
        let package = match &tizen.package {
            Some(package) => tizen_web::check_package_id(package)?.to_owned(),
            None => tizen_web::package_id(id),
        };
        let app = match &tizen.app_id {
            Some(app) => tizen_web::check_app_id(app, &package)?.to_owned(),
            None => format!("{package}.{}", tizen_web::app_name(name)),
        };
        let version = tizen
            .version
            .as_deref()
            .or(config.version.as_deref())
            .unwrap_or("1.0.0");
        tizen_web::check_version(version)?;
        // A TPK's version is `x.y.z`: `2.1` is `2.1.0`.
        let version = if version.split('.').count() == 2 {
            format!("{version}.0")
        } else {
            version.to_owned()
        };
        let api_version = tizen.api_version.as_deref().unwrap_or(API_VERSION);
        tizen_web::check_api_version(api_version)?;
        let mut privileges: Vec<String> = PRIVILEGES.iter().map(|p| (*p).to_owned()).collect();
        for privilege in &tizen.privileges {
            check_privilege(privilege)?;
            if !privileges.contains(privilege) {
                privileges.push(privilege.clone());
            }
        }
        Ok(Manifest {
            package,
            app,
            label: name.to_owned(),
            version,
            api_version: api_version.to_owned(),
            privileges,
            icon: config.icon.is_some(),
            arch: None,
        })
    }

    /// The development TPK's: one package for every app.
    pub(crate) fn dev() -> Manifest {
        Manifest {
            package: DEV_PACKAGE.to_owned(),
            app: format!("{DEV_PACKAGE}.NshDev"),
            label: "NSH Dev".to_owned(),
            version: "1.0.0".to_owned(),
            api_version: API_VERSION.to_owned(),
            privileges: PRIVILEGES.iter().map(|p| (*p).to_owned()).collect(),
            icon: false,
            arch: None,
        }
    }

    /// Its version, with the architecture's fourth number in a
    /// multi-architecture upload.
    pub(crate) fn full_version(&self) -> String {
        match self.arch {
            Some(arch) => format!("{}.{}", self.version, arch.number()),
            None => self.version.clone(),
        }
    }

    /// `tizen-manifest.xml`: a .NET UI application (the host) on the
    /// `tv-samsung` profile, as dXn.GpuiTizen's `host/tizen-manifest.template.xml`.
    pub(crate) fn xml(&self) -> String {
        use tizen_web::escape;
        let mut xml = String::new();
        let _ = writeln!(xml, r#"<?xml version="1.0" encoding="utf-8"?>"#);
        let _ = writeln!(
            xml,
            r#"<manifest xmlns="http://tizen.org/ns/packages" api-version="{}" package="{}" version="{}">"#,
            escape(&self.api_version),
            escape(&self.package),
            escape(&self.full_version())
        );
        let _ = writeln!(xml, r#"  <profile name="tv-samsung" />"#);
        let _ = writeln!(
            xml,
            r#"  <ui-application appid="{}" exec="{HOST}" type="dotnet" multiple="false" taskmanage="true" nodisplay="false" launch_mode="single">"#,
            escape(&self.app)
        );
        let _ = writeln!(xml, "    <label>{}</label>", escape(&self.label));
        if self.icon {
            let _ = writeln!(xml, "    <icon>icon.png</icon>");
        }
        let _ = writeln!(
            xml,
            r#"    <metadata key="http://tizen.org/metadata/prefer_dotnet_aot" value="true" />"#
        );
        let _ = writeln!(xml, "  </ui-application>");
        let _ = writeln!(xml, "  <privileges>");
        for privilege in &self.privileges {
            let _ = writeln!(xml, "    <privilege>{}</privilege>", escape(privilege));
        }
        let _ = writeln!(xml, "  </privileges>");
        if let Some(arch) = self.arch {
            let _ = writeln!(
                xml,
                r#"  <feature name="{}">true</feature>"#,
                arch.feature()
            );
        }
        xml.push_str("</manifest>\n");
        xml
    }
}

/// A privilege is a URI: Tizen's (`http://tizen.org/privilege/...`) or
/// Samsung's (`http://developer.samsung.com/privilege/...`).
fn check_privilege(privilege: &str) -> anyhow::Result<()> {
    let ok = ["http://", "https://"]
        .iter()
        .any(|scheme| privilege.starts_with(scheme))
        && privilege.contains("/privilege/")
        && !privilege.contains(char::is_whitespace);
    if !ok {
        bail!(
            "nsh.json tizen.privileges `{privilege}`: a privilege's URI, as `http://tizen.org/privilege/internet`"
        );
    }
    Ok(())
}

/// The architectures a package is made for: `requested`, else armv7l
/// (the TVs'), each once, in order.
pub(crate) fn package_archs(requested: &[Arch]) -> Vec<Arch> {
    let mut archs: Vec<Arch> = requested.to_vec();
    archs.sort();
    archs.dedup();
    if archs.is_empty() {
        archs.push(Arch::Armv7l);
    }
    archs
}

/// One file of a staged package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    /// A copy of a file.
    Copy { from: PathBuf, to: PathBuf },
    /// The app's icon, as TVs show it (512×423).
    Icon { from: PathBuf, to: PathBuf },
    /// Contents written here.
    Write { to: PathBuf, contents: Vec<u8> },
}

impl Entry {
    /// Its path in the package.
    pub(crate) fn path(&self) -> &Path {
        match self {
            Entry::Copy { to, .. } | Entry::Icon { to, .. } | Entry::Write { to, .. } => to,
        }
    }
}

/// What a package holds of the app, `res/nsh/` (`nsh-platform-tizen`'s
/// `package.rs`): the bundle (none for an app of Rust only), the
/// capabilities, `app.json`, the assets (their path in `assets/`, file).
#[derive(Debug, Clone, Default)]
pub(crate) struct AppFiles {
    pub(crate) bundle: Option<Vec<u8>>,
    pub(crate) capabilities: Option<String>,
    pub(crate) app_json: String,
    pub(crate) assets: Vec<(String, PathBuf)>,
    pub(crate) icon: Option<PathBuf>,
}

/// The staged TPK for `arch` from the runtime variant at `runtime`
/// (`runtime/tizen/release` or `dev`): the host in `bin/`, the library in
/// `lib/`, the app's files in `res/nsh/`, the icon, the manifest.
pub(crate) fn stage(runtime: &Path, arch: Arch, app: &AppFiles, manifest: &Manifest) -> Vec<Entry> {
    let mut entries = vec![Entry::Copy {
        from: runtime.join(HOST),
        to: Path::new("bin").join(HOST),
    }];
    if runtime.join(HOST_DEPS).exists() {
        entries.push(Entry::Copy {
            from: runtime.join(HOST_DEPS),
            to: Path::new("bin").join(HOST_DEPS),
        });
    }
    entries.push(Entry::Copy {
        from: runtime.join(arch.name()).join("libnsh.so"),
        to: PathBuf::from("lib/libnsh.so"),
    });
    let res = Path::new("res/nsh");
    if let Some(bundle) = &app.bundle {
        entries.push(Entry::Write {
            to: res.join("bundle.nshc"),
            contents: bundle.clone(),
        });
    }
    if let Some(capabilities) = &app.capabilities {
        entries.push(Entry::Write {
            to: res.join("capabilities.json"),
            contents: capabilities.clone().into_bytes(),
        });
    }
    entries.push(Entry::Write {
        to: res.join("app.json"),
        contents: app.app_json.clone().into_bytes(),
    });
    for (path, from) in &app.assets {
        entries.push(Entry::Copy {
            from: from.clone(),
            to: res.join("assets").join(path),
        });
    }
    if let Some(icon) = &app.icon {
        entries.push(Entry::Icon {
            from: icon.clone(),
            to: PathBuf::from("shared/res/icon.png"),
        });
    }
    entries.push(Entry::Write {
        to: PathBuf::from("tizen-manifest.xml"),
        contents: manifest.xml().into_bytes(),
    });
    entries
}

/// Writes `entries` into `dir`, emptied first.
pub(crate) fn write_staged(dir: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("remove {}", dir.display()))?;
    }
    fs::create_dir_all(dir.join("res")).with_context(|| format!("create {}", dir.display()))?;
    for entry in entries {
        let to = dir.join(entry.path());
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        match entry {
            Entry::Copy { from, .. } => {
                fs::copy(from, &to).with_context(|| format!("copy {}", from.display()))?;
            }
            Entry::Icon { from, .. } => tizen_web::write_icon(from, &to)?,
            Entry::Write { contents, .. } => fs::write(&to, contents)?,
        }
    }
    Ok(())
}

/// The signed package's file name: `<name>.tpk`, or in a multi-architecture
/// upload `<package>-<version>.<n>-<arch>.tpk`.
pub(crate) fn tpk_name(manifest: &Manifest, name: &str) -> String {
    match manifest.arch {
        Some(arch) => format!(
            "{}-{}-{}.tpk",
            manifest.package,
            manifest.full_version(),
            arch.file_name()
        ),
        None => format!("{name}.tpk"),
    }
}

/// The multi-architecture upload's file name: `<package>-<version>.zip`.
pub(crate) fn zip_name(manifest: &Manifest) -> String {
    format!("{}-{}.zip", manifest.package, manifest.version)
}

/// `tz`'s command that signs `unsigned` (the staged package, zipped) with
/// certificate profile `profile` into `signed`: its repackaging of a
/// package. `tz pack` signs only a `tz` project or, with `-b`, a package;
/// given the staged directory it answers "is not a valid project".
pub(crate) fn sign_command(tz: &Path, profile: &str, unsigned: &Path, signed: &Path) -> Command {
    let mut command = Command::new(tz);
    command
        .args(["pack", "-t", "tpk", "-s", profile, "-b"])
        .arg(unsigned)
        .arg("-o")
        .arg(signed);
    command
}

/// The unsigned package [`sign`] zips the staged one into, beside `signed`.
fn unsigned_path(signed: &Path) -> PathBuf {
    signed.with_extension("unsigned.tpk")
}

/// `tz`'s command that installs the package at `tpk` on the device
/// `serial`. A TV, and the TV emulator, has no `pkgcmd`: `sdb install`
/// hands a `.tpk` to the web-app installer, which rejects it while `sdb`
/// exits 0. `tz install` uses the TVs' installer (`0 vd_appinstall`) and
/// fails when it does.
pub(crate) fn install_command(tz: &Path, tpk: &Path, serial: &str) -> Command {
    let mut command = Command::new(tz);
    command
        .args(["install", "-p"])
        .arg(tpk)
        .args(["-e", serial]);
    command
}

/// Signs the staged package in `staging` with `profile` into `to`.
fn sign(profile: &str, staging: &Path, to: &Path) -> anyhow::Result<()> {
    let step = ui::Step::start(format!("signing {} with `{profile}`", ui::path(staging)));
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let unsigned = unsigned_path(to);
    tizen_web::zip_dir(staging, &unsigned)?;
    let _ = fs::remove_file(to);
    let signed = tizen_web::run_tz(
        sign_command(&tizen_web::tz()?, profile, &unsigned, to),
        "sign the package",
    );
    let _ = fs::remove_file(&unsigned);
    signed?;
    if !to.is_file() {
        bail!("tz pack wrote no {}", to.display());
    }
    step.done(format!("packaged {}", ui::path(to)));
    Ok(())
}

/// Installs the package at `tpk` on `device` ([`install_command`]).
fn install_package(device: &Sdb, tpk: &Path) -> anyhow::Result<()> {
    tizen_web::run_tz(
        install_command(&tizen_web::tz()?, tpk, &device.serial),
        "install the package",
    )
}

/// The app's files in a package: its bundle, capabilities and assets, and
/// `app.json` (name, id, appearance, window).
fn app_files(config: &AppConfig, name: &str, id: &str, built: &Built) -> AppFiles {
    let mut info = serde_json::json!({ "id": id, "name": name });
    if let Some(appearance) = &config.appearance {
        info["appearance"] = appearance.as_str().into();
    }
    AppFiles {
        bundle: (!built.rust_root).then(|| built.bytecode.clone()),
        capabilities: built.capabilities_json(),
        app_json: crate::package::app_json(info, &[], config.window.as_ref()),
        assets: built
            .assets
            .iter()
            .map(|a| (a.path.clone(), a.source.clone()))
            .collect(),
        icon: config.icon(PackageTarget::Tizen).map(Path::to_path_buf),
    }
}

/// `nsh package --target tizen`: a TPK per architecture of `archs` from the
/// release runtime, signed with the certificate profile (`--sign`, else
/// `nsh.json` `tizen.signing`); of several, the TVs' also go into the
/// multi-architecture `.zip`. With `--run`, the package of the TV's architecture is
/// installed and launched there.
pub(crate) fn package(
    options: &PackageOptions<'_>,
    config: &AppConfig,
    name: &str,
    id: &str,
    built: &Built,
) -> anyhow::Result<()> {
    let manifest = Manifest::of(config, name, id)?;
    let Some(profile) = options.sign.or(config.tizen.signing.as_deref()) else {
        bail!("{NO_PROFILE}");
    };
    let device = if options.run {
        Some(Sdb::connect(None)?)
    } else {
        None
    };
    let mut archs = package_archs(options.tizen_archs);
    if let Some(device) = &device
        && options.tizen_archs.is_empty()
    {
        archs = vec![device.arch];
    }
    let runtime = crate::runtime::tizen(false, &archs)?;
    let app = app_files(config, name, id, built);
    let multi = archs.len() > 1;
    let mut packages = Vec::new();
    for &arch in &archs {
        let manifest = Manifest {
            arch: multi.then_some(arch),
            ..manifest.clone()
        };
        let staging = options
            .out
            .join("tizen")
            .join(format!("{name}-{}", arch.name()));
        write_staged(&staging, &stage(&runtime, arch, &app, &manifest))?;
        let tpk = options.out.join(tpk_name(&manifest, name));
        sign(profile, &staging, &tpk)?;
        packages.push((arch, tpk));
    }
    if multi {
        let zip = options.out.join(zip_name(&manifest));
        let upload = store_packages(&packages);
        write_zip(&zip, &upload)?;
        ui::done(format!(
            "wrote the multi-architecture upload {}",
            ui::path(&zip)
        ));
        if upload.len() < packages.len() {
            ui::note("the emulator's package (x86_64) is not part of the upload");
        }
    }
    if let Some(device) = device {
        let tpk = packages
            .iter()
            .find(|(arch, _)| *arch == device.arch)
            .map(|(_, tpk)| tpk)
            .with_context(|| {
                format!(
                    "{} runs {}, which --arch leaves out",
                    device.serial,
                    device.arch.name()
                )
            })?;
        let step = ui::Step::start(format!("installing {} on {}", manifest.app, device.serial));
        install_package(&device, tpk)?;
        device.run(&device.launch(&manifest.app))?;
        step.done(format!("launched {} on {}", manifest.app, device.serial));
    }
    Ok(())
}

/// The packages of `packages` the multi-architecture upload takes: the
/// TVs' ([`Arch::in_store`]); the emulator's stays a package of its own.
pub(crate) fn store_packages(packages: &[(Arch, PathBuf)]) -> Vec<(Arch, PathBuf)> {
    packages
        .iter()
        .filter(|(arch, _)| arch.in_store())
        .cloned()
        .collect()
}

/// The multi-architecture upload: each package at the top level, stored.
fn write_zip(zip: &Path, packages: &[(Arch, PathBuf)]) -> anyhow::Result<()> {
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};
    let mut writer =
        ZipWriter::new(fs::File::create(zip).with_context(|| format!("create {}", zip.display()))?);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (_, tpk) in packages {
        let name = tpk
            .file_name()
            .context("a package without a file name")?
            .to_string_lossy();
        writer.start_file(name, options)?;
        writer.write_all(&fs::read(tpk)?)?;
    }
    writer.finish()?;
    Ok(())
}

/// The Tizen SDK's `sdb`: on `PATH`, else in the SDK (`TIZEN_SDK`, else
/// `~/tizen-studio`) under `tools/`.
fn sdb() -> anyhow::Result<PathBuf> {
    let file = format!("sdb{}", std::env::consts::EXE_SUFFIX);
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file());
    let sdk = std::env::var_os("TIZEN_SDK")
        .filter(|sdk| !sdk.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join("tizen-studio"))
        });
    on_path
        .or(sdk.map(|sdk| sdk.join("tools").join(&file)).filter(|p| p.is_file()))
        .context("no `sdb` (the Tizen SDK's device bridge) on PATH or in the Tizen SDK's tools/ (TIZEN_SDK): install the Tizen SDK and connect the TV with `sdb connect <its address>`")
}

/// The serial of the device to use from `sdb devices`' output: `wanted`,
/// else the only one connected.
pub(crate) fn pick_device(devices: &str, wanted: Option<&str>) -> anyhow::Result<String> {
    let connected: Vec<&str> = devices
        .lines()
        .filter(|line| !line.starts_with("List of devices") && !line.trim().is_empty())
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            (fields.next() == Some("device")).then_some(serial)
        })
        .collect();
    match (wanted, connected.as_slice()) {
        (Some(wanted), _) if connected.contains(&wanted) => Ok(wanted.to_owned()),
        (Some(wanted), _) => bail!(
            "no device {wanted} in `sdb devices` (connected: {})",
            if connected.is_empty() {
                "none".to_owned()
            } else {
                connected.join(", ")
            }
        ),
        (None, [one]) => Ok((*one).to_owned()),
        (None, []) => bail!(
            "no TV or emulator in `sdb devices`: turn on the TV's Developer mode with this machine's address, \
             then `sdb connect <the TV's address>`"
        ),
        (None, many) => bail!(
            "several devices in `sdb devices` ({}): pick one with --device",
            many.join(", ")
        ),
    }
}

/// What `sdb capability` says of a device: its architecture, and whether
/// it speaks Samsung's secure protocol (a TV: no shell, apps started with
/// `0 execute`; the emulator has a shell and `app_launcher`).
pub(crate) fn capabilities(text: &str) -> anyhow::Result<(Arch, bool)> {
    let value = |key: &str| {
        text.lines()
            .filter_map(|line| line.trim().split_once(':'))
            .find(|(k, _)| k.trim() == key)
            .map(|(_, v)| v.trim().to_owned())
    };
    let cpu = value("cpu_arch").context("`sdb capability` names no cpu_arch")?;
    let arch = Arch::of_device(&cpu).with_context(|| {
        format!("the device runs {cpu}, which NSH's Tizen runtime is not built for")
    })?;
    let secure = value("secure_protocol").as_deref() == Some("enabled");
    Ok((arch, secure))
}

/// A device `sdb` reaches: the commands for it are built by its methods
/// (each `sdb`'s arguments), run by [`Sdb::run`].
#[derive(Debug, Clone)]
pub(crate) struct Sdb {
    pub(crate) program: PathBuf,
    pub(crate) serial: String,
    pub(crate) arch: Arch,
    /// A TV (Samsung's secure protocol): no shell.
    pub(crate) secure: bool,
}

impl Sdb {
    /// The device `wanted` (its serial), else the only one connected.
    pub(crate) fn connect(wanted: Option<&str>) -> anyhow::Result<Sdb> {
        let program = sdb()?;
        let devices = output(Command::new(&program).arg("devices"), "list the devices")?;
        let serial = pick_device(&devices, wanted)?;
        let text = output(
            Command::new(&program).args(["-s", &serial, "capability"]),
            "ask the device what it is",
        )?;
        let (arch, secure) = capabilities(&text)?;
        Ok(Sdb {
            program,
            serial,
            arch,
            secure,
        })
    }

    fn args<const N: usize>(&self, args: [&str; N]) -> Vec<String> {
        let mut all = vec!["-s".to_owned(), self.serial.clone()];
        all.extend(args.iter().map(|a| (*a).to_owned()));
        all
    }

    /// Starts application `app`.
    pub(crate) fn launch(&self, app: &str) -> Vec<String> {
        if self.secure {
            self.args(["shell", "0", "execute", app])
        } else {
            self.args(["shell", "app_launcher", "-s", app])
        }
    }

    /// Stops application `app`.
    pub(crate) fn stop(&self, app: &str) -> Vec<String> {
        if self.secure {
            self.args(["shell", "0", "kill", app])
        } else {
            self.args(["shell", "app_launcher", "-k", app])
        }
    }

    /// Maps the TV's port `device` to this machine's `local` (the device
    /// connects to its own 127.0.0.1).
    pub(crate) fn reverse(&self, device: u16, local: u16) -> Vec<String> {
        self.args(["reverse", &format!("tcp:{device}"), &format!("tcp:{local}")])
    }

    /// Removes [`Sdb::reverse`]'s mapping of `device`.
    pub(crate) fn remove_reverse(&self, device: u16) -> Vec<String> {
        self.args(["reverse", "--remove", &format!("tcp:{device}")])
    }

    /// Maps this machine's port `local` to the TV's `device`.
    pub(crate) fn forward(&self, local: u16, device: u16) -> Vec<String> {
        self.args(["forward", &format!("tcp:{local}"), &format!("tcp:{device}")])
    }

    /// Removes [`Sdb::forward`]'s mapping of `local`.
    pub(crate) fn remove_forward(&self, local: u16) -> Vec<String> {
        self.args(["forward", "--remove", &format!("tcp:{local}")])
    }

    /// Runs `sdb` with `args`; its output if it succeeds.
    pub(crate) fn run(&self, args: &[String]) -> anyhow::Result<String> {
        output(Command::new(&self.program).args(args), &args[2..].join(" "))
    }
}

/// Runs `command` (an `sdb` one), its output if it succeeds; `sdb` says
/// some failures with status 0, as `error: ...` lines.
fn output(command: &mut Command, doing: &str) -> anyhow::Result<String> {
    let out = command
        .output()
        .with_context(|| format!("run sdb to {doing}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let failed = text
        .lines()
        .any(|line| line.trim_start().starts_with("error:"));
    if !out.status.success() || failed {
        return Err(ui::failure(format!("sdb could not {doing}"), text));
    }
    Ok(text)
}

/// One `sdb` command of [`open_sequence`], with what its failure means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// Stops what an earlier session left running; fails when nothing
    /// runs, which is fine.
    Stop(Vec<String>),
    /// Maps the TV's dev port to the dev server; refused, the library is
    /// reached through `sdb forward` and the relay instead.
    Reverse(Vec<String>),
    /// Launches the development runtime; its failure ends the run.
    Launch(Vec<String>),
}

/// How `nsh run --target tizen` and each `open` of `nsh test --target
/// tizen` start the development runtime once it is installed, in order:
/// what an earlier session left running stopped, the dev port mapped to
/// the dev server's `port`, the runtime launched ([`open`] runs them).
pub(crate) fn open_sequence(sdb: &Sdb, port: u16) -> Vec<Step> {
    let app = Manifest::dev().app;
    vec![
        Step::Stop(sdb.stop(&app)),
        Step::Reverse(sdb.reverse(DEV_PORT, port)),
        Step::Launch(sdb.launch(&app)),
    ]
}

/// What [`open`] runs its steps with: `sdb`, and for a refused reverse
/// mapping a free port of this machine and the relay from it.
pub(crate) trait Bridge {
    /// Runs `sdb` with `args`; its output if it succeeds.
    fn run(&mut self, args: &[String]) -> anyhow::Result<String>;
    /// A free port of this machine for `sdb forward`.
    fn free_port(&mut self) -> anyhow::Result<u16>;
    /// Relays the connections made to `local` to the dev server until
    /// what it returns is dropped.
    fn relay(&mut self, local: u16) -> Relay;
}

/// The device itself: `sdb`'s commands run, connections relayed to the dev
/// server at `server`.
pub(crate) struct Live<'a> {
    pub(crate) sdb: &'a Sdb,
    pub(crate) server: u16,
}

impl Bridge for Live<'_> {
    fn run(&mut self, args: &[String]) -> anyhow::Result<String> {
        self.sdb.run(args)
    }

    fn free_port(&mut self) -> anyhow::Result<u16> {
        free_port()
    }

    fn relay(&mut self, local: u16) -> Relay {
        relay(local, self.server)
    }
}

/// What [`open`] mapped, which [`close_sequence`] removes.
#[derive(Debug, Default)]
pub(crate) struct Opened {
    /// The TV's dev port, mapped by `sdb reverse`.
    pub(crate) reverse: bool,
    /// This machine's port mapped by `sdb forward` to the TV's listening
    /// library, and its relay.
    pub(crate) forward: Option<(u16, Relay)>,
}

/// Runs [`open_sequence`] for the dev server at `port` through `bridge`,
/// each step with its failure policy, recording in `opened` what it mapped
/// (also when a later step fails, for [`close_sequence`]).
pub(crate) fn open(
    sdb: &Sdb,
    port: u16,
    bridge: &mut impl Bridge,
    opened: &mut Opened,
) -> anyhow::Result<()> {
    for step in open_sequence(sdb, port) {
        match step {
            // An earlier session's library would connect first.
            Step::Stop(stop) => {
                let _ = bridge.run(&stop);
            }
            Step::Reverse(reverse) => match bridge.run(&reverse) {
                Ok(_) => opened.reverse = true,
                Err(error) => {
                    // A TV without `sdb reverse`: the library listens once
                    // the reverse port keeps refusing, and this machine
                    // connects to it through `sdb forward`, relaying to the
                    // dev server.
                    ui::warn(format!(
                        "sdb reverse failed ({error:#}): relaying through sdb forward instead (the app's assets need sdb reverse)"
                    ));
                    let local = bridge.free_port()?;
                    bridge.run(&sdb.forward(local, LISTEN_PORT))?;
                    opened.forward = Some((local, bridge.relay(local)));
                }
            },
            Step::Launch(launch) => {
                bridge.run(&launch)?;
            }
        }
    }
    Ok(())
}

/// The `sdb` commands that end a session: the runtime stopped, what
/// [`open`] mapped removed.
pub(crate) fn close_sequence(sdb: &Sdb, opened: &Opened) -> Vec<Vec<String>> {
    let mut sequence = vec![sdb.stop(&Manifest::dev().app)];
    if opened.reverse {
        sequence.push(sdb.remove_reverse(DEV_PORT));
    }
    if let Some((local, _)) = &opened.forward {
        sequence.push(sdb.remove_forward(*local));
    }
    sequence
}

/// What identifies a build of the development TPK: the runtime's files'
/// sizes and times (`runtime`, the `dev` variant) for `arch`.
fn runtime_version(runtime: &Path, arch: Arch) -> String {
    [
        runtime.join(HOST),
        runtime.join(arch.name()).join("libnsh.so"),
    ]
    .iter()
    .filter_map(|file| {
        let meta = fs::metadata(file).ok()?;
        let modified = meta
            .modified()
            .ok()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Some(format!("{}:{}:{modified}", file.display(), meta.len()))
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Whether the device `serial` has the build `version` the `installed`
/// record says was installed there (lines of `<serial> <version digest>`).
pub(crate) fn installed(record: &str, serial: &str, version: &str) -> bool {
    let digest = digest(version);
    record
        .lines()
        .any(|line| line.split_once(' ') == Some((serial, digest.as_str())))
}

/// [`installed`]'s record with `serial` now having `version`.
pub(crate) fn record_install(record: &str, serial: &str, version: &str) -> String {
    let mut lines: Vec<String> = record
        .lines()
        .filter(|line| line.split_once(' ').is_none_or(|(s, _)| s != serial))
        .map(str::to_owned)
        .collect();
    lines.push(format!("{serial} {}", digest(version)));
    lines.join("\n") + "\n"
}

fn digest(text: &str) -> String {
    use sha2::{Digest as _, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))[..16].to_owned()
}

/// Builds (when the runtime changed) and signs the development TPK for the
/// device's architecture into `work` (`.nsh/tizen`), returning it and
/// whether the device needs it installed.
fn dev_package(sdb: &Sdb, work: &Path, profile: &str) -> anyhow::Result<(PathBuf, String, bool)> {
    let runtime = crate::runtime::tizen(true, &[sdb.arch])?;
    let version = runtime_version(&runtime, sdb.arch);
    let tpk = work.join(format!("NshDev-{}.tpk", sdb.arch.name()));
    let stamp = work.join(format!("dev-{}.stamp", sdb.arch.name()));
    let built =
        tpk.exists() && fs::read_to_string(&stamp).ok().as_deref() == Some(version.as_str());
    if !built {
        let manifest = Manifest::dev();
        let staging = work.join(format!("dev-{}", sdb.arch.name()));
        let app = AppFiles {
            app_json: r#"{"name":"NSH Dev"}"#.to_owned(),
            ..AppFiles::default()
        };
        write_staged(&staging, &stage(&runtime, sdb.arch, &app, &manifest))?;
        sign(profile, &staging, &tpk)?;
        fs::write(&stamp, &version)?;
    }
    let record = fs::read_to_string(work.join("installed")).unwrap_or_default();
    let install = !installed(&record, &sdb.serial, &version);
    Ok((tpk, version, install))
}

/// The device (`device`: its serial; by default the only one) with the
/// development TPK installed: built into `work` and signed with the
/// certificate profile `signing` (`nsh.json` `tizen.signing`) when the
/// runtime changed, and installed when the device does not have this build.
pub(crate) fn install_dev(
    work: &Path,
    signing: Option<&str>,
    device: Option<&str>,
) -> anyhow::Result<Sdb> {
    let Some(profile) = signing else {
        bail!("{NO_PROFILE} (nsh.json tizen.signing, for the development package)");
    };
    fs::create_dir_all(work)?;
    let sdb = Sdb::connect(device)?;
    let (tpk, version, install) = dev_package(&sdb, work, profile)?;
    if install {
        let step = ui::Step::start(format!(
            "installing the development package on {}",
            sdb.serial
        ));
        install_package(&sdb, &tpk)?;
        let record = fs::read_to_string(work.join("installed")).unwrap_or_default();
        fs::write(
            work.join("installed"),
            record_install(&record, &sdb.serial, &version),
        )?;
        step.done(format!(
            "installed the development package on {}",
            sdb.serial
        ));
    }
    Ok(sdb)
}

/// `nsh run --target tizen`'s device side, once the dev server listens on
/// `port`: the development TPK installed on the device (`device`: its
/// serial; by default the only one) when it does not have this build,
/// then [`open`]'s sequence. Returns what undoes it.
pub(crate) fn run_dev(entry: &Path, device: Option<&str>, port: u16) -> anyhow::Result<DevRun> {
    let entry = dunce::canonicalize(entry)?;
    let work = entry
        .parent()
        .context("entry has no directory")?
        .join(".nsh/tizen");
    let config = crate::package::read_config(&entry)?;
    let sdb = install_dev(&work, config.tizen.signing.as_deref(), device)?;
    let mut run = DevRun {
        sdb: sdb.clone(),
        opened: Opened::default(),
    };
    let step = ui::Step::start("launching the development runtime");
    open(
        &sdb,
        port,
        &mut Live {
            sdb: &sdb,
            server: port,
        },
        &mut run.opened,
    )?;
    step.done(format!(
        "launched the development runtime on {}",
        sdb.serial
    ));
    Ok(run)
}

/// What [`run_dev`] set up on the device, undone when dropped: the app
/// stopped, the port mappings removed ([`close_sequence`]).
pub(crate) struct DevRun {
    sdb: Sdb,
    opened: Opened,
}

impl Drop for DevRun {
    fn drop(&mut self) {
        for command in close_sequence(&self.sdb, &self.opened) {
            let _ = self.sdb.run(&command);
        }
    }
}

/// What a cross build of an app's Tizen library needs of this machine
/// (dXn.GpuiTizen's `cargo xtask cross` finds the same): the `tizen`
/// component's target specs and the nightly they build with, clang, the
/// Tizen 8.0 rootstrap (the sysroot) and the Tizen SDK's GCC 9.2 toolchain
/// (the start files and `libgcc_s` the rootstrap leaves out, and GNU ld).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cross {
    /// `runtime/tizen/targets`.
    pub(crate) targets: PathBuf,
    /// The nightly (`targets/toolchain`).
    pub(crate) toolchain: String,
    /// Its `lib/rustlib/<host>/bin` (`llvm-ar`).
    pub(crate) rustlib_bin: PathBuf,
    /// `TIZEN_CLANG`, else `clang`.
    pub(crate) clang: String,
    pub(crate) rootstrap: PathBuf,
    pub(crate) gcc: PathBuf,
}

impl Arch {
    /// The triple clang compiles C code for (the rootstrap's GCC's).
    fn clang_triple(self) -> &'static str {
        match self {
            Arch::Armv7l => "armv7l-tizen-linux-gnueabi",
            Arch::Aarch64 => "aarch64-tizen-linux-gnu",
            Arch::X86_64 => "x86_64-tizen-linux-gnu",
        }
    }

    /// C flags of its ABI (32-bit ARM's soft-float calls with NEON).
    fn c_flags(self) -> &'static [&'static str] {
        match self {
            Arch::Armv7l => &["-march=armv7-a", "-mfloat-abi=softfp", "-mfpu=neon"],
            Arch::Aarch64 | Arch::X86_64 => &[],
        }
    }

    /// `TIZEN_ROOTSTRAP_<ARCH>`, `TIZEN_GCC_TOOLCHAIN_<ARCH>`'s suffix.
    fn env_name(self) -> &'static str {
        match self {
            Arch::Armv7l => "ARMV7",
            Arch::Aarch64 => "AARCH64",
            Arch::X86_64 => "X86_64",
        }
    }

    /// The rootstrap in the Tizen SDK (Tizen 8.0's common profile).
    fn sdk_rootstrap(self) -> &'static str {
        match self {
            Arch::Armv7l => "platforms/tizen-8.0/tizen/rootstraps/tizen-8.0-device.core",
            Arch::Aarch64 => "platforms/tizen-8.0/tizen/rootstraps/tizen-8.0-device64.core",
            Arch::X86_64 => "platforms/tizen-8.0/tizen/rootstraps/tizen-8.0-emulator64.core",
        }
    }

    /// The GCC toolchain in the Tizen SDK.
    fn sdk_gcc(self) -> &'static str {
        match self {
            Arch::Armv7l => "tools/arm-linux-gnueabi-gcc-9.2",
            Arch::Aarch64 => "tools/aarch64-linux-gnu-gcc-9.2",
            Arch::X86_64 => "tools/x86_64-linux-gnu-gcc-9.2",
        }
    }

    /// GCC's triple, which names its directories in the toolchain.
    fn gcc_triple(self) -> &'static str {
        match self {
            Arch::Armv7l => "arm-tizen-linux-gnueabi",
            Arch::Aarch64 => "aarch64-tizen-linux-gnu",
            Arch::X86_64 => "x86_64-tizen-linux-gnu",
        }
    }
}

/// The cross build of an app's Tizen library for `arch` (dXn.GpuiTizen's
/// `cross::plan`): Cargo's arguments before its subcommand (the nightly's
/// `build-std` for the custom target), the environment (the C compiler,
/// archiver and flags for the target, the linker and its arguments:
/// the rootstrap as sysroot, GNU ld and GCC's start files), and the target
/// spec.
pub(crate) fn cross_plan(
    arch: Arch,
    cross: &Cross,
) -> (Vec<String>, Vec<(String, String)>, PathBuf) {
    let spec = cross.targets.join(format!("{}.json", arch.spec()));
    let suffix = arch.spec().replace('-', "_");
    let triple = arch.clang_triple();
    let sysroot = format!("--sysroot={}", cross.rootstrap.display());
    let mut c_flags = vec![format!("--target={triple}"), sysroot.clone()];
    c_flags.extend(arch.c_flags().iter().map(|f| (*f).to_owned()));
    // zstd's weak tracing hooks would be imports outside Tizen's API.
    c_flags.push("-DZSTD_TRACE=0".to_owned());
    let gcc_lib = cross
        .gcc
        .join("lib/gcc")
        .join(arch.gcc_triple())
        .join("9.2.0");
    let runtime = cross.gcc.join(arch.gcc_triple()).join(match arch {
        Arch::Armv7l => "lib",
        Arch::Aarch64 | Arch::X86_64 => "lib64",
    });
    let mut link = vec![
        format!("--target={triple}"),
        sysroot,
        // GNU ld: the rootstraps' libraries list imports lld rejects.
        format!(
            "--ld-path={}",
            cross
                .gcc
                .join(arch.gcc_triple())
                .join("bin/ld.bfd")
                .display()
        ),
        format!("-B{}", gcc_lib.display()),
        format!("-L{}", gcc_lib.display()),
        format!("-L{}", runtime.display()),
    ];
    link.extend(arch.c_flags().iter().map(|f| (*f).to_owned()));
    let rustflags: Vec<String> = link.iter().map(|a| format!("-Clink-arg={a}")).collect();
    let env = vec![
        ("RUSTUP_TOOLCHAIN".to_owned(), cross.toolchain.clone()),
        (format!("CC_{suffix}"), cross.clang.clone()),
        (format!("CFLAGS_{suffix}"), c_flags.join(" ")),
        (
            format!("AR_{suffix}"),
            cross.rustlib_bin.join("llvm-ar").display().to_string(),
        ),
        (
            format!("BINDGEN_EXTRA_CLANG_ARGS_{suffix}"),
            c_flags.join(" "),
        ),
        (
            format!("CARGO_TARGET_{}_LINKER", suffix.to_uppercase()),
            cross.clang.clone(),
        ),
        ("CARGO_ENCODED_RUSTFLAGS".to_owned(), rustflags.join("\x1f")),
    ];
    let args = vec![
        "-Zbuild-std=std,panic_abort".to_owned(),
        "-Zjson-target-spec".to_owned(),
    ];
    (args, env, spec)
}

/// Where the Tizen SDK is: `TIZEN_SDK`, else `~/tizen-studio`.
fn sdk_root() -> Option<PathBuf> {
    std::env::var_os("TIZEN_SDK")
        .filter(|sdk| !sdk.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("tizen-studio")))
}

/// The Tizen SDK's package manager installing `package`, as dXn.GpuiTizen's
/// `docs/setup.md` gives it.
fn package_manager(package: &str) -> String {
    format!(
        "~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license {package}"
    )
}

/// What to do without the Tizen 8.0 rootstrap for `arch`.
pub(crate) fn rootstrap_hint(arch: Arch) -> String {
    format!(
        "building the app's crates for {} needs the Tizen 8.0 rootstrap: install it with the Tizen SDK's \
         package manager (`{}`) or set TIZEN_ROOTSTRAP_{}",
        arch.name(),
        package_manager("TIZEN-8.0-NativeAppDevelopment-CLI"),
        arch.env_name()
    )
}

/// What to do without the Tizen SDK's GCC 9.2 toolchain for `arch`.
pub(crate) fn gcc_hint(arch: Arch) -> String {
    format!(
        "building the app's crates for {} needs the Tizen SDK's GCC 9.2 toolchain: install it \
         (`{}`) or set TIZEN_GCC_TOOLCHAIN_{}",
        arch.name(),
        package_manager("NativeToolchain-Gcc-9.2"),
        arch.env_name()
    )
}

/// A Cargo command set up to cross-build for `arch` with the target specs
/// in `targets` ([`cross_plan`]), and the spec to build with
/// (`--target <spec>`), once this machine has what it needs; with the
/// nightly's `lib/rustlib/<host>/bin` ([`strip_command`]'s `llvm-strip`).
pub(crate) fn cross_command(
    arch: Arch,
    targets: &Path,
) -> anyhow::Result<(Command, PathBuf, PathBuf)> {
    let toolchain = fs::read_to_string(targets.join("toolchain"))
        .with_context(|| format!("read {}", targets.join("toolchain").display()))?
        .trim()
        .to_owned();
    let found = |env: String, in_sdk: &str| -> Option<PathBuf> {
        std::env::var_os(&env)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| sdk_root().map(|sdk| sdk.join(in_sdk)))
            .filter(|dir| dir.is_dir())
    };
    let rootstrap = found(
        format!("TIZEN_ROOTSTRAP_{}", arch.env_name()),
        arch.sdk_rootstrap(),
    )
    .with_context(|| rootstrap_hint(arch))?;
    let gcc = found(
        format!("TIZEN_GCC_TOOLCHAIN_{}", arch.env_name()),
        arch.sdk_gcc(),
    )
    .with_context(|| gcc_hint(arch))?;
    let libdir = Command::new("rustc")
        .env("RUSTUP_TOOLCHAIN", &toolchain)
        .args(["--print", "target-libdir"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .with_context(|| {
            format!(
                "the app's crates build for Tizen with {toolchain}: rustup toolchain install {toolchain} \
                 --component rust-src,llvm-tools"
            )
        })?;
    let rustlib_bin = PathBuf::from(String::from_utf8_lossy(&libdir.stdout).trim())
        .parent()
        .context("rustc's target libdir")?
        .join("bin");
    let cross = Cross {
        targets: targets.to_owned(),
        toolchain,
        rustlib_bin,
        clang: std::env::var("TIZEN_CLANG").unwrap_or_else(|_| "clang".to_owned()),
        rootstrap,
        gcc,
    };
    let (args, env, spec) = cross_plan(arch, &cross);
    // Rustup's `cargo`, which takes the nightly from `RUSTUP_TOOLCHAIN`.
    let mut command = Command::new("cargo");
    command.args(args).envs(env).env_remove("RUSTFLAGS");
    Ok((command, spec, cross.rustlib_bin))
}

/// The one symbol a TV library exports: the C# host's P/Invoke
/// ([`symbols::check_library`] checks it is the only one).
pub(crate) const ENTRY: &str = "nsh_tizen_main";

/// The symbol allowlist in the component's `targets/` (dXn.GpuiTizen's
/// `symbols.allow`, which `cargo xtask tizen` copies there).
pub(crate) const ALLOWLIST: &str = "symbols.allow";

/// The nightly's `llvm-strip` (in `rustlib_bin`) writing `library` to `to`
/// without what the loader does not need (`--strip-unneeded`: the export
/// stays), as `cargo xtask tizen` strips the runtime.
pub(crate) fn strip_command(rustlib_bin: &Path, library: &Path, to: &Path) -> Command {
    let mut command =
        Command::new(rustlib_bin.join(format!("llvm-strip{}", std::env::consts::EXE_SUFFIX)));
    command
        .arg("--strip-unneeded")
        .arg("-o")
        .arg(to)
        .arg(library);
    command
}

/// A free port on this machine.
fn free_port() -> anyhow::Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// A relay of [`relay`], which stops once this is dropped (after the
/// session it carries, if any).
#[derive(Debug, Default)]
pub(crate) struct Relay(Arc<AtomicBool>);

impl Drop for Relay {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Relays, until what it returns is dropped, each connection made to this
/// machine's `local` port (`sdb forward` to the TV's listening library) to
/// the dev server at `server`, one after another.
fn relay(local: u16, server: u16) -> Relay {
    let relay = Relay::default();
    let stopped = relay.0.clone();
    std::thread::spawn(move || {
        while !stopped.load(Ordering::Relaxed) {
            // `sdb forward` takes the connection whether or not the
            // library listens yet: a session is one that carries bytes.
            if let Ok(tv) = TcpStream::connect(("127.0.0.1", local))
                && let Ok(dev) = TcpStream::connect(("127.0.0.1", server))
            {
                pipe(tv, dev);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    relay
}

/// Copies bytes both ways between `a` and `b` until either closes.
fn pipe(a: TcpStream, b: TcpStream) {
    let (Ok(a2), Ok(b2)) = (a.try_clone(), b.try_clone()) else {
        return;
    };
    let copy = |mut from: TcpStream, mut to: TcpStream| {
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match from.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if to.write_all(&buffer[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = to.shutdown(std::net::Shutdown::Both);
        let _ = from.shutdown(std::net::Shutdown::Both);
    };
    let there = std::thread::spawn(move || copy(a, b2));
    copy(b, a2);
    let _ = there.join();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(tizen: serde_json::Value) -> AppConfig {
        serde_json::from_value(serde_json::json!({ "tizen": tizen, "version": "2.1" })).unwrap()
    }

    fn device(secure: bool) -> Sdb {
        Sdb {
            program: PathBuf::from("sdb"),
            serial: "emulator-26101".into(),
            arch: Arch::X86_64,
            secure,
        }
    }

    fn line(rest: &[&str]) -> Vec<String> {
        ["-s", "emulator-26101"]
            .iter()
            .chain(rest)
            .map(|s| (*s).to_owned())
            .collect()
    }

    #[test]
    fn the_manifest_takes_the_tizen_section_and_its_defaults() {
        let manifest = Manifest::of(
            &config(serde_json::json!({})),
            "Hello, TV!",
            "com.acme.hello",
        )
        .unwrap();
        assert_eq!(manifest.package, tizen_web::package_id("com.acme.hello"));
        assert_eq!(manifest.app, format!("{}.HelloTV", manifest.package));
        // A TPK's version has three numbers.
        assert_eq!(manifest.version, "2.1.0");
        assert_eq!(manifest.api_version, "8.0");
        assert_eq!(manifest.privileges, ["http://tizen.org/privilege/internet"]);
        let manifest = Manifest::of(
            &config(serde_json::json!({
                "package": "AbCdE12345",
                "appId": "AbCdE12345.Counter",
                "version": "3.4.5",
                "apiVersion": "9.0",
                "privileges": [
                    "http://tizen.org/privilege/internet",
                    "http://developer.samsung.com/privilege/productinfo"
                ],
                "signing": "tv"
            })),
            "Counter",
            "com.acme.counter",
        )
        .unwrap();
        assert_eq!(
            (
                manifest.package.as_str(),
                manifest.app.as_str(),
                manifest.version.as_str()
            ),
            ("AbCdE12345", "AbCdE12345.Counter", "3.4.5")
        );
        assert_eq!(manifest.api_version, "9.0");
        assert_eq!(
            manifest.privileges,
            [
                "http://tizen.org/privilege/internet",
                "http://developer.samsung.com/privilege/productinfo"
            ]
        );
    }

    #[test]
    fn values_tizen_refuses_are_refused() {
        for (tizen, message) in [
            (
                serde_json::json!({ "package": "short" }),
                "tizen.package `short`",
            ),
            (
                serde_json::json!({ "package": "AbCdE1234!" }),
                "tizen.package `AbCdE1234!`",
            ),
            (
                serde_json::json!({ "package": "AbCdE12345", "appId": "Other12345.App" }),
                "tizen.appId `Other12345.App`",
            ),
            (
                serde_json::json!({ "apiVersion": "8" }),
                "tizen.apiVersion `8`",
            ),
            (
                serde_json::json!({ "version": "1.0.0-beta" }),
                "version `1.0.0-beta`",
            ),
            (
                serde_json::json!({ "privileges": ["internet"] }),
                "tizen.privileges `internet`",
            ),
        ] {
            let error = Manifest::of(&config(tizen.clone()), "App", "com.acme.app").unwrap_err();
            assert!(error.to_string().contains(message), "{tizen}: {error}");
        }
    }

    #[test]
    fn the_manifest_is_a_dotnet_ui_application_on_the_tv_profile() {
        let manifest = Manifest {
            package: "AbCdE12345".into(),
            app: "AbCdE12345.Counter".into(),
            label: "Tom & Jerry".into(),
            version: "1.2.3".into(),
            api_version: "8.0".into(),
            privileges: vec!["http://tizen.org/privilege/internet".into()],
            icon: true,
            arch: None,
        };
        assert_eq!(
            manifest.xml(),
            r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns="http://tizen.org/ns/packages" api-version="8.0" package="AbCdE12345" version="1.2.3">
  <profile name="tv-samsung" />
  <ui-application appid="AbCdE12345.Counter" exec="Nsh.Tizen.Host.dll" type="dotnet" multiple="false" taskmanage="true" nodisplay="false" launch_mode="single">
    <label>Tom &amp; Jerry</label>
    <icon>icon.png</icon>
    <metadata key="http://tizen.org/metadata/prefer_dotnet_aot" value="true" />
  </ui-application>
  <privileges>
    <privilege>http://tizen.org/privilege/internet</privilege>
  </privileges>
</manifest>
"#
        );
        // One package of a multi-architecture upload: its fourth number and
        // CPU feature.
        let aarch64 = Manifest {
            arch: Some(Arch::Aarch64),
            icon: false,
            ..manifest.clone()
        };
        let xml = aarch64.xml();
        assert!(xml.contains(r#"version="1.2.3.2""#), "{xml}");
        assert!(xml.ends_with(
            "  <feature name=\"http://tizen.org/feature/platform.core.cpu.arch.armv8\">true</feature>\n</manifest>\n"
        ), "{xml}");
        assert!(!xml.contains("<icon>"), "{xml}");
        assert_eq!(
            tpk_name(&aarch64, "Counter"),
            "AbCdE12345-1.2.3.2-aarch64.tpk"
        );
        assert_eq!(tpk_name(&manifest, "Counter"), "Counter.tpk");
        assert_eq!(
            tpk_name(
                &Manifest {
                    arch: Some(Arch::Armv7l),
                    ..manifest.clone()
                },
                "Counter"
            ),
            "AbCdE12345-1.2.3.1-armv7.tpk"
        );
        assert_eq!(zip_name(&manifest), "AbCdE12345-1.2.3.zip");
        assert_eq!(Manifest::dev().app, "NshDevTizn.NshDev");
    }

    #[test]
    fn the_staged_package_has_the_host_the_library_and_the_app() {
        let runtime = std::env::temp_dir().join(format!("nsh-tizen-stage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&runtime);
        fs::create_dir_all(&runtime).unwrap();
        fs::write(runtime.join(HOST_DEPS), "{}").unwrap();
        let app = AppFiles {
            bundle: Some(b"nshc".to_vec()),
            capabilities: Some("{}".into()),
            app_json: r#"{"id":"com.acme.counter"}"#.into(),
            assets: vec![("img/logo.png".into(), PathBuf::from("/app/img/logo.png"))],
            icon: Some(PathBuf::from("/app/icon.png")),
        };
        let manifest = Manifest::dev();
        let entries = stage(&runtime, Arch::Armv7l, &app, &manifest);
        let paths: Vec<String> = entries
            .iter()
            // Where they go in the package, whatever this machine's separator.
            .map(|e| e.path().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(
            paths,
            [
                "bin/Nsh.Tizen.Host.dll",
                "bin/Nsh.Tizen.Host.deps.json",
                "lib/libnsh.so",
                "res/nsh/bundle.nshc",
                "res/nsh/capabilities.json",
                "res/nsh/app.json",
                "res/nsh/assets/img/logo.png",
                "shared/res/icon.png",
                "tizen-manifest.xml",
            ]
        );
        assert_eq!(
            entries[2],
            Entry::Copy {
                from: runtime.join("armv7l/libnsh.so"),
                to: PathBuf::from("lib/libnsh.so"),
            }
        );
        assert_eq!(
            entries.last(),
            Some(&Entry::Write {
                to: PathBuf::from("tizen-manifest.xml"),
                contents: manifest.xml().into_bytes(),
            })
        );
        // An app of Rust only has no bundle; without `.deps.json` the host
        // goes alone.
        fs::remove_file(runtime.join(HOST_DEPS)).unwrap();
        let rust = AppFiles {
            bundle: None,
            icon: None,
            ..app
        };
        let paths: Vec<PathBuf> = stage(&runtime, Arch::X86_64, &rust, &manifest)
            .iter()
            .map(|e| e.path().to_owned())
            .collect();
        assert!(!paths.contains(&PathBuf::from("res/nsh/bundle.nshc")));
        assert!(!paths.contains(&PathBuf::from("bin/Nsh.Tizen.Host.deps.json")));
        let _ = fs::remove_dir_all(&runtime);
    }

    /// A TV takes a package signed from a zip of the staged tree
    /// (`tz pack -b`), installed by `tz install`, never `sdb install`.
    mod tz_commands {
        use super::*;

        fn args(command: &Command) -> Vec<String> {
            command
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect()
        }

        #[test]
        fn tz_re_signs_the_zipped_staged_package_into_the_named_one() {
            let signed = Path::new("/out/AbCdE12345-1.0.0-armv7l.tpk");
            let unsigned = unsigned_path(signed);
            assert_eq!(
                unsigned,
                Path::new("/out/AbCdE12345-1.0.0-armv7l.unsigned.tpk")
            );
            assert_eq!(
                args(&sign_command(Path::new("tz"), "tv", &unsigned, signed)),
                [
                    "pack",
                    "-t",
                    "tpk",
                    "-s",
                    "tv",
                    "-b",
                    "/out/AbCdE12345-1.0.0-armv7l.unsigned.tpk",
                    "-o",
                    "/out/AbCdE12345-1.0.0-armv7l.tpk",
                ]
            );
        }

        #[test]
        fn tz_installs_on_the_device_by_its_serial() {
            assert_eq!(
                args(&install_command(
                    Path::new("tz"),
                    Path::new("/w/NshDev-armv7l.tpk"),
                    "192.168.1.20:26101"
                )),
                [
                    "install",
                    "-p",
                    "/w/NshDev-armv7l.tpk",
                    "-e",
                    "192.168.1.20:26101"
                ]
            );
        }
    }

    #[test]
    fn the_architectures_default_to_the_tvs() {
        assert_eq!(package_archs(&[]), [Arch::Armv7l]);
        assert_eq!(
            package_archs(&[Arch::X86_64, Arch::Armv7l, Arch::X86_64]),
            [Arch::Armv7l, Arch::X86_64]
        );
        assert_eq!(
            [Arch::Armv7l, Arch::Aarch64, Arch::X86_64].map(Arch::name),
            ["armv7l", "aarch64", "x86_64"]
        );
    }

    #[test]
    fn the_device_is_the_one_sdb_lists_or_the_one_named() {
        let devices = "List of devices attached \n192.168.1.20:26101\tdevice\tUE43TU7000\nemulator-26101\toffline\tT-8.0\n";
        assert_eq!(pick_device(devices, None).unwrap(), "192.168.1.20:26101");
        assert_eq!(
            pick_device(devices, Some("192.168.1.20:26101")).unwrap(),
            "192.168.1.20:26101"
        );
        assert!(pick_device(devices, Some("emulator-26101")).is_err());
        assert!(
            pick_device("List of devices attached\n", None)
                .unwrap_err()
                .to_string()
                .contains("sdb connect")
        );
        let two = "List of devices attached\na\tdevice\tx\nb\tdevice\ty\n";
        assert!(
            pick_device(two, None)
                .unwrap_err()
                .to_string()
                .contains("--device")
        );
        let tv = "secure_protocol:enabled\nintershell_support:disabled\ncpu_arch:armv7\nsdk_toolpath:/home/owner\n";
        assert_eq!(capabilities(tv).unwrap(), (Arch::Armv7l, true));
        let emulator = "secure_protocol:disabled\ncpu_arch:x86_64\n";
        assert_eq!(capabilities(emulator).unwrap(), (Arch::X86_64, false));
        assert!(capabilities("cpu_arch:riscv64\n").is_err());
        assert!(capabilities("secure_protocol:enabled\n").is_err());
    }

    /// A device whose commands are recorded, failing those that contain
    /// one of `fails`.
    struct Fake {
        fails: Vec<&'static str>,
        ran: Vec<Vec<String>>,
        relayed: Vec<u16>,
    }

    impl Fake {
        fn failing(fails: &[&'static str]) -> Fake {
            Fake {
                fails: fails.to_vec(),
                ran: Vec::new(),
                relayed: Vec::new(),
            }
        }
    }

    impl Bridge for Fake {
        fn run(&mut self, args: &[String]) -> anyhow::Result<String> {
            self.ran.push(args.to_vec());
            let line = args.join(" ");
            match self.fails.iter().find(|f| line.contains(*f)) {
                Some(fail) => bail!("sdb could not {fail}"),
                None => Ok(String::new()),
            }
        }

        fn free_port(&mut self) -> anyhow::Result<u16> {
            Ok(50000)
        }

        fn relay(&mut self, local: u16) -> Relay {
            self.relayed.push(local);
            Relay::default()
        }
    }

    #[test]
    fn the_runtime_opens_as_its_sequence_says_each_step_with_its_failure_policy() {
        let tv = device(true);
        assert_eq!(
            open_sequence(&tv, 8791),
            [
                Step::Stop(line(&["shell", "0", "kill", "NshDevTizn.NshDev"])),
                Step::Reverse(line(&["reverse", "tcp:8790", "tcp:8791"])),
                Step::Launch(line(&["shell", "0", "execute", "NshDevTizn.NshDev"])),
            ]
        );
        // Nothing ran: the stop fails, and the runtime still starts.
        let mut fake = Fake::failing(&["kill"]);
        let mut opened = Opened::default();
        open(&tv, 8791, &mut fake, &mut opened).unwrap();
        assert_eq!(
            fake.ran,
            [
                line(&["shell", "0", "kill", "NshDevTizn.NshDev"]),
                line(&["reverse", "tcp:8790", "tcp:8791"]),
                line(&["shell", "0", "execute", "NshDevTizn.NshDev"]),
            ]
        );
        assert!(opened.reverse && opened.forward.is_none());
        assert!(fake.relayed.is_empty());
        assert_eq!(
            close_sequence(&tv, &opened),
            [
                line(&["shell", "0", "kill", "NshDevTizn.NshDev"]),
                line(&["reverse", "--remove", "tcp:8790"]),
            ]
        );
        // No `sdb reverse` (on the emulator, a shell): `sdb forward` to the
        // library's listening port, relayed.
        let emulator = device(false);
        let mut fake = Fake::failing(&["reverse"]);
        let mut opened = Opened::default();
        open(&emulator, 9000, &mut fake, &mut opened).unwrap();
        assert_eq!(
            fake.ran,
            [
                line(&["shell", "app_launcher", "-k", "NshDevTizn.NshDev"]),
                line(&["reverse", "tcp:8790", "tcp:9000"]),
                line(&["forward", "tcp:50000", "tcp:8791"]),
                line(&["shell", "app_launcher", "-s", "NshDevTizn.NshDev"]),
            ]
        );
        assert!(!opened.reverse);
        assert_eq!(
            opened.forward.as_ref().map(|(local, _)| *local),
            Some(50000)
        );
        assert_eq!(fake.relayed, [50000]);
        assert_eq!(
            close_sequence(&emulator, &opened),
            [
                line(&["shell", "app_launcher", "-k", "NshDevTizn.NshDev"]),
                line(&["forward", "--remove", "tcp:50000"]),
            ]
        );
        // A failed launch fails the open; what it mapped is recorded, to be
        // removed.
        let mut fake = Fake::failing(&["execute"]);
        let mut opened = Opened::default();
        let error = open(&tv, 8791, &mut fake, &mut opened).unwrap_err();
        assert!(error.to_string().contains("execute"), "{error}");
        assert!(opened.reverse);
        // A refused forward too.
        let mut fake = Fake::failing(&["reverse", "forward"]);
        let mut opened = Opened::default();
        assert!(open(&tv, 8791, &mut fake, &mut opened).is_err());
        assert_eq!(fake.ran.len(), 3, "{:?}", fake.ran);
        assert!(opened.forward.is_none());
    }

    fn args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn an_apps_library_is_stripped_as_the_runtime_is() {
        let strip = strip_command(
            Path::new("/rustlib/bin"),
            Path::new("/t/libapp.so"),
            Path::new("/out/armv7l/libnsh.so"),
        );
        assert_eq!(
            strip.get_program(),
            Path::new("/rustlib/bin").join(format!("llvm-strip{}", std::env::consts::EXE_SUFFIX))
        );
        assert_eq!(
            args(&strip),
            [
                "--strip-unneeded",
                "-o",
                "/out/armv7l/libnsh.so",
                "/t/libapp.so"
            ]
        );
    }

    #[test]
    fn the_sdk_hints_give_the_package_managers_command() {
        let rootstrap = rootstrap_hint(Arch::Armv7l);
        assert_eq!(
            rootstrap,
            "building the app's crates for armv7l needs the Tizen 8.0 rootstrap: install it with the Tizen SDK's \
             package manager (`~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license \
             TIZEN-8.0-NativeAppDevelopment-CLI`) or set TIZEN_ROOTSTRAP_ARMV7"
        );
        let gcc = gcc_hint(Arch::Aarch64);
        assert_eq!(
            gcc,
            "building the app's crates for aarch64 needs the Tizen SDK's GCC 9.2 toolchain: install it \
             (`~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license \
             NativeToolchain-Gcc-9.2`) or set TIZEN_GCC_TOOLCHAIN_AARCH64"
        );
        for hint in [rootstrap, gcc] {
            assert!(!hint.contains("  "), "{hint}");
        }
    }

    #[test]
    fn the_upload_takes_the_tvs_packages_not_the_emulators() {
        assert_eq!(
            [Arch::Armv7l, Arch::Aarch64, Arch::X86_64].map(Arch::in_store),
            [true, true, false]
        );
        let packages = vec![
            (Arch::Armv7l, PathBuf::from("a-1.0.0.1-armv7.tpk")),
            (Arch::Aarch64, PathBuf::from("a-1.0.0.2-aarch64.tpk")),
            (Arch::X86_64, PathBuf::from("a-1.0.0.3-x86_64.tpk")),
        ];
        assert_eq!(store_packages(&packages), packages[..2]);
    }

    #[test]
    fn the_package_is_installed_once_per_device_and_build() {
        let record = record_install("", "tv-1", "build a");
        assert!(installed(&record, "tv-1", "build a"));
        assert!(!installed(&record, "tv-1", "build b"));
        assert!(!installed(&record, "tv-2", "build a"));
        let record = record_install(&record, "tv-2", "build a");
        let record = record_install(&record, "tv-1", "build b");
        assert!(installed(&record, "tv-1", "build b"));
        assert!(!installed(&record, "tv-1", "build a"));
        assert!(installed(&record, "tv-2", "build a"));
        assert_eq!(record.lines().count(), 2);
    }

    #[test]
    fn an_apps_crates_cross_build_with_the_components_specs() {
        let cross = Cross {
            targets: PathBuf::from("/nsh/runtime/tizen/targets"),
            toolchain: "nightly-2026-10-18".into(),
            rustlib_bin: PathBuf::from("/rustlib/bin"),
            clang: "clang".into(),
            rootstrap: PathBuf::from("/sdk/rootstrap"),
            gcc: PathBuf::from("/sdk/gcc"),
        };
        let (args, env, spec) = cross_plan(Arch::Armv7l, &cross);
        assert_eq!(args, ["-Zbuild-std=std,panic_abort", "-Zjson-target-spec"]);
        assert_eq!(
            spec,
            Path::new("/nsh/runtime/tizen/targets/armv7-tizen-linux-gnueabi.json")
        );
        let get = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        assert_eq!(get("RUSTUP_TOOLCHAIN"), Some("nightly-2026-10-18"));
        assert_eq!(get("CC_armv7_tizen_linux_gnueabi"), Some("clang"));
        assert_eq!(
            get("CFLAGS_armv7_tizen_linux_gnueabi"),
            Some(
                "--target=armv7l-tizen-linux-gnueabi --sysroot=/sdk/rootstrap -march=armv7-a -mfloat-abi=softfp -mfpu=neon -DZSTD_TRACE=0"
            )
        );
        assert_eq!(
            get("AR_armv7_tizen_linux_gnueabi").map(Path::new),
            Some(Path::new("/rustlib/bin/llvm-ar"))
        );
        assert_eq!(
            get("CARGO_TARGET_ARMV7_TIZEN_LINUX_GNUEABI_LINKER"),
            Some("clang")
        );
        // The SDK's paths as this machine joins them: `/` for the comparison.
        let rustflags = get("CARGO_ENCODED_RUSTFLAGS").unwrap().replace('\\', "/");
        let rustflags: Vec<&str> = rustflags.split('\x1f').collect();
        assert_eq!(
            rustflags,
            [
                "-Clink-arg=--target=armv7l-tizen-linux-gnueabi",
                "-Clink-arg=--sysroot=/sdk/rootstrap",
                "-Clink-arg=--ld-path=/sdk/gcc/arm-tizen-linux-gnueabi/bin/ld.bfd",
                "-Clink-arg=-B/sdk/gcc/lib/gcc/arm-tizen-linux-gnueabi/9.2.0",
                "-Clink-arg=-L/sdk/gcc/lib/gcc/arm-tizen-linux-gnueabi/9.2.0",
                "-Clink-arg=-L/sdk/gcc/arm-tizen-linux-gnueabi/lib",
                "-Clink-arg=-march=armv7-a",
                "-Clink-arg=-mfloat-abi=softfp",
                "-Clink-arg=-mfpu=neon",
            ]
        );
        let (_, env, _) = cross_plan(Arch::X86_64, &cross);
        let flags = env
            .iter()
            .find(|(k, _)| k == "CARGO_ENCODED_RUSTFLAGS")
            .unwrap()
            .1
            .replace('\\', "/");
        assert!(
            flags.contains("-L/sdk/gcc/x86_64-tizen-linux-gnu/lib64"),
            "{flags}"
        );
    }
}
