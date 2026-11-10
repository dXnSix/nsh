//! Samsung TVs through their browser engine: `nsh package --target
//! tizen-web` writes a Tizen web app (`.wgt`), the web's site
//! (`web::write_site`) with Tizen's manifest and a TV shim, signed with the
//! Tizen SDK's `tz`. The app runs as on the web (`platform.os` is `"web"`),
//! in the TV idiom (`platform.tv`).
//!
//! ```text
//! <out>/tizen-web/<name>/         the package's files: the web's site, files as they are (no .br / .gz), and
//!   config.xml                    Tizen's manifest ([`Widget`]: ids, version, privileges, landscape, icon)
//!   icon.png                      the app's icon, 512×423 (Samsung's size)
//!   nsh-tizen.<digest>.js         the TV shim (`tizen_web/shim.js`), run before `nsh.js`
//! <out>/tizen-web/<name>.wgt      the files zipped, unsigned
//! <out>/<name>.wgt                signed with a certificate profile (`--sign`, `nsh.json` `tizen.signing`)
//! ```
//!
//! The page runs the shim, then creates the player with `idiom: "tv"` and
//! no history entries (the remote's Back is the shim's). `nsh run --target
//! tizen-web` serves the same page from the dev server ([`dev_page`]) and
//! installs a development package whose page opens it ([`run_dev`]); `nsh
//! test --target tizen-web` opens it in headless Chromium. `--probe`
//! writes a package that tries how the page loads its files on a TV
//! ([`probe`]). Only signing, installing and launching run the SDK
//! (`tz`); everything else is written here.

use std::{
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, bail};
use sha2::{Digest as _, Sha256};

use crate::{
    build::Built,
    package::{AppConfig, PackageOptions},
    ui, web,
};

/// The TV shim: a classic script the page runs before `nsh.js`.
const SHIM: &str = include_str!("tizen_web/shim.js");
/// What the shim is named on the page (with its digest in a package).
const SHIM_NAME: &str = "nsh-tizen.js";
/// The development package's page, which opens the dev server's.
const DEV_PAGE: &str = include_str!("tizen_web/dev.html");
/// The loader probe's page and files ([`probe`]).
const PROBE_PAGE: &str = include_str!("tizen_web/probe/index.html");
const PROBE_FILES: &[(&str, &str)] = &[
    ("probe/module.js", include_str!("tizen_web/probe/module.js")),
    (
        "probe/import-map.js",
        include_str!("tizen_web/probe/import-map.js"),
    ),
    ("probe/mapped.js", include_str!("tizen_web/probe/mapped.js")),
    ("probe/chunk.js", include_str!("tizen_web/probe/chunk.js")),
    (
        "probe/classic.js",
        include_str!("tizen_web/probe/classic.js"),
    ),
];
/// The probe's WebAssembly module: `answer()` returns 42.
const PROBE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // magic, version
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // types: () -> i32
    0x03, 0x02, 0x01, 0x00, // functions: one of type 0
    0x07, 0x0a, 0x01, 0x06, b'a', b'n', b's', b'w', b'e', b'r', 0x00, 0x00, // export "answer"
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b, // code: i32.const 42
];

/// `required_version` without `nsh.json` `tizen.apiVersion`: Tizen 8.0,
/// the TVs of 2024 on, whose engines run the web runtime as built.
const API_VERSION: &str = "8.0";
/// What every package asks for: the network, and the remote's keys
/// (`tizen.tvinputdevice`).
const PRIVILEGES: &[&str] = &[
    "http://tizen.org/privilege/internet",
    "http://tizen.org/privilege/tv.inputdevice",
];
/// The development package's id, one for every app (it opens whichever
/// app's dev server it was written for), as the other targets have one
/// development host.
const DEV_PACKAGE: &str = "NshDevHost";
/// The loader probe's package id.
const PROBE_PACKAGE: &str = "NshLdProbe";

/// A Tizen web app's manifest, `config.xml` ([`Widget::config_xml`]).
#[derive(Debug)]
struct Widget {
    /// The widget's IRI ([`widget_iri`]).
    iri: String,
    /// The package id: 10 letters and digits.
    package: String,
    /// The application id, `<package>.<name>`.
    app: String,
    /// The name the TV shows.
    name: String,
    /// `x.y.z` of numbers.
    version: String,
    /// The lowest Tizen that installs it (`required_version`).
    required_version: String,
    /// Whether the package has `icon.png`.
    icon: bool,
    privileges: Vec<String>,
    /// The origins the page may reach (`<access>`; `*`: any, the app's
    /// `capabilities` and the page's CSP limiting its requests).
    access: Vec<String>,
    /// The hosts the page may navigate to (`<tizen:allow-navigation>`):
    /// the development package's dev server.
    navigation: Vec<String>,
}

impl Widget {
    /// The app's manifest, from `nsh.json` (`tizen`, `version`, `icon`),
    /// its display `name` and its `id`; checked as Tizen checks it.
    fn of(config: &AppConfig, name: &str, id: &str) -> anyhow::Result<Widget> {
        let tizen = &config.tizen;
        let package = match &tizen.package_id {
            Some(package) => check_package_id(package)?.to_owned(),
            None => package_id(id),
        };
        let app = match &tizen.app_id {
            Some(app) => check_app_id(app, &package)?.to_owned(),
            None => format!("{package}.{}", app_name(name)),
        };
        let version = tizen
            .version
            .as_deref()
            .or(config.version.as_deref())
            .unwrap_or("1.0.0");
        check_version(version)?;
        let required_version = tizen.api_version.as_deref().unwrap_or(API_VERSION);
        check_api_version(required_version)?;
        let mut privileges: Vec<String> = PRIVILEGES.iter().map(|p| (*p).to_owned()).collect();
        for privilege in &tizen.privileges {
            if !privileges.contains(privilege) {
                privileges.push(privilege.clone());
            }
        }
        Ok(Widget {
            iri: widget_iri(id),
            package,
            app,
            name: name.to_owned(),
            version: version.to_owned(),
            required_version: required_version.to_owned(),
            icon: config.icon.is_some(),
            privileges,
            access: vec!["*".to_owned()],
            navigation: Vec::new(),
        })
    }

    /// `config.xml`.
    fn config_xml(&self) -> String {
        let mut xml = String::new();
        let _ = writeln!(xml, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        let _ = writeln!(
            xml,
            r#"<widget xmlns="http://www.w3.org/ns/widgets" xmlns:tizen="http://tizen.org/ns/widgets" id="{}" version="{}" viewmodes="maximized">"#,
            escape(&self.iri),
            escape(&self.version)
        );
        let _ = writeln!(
            xml,
            r#"  <tizen:application id="{}" package="{}" required_version="{}"/>"#,
            escape(&self.app),
            escape(&self.package),
            escape(&self.required_version)
        );
        let _ = writeln!(xml, r#"  <content src="index.html"/>"#);
        let _ = writeln!(
            xml,
            r#"  <feature name="http://tizen.org/feature/screen.size.all"/>"#
        );
        if self.icon {
            let _ = writeln!(xml, r#"  <icon src="icon.png"/>"#);
        }
        let _ = writeln!(xml, "  <name>{}</name>", escape(&self.name));
        let _ = writeln!(xml, r#"  <tizen:profile name="tv-samsung"/>"#);
        for privilege in &self.privileges {
            let _ = writeln!(xml, r#"  <tizen:privilege name="{}"/>"#, escape(privilege));
        }
        for origin in &self.access {
            let _ = writeln!(
                xml,
                r#"  <access origin="{}" subdomains="{}"/>"#,
                escape(origin),
                origin == "*"
            );
        }
        for host in &self.navigation {
            let _ = writeln!(
                xml,
                "  <tizen:allow-navigation>{}</tizen:allow-navigation>",
                escape(host)
            );
        }
        let _ = writeln!(
            xml,
            r#"  <tizen:setting screen-orientation="landscape" context-menu="disable" background-support="disable" encryption="disable" install-location="auto" hwkey-event="enable"/>"#
        );
        xml.push_str("</widget>\n");
        xml
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The package id made from the app's id: 10 letters and digits, the first
/// a letter, the same for the same id.
fn package_id(id: &str) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    const DIGITS: &[u8] = b"0123456789";
    let hash = Sha256::digest(id.as_bytes());
    hash.iter()
        .take(10)
        .enumerate()
        .map(|(i, byte)| {
            let alphabet = if i == 0 {
                LETTERS.to_vec()
            } else {
                [LETTERS, DIGITS].concat()
            };
            alphabet[*byte as usize % alphabet.len()] as char
        })
        .collect()
}

fn check_package_id(package: &str) -> anyhow::Result<&str> {
    if package.len() != 10 || !package.bytes().all(|b| b.is_ascii_alphanumeric()) {
        bail!("nsh.json tizen.packageId `{package}`: Tizen takes 10 letters and digits");
    }
    Ok(package)
}

fn check_app_id<'a>(app: &'a str, package: &str) -> anyhow::Result<&'a str> {
    let name = app.strip_prefix(package).and_then(|r| r.strip_prefix('.'));
    match name {
        Some(name)
            if !name.is_empty()
                && name.len() <= 52
                && name.bytes().all(|b| b.is_ascii_alphanumeric()) =>
        {
            Ok(app)
        }
        _ => bail!(
            "nsh.json tizen.appId `{app}`: Tizen takes `{package}.<name>`, the name 1 to 52 letters and digits"
        ),
    }
}

/// The application id's name for the app's display name: its letters and
/// digits (`App` if it has none), at most 52.
fn app_name(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(52)
        .collect();
    if name.is_empty() {
        "App".to_owned()
    } else {
        name
    }
}

/// Tizen's versions: `x.y.z` (or `x.y`), x and y at most 255, z at most
/// 65535.
fn check_version(version: &str) -> anyhow::Result<()> {
    let parts: Vec<&str> = version.split('.').collect();
    let limits = [255u32, 255, 65535];
    let ok = (2..=3).contains(&parts.len())
        && parts.iter().zip(limits).all(|(part, limit)| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && part.parse::<u32>().is_ok_and(|n| n <= limit)
        });
    if !ok {
        bail!(
            "version `{version}`: a Tizen package's is `x.y.z` of numbers (x and y up to 255, z up to 65535); set nsh.json tizen.version"
        );
    }
    Ok(())
}

fn check_api_version(version: &str) -> anyhow::Result<()> {
    let parts: Vec<&str> = version.split('.').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        bail!("nsh.json tizen.apiVersion `{version}`: a Tizen version, as `8.0`");
    }
    Ok(())
}

/// The widget's IRI for app id `id`: `com.example.tv.counter` is
/// `http://example.com/tv/counter`, an id of fewer parts `http://<id>`.
fn widget_iri(id: &str) -> String {
    let parts: Vec<&str> = id.split('.').collect();
    match parts.as_slice() {
        [top, domain, rest @ ..] if !rest.is_empty() => {
            format!("http://{domain}.{top}/{}", rest.join("/"))
        }
        _ => format!("http://{id}"),
    }
}

/// `nsh package --target tizen-web`: the site, the manifest and the icon
/// into `<out>/tizen-web/<name>/`, zipped, signed when there is a
/// certificate profile; with `--run`, installed and launched on the TV.
pub(crate) fn package(
    options: &PackageOptions<'_>,
    config: &AppConfig,
    name: &str,
    id: &str,
    built: &Built,
) -> anyhow::Result<()> {
    let widget = Widget::of(config, name, id)?;
    let root = options.out.join("tizen-web");
    let dir = root.join(name);
    web::write_site(
        options,
        config,
        name,
        id,
        built,
        &dir,
        &web::Variant {
            scripts: &[(SHIM_NAME, SHIM)],
            idiom: Some("tv"),
            history: Some(false),
            installed: true,
        },
    )?;
    if let Some(icon) = &config.icon {
        write_icon(icon, &dir.join("icon.png"))?;
    }
    fs::write(dir.join("config.xml"), widget.config_xml())?;
    let unsigned = root.join(format!("{name}.wgt"));
    zip_dir(&dir, &unsigned)?;
    let profile = options.sign.or(config.tizen.signing.as_deref());
    let Some(profile) = profile else {
        ui::warn(format!(
            "{} is unsigned: TVs install signed packages; pass --sign <profile> (a certificate profile of Samsung's Certificate Manager) or set nsh.json tizen.signing",
            ui::path(&unsigned)
        ));
        if options.run {
            bail!("--run installs a signed package: pass --sign <profile>");
        }
        return Ok(());
    };
    let signed = options.out.join(format!("{name}.wgt"));
    sign(profile, &unsigned, &signed)?;
    if options.run {
        install_and_launch(&signed, &widget.app, None)?;
    }
    Ok(())
}

/// The app's icon as Tizen TVs show it: 512×423, the square icon centered.
fn write_icon(icon: &Path, to: &Path) -> anyhow::Result<()> {
    let image = image::open(icon).with_context(|| format!("read {}", icon.display()))?;
    let side = 423;
    let scaled = image.resize_to_fill(side, side, image::imageops::FilterType::Lanczos3);
    let mut canvas = image::RgbaImage::new(512, side);
    image::imageops::overlay(
        &mut canvas,
        &scaled.to_rgba8(),
        (512 - i64::from(side)) / 2,
        0,
    );
    canvas
        .save_with_format(to, image::ImageFormat::Png)
        .with_context(|| format!("write {}", to.display()))
}

/// Zips the files of `dir` into `archive`, at its root, in a fixed order;
/// the archive appears whole.
fn zip_dir(dir: &Path, archive: &Path) -> anyhow::Result<()> {
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    let part = archive.with_extension("wgt.part");
    let mut zip = ZipWriter::new(
        fs::File::create(&part).with_context(|| format!("create {}", part.display()))?,
    );
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut pending = vec![(dir.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if entry.file_type()?.is_dir() {
                pending.push((entry.path(), format!("{name}/")));
            } else {
                zip.start_file(name, options)?;
                zip.write_all(&fs::read(entry.path())?)?;
            }
        }
    }
    zip.finish()?;
    fs::rename(&part, archive).with_context(|| format!("write {}", archive.display()))
}

/// The Tizen SDK's `tz`: on `PATH`, else where the SDK installs it
/// (`~/tizen-studio/tools/tizen-core/tz`; `docs/dev/web.md`).
fn tz() -> anyhow::Result<PathBuf> {
    let file = format!("tz{}", std::env::consts::EXE_SUFFIX);
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file());
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let installed = home.map(|home| {
        PathBuf::from(home)
            .join("tizen-studio/tools/tizen-core")
            .join(&file)
    });
    on_path
        .or(installed.filter(|path| path.is_file()))
        .context("no `tz` (the Tizen SDK's command line) on PATH or in ~/tizen-studio/tools/tizen-core: install the Tizen SDK to sign and install packages")
}

/// `tz`'s command that signs `unsigned` with certificate profile
/// `profile` into `signed` (its repackaging of a package).
fn sign_command(tz: &Path, profile: &str, unsigned: &Path, signed: &Path) -> Command {
    let mut command = Command::new(tz);
    command
        .args(["pack", "-t", "wgt", "-s", profile, "-b"])
        .arg(unsigned)
        .arg("-o")
        .arg(signed);
    command
}

/// `tz`'s commands that install the package at `wgt` and launch
/// application `app` on the TV `serial` (the one `tz` picks without it).
fn install_commands(tz: &Path, wgt: &Path, app: &str, serial: Option<&str>) -> [Command; 2] {
    let mut install = Command::new(tz);
    install.args(["install", "-p"]).arg(wgt);
    let mut launch = Command::new(tz);
    launch.args(["run", "-p", app]);
    for command in [&mut install, &mut launch] {
        if let Some(serial) = serial {
            command.args(["-e", serial]);
        }
    }
    [install, launch]
}

/// Runs a step's `tz` command; its output only if it fails.
fn run_tz(mut command: Command, doing: &str) -> anyhow::Result<()> {
    let (status, output) =
        ui::capture(&mut command, |_| {}).with_context(|| format!("run tz to {doing}"))?;
    if !status.success() {
        return Err(ui::failure(
            format!("tz could not {doing} ({status})"),
            output,
        ));
    }
    Ok(())
}

fn sign(profile: &str, unsigned: &Path, signed: &Path) -> anyhow::Result<()> {
    let step = ui::Step::start(format!("signing {} with `{profile}`", ui::path(unsigned)));
    let _ = fs::remove_file(signed);
    run_tz(
        sign_command(&tz()?, profile, unsigned, signed),
        "sign the package",
    )?;
    step.done(format!("packaged {}", ui::path(signed)));
    Ok(())
}

fn install_and_launch(wgt: &Path, app: &str, serial: Option<&str>) -> anyhow::Result<()> {
    let step = ui::Step::start(format!("installing {app} on the TV"));
    let [install, launch] = install_commands(&tz()?, wgt, app, serial);
    run_tz(install, "install the package")?;
    run_tz(launch, "launch the app")?;
    step.done(format!("launched {app} on the TV"));
    Ok(())
}

/// The dev server's page as a Tizen web app's (`nsh run` and `nsh test
/// --target tizen-web`): the web host's page and files (`web::dev_host`)
/// with the TV shim before `nsh.js`, the TV idiom and no history entries;
/// `web::with_page_host` fills in the rest.
pub(crate) fn dev_page((page, mut files): (String, web::Files)) -> (String, web::Files) {
    files.push((SHIM_NAME.to_owned(), SHIM.as_bytes().to_vec()));
    let page = page
        .replace(
            "{{scripts}}",
            &format!("<script src=\"./{SHIM_NAME}\"></script>"),
        )
        .replace("{{idiom}}", "\"tv\"")
        .replace("{{history}}", "false");
    (page, files)
}

/// The development package for the dev server at `url` (`http://<LAN
/// address>:<port>/`): a page that opens the server's, allowed to reach and
/// open it, in `dir`.
fn write_dev_package(dir: &Path, url: &str) -> anyhow::Result<Widget> {
    let origin = url.trim_end_matches('/');
    let host = origin
        .split_once("://")
        .map_or(origin, |(_, host)| host)
        .to_owned();
    let widget = Widget {
        iri: widget_iri("com.longbridge.nsh.dev"),
        package: DEV_PACKAGE.to_owned(),
        app: format!("{DEV_PACKAGE}.NshDev"),
        name: "NSH Dev".to_owned(),
        version: "1.0.0".to_owned(),
        required_version: API_VERSION.to_owned(),
        icon: false,
        privileges: PRIVILEGES.iter().map(|p| (*p).to_owned()).collect(),
        access: vec![origin.to_owned()],
        navigation: vec![host],
    };
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("remove {}", dir.display()))?;
    }
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let url_json = serde_json::to_string(url)?.replace("</", "<\\/");
    fs::write(
        dir.join("index.html"),
        DEV_PAGE
            .replace("{{url_json}}", &url_json)
            .replace("{{url}}", &escape(url)),
    )?;
    fs::write(dir.join("config.xml"), widget.config_xml())?;
    Ok(widget)
}

/// `nsh run --target tizen-web`'s package: [`write_dev_package`] in the
/// app's `.nsh/tizen-web/dev/`, zipped; with a certificate profile
/// (`nsh.json` `tizen.signing`) signed, installed on the TV (`device`: its
/// serial) and launched. Without one the page opens in a browser here only.
pub(crate) fn run_dev(entry: &Path, url: &str, device: Option<&str>) -> anyhow::Result<()> {
    let entry = dunce::canonicalize(entry)?;
    let work = entry
        .parent()
        .context("entry has no directory")?
        .join(".nsh/tizen-web");
    let dir = work.join("dev");
    let widget = write_dev_package(&dir, url)?;
    let unsigned = work.join("dev.wgt");
    zip_dir(&dir, &unsigned)?;
    let config = crate::package::read_config(&entry)?;
    let Some(profile) = config.tizen.signing.as_deref() else {
        ui::warn(format!(
            "{} is unsigned: set nsh.json tizen.signing (a certificate profile) to install it on a TV",
            ui::path(&unsigned)
        ));
        return Ok(());
    };
    let signed = work.join("NshDev.wgt");
    sign(profile, &unsigned, &signed)?;
    install_and_launch(&signed, &widget.app, device)
}

/// `nsh package --target tizen-web --probe`: a package with the probe's
/// page and files in `<out>/tizen-web/probe/`, zipped, signed as an app's
/// package is. Its page tries the ways the app's page loads its files and
/// shows what each did on the TV's screen.
pub(crate) fn probe(options: &PackageOptions<'_>) -> anyhow::Result<()> {
    let root = options.out.join("tizen-web");
    let dir = root.join("probe");
    let widget = write_probe(&dir)?;
    let unsigned = root.join("probe.wgt");
    zip_dir(&dir, &unsigned)?;
    ui::done(format!("wrote the loader probe {}", ui::path(&dir)));
    let profile = options.sign.or(options.config.tizen.signing.as_deref());
    let Some(profile) = profile else {
        ui::warn(format!(
            "{} is unsigned: pass --sign <profile> to sign it",
            ui::path(&unsigned)
        ));
        if options.run {
            bail!("--run installs a signed package: pass --sign <profile>");
        }
        return Ok(());
    };
    let signed = options.out.join("NshProbe.wgt");
    sign(profile, &unsigned, &signed)?;
    if options.run {
        install_and_launch(&signed, &widget.app, None)?;
    }
    Ok(())
}

/// The probe's files in `dir`.
fn write_probe(dir: &Path) -> anyhow::Result<Widget> {
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("remove {}", dir.display()))?;
    }
    fs::create_dir_all(dir.join("probe")).with_context(|| format!("create {}", dir.display()))?;
    fs::write(dir.join("index.html"), PROBE_PAGE)?;
    for (path, text) in PROBE_FILES {
        fs::write(dir.join(path), text)?;
    }
    fs::write(dir.join("probe/probe.wasm"), PROBE_WASM)?;
    let widget = Widget {
        iri: widget_iri("com.longbridge.nsh.probe"),
        package: PROBE_PACKAGE.to_owned(),
        app: format!("{PROBE_PACKAGE}.Probe"),
        name: "NSH Probe".to_owned(),
        version: "1.0.0".to_owned(),
        required_version: API_VERSION.to_owned(),
        icon: false,
        privileges: PRIVILEGES.iter().map(|p| (*p).to_owned()).collect(),
        access: vec!["*".to_owned()],
        navigation: Vec::new(),
    };
    fs::write(dir.join("config.xml"), widget.config_xml())?;
    Ok(widget)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_id_is_ten_letters_and_digits_made_from_the_app_id() {
        let id = package_id("com.acme.counter");
        assert_eq!(id.len(), 10);
        assert!(id.starts_with(|c: char| c.is_ascii_alphabetic()), "{id}");
        assert!(check_package_id(&id).is_ok());
        assert_eq!(id, package_id("com.acme.counter"));
        assert_ne!(id, package_id("com.acme.counter2"));
    }

    #[test]
    fn versions_are_tizens() {
        for version in ["1.0.0", "255.255.65535", "2.1"] {
            assert!(check_version(version).is_ok(), "{version}");
        }
        for version in [
            "1",
            "1.0.0.0",
            "256.0.0",
            "1.0.65536",
            "1.0.0-beta",
            "1..0",
            "v1.0",
        ] {
            assert!(check_version(version).is_err(), "{version}");
        }
        assert!(check_api_version("8.0").is_ok());
        assert!(check_api_version("8").is_err());
        assert_eq!(app_name("Hello, TV!"), "HelloTV");
        assert_eq!(app_name("计数器"), "App");
    }

    #[test]
    fn the_widget_iri_is_the_app_ids_domain_and_path() {
        assert_eq!(
            widget_iri("com.example.tv.counter"),
            "http://example.com/tv/counter"
        );
        assert_eq!(
            widget_iri("com.example.counter"),
            "http://example.com/counter"
        );
        assert_eq!(widget_iri("counter"), "http://counter");
    }

    #[test]
    fn tz_signs_installs_and_launches() {
        let args = |command: &Command| {
            command
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        let tz = Path::new("tz");
        let sign = sign_command(tz, "tv", Path::new("a.wgt"), Path::new("b.wgt"));
        assert_eq!(
            args(&sign),
            [
                "pack", "-t", "wgt", "-s", "tv", "-b", "a.wgt", "-o", "b.wgt"
            ]
        );
        let [install, launch] = install_commands(tz, Path::new("b.wgt"), "Pkg0123456.App", None);
        assert_eq!(args(&install), ["install", "-p", "b.wgt"]);
        assert_eq!(args(&launch), ["run", "-p", "Pkg0123456.App"]);
        let [install, launch] =
            install_commands(tz, Path::new("b.wgt"), "Pkg0123456.App", Some("tv:26101"));
        assert_eq!(args(&install), ["install", "-p", "b.wgt", "-e", "tv:26101"]);
        assert_eq!(
            args(&launch),
            ["run", "-p", "Pkg0123456.App", "-e", "tv:26101"]
        );
    }

    #[test]
    fn the_dev_page_runs_the_shim_on_a_tv() {
        let page = "{{scripts}}\nconst HISTORY = {{history}}; const IDIOM = {{idiom}};".to_owned();
        let (page, files) = dev_page((page, Vec::new()));
        assert_eq!(
            page,
            "<script src=\"./nsh-tizen.js\"></script>\nconst HISTORY = false; const IDIOM = \"tv\";"
        );
        assert_eq!(files, [(SHIM_NAME.to_owned(), SHIM.as_bytes().to_vec())]);
    }

    #[test]
    fn names_and_urls_are_escaped_in_the_manifest_and_the_dev_page() {
        let mut config = AppConfig::default();
        config.tizen.package_id = Some("AbCdE12345".to_owned());
        let widget = Widget::of(&config, "Tom & \"Jerry\" <TV>", "com.acme.tj").unwrap();
        assert_eq!(widget.app, "AbCdE12345.TomJerryTV");
        let xml = widget.config_xml();
        assert!(
            xml.contains("<name>Tom &amp; &quot;Jerry&quot; &lt;TV&gt;</name>"),
            "{xml}"
        );
        let dir = std::env::temp_dir().join(format!("nsh-tizen-dev-{}", std::process::id()));
        write_dev_package(&dir, "http://10.0.0.2:8790/</script>").unwrap();
        let page = fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(
            page.contains(r#"const URL_ = "http://10.0.0.2:8790/<\/script>";"#),
            "{page}"
        );
        assert!(
            page.contains("<small>http://10.0.0.2:8790/&lt;/script&gt;</small>"),
            "{page}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
