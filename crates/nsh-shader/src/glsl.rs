//! OpenGL 3.30 (Linux) and OpenGL ES 3.00 / WebGL 2 (Linux, the web): naga's GLSL writer over
//! the draw glue, a source per stage.

use naga::back::glsl;

use crate::{Error, Input, Shader, glue::Glue, internal};

/// The GLSL a program is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlslVersion {
    /// OpenGL ES 3.00 and WebGL 2 (`#version 300 es`).
    #[default]
    Es300,
    /// OpenGL 3.30 core.
    Core330,
}

/// A uniform block of a stage and the buffer it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct GlslBlock {
    /// The block's name (`glGetUniformBlockIndex`).
    pub name: String,
    /// Which buffer: [`GlslProgram::NSH`], [`GlslProgram::FRAME`] or [`GlslProgram::UNIFORMS`].
    pub buffer: u32,
}

/// A shader as GLSL: a vertex and a fragment shader to draw a triangle strip of four vertices over
/// the frame's area (`gl_VertexID` 0 to 3, no attributes).
///
/// The stages read the three buffers as uniform blocks (`blocks`; bind each block to its
/// buffer's binding point), the fragment shader the textures as samplers (`samplers`: each one's
/// name and input; linear filtering, clamped to edge, the program returns transparent outside an
/// input itself).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct GlslProgram {
    /// The vertex shader.
    pub vertex: String,
    /// The fragment shader.
    pub fragment: String,
    /// The uniform blocks of both stages (a block may be named differently in each).
    pub blocks: Vec<GlslBlock>,
    /// The fragment shader's samplers and the inputs they are.
    pub samplers: Vec<(String, Input)>,
}

impl GlslProgram {
    /// The buffer of `nsh`.
    pub const NSH: u32 = 0;
    /// The buffer of the frame.
    pub const FRAME: u32 = 1;
    /// The buffer of the uniforms.
    pub const UNIFORMS: u32 = 2;
}

impl Shader {
    /// The shader in GLSL `version`.
    pub fn to_glsl(&self, version: GlslVersion) -> Result<GlslProgram, Error> {
        let (module, info) = self.module(Glue::Draw)?;
        let options = glsl::Options {
            version: match version {
                GlslVersion::Es300 => glsl::Version::Embedded {
                    version: 300,
                    is_webgl: true,
                },
                GlslVersion::Core330 => glsl::Version::Desktop(330),
            },
            writer_flags: glsl::WriterFlags::empty(),
            ..Default::default()
        };
        let mut program = GlslProgram::default();
        for (stage, entry) in [
            (naga::ShaderStage::Vertex, "nsh_vertex"),
            (naga::ShaderStage::Fragment, "nsh_fragment"),
        ] {
            let pipeline = glsl::PipelineOptions {
                shader_stage: stage,
                entry_point: entry.into(),
                multiview: None,
            };
            let mut out = String::new();
            let mut writer =
                glsl::Writer::new(&mut out, &module, &info, &options, &pipeline, crate::BOUNDS)
                    .map_err(|e| internal(Error::new(e.to_string())))?;
            let reflection = writer
                .write()
                .map_err(|e| internal(Error::new(e.to_string())))?;
            for (global, name) in reflection.uniforms {
                let buffer = match module.global_variables[global].binding {
                    Some(naga::ResourceBinding { group: 0, binding }) => binding,
                    _ => continue,
                };
                program.blocks.push(GlslBlock { name, buffer });
            }
            if stage == naga::ShaderStage::Fragment {
                for (name, mapping) in reflection.texture_mapping {
                    let binding = module.global_variables[mapping.texture]
                        .binding
                        .map_or(0, |b| b.binding);
                    if let Some(input) = Input::from_index(binding as usize - 1) {
                        program.samplers.push((name, input));
                    }
                }
                program.samplers.sort_by_key(|(_, input)| input.index());
                program.fragment = out;
            } else {
                program.vertex = out;
            }
        }
        program
            .blocks
            .sort_by(|a, b| (a.buffer, &a.name).cmp(&(b.buffer, &b.name)));
        Ok(program)
    }
}
