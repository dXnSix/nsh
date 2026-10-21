//! Programs for the platforms' own compilers, which run outside these tests.
//!
//! `NSH_SHADER_EXPORT=<dir>` writes every corpus shader's programs to `<dir>` (`.metal`, `.agsl`,
//! `.vert` / `.frag` for GLSL ES 3.00, `.d2d.hlsl` / `.d3d11.hlsl`); `NSH_SHADER_CORPUS=<dir>` adds
//! the `.wgsl` files of `<dir>` (a corpus of someone else's shaders) and prints which compile.
//! `xcrun metal` compiles the Metal programs in `metal_compiles` (macOS, ignored by default);
//! Windows' `D3DCompile` the HLSL ones in `hlsl_compiles` (Windows).

use std::path::{Path, PathBuf};

use nsh_shader::{GlslVersion, Limits, compile};

use crate::corpus;

fn extra_corpus() -> Vec<(String, String)> {
    let Some(dir) = std::env::var_os("NSH_SHADER_CORPUS") else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = std::fs::read_dir(dir)
        .expect("NSH_SHADER_CORPUS")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "wgsl").then(|| {
                (
                    p.file_stem().unwrap().to_string_lossy().into_owned(),
                    std::fs::read_to_string(&p).ok().unwrap_or_default(),
                )
            })
        })
        .collect();
    out.sort();
    out
}

fn export(dir: &Path, name: &str, source: &str) -> Result<(), String> {
    let shader = compile(source, &Limits::default()).map_err(|e| e.render(name, source))?;
    let write =
        |ext: &str, text: &str| std::fs::write(dir.join(format!("{name}.{ext}")), text).unwrap();
    write(
        "metal",
        &shader.to_msl().map_err(|e| format!("msl: {e}"))?.source,
    );
    let glsl = shader
        .to_glsl(GlslVersion::Es300)
        .map_err(|e| format!("glsl: {e}"))?;
    write("vert", &glsl.vertex);
    write("frag", &glsl.fragment);
    let glsl = shader
        .to_glsl(GlslVersion::Core330)
        .map_err(|e| format!("glsl: {e}"))?;
    write("330.vert", &glsl.vertex);
    write("330.frag", &glsl.fragment);
    let hlsl = shader.to_hlsl().map_err(|e| format!("hlsl: {e}"))?;
    write("d2d.hlsl", &hlsl.d2d);
    write("d3d11.hlsl", &hlsl.d3d11);
    write(
        "agsl",
        &shader.to_agsl().map_err(|e| format!("agsl: {e}"))?.source,
    );
    Ok(())
}

#[test]
fn export_programs() {
    let Some(dir) = std::env::var_os("NSH_SHADER_EXPORT").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    for (name, source) in corpus() {
        export(&dir, &name, &source).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let extra = extra_corpus();
    let mut failed = Vec::new();
    for (name, source) in &extra {
        if let Err(e) = export(&dir, name, source) {
            failed.push(format!("{name}: {}", e.lines().next().unwrap_or_default()));
        }
    }
    if !extra.is_empty() {
        println!("{} of {} compile:", extra.len() - failed.len(), extra.len());
        for f in &failed {
            println!("  {f}");
        }
    }
}

#[test]
#[ignore = "needs Xcode's Metal toolchain"]
fn metal_compiles() {
    let dir = std::env::temp_dir().join(format!("nsh-shader-metal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, source) in corpus() {
        let msl = compile(&source, &Limits::default())
            .unwrap()
            .to_msl()
            .unwrap();
        let path = dir.join(format!("{name}.metal"));
        std::fs::write(&path, &msl.source).unwrap();
        for sdk in ["macosx", "iphoneos"] {
            let out = std::process::Command::new("xcrun")
                .args(["-sdk", sdk, "metal", "-c"])
                .arg(&path)
                .arg("-o")
                .arg(dir.join(format!("{name}-{sdk}.air")))
                .output()
                .expect("xcrun");
            assert!(
                out.status.success(),
                "{name} ({sdk}):\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn hlsl_compiles() {
    use std::ffi::CString;

    use windows::{
        Win32::Graphics::Direct3D::{Fxc::D3DCompile, ID3DBlob, ID3DInclude},
        core::PCSTR,
    };

    for (name, source) in corpus() {
        let hlsl = compile(&source, &Limits::default())
            .unwrap()
            .to_hlsl()
            .unwrap();
        let stages = [
            (&hlsl.d2d, nsh_shader::HlslProgram::D2D_ENTRY, "ps_5_0"),
            (&hlsl.d3d11, hlsl.vertex.as_str(), "vs_5_0"),
            (&hlsl.d3d11, hlsl.fragment.as_str(), "ps_5_0"),
        ];
        for (code, entry, target) in stages {
            let (entry, target) = (CString::new(entry).unwrap(), CString::new(target).unwrap());
            let mut blob: Option<ID3DBlob> = None;
            let mut errors: Option<ID3DBlob> = None;
            // SAFETY: the source, entry point and target outlive the call; the blobs are out
            // parameters D3DCompile fills.
            let result = unsafe {
                D3DCompile(
                    code.as_ptr().cast(),
                    code.len(),
                    PCSTR::null(),
                    None,
                    None::<&ID3DInclude>,
                    PCSTR(entry.as_ptr().cast()),
                    PCSTR(target.as_ptr().cast()),
                    0,
                    0,
                    &mut blob,
                    Some(&mut errors),
                )
            };
            let message = errors.map(|e| {
                // SAFETY: the blob holds the compiler's messages, `GetBufferSize` bytes of text.
                let bytes = unsafe {
                    std::slice::from_raw_parts(e.GetBufferPointer().cast::<u8>(), e.GetBufferSize())
                };
                String::from_utf8_lossy(bytes).into_owned()
            });
            assert!(
                result.is_ok() && blob.is_some(),
                "{name} ({entry:?}, {target:?}): {message:?}\n{code}"
            );
        }
    }
}
