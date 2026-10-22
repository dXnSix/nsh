//! NSH's shaders: WGSL in NSH's profile, checked, reflected and translated for each platform.
//!
//! An app writes a fragment function in WGSL:
//!
//! ```wgsl
//! struct Uniforms { tint: vec4f, radius: f32 }
//!
//! fn main(p: vec2f, u: Uniforms) -> vec4f {
//!     let d = length(p - nsh.size * 0.5);
//!     return u.tint * (1.0 - smoothstep(u.radius - 1.0, u.radius, d));
//! }
//! ```
//!
//! `p` is the pixel's position in points from the view's top-left, the result a premultiplied sRGB
//! color. NSH's prelude gives the shader `nsh` (`size`, `scale`, `time`, `progress`), its inputs
//! (`sample_content`, `sample_backdrop`, `sample_from`, `sample_to`, each taking a position in
//! points like `p`) and `sample_image(u.field, p)` for the images among its uniforms (fields of type
//! `Image`, stretched over the view; `u.field.size` is the image's size in points).
//!
//! [`compile`] parses the source with naga, checks it against NSH's profile (the WGSL that Android's
//! AGSL can say too; `profile.rs`) and the [`Limits`], and reflects its uniforms and what it reads
//! ([`Reflection`]) (`docs/dev/shaders.md`).

mod error;
mod glue;
mod profile;
mod reflect;

pub use error::Error;
use naga::{Module, valid::ModuleInfo};
pub use reflect::{Reflection, Uniform, UniformType};

use crate::glue::{Glue, Spec};

/// What a shader samples besides its uniforms, in the order of the frame buffer's `inputs`
/// ([`layout`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Input {
    /// What the element draws, under an effect (`sample_content`).
    Content,
    /// What is behind the element (`sample_backdrop`).
    Backdrop,
    /// What a transition leaves (`sample_from`).
    From,
    /// What a transition shows (`sample_to`).
    To,
    /// The uniforms' image of this slot (their `Image` fields in order; `sample_image`).
    Image(u8),
}

impl Input {
    /// Its index among a frame's inputs.
    pub const fn index(self) -> usize {
        match self {
            Self::Content => 0,
            Self::Backdrop => 1,
            Self::From => 2,
            Self::To => 3,
            Self::Image(slot) => 4 + slot as usize,
        }
    }

    /// The input of an index among a frame's inputs.
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Content),
            1 => Some(Self::Backdrop),
            2 => Some(Self::From),
            3 => Some(Self::To),
            i if i < INPUTS => Some(Self::Image((i - 4) as u8)),
            _ => None,
        }
    }
}

const INPUTS: usize = 4 + layout::MAX_IMAGES;

/// The buffers NSH fills for every shader (byte offsets; all values `f32`).
///
/// A program reads three uniform buffers: `nsh` (what the app's code reads), the frame (where the
/// program draws and where its inputs are) and the app's uniforms, laid out as WGSL lays out their
/// struct in the uniform address space ([`Reflection::uniforms`]).
pub mod layout {
    /// Images a shader takes at most.
    pub const MAX_IMAGES: usize = 8;

    /// The size of the `nsh` buffer.
    pub const NSH_SIZE: u32 = 32;
    /// `nsh.size`: the view's width and height in points.
    pub const NSH_VIEW_SIZE: u32 = 0;
    /// `nsh.scale`: pixels per point.
    pub const NSH_SCALE: u32 = 8;
    /// `nsh.time`: seconds since the shader first drew.
    pub const NSH_TIME: u32 = 12;
    /// `nsh.progress`: an effect's or a transition's progress, 0 to 1.
    pub const NSH_PROGRESS: u32 = 16;

    /// The size of the frame buffer.
    pub const FRAME_SIZE: u32 = FRAME_INPUTS + INPUT_STRIDE * super::INPUTS as u32;
    /// The rect the program draws, in points from the view's top-left: x, y, width, height.
    pub const FRAME_AREA: u32 = 0;
    /// Direct2D: the pixel of the effect's output where the view's top-left is (x, y).
    pub const FRAME_ORIGIN: u32 = 16;
    /// The inputs, in [`Input::index`](super::Input::index) order.
    pub const FRAME_INPUTS: u32 = 32;
    /// The size of an input.
    pub const INPUT_STRIDE: u32 = 32;
    /// Where the input is, in points from the view's top-left: x, y, width, height. A negative
    /// height flips it (OpenGL's textures).
    pub const INPUT_RECT: u32 = 0;
    /// AGSL: the input's size in its own coordinates (pixels): width, height.
    pub const INPUT_PX: u32 = 16;
}

/// Limits a shader is held to, enforced when it is compiled (`docs/dev/sandbox.md`).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Limits {
    /// Texture samples a pixel takes at most: each `sample_*` call, times the iterations of the
    /// loops around it, the larger branch of an `if`.
    pub max_samples: u32,
    /// Loop iterations a pixel runs at most, nested loops multiplied.
    pub max_iterations: u32,
    /// The longest source, in bytes.
    pub max_source_len: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_samples: 128,
            max_iterations: 4096,
            max_source_len: 64 * 1024,
        }
    }
}

/// A checked shader: its source and what it reads.
#[derive(Debug)]
pub struct Shader {
    source: String,
    reflection: Reflection,
}

/// Parses `source`, checks it against NSH's profile and `limits`, and reflects it.
pub fn compile(source: &str, limits: &Limits) -> Result<Shader, Error> {
    if source.len() > limits.max_source_len {
        return Err(Error::new(format!(
            "a shader's source has up to {} bytes",
            limits.max_source_len
        )));
    }
    let module = parse(
        &glue::source(source, Glue::Signature, Spec::default()),
        source,
    )?;
    let uniforms = profile::signature(&module, source)?;
    let spec = Spec {
        uniforms: uniforms.as_deref(),
    };
    let module = parse(&glue::source(source, Glue::Check, spec), source)?;
    let info = validate(&module, source)?;
    let analysis = profile::check(&module, &info, source, limits)?;
    let reflection = reflect::reflect(&module, &analysis);
    Ok(Shader {
        source: source.to_string(),
        reflection,
    })
}

fn parse(full: &str, user: &str) -> Result<Module, Error> {
    naga::front::wgsl::parse_str(full).map_err(|error| Error::from_parse(&error, user))
}

fn validate(module: &Module, user: &str) -> Result<ModuleInfo, Error> {
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(module)
    .map_err(|error| Error::from_validation(&error, user))
}

impl Shader {
    /// The source the app wrote.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Its uniforms and what it reads.
    pub fn reflection(&self) -> &Reflection {
        &self.reflection
    }
}
