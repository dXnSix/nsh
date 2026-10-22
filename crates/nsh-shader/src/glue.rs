//! NSH's prelude and entry points, as WGSL appended to the app's source.
//!
//! The app's source comes first, so naga's spans in it are the app's own; what follows declares
//! `nsh`, `Image` and the `sample_*` functions the source calls, and for a writer the resources and
//! entry points that run its `main`. The checker sees stubs (`Glue::Check`); each writer gets the
//! real functions over its target's resources (`Glue::Draw`, `Glue::D2d`).

use std::fmt::Write as _;

use crate::{INPUTS, Input, layout};

/// Which prelude to append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glue {
    /// Stubs, no entry point: to find the app's `main` and its uniforms' struct.
    Signature,
    /// Stubs and a fragment entry point that loads the uniforms: what the checker and the AGSL
    /// writer read (naga validates the uniforms' layout against the uniform address space).
    Check,
    /// Resources and the `nsh_vertex` / `nsh_fragment` entry points (MSL, GLSL, Direct3D 11).
    #[cfg_attr(
        not(any(feature = "msl", feature = "glsl", feature = "hlsl")),
        allow(dead_code)
    )]
    Draw,
    /// A Direct2D custom effect's pixel shader: one constant buffer, inputs sampled at Direct2D's
    /// texel coordinates (`hlsl.rs` writes its entry point).
    #[cfg_attr(not(feature = "hlsl"), allow(dead_code))]
    D2d,
}

/// What the glue needs to know about the app's shader.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Spec<'a> {
    /// The struct `main` takes its uniforms in, if it takes any.
    pub uniforms: Option<&'a str>,
    /// The inputs the shader samples (bit `i`: `Input` index `i`).
    pub inputs: u32,
}

// `Image`'s `slot` (its x) says which texture `sample_image` samples; a vector of four aligns
// `Image` to 16 bytes, as the uniform address space wants a struct field aligned.
const STRUCTS: &str = "
struct Nsh { size: vec2f, scale: f32, time: f32, progress: f32 }
struct Image { size: vec2f, slot: vec4i }
struct NshInput { rect: vec4f, px: vec4f }
";

/// The app's source followed by the prelude `glue` asks for.
pub(crate) fn source(user: &str, glue: Glue, spec: Spec<'_>) -> String {
    let mut out = String::with_capacity(user.len() + 4096);
    out.push_str(user);
    // A line comment at the end of the app's source must not swallow the prelude.
    out.push_str("\n// NSH prelude\n");
    out.push_str(STRUCTS);
    let _ = writeln!(
        out,
        "struct NshFrame {{ area: vec4f, origin: vec4f, inputs: array<NshInput, {INPUTS}> }}"
    );
    match glue {
        Glue::Signature | Glue::Check => stubs(&mut out, glue, spec),
        Glue::Draw => draw(&mut out, spec),
        Glue::D2d => d2d(&mut out, spec),
    }
    out
}

const SAMPLERS: [(&str, Input); 4] = [
    ("sample_content", Input::Content),
    ("sample_backdrop", Input::Backdrop),
    ("sample_from", Input::From),
    ("sample_to", Input::To),
];

fn stubs(out: &mut String, glue: Glue, spec: Spec<'_>) {
    out.push_str("@group(0) @binding(0) var<uniform> nsh: Nsh;\n");
    for (name, _) in SAMPLERS {
        let _ = writeln!(
            out,
            "fn {name}(p: vec2f) -> vec4f {{ return nsh_stub(p); }}"
        );
    }
    out.push_str(
        "fn sample_image(image: Image, p: vec2f) -> vec4f { return nsh_stub(p) * image.size.x; }\n",
    );
    out.push_str("fn nsh_stub(p: vec2f) -> vec4f { return vec4f(p, p) * 0.0; }\n");
    if glue == Glue::Check {
        let call = match spec.uniforms {
            Some(ty) => {
                let _ = writeln!(out, "@group(0) @binding(2) var<uniform> nsh_u: {ty};");
                "main(p, nsh_u)"
            }
            None => "main(p)",
        };
        let _ = writeln!(
            out,
            "@fragment fn nsh_check(@location(0) p: vec2f) -> @location(0) vec4f {{ return {call}; }}"
        );
    }
}

/// `sample_*` over `nsh_sample_<i>`, the functions that sample input `i` (each target its own).
fn samplers(out: &mut String, spec: Spec<'_>) {
    for (name, input) in SAMPLERS {
        let i = input.index();
        if spec.inputs & (1 << i) != 0 {
            let _ = writeln!(
                out,
                "fn {name}(p: vec2f) -> vec4f {{ return nsh_sample_{i}(p); }}"
            );
        } else {
            let _ = writeln!(
                out,
                "fn {name}(p: vec2f) -> vec4f {{ return vec4f(p, p) * 0.0; }}"
            );
        }
    }
    out.push_str("fn sample_image(image: Image, p: vec2f) -> vec4f {\n");
    for slot in 0..layout::MAX_IMAGES {
        let i = Input::Image(slot as u8).index();
        if spec.inputs & (1 << i) != 0 {
            let _ = writeln!(
                out,
                "  if image.slot.x == {slot} {{ return nsh_sample_{i}(p); }}"
            );
        }
    }
    out.push_str("  return vec4f(p, p) * 0.0;\n}\n");
}

/// Where `p` is in input `i`, from 0 to 1 across the input's rect; outside it the input is transparent.
fn decal(out: &mut String, frame: &str, i: usize) {
    let _ = writeln!(out, "  let input = {frame}.inputs[{i}];");
    out.push_str("  let uv = (p - input.rect.xy) / input.rect.zw;\n");
    out.push_str("  if any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) { return vec4f(0.0); }\n");
}

fn used(spec: Spec<'_>) -> impl Iterator<Item = usize> {
    (0..INPUTS).filter(move |i| spec.inputs & (1 << i) != 0)
}

fn draw(out: &mut String, spec: Spec<'_>) {
    out.push_str("@group(0) @binding(0) var<uniform> nsh: Nsh;\n");
    out.push_str("@group(0) @binding(1) var<uniform> nsh_frame: NshFrame;\n");
    if let Some(ty) = spec.uniforms {
        let _ = writeln!(out, "@group(0) @binding(2) var<uniform> nsh_u: {ty};");
    }
    if spec.inputs != 0 {
        out.push_str("@group(1) @binding(0) var nsh_sampler: sampler;\n");
    }
    for i in used(spec) {
        let _ = writeln!(
            out,
            "@group(1) @binding({}) var nsh_texture_{i}: texture_2d<f32>;",
            i + 1
        );
        let _ = writeln!(out, "fn nsh_sample_{i}(p: vec2f) -> vec4f {{");
        decal(out, "nsh_frame", i);
        let _ = writeln!(
            out,
            "  return textureSampleLevel(nsh_texture_{i}, nsh_sampler, uv, 0.0);\n}}"
        );
    }
    samplers(out, spec);
    let call = if spec.uniforms.is_some() {
        "main(v.p, nsh_u)"
    } else {
        "main(v.p)"
    };
    let _ = write!(
        out,
        "struct NshVarying {{ @builtin(position) position: vec4f, @location(0) p: vec2f }}
@vertex fn nsh_vertex(@builtin(vertex_index) index: u32) -> NshVarying {{
  let corner = vec2f(f32(index & 1u), f32((index >> 1u) & 1u));
  let area = nsh_frame.area;
  return NshVarying(vec4f(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0), area.xy + corner * area.zw);
}}
@fragment fn nsh_fragment(v: NshVarying) -> @location(0) vec4f {{ return {call}; }}
"
    );
}

fn d2d(out: &mut String, spec: Spec<'_>) {
    let uniforms = spec
        .uniforms
        .map(|ty| format!(", u: {ty}"))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "struct NshD2d {{ nsh: Nsh, frame: NshFrame{uniforms} }}"
    );
    out.push_str("@group(0) @binding(0) var<uniform> nsh_d2d: NshD2d;\n");
    out.push_str("var<private> nsh: Nsh;\n");
    let inputs: Vec<usize> = used(spec).collect();
    if !inputs.is_empty() {
        out.push_str("@group(1) @binding(0) var nsh_sampler: sampler;\n");
        out.push_str("var<private> nsh_d2d_scene: vec2f;\n");
        let _ = writeln!(
            out,
            "var<private> nsh_d2d_texel: array<vec4f, {}>;",
            inputs.len()
        );
    }
    for (k, &i) in inputs.iter().enumerate() {
        let _ = writeln!(
            out,
            "@group(1) @binding({}) var nsh_texture_{i}: texture_2d<f32>;",
            i + 1
        );
        let _ = writeln!(out, "fn nsh_sample_{i}(p: vec2f) -> vec4f {{");
        decal(out, "nsh_d2d.frame", i);
        // Direct2D hands each input's texel coordinates of this pixel (xy) and the size of a
        // texel (zw): the input may be a part of an atlas, so the texture's 0..1 is not the input's.
        let _ = writeln!(out, "  let t = nsh_d2d_texel[{k}];");
        out.push_str(
            "  let at = t.xy + (p * nsh.scale + nsh_d2d.frame.origin.xy - nsh_d2d_scene) * t.zw;\n",
        );
        let _ = writeln!(
            out,
            "  return textureSampleLevel(nsh_texture_{i}, nsh_sampler, at, 0.0);\n}}"
        );
    }
    samplers(out, spec);
    let params: String = (0..inputs.len())
        .map(|k| format!(", t{k}: vec4f"))
        .collect();
    let _ = writeln!(out, "fn nsh_d2d_run(scene: vec4f{params}) -> vec4f {{");
    out.push_str("  nsh = nsh_d2d.nsh;\n");
    if !inputs.is_empty() {
        out.push_str("  nsh_d2d_scene = scene.xy;\n");
        let texels: Vec<String> = (0..inputs.len()).map(|k| format!("t{k}")).collect();
        let _ = writeln!(out, "  nsh_d2d_texel = array({});", texels.join(", "));
    }
    out.push_str("  let p = (scene.xy - nsh_d2d.frame.origin.xy) / nsh.scale;\n");
    let call = if spec.uniforms.is_some() {
        "main(p, nsh_d2d.u)"
    } else {
        "main(p)"
    };
    let _ = writeln!(out, "  return {call};\n}}");
    // naga writes functions an entry point reaches; Direct2D's entry point is `hlsl.rs`'s.
    let zeros: String = (0..inputs.len()).map(|_| ", vec4f(0.0)").collect();
    let _ = writeln!(
        out,
        "@fragment fn nsh_d2d_unused() -> @location(0) vec4f {{ return nsh_d2d_run(vec4f(0.0){zeros}); }}"
    );
}
