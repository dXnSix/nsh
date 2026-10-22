//! Metal (iOS, macOS): naga's MSL writer over the draw glue.

use naga::back::msl;

use crate::{Error, Input, Shader, glue::Glue, internal};

/// A shader as Metal source: a vertex and a fragment function to draw a triangle strip of four
/// vertices over the frame's area.
///
/// Both functions read the `nsh` buffer at index [`MslProgram::NSH`], the frame at
/// [`MslProgram::FRAME`] and the uniforms at [`MslProgram::UNIFORMS`]; the fragment function the
/// textures of `textures` at their indexes, with a sampler of its own (linear, clamped to zero).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct MslProgram {
    /// The source, for `MTLDevice.newLibraryWithSource`.
    pub source: String,
    /// The vertex function's name.
    pub vertex: String,
    /// The fragment function's name.
    pub fragment: String,
    /// The inputs the fragment function samples, at texture indexes 0, 1, ...
    pub textures: Vec<Input>,
}

impl MslProgram {
    /// The buffer index of `nsh`.
    pub const NSH: u8 = 0;
    /// The buffer index of the frame.
    pub const FRAME: u8 = 1;
    /// The buffer index of the uniforms.
    pub const UNIFORMS: u8 = 2;
}

impl Shader {
    /// The shader in the Metal Shading Language.
    pub fn to_msl(&self) -> Result<MslProgram, Error> {
        let (module, info) = self.module(Glue::Draw)?;
        let textures = self.inputs();
        let mut resources = msl::BindingMap::default();
        let buffer = |slot| msl::BindTarget {
            buffer: Some(slot),
            ..Default::default()
        };
        let binding = |group, binding| naga::ResourceBinding { group, binding };
        resources.insert(binding(0, 0), buffer(MslProgram::NSH));
        resources.insert(binding(0, 1), buffer(MslProgram::FRAME));
        resources.insert(binding(0, 2), buffer(MslProgram::UNIFORMS));
        resources.insert(
            binding(1, 0),
            msl::BindTarget {
                sampler: Some(msl::BindSamplerTarget::Inline(0)),
                ..Default::default()
            },
        );
        for (slot, input) in textures.iter().enumerate() {
            resources.insert(
                binding(1, input.index() as u32 + 1),
                msl::BindTarget {
                    texture: Some(slot as u8),
                    ..Default::default()
                },
            );
        }
        let entry = msl::EntryPointResources {
            resources,
            immediates_buffer: None,
            sizes_buffer: None,
        };
        let mut options = msl::Options {
            lang_version: (2, 1),
            inline_samplers: vec![msl::sampler::InlineSampler {
                coord: msl::sampler::Coord::Normalized,
                address: [msl::sampler::Address::ClampToZero; 3],
                border_color: msl::sampler::BorderColor::TransparentBlack,
                mag_filter: msl::sampler::Filter::Linear,
                min_filter: msl::sampler::Filter::Linear,
                mip_filter: None,
                lod_clamp: None,
                max_anisotropy: None,
                compare_func: msl::sampler::CompareFunc::Never,
            }],
            fake_missing_bindings: false,
            bounds_check_policies: crate::BOUNDS,
            // The profile's loops are bounded.
            force_loop_bounding: false,
            ..Default::default()
        };
        options
            .per_entry_point_map
            .insert("nsh_vertex".into(), entry.clone());
        options
            .per_entry_point_map
            .insert("nsh_fragment".into(), entry);
        let pipeline = msl::PipelineOptions::default();
        let (source, translation) = msl::write_string(&module, &info, &options, &pipeline)
            .map_err(|e| internal(Error::new(e.to_string())))?;
        let name = |i: usize| -> Result<String, Error> {
            match translation.entry_point_names.get(i) {
                Some(Ok(name)) => Ok(name.clone()),
                Some(Err(e)) => Err(internal(Error::new(e.to_string()))),
                None => Err(internal(Error::new("an entry point is missing"))),
            }
        };
        let index = |n: &str| {
            module
                .entry_points
                .iter()
                .position(|e| e.name == n)
                .unwrap_or_default()
        };
        Ok(MslProgram {
            vertex: name(index("nsh_vertex"))?,
            fragment: name(index("nsh_fragment"))?,
            source,
            textures,
        })
    }
}
