//! Windows: a Direct2D custom effect's pixel shader, and Direct3D 11 shaders for the devices or
//! effects Direct2D cannot run it on (`docs/dev/shaders.md` "Windows"). naga's HLSL writer over the
//! glue, compiled with `D3DCompile` at shader model 5.0.

use std::fmt::Write as _;

use naga::back::hlsl;

use crate::{Error, Input, Shader, glue::Glue, internal};

/// A shader as HLSL (shader model 5.0).
///
/// `d2d` is the pixel shader of a Direct2D custom effect whose inputs are `textures`, in order:
/// compile its entry point [`HlslProgram::D2D_ENTRY`] as `ps_5_0`; it reads one constant buffer
/// (`b0`) holding `nsh`, the frame and the uniforms at [`HlslProgram::D2D_NSH`],
/// [`HlslProgram::D2D_FRAME`] and `d2d_uniforms`, and Direct2D's scene position and each input's
/// texel coordinates (`SCENE_POSITION`, `TEXCOORD0`...).
///
/// `d3d11` has a vertex shader (`nsh_vertex`, `vs_5_0`) and a pixel shader (`nsh_fragment`,
/// `ps_5_0`) that draw a triangle strip of four vertices (`SV_VertexID`, no input layout) over the
/// frame's area; constant buffers `b0` (`nsh`), `b1` (the frame), `b2` (the uniforms), textures
/// `t0`... (`textures`), one sampler `s0` (linear, clamped).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct HlslProgram {
    /// The Direct2D pixel shader.
    pub d2d: String,
    /// The uniforms' offset in Direct2D's constant buffer.
    pub d2d_uniforms: u32,
    /// The size of Direct2D's constant buffer.
    pub d2d_size: u32,
    /// The Direct3D 11 shaders.
    pub d3d11: String,
    /// The vertex shader's name in `d3d11`.
    pub vertex: String,
    /// The pixel shader's name in `d3d11`.
    pub fragment: String,
    /// The inputs: Direct2D's effect inputs, Direct3D's textures, in order.
    pub textures: Vec<Input>,
}

impl HlslProgram {
    /// The entry point of `d2d`.
    pub const D2D_ENTRY: &str = "NshD2dMain";
    /// The offset of `nsh` in Direct2D's constant buffer.
    pub const D2D_NSH: u32 = 0;
    /// The offset of the frame in Direct2D's constant buffer.
    pub const D2D_FRAME: u32 = 32;
}

fn options(textures: &[Input]) -> hlsl::Options {
    let target = |register| hlsl::BindTarget {
        register,
        ..Default::default()
    };
    let mut binding_map = hlsl::BindingMap::default();
    let binding = |group, binding| naga::ResourceBinding { group, binding };
    for b in 0..3 {
        binding_map.insert(binding(0, b), target(b));
    }
    binding_map.insert(binding(1, 0), target(0));
    for (register, input) in textures.iter().enumerate() {
        binding_map.insert(
            binding(1, input.index() as u32 + 1),
            target(register as u32),
        );
    }
    // naga's sampler heap declarations, which `plain_sampler` replaces.
    let mut sampler_buffer_binding_map = hlsl::SamplerIndexBufferBindingMap::default();
    sampler_buffer_binding_map.insert(hlsl::SamplerIndexBufferKey { group: 1 }, target(0));
    hlsl::Options {
        shader_model: hlsl::ShaderModel::V5_0,
        binding_map,
        sampler_buffer_binding_map,
        fake_missing_bindings: false,
        // The profile's loops are bounded.
        force_loop_bounding: false,
        ..Default::default()
    }
}

/// naga declares samplers as indexes into a Direct3D 12 sampler heap; Direct3D 11 and Direct2D
/// bind the one sampler at `s0`.
fn plain_sampler(source: &str) -> Result<String, Error> {
    let mut out = String::with_capacity(source.len());
    let mut replaced = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("SamplerState nagaSamplerHeap[")
            || trimmed.starts_with("SamplerComparisonState nagaComparisonSamplerHeap[")
            || (trimmed.starts_with("StructuredBuffer<uint> nagaGroup")
                && trimmed.contains("SamplerIndexArray"))
        {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("static const SamplerState ")
            && let Some((name, _)) = rest.split_once(" = nagaSamplerHeap[")
        {
            let _ = writeln!(out, "SamplerState {name} : register(s0);");
            replaced = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if out.contains("nagaSamplerHeap")
        || out.contains("SamplerIndexArray")
        || (!replaced && source.contains("SamplerState"))
    {
        return Err(internal(Error::new("naga's samplers changed")));
    }
    Ok(out)
}

fn write(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    options: &hlsl::Options,
) -> Result<(String, hlsl::ReflectionInfo), Error> {
    let mut out = String::new();
    let pipeline = hlsl::PipelineOptions::default();
    let mut writer = hlsl::Writer::new(&mut out, options, &pipeline);
    let reflection = writer
        .write(module, info, None)
        .map_err(|e| internal(Error::new(e.to_string())))?;
    Ok((plain_sampler(&out)?, reflection))
}

impl Shader {
    /// The shader in HLSL, for Direct2D and Direct3D 11.
    pub fn to_hlsl(&self) -> Result<HlslProgram, Error> {
        let textures = self.inputs();
        let options = options(&textures);

        let (module, info) = self.module(Glue::Draw)?;
        let (d3d11, reflection) = write(&module, &info, &options)?;
        let name = |entry: &str| -> Result<String, Error> {
            let index = module
                .entry_points
                .iter()
                .position(|e| e.name == entry)
                .unwrap_or_default();
            match reflection.entry_point_names.get(index) {
                Some(Ok(name)) => Ok(name.clone()),
                _ => Err(internal(Error::new("an entry point is missing"))),
            }
        };
        let (vertex, fragment) = (name("nsh_vertex")?, name("nsh_fragment")?);

        let (module, info) = self.module(Glue::D2d)?;
        let (mut d2d, _) = write(&module, &info, &options)?;
        let combined = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("NshD2d"))
            .map(|(_, t)| &t.inner);
        let (d2d_uniforms, d2d_size) = match combined {
            Some(naga::TypeInner::Struct { members, span }) => (
                members.get(2).map_or(*span, |m| m.offset),
                span.div_ceil(16) * 16,
            ),
            _ => {
                return Err(internal(Error::new(
                    "Direct2D's constant buffer is missing",
                )));
            }
        };
        // Direct2D's vertex shader hands the position, the scene position and a texel coordinate
        // per input, in this order.
        let texels: String = (0..textures.len())
            .map(|k| format!(", float4 t{k} : TEXCOORD{k}"))
            .collect();
        let args: String = (0..textures.len()).map(|k| format!(", t{k}")).collect();
        let _ = write!(
            d2d,
            "\nfloat4 {}(float4 position : SV_POSITION, float4 scene : SCENE_POSITION{texels}) : SV_Target\n{{\n    return nsh_d2d_run(scene{args});\n}}\n",
            HlslProgram::D2D_ENTRY
        );
        Ok(HlslProgram {
            d2d,
            d2d_uniforms,
            d2d_size,
            d3d11,
            vertex,
            fragment,
            textures,
        })
    }
}
