//! What a shader takes and reads: its uniforms and their layout, its inputs, its clocks.

use naga::{Module, ScalarKind, TypeInner, VectorSize};

use crate::{Input, profile::Analysis};

/// The type of a uniform (a field of the struct `main` takes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UniformType {
    /// `f32`.
    #[default]
    F32,
    /// `i32`.
    I32,
    /// `vec2f`.
    Vec2,
    /// `vec3f`.
    Vec3,
    /// `vec4f`: also a color (premultiplied sRGB).
    Vec4,
    /// `vec2i`.
    IVec2,
    /// `vec3i`.
    IVec3,
    /// `vec4i`.
    IVec4,
    /// `mat2x2f`.
    Mat2,
    /// `mat3x3f`.
    Mat3,
    /// `mat4x4f`.
    Mat4,
    /// `Image`: an image sampled by `sample_image`; the slot is its texture's.
    Image(u8),
}

impl UniformType {
    /// Its number of `f32` / `i32` values (an image: its size, two).
    pub const fn components(self) -> u32 {
        match self {
            Self::F32 | Self::I32 => 1,
            Self::Vec2 | Self::IVec2 | Self::Image(_) => 2,
            Self::Vec3 | Self::IVec3 => 3,
            Self::Vec4 | Self::IVec4 | Self::Mat2 => 4,
            Self::Mat3 => 9,
            Self::Mat4 => 16,
        }
    }

    /// Whether its values are `i32`.
    pub const fn is_int(self) -> bool {
        matches!(self, Self::I32 | Self::IVec2 | Self::IVec3 | Self::IVec4)
    }

    /// For a matrix, its columns: a column's values are laid out contiguously and columns 16 bytes
    /// apart (`mat2x2f`: 8, WGSL's layout).
    pub const fn columns(self) -> Option<(u32, u32)> {
        match self {
            Self::Mat2 => Some((2, 8)),
            Self::Mat3 => Some((3, 16)),
            Self::Mat4 => Some((4, 16)),
            _ => None,
        }
    }

    /// Its spelling in WGSL.
    pub const fn wgsl(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::I32 => "i32",
            Self::Vec2 => "vec2f",
            Self::Vec3 => "vec3f",
            Self::Vec4 => "vec4f",
            Self::IVec2 => "vec2i",
            Self::IVec3 => "vec3i",
            Self::IVec4 => "vec4i",
            Self::Mat2 => "mat2x2f",
            Self::Mat3 => "mat3x3f",
            Self::Mat4 => "mat4x4f",
            Self::Image(_) => "Image",
        }
    }
}

/// A field of the uniforms' struct.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Uniform {
    /// Its name in the source.
    pub name: String,
    /// Its type (an array's element type).
    pub ty: UniformType,
    /// Its byte offset in the uniforms' buffer.
    pub offset: u32,
    /// For an array, its length.
    pub count: Option<u32>,
    /// For an array, the bytes from one element to the next.
    pub stride: u32,
}

/// What a shader takes and reads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Reflection {
    /// The name of the struct `main` takes its uniforms in, if it takes any.
    pub uniforms_struct: Option<String>,
    /// Its fields, in order.
    pub uniforms: Vec<Uniform>,
    /// The size of the uniforms' buffer, in bytes (a multiple of 16).
    pub uniforms_size: u32,
    /// The inputs `main` samples (its images by slot).
    pub inputs: Vec<Input>,
    /// Whether `main` reads `nsh.time`: the shader draws every frame while it is shown.
    pub time: bool,
    /// Whether `main` reads `nsh.progress`.
    pub progress: bool,
    /// Texture samples a pixel takes at most.
    pub samples: u32,
    /// Loop iterations a pixel runs at most.
    pub iterations: u32,
}

impl Reflection {
    /// Whether `main` samples `input`.
    pub fn samples_input(&self, input: Input) -> bool {
        self.inputs.contains(&input)
    }

    /// A uniform by name.
    pub fn uniform(&self, name: &str) -> Option<&Uniform> {
        self.uniforms.iter().find(|u| u.name == name)
    }
}

pub(crate) fn reflect(module: &Module, analysis: &Analysis) -> Reflection {
    let mut out = Reflection {
        inputs: (0..crate::INPUTS)
            .filter(|i| analysis.inputs & (1 << i) != 0)
            .filter_map(Input::from_index)
            .collect(),
        time: analysis.time,
        progress: analysis.progress,
        samples: analysis.samples,
        iterations: analysis.iterations,
        ..Default::default()
    };
    let Some(u) = analysis.uniforms else {
        return out;
    };
    let ty = &module.types[u];
    out.uniforms_struct = ty.name.clone();
    let TypeInner::Struct { ref members, span } = ty.inner else {
        return out;
    };
    out.uniforms_size = span.div_ceil(16) * 16;
    let mut slot = 0;
    for member in members {
        let (element, count, stride) = match module.types[member.ty].inner {
            TypeInner::Array {
                base,
                size: naga::ArraySize::Constant(n),
                stride,
            } => (base, Some(n.get()), stride),
            _ => (member.ty, None, 0),
        };
        let ty = if Some(element) == analysis.image {
            slot += 1;
            UniformType::Image(slot - 1)
        } else {
            // The profile allows no other types.
            uniform_type(&module.types[element].inner).unwrap_or_default()
        };
        out.uniforms.push(Uniform {
            name: member.name.clone().unwrap_or_default(),
            ty,
            offset: member.offset,
            count,
            stride,
        });
    }
    out
}

fn uniform_type(inner: &TypeInner) -> Option<UniformType> {
    use UniformType as U;
    Some(match *inner {
        TypeInner::Scalar(s) if s.kind == ScalarKind::Float => U::F32,
        TypeInner::Scalar(_) => U::I32,
        TypeInner::Vector { size, scalar } => match (size, scalar.kind == ScalarKind::Float) {
            (VectorSize::Bi, true) => U::Vec2,
            (VectorSize::Tri, true) => U::Vec3,
            (VectorSize::Quad, true) => U::Vec4,
            (VectorSize::Bi, false) => U::IVec2,
            (VectorSize::Tri, false) => U::IVec3,
            (VectorSize::Quad, false) => U::IVec4,
        },
        TypeInner::Matrix { columns, .. } => match columns {
            VectorSize::Bi => U::Mat2,
            VectorSize::Tri => U::Mat3,
            VectorSize::Quad => U::Mat4,
        },
        _ => return None,
    })
}
