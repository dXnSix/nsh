//! `--target tizen-web`: the packages `nsh package` writes. A fake web runtime
//! (`NSH_RUNTIME`: a linkable module of three functions, the crate's
//! `nsh.js` and `browser/`) packages without building the real one, and a
//! fake `tz` on `PATH` stands in for the Tizen SDK.
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
/// fake web runtime in `runtime/web/` and the fake `tz` in `bin/`.
fn app(config: &str) -> Directory {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = Directory(std::env::temp_dir().join(format!(
        "nsh-cli-tizen-web-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    let root = &dir.0;
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app/App.tsx"),
        "import { mount, Text } from 'nsh'; mount(() => <Text>Hello</Text>);",
    )
    .unwrap();
    fs::write(root.join("app/nsh.json"), config).unwrap();
    let web = root.join("runtime/web");
    fs::create_dir_all(&web).unwrap();
    fs::write(web.join("nsh-browser.wasm"), linkable_runtime()).unwrap();
    fs::write(web.join("nsh-dev.wasm"), linkable_runtime()).unwrap();
    let crate_web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../nsh-platform-web/web");
    std::os::unix::fs::symlink(crate_web.join("nsh.js"), web.join("nsh.js")).unwrap();
    std::os::unix::fs::symlink(crate_web.join("browser"), web.join("browser")).unwrap();
    // `tz`: its arguments, one line a call; `pack` writes the package `-o`
    // names, as signing does.
    fs::create_dir_all(root.join("bin")).unwrap();
    let tz = root.join("bin/tz");
    fs::write(
        &tz,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$TZ_LOG"
if [ "$1" = pack ]; then
  while [ $# -gt 0 ]; do
    if [ "$1" = -o ]; then echo signed > "$2"; fi
    shift
  done
fi
"#,
    )
    .unwrap();
    fs::set_permissions(&tz, fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

/// `nsh <args> <app>` with the fake runtime and `tz`.
fn nsh(dir: &Directory) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nsh"));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.0.join("bin")];
    paths.extend(std::env::split_paths(&path));
    command
        .env("NSH_RUNTIME", dir.0.join("runtime"))
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("TZ_LOG", dir.0.join("tz.log"))
        .env("NO_COLOR", "1");
    command
}

fn package(dir: &Directory, extra: &[&str]) -> Output {
    nsh(dir)
        .args(["package", "--target", "tizen-web", "--out"])
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
                let relative = path.strip_prefix(dir).unwrap();
                out.push(relative.to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out
}

/// The names in the zip archive `wgt`, sorted.
fn entries(wgt: &Path) -> Vec<String> {
    // A zip's central directory: signature 0x02014b50, the name's length
    // at 28, the name at 46.
    let bytes = fs::read(wgt).unwrap();
    let mut names = Vec::new();
    for at in 0..bytes.len().saturating_sub(46) {
        if bytes[at..at + 4] == [0x50, 0x4b, 0x01, 0x02] {
            let len = u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]) as usize;
            names.push(String::from_utf8_lossy(&bytes[at + 46..at + 46 + len]).into_owned());
        }
    }
    names.sort();
    names
}

/// The shim's source, which packages carry as they are.
fn shim_source() -> String {
    read(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tizen_web/shim.js"))
}

#[test]
fn a_package_is_the_web_site_with_a_manifest_and_the_tv_shim() {
    let dir = app(r#"{ "id": "com.acme.tv.counter", "name": "Counter" }"#);
    let output = package(&dir, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let site = dir.0.join("out/tizen-web/Counter");
    let names = files(&site);
    let shim = names
        .iter()
        .find(|n| n.starts_with("nsh-tizen.") && n.ends_with(".js"))
        .unwrap_or_else(|| panic!("no shim in {names:?}"));
    assert_eq!(read(site.join(shim)), shim_source());
    for file in ["index.html", "config.xml", "manifest.webmanifest"] {
        assert!(names.iter().any(|n| n == file), "{file} in {names:?}");
    }
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("nsh.") && n.ends_with(".wasm"))
    );
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("app.") && n.ends_with(".js"))
    );
    // Installed files are read as they are.
    assert!(
        !names
            .iter()
            .any(|n| n.ends_with(".br") || n.ends_with(".gz"))
    );

    let config = read(site.join("config.xml"));
    assert!(
        config.contains(r#"id="http://acme.com/tv/counter""#),
        "{config}"
    );
    let package = config
        .split(r#"package=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    assert_eq!(package.len(), 10);
    assert!(package.bytes().all(|b| b.is_ascii_alphanumeric()));
    assert!(
        config.contains(&format!(
            r#"<tizen:application id="{package}.Counter" package="{package}" required_version="8.0"/>"#
        )),
        "{config}"
    );
    for part in [
        r#"<content src="index.html"/>"#,
        "<name>Counter</name>",
        r#"<tizen:privilege name="http://tizen.org/privilege/internet"/>"#,
        r#"<tizen:privilege name="http://tizen.org/privilege/tv.inputdevice"/>"#,
        r#"<tizen:profile name="tv-samsung"/>"#,
        r#"screen-orientation="landscape""#,
        r#"version="1.0.0""#,
    ] {
        assert!(config.contains(part), "{part} in {config}");
    }
    assert!(!config.contains("<icon"), "{config}");

    // The shim runs first, then the runtime starts the player on a TV,
    // without history entries (the remote's Back is the shim's).
    let page = read(site.join("index.html"));
    let shim_at = page
        .find(&format!(r#"<script src="{shim}"></script>"#))
        .unwrap_or_else(|| panic!("{page}"));
    assert!(shim_at < page.find(r#"<script type="module">"#).unwrap());
    assert!(page.contains(r#""idiom":"tv""#), "{page}");
    assert!(page.contains(r#""history":false"#), "{page}");
    assert!(page.contains("idiom: app.idiom"), "{page}");

    // Zipped, unsigned: there is no profile.
    let unsigned = dir.0.join("out/tizen-web/Counter.wgt");
    assert_eq!(entries(&unsigned), names);
    assert!(!dir.0.join("out/Counter.wgt").exists());
    assert!(stderr(&output).contains("unsigned"), "{}", stderr(&output));
    assert!(!dir.0.join("tz.log").exists());
}

#[test]
fn the_tizen_section_names_the_package_and_its_privileges() {
    let dir = app(
        r#"{ "id": "com.acme.counter", "name": "Counter", "version": "2.1", "icon": "icon.png",
            "web": { "base": "/apps/counter/" },
            "tizen": { "packageId": "AbCdE12345", "appId": "AbCdE12345.Count", "version": "3.4.5",
                "apiVersion": "9.0", "privileges": ["http://tizen.org/privilege/tv.audio",
                "http://tizen.org/privilege/internet"] } }"#,
    );
    image::RgbaImage::new(64, 64)
        .save(dir.0.join("app/icon.png"))
        .unwrap();
    let output = package(&dir, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let site = dir.0.join("out/tizen-web/Counter");
    let config = read(site.join("config.xml"));
    for part in [
        r#"<tizen:application id="AbCdE12345.Count" package="AbCdE12345" required_version="9.0"/>"#,
        r#"version="3.4.5""#,
        r#"<icon src="icon.png"/>"#,
        r#"<tizen:privilege name="http://tizen.org/privilege/tv.audio"/>"#,
    ] {
        assert!(config.contains(part), "{part} in {config}");
    }
    assert_eq!(config.matches("privilege/internet").count(), 1, "{config}");
    // Samsung's icon size.
    assert_eq!(
        image::image_dimensions(site.join("icon.png")).unwrap(),
        (512, 423)
    );
    // An installed page loads its files where it is: no `web.base`.
    assert!(!read(site.join("index.html")).contains("<base href=\""));
}

#[test]
fn ids_and_versions_tizen_refuses_are_refused() {
    for (config, message) in [
        (
            r#""tizen": { "packageId": "short" }"#,
            "tizen.packageId `short`",
        ),
        (
            r#""tizen": { "packageId": "AbCdE12345", "appId": "Other12345.App" }"#,
            "tizen.appId `Other12345.App`",
        ),
        (r#""version": "1.0.0-beta""#, "version `1.0.0-beta`"),
        (
            r#""tizen": { "apiVersion": "eight" }"#,
            "tizen.apiVersion `eight`",
        ),
    ] {
        let dir = app(&format!(r#"{{ "id": "com.acme.counter", {config} }}"#));
        let output = package(&dir, &[]);
        assert!(!output.status.success(), "{config}");
        assert!(stderr(&output).contains(message), "{}", stderr(&output));
        assert!(!dir.0.join("out/tizen-web").exists(), "{config}");
    }
}

#[test]
fn a_profile_signs_the_package_with_tz() {
    let dir = app(
        r#"{ "id": "com.acme.counter", "name": "Counter", "tizen": { "signing": "from-nsh-json" } }"#,
    );
    let output = package(&dir, &["--sign", "tv-profile"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let unsigned = dir.0.join("out/tizen-web/Counter.wgt");
    let signed = dir.0.join("out/Counter.wgt");
    assert_eq!(
        read(dir.0.join("tz.log")).trim(),
        format!(
            "pack -t wgt -s tv-profile -b {} -o {}",
            unsigned.display(),
            signed.display()
        )
    );
    assert!(signed.exists());
    // Without --sign, nsh.json's profile.
    fs::remove_file(dir.0.join("tz.log")).unwrap();
    let output = package(&dir, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(read(dir.0.join("tz.log")).starts_with("pack -t wgt -s from-nsh-json -b "));
}

/// A web runtime as wasm-ld writes it with `--emit-relocs`, as small as
/// `nsh package` links: three functions (`b` and `a` exported), a table,
/// a memory, data and their relocations (the pruner's test module,
/// `src/web/prune.rs`).
fn linkable_runtime() -> Vec<u8> {
    fn uleb(out: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }
    /// Five bytes, as relocated immediates are (`sleb` for 0x41's).
    fn padded(out: &mut Vec<u8>, mut value: u32) {
        for i in 0..5 {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            out.push(if i < 4 { byte | 0x80 } else { byte });
        }
    }
    fn section(out: &mut Vec<u8>, id: u8, contents: &[u8]) {
        out.push(id);
        uleb(out, contents.len() as u64);
        out.extend_from_slice(contents);
    }
    fn name(out: &mut Vec<u8>, name: &str) {
        uleb(out, name.len() as u64);
        out.extend_from_slice(name.as_bytes());
    }
    let mut wasm = b"\0asm\x01\0\0\0".to_vec();
    section(&mut wasm, 1, &[1, 0x60, 0, 1, 0x7f]);
    section(&mut wasm, 3, &[3, 0, 0, 0]);
    section(&mut wasm, 4, &[1, 0x70, 1, 3, 3]);
    section(&mut wasm, 5, &[1, 0, 1]);
    let mut exports = vec![3];
    for (export, kind, index) in [("b", 0, 0), ("a", 0, 1), ("memory", 2, 0)] {
        name(&mut exports, export);
        exports.extend_from_slice(&[kind, index]);
    }
    section(&mut wasm, 7, &exports);
    section(&mut wasm, 9, &[1, 0, 0x41, 1, 0x0b, 2, 0, 2]);
    let mut relocs = Vec::new();
    let mut code = vec![3];
    for ops in [
        &[(0x41, 1, 0, 1), (0x41, 4, 4, 1024)][..],
        &[(0x10, 0, 2, 2), (0x41, 4, 3, 1040)],
        &[(0x41, 1, 2, 2)],
    ] {
        let mut body = vec![0];
        let mut at = Vec::new();
        for (index, &(op, ty, symbol, value)) in ops.iter().enumerate() {
            if index > 0 {
                body.push(0x1a);
            }
            body.push(op);
            at.push((ty, body.len(), symbol));
            padded(&mut body, value);
        }
        body.push(0x0b);
        code.push(body.len() as u8);
        for (ty, offset, symbol) in at {
            relocs.push((ty, code.len() + offset, symbol));
        }
        code.extend_from_slice(&body);
    }
    section(&mut wasm, 10, &code);
    let mut data = vec![1, 0, 0x41, 0x80, 0x08, 0x0b, 24];
    data.extend_from_slice(&[0xaa; 16]);
    let pointer_at = data.len();
    data.extend_from_slice(&1044u32.to_le_bytes());
    data.extend_from_slice(&[0xbb; 4]);
    section(&mut wasm, 11, &data);
    let mut linking = Vec::new();
    name(&mut linking, "linking");
    linking.push(2);
    let mut symbols = vec![5];
    for (index, func) in ["b", "a", "c"].iter().enumerate() {
        symbols.extend_from_slice(&[0, 0, index as u8]);
        name(&mut symbols, func);
    }
    for (symbol, offset, size) in [("d0", 16, 8), ("d1", 0, 16)] {
        symbols.extend_from_slice(&[1, 2]);
        name(&mut symbols, symbol);
        symbols.extend_from_slice(&[0, offset, size]);
    }
    linking.push(8);
    uleb(&mut linking, symbols.len() as u64);
    linking.extend_from_slice(&symbols);
    let mut info = vec![1];
    name(&mut info, ".data");
    info.extend_from_slice(&[4, 0]);
    linking.push(5);
    uleb(&mut linking, info.len() as u64);
    linking.extend_from_slice(&info);
    section(&mut wasm, 0, &linking);
    // The code section is the sixth (from 0), the data section the seventh.
    let mut reloc = Vec::new();
    name(&mut reloc, "reloc.CODE");
    reloc.extend_from_slice(&[6, relocs.len() as u8]);
    for (ty, at, symbol) in relocs {
        reloc.push(ty);
        uleb(&mut reloc, at as u64);
        uleb(&mut reloc, symbol as u64);
        if ty == 4 {
            reloc.push(0);
        }
    }
    section(&mut wasm, 0, &reloc);
    let mut reloc = Vec::new();
    name(&mut reloc, "reloc.DATA");
    reloc.extend_from_slice(&[7, 1, 5, pointer_at as u8, 3, 4]);
    section(&mut wasm, 0, &reloc);
    wasm
}
