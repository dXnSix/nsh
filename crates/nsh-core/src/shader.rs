//! Shaders (`<Shader>`, `createShader`, `.wgsl` imports): programs apps
//! write in WGSL, checked and translated by `nsh-shader`
//! (`docs/dev/shaders.md`).
//!
//! A [`ShaderSource`] is a checked shader with an id unique in its engine.
//! A shader view's props are its source and its uniforms' values
//! ([`UniformValue`], by name); at commit the core packs them into the
//! uniform buffer the program reads ([`pack`]: WGSL's layout, colors
//! premultiplied, images by slot) and binds the animated ones to the
//! buffer's components ([`crate::animation::AnimationProp::ShaderUniform`]),
//! so animations change them on the UI thread without JS. The program, in
//! the platform's language ([`ShaderLanguage`]), is translated on the
//! engine thread the first time a view draws it and sent once
//! ([`crate::MountItem::ShaderProgram`]); each view then gets what it draws
//! ([`ShaderFill`], [`crate::MountItem::Shader`]) when that changes.

use std::sync::Arc;

#[cfg(feature = "shader-hlsl")]
pub use nsh_shader::HlslProgram;
#[cfg(feature = "shader-msl")]
pub use nsh_shader::MslProgram;
#[cfg(feature = "shader-agsl")]
pub use nsh_shader::{AgslProgram, AgslUniform, AgslValue};
pub use nsh_shader::{
    Error as ShaderError, Input as ShaderInput, Limits as ShaderLimits, Reflection, Shader,
    Uniform, UniformType, compile, layout,
};
#[cfg(feature = "shader-glsl")]
pub use nsh_shader::{GlslBlock, GlslProgram, GlslVersion};

use crate::{
    ImageSource,
    animation::{AnimationValue, NodeId as AnimationNode},
    geometry::{Bounds, DevicePixels, Percentage, Point, Points, Size},
    js::host::{HostCx, HostInput, HostValue},
    style::Color,
    ui::TsTypes,
};

/// A checked shader an app made (`createShader(source)`, a `.wgsl`
/// import), with an id unique in its engine: the same source has the same
/// id.
#[derive(Debug, Clone)]
pub struct ShaderSource {
    /// Its id.
    pub id: u32,
    /// The shader.
    pub shader: Arc<Shader>,
}

impl ShaderSource {
    /// Checks `source` (WGSL) against NSH's profile and `limits`
    /// ([`compile`]), with a new id. Rust components make their shaders
    /// once and keep them: each call compiles again.
    pub fn new(source: &str, limits: &ShaderLimits) -> Result<Self, ShaderError> {
        Ok(Self::with_id(
            Self::next_id(),
            Arc::new(compile(source, limits)?),
        ))
    }

    /// `shader` with `id` ([`Self::next_id`]).
    pub fn with_id(id: u32, shader: Arc<Shader>) -> Self {
        Self { id, shader }
    }

    /// An id no other shader of the process has: programs are kept by id.
    pub fn next_id() -> u32 {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

impl PartialEq for ShaderSource {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// What `createShader` returned.
impl HostValue for ShaderSource {
    fn decode<I: HostInput>(input: &I, _: &mut HostCx) -> Option<Self> {
        input.shader()
    }

    fn ts_type(_: &mut TsTypes) -> String {
        "ShaderAsset<any>".into()
    }
}

/// The language a platform runs shaders in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ShaderLanguage {
    /// None: shader views draw their children only.
    #[default]
    None,
    /// Metal (iOS, macOS).
    Msl,
    /// OpenGL ES 3.00 / WebGL 2 (the web, GTK's GLES contexts).
    GlslEs300,
    /// OpenGL 3.30 core (GTK's desktop contexts).
    Glsl330,
    /// HLSL for Direct2D and Direct3D 11 (Windows).
    Hlsl,
    /// AGSL (Android 13 and later).
    Agsl,
}

/// How a platform runs shaders of one kind (`shaderSupport()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Support {
    /// It does not: the element draws its children only (a transition
    /// runs its fallback).
    #[default]
    No,
    /// From a picture of the input, taken again when it changes.
    Snapshot,
    /// On the live input, every frame.
    Live,
}

impl Support {
    /// What `shaderSupport()` says: `"live"`, `"snapshot"` or `false`.
    pub const fn js_name(self) -> Option<&'static str> {
        match self {
            Support::No => None,
            Support::Snapshot => Some("snapshot"),
            Support::Live => Some("live"),
        }
    }
}

/// What shaders a platform runs, and in which language
/// ([`crate::platform::Environment::shaders`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ShaderSupport {
    /// The language its programs are in.
    pub language: ShaderLanguage,
    /// `<Shader>` filling a view.
    pub fill: Support,
    /// `<Shader>` reading what is behind it (`sample_backdrop`).
    pub backdrop: Support,
    /// `<ShaderEffect>`: children drawn through a shader.
    pub effect: Support,
    /// Shader transitions.
    pub transition: Support,
}

impl ShaderSupport {
    /// Fills drawn live in `language`.
    pub const fn fills(language: ShaderLanguage) -> Self {
        Self {
            language,
            fill: Support::Live,
            backdrop: Support::No,
            effect: Support::No,
            transition: Support::No,
        }
    }
}

/// A program in a platform's language.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProgramCode {
    /// No language: nothing draws it (a headless engine).
    None,
    /// Metal.
    #[cfg(feature = "shader-msl")]
    Msl(MslProgram),
    /// GLSL.
    #[cfg(feature = "shader-glsl")]
    Glsl(GlslProgram),
    /// HLSL.
    #[cfg(feature = "shader-hlsl")]
    Hlsl(HlslProgram),
    /// AGSL.
    #[cfg(feature = "shader-agsl")]
    Agsl(AgslProgram),
}

/// A shader's program as the platform runs it, sent once per engine
/// ([`crate::MountItem::ShaderProgram`]); views name it by `id`.
#[derive(Debug)]
#[non_exhaustive]
pub struct ShaderProgram {
    /// The shader's id ([`ShaderSource::id`]).
    pub id: u32,
    /// The code.
    pub code: ProgramCode,
    /// What it reads.
    pub reflection: Reflection,
}

impl ShaderProgram {
    /// Translates `source` into `language`. An error is NSH's (the shader
    /// was checked when it was made) and leaves the views drawing their
    /// children only.
    pub fn translate(source: &ShaderSource, language: ShaderLanguage) -> Result<Self, ShaderError> {
        let shader = &source.shader;
        let code = match language {
            #[cfg(feature = "shader-msl")]
            ShaderLanguage::Msl => ProgramCode::Msl(shader.to_msl()?),
            #[cfg(feature = "shader-glsl")]
            ShaderLanguage::GlslEs300 => ProgramCode::Glsl(shader.to_glsl(GlslVersion::Es300)?),
            #[cfg(feature = "shader-glsl")]
            ShaderLanguage::Glsl330 => ProgramCode::Glsl(shader.to_glsl(GlslVersion::Core330)?),
            #[cfg(feature = "shader-hlsl")]
            ShaderLanguage::Hlsl => ProgramCode::Hlsl(shader.to_hlsl()?),
            #[cfg(feature = "shader-agsl")]
            ShaderLanguage::Agsl => ProgramCode::Agsl(shader.to_agsl()?),
            _ => ProgramCode::None,
        };
        Ok(Self {
            id: source.id,
            code,
            reflection: shader.reflection().clone(),
        })
    }
}

/// The value of a uniform as the app gives it: a number, a list of them
/// (vectors, matrices column by column, arrays), a color, an animated
/// value, or an image.
#[derive(Debug, Clone, PartialEq)]
pub enum UniformValue {
    /// A number.
    Number(f32),
    /// A CSS color: a vector of 4 is its premultiplied RGBA, of 3 its
    /// premultiplied RGB.
    Color(Color),
    /// An animated value (a number or a color), bound to the buffer.
    Animated(AnimationNode),
    /// A list: a vector's or matrix's components, an array's elements.
    List(Vec<UniformValue>),
    /// An image (an `Image` uniform).
    Image(ImageSource),
}

/// The `uniforms` prop: values by name, in the order the app wrote them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Uniforms(pub Vec<(Arc<str>, UniformValue)>);

impl Uniforms {
    /// No values: every uniform is 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// `name` set to `value` (a number, a color, an animated value, an
    /// image, a list of numbers).
    pub fn set(mut self, name: &str, value: impl Into<UniformValue>) -> Self {
        self.0.push((Arc::from(name), value.into()));
        self
    }
}

impl From<f32> for UniformValue {
    fn from(n: f32) -> Self {
        UniformValue::Number(n)
    }
}

impl<const N: usize> From<[f32; N]> for UniformValue {
    fn from(list: [f32; N]) -> Self {
        UniformValue::List(list.into_iter().map(UniformValue::Number).collect())
    }
}

impl From<crate::color::Hsla> for UniformValue {
    fn from(color: crate::color::Hsla) -> Self {
        UniformValue::Color(color.into())
    }
}

impl From<crate::color::Rgba> for UniformValue {
    fn from(color: crate::color::Rgba) -> Self {
        UniformValue::Color(color.into())
    }
}

impl From<ImageSource> for UniformValue {
    fn from(image: ImageSource) -> Self {
        UniformValue::Image(image)
    }
}

/// The value as it moves, written on the UI thread; the app keeps the
/// value while the shader reads it.
impl<A: crate::ui::Animated> From<&A> for UniformValue {
    fn from(a: &A) -> Self {
        UniformValue::Animated(a.animated_node().id())
    }
}

impl UniformValue {
    fn decode_value<I: HostInput>(input: &I, cx: &mut HostCx) -> Option<Self> {
        if let Some(n) = input.as_number() {
            return Some(UniformValue::Number(n as f32));
        }
        // A string is a color, else an image's URI (`pack` warns about
        // one a uniform does not take).
        if let Some(s) = input.as_string() {
            return match crate::css::color(&s) {
                Some(c) => Some(UniformValue::Color(c)),
                None => ImageSource::decode(input, cx).map(UniformValue::Image),
            };
        }
        if let Some(node) = input.animated_node() {
            return Some(UniformValue::Animated(node));
        }
        if let Some(list) = input.as_list() {
            use crate::js::host::HostList;
            let mut out = Vec::with_capacity(list.len());
            for i in 0..list.len() {
                out.push(
                    list.item(i)
                        .and_then(|v| Self::decode_value(&v, cx))
                        .unwrap_or(UniformValue::Number(0.0)),
                );
            }
            return Some(UniformValue::List(out));
        }
        if !input.is_object() {
            return None;
        }
        // A point or a size is a vector of two.
        for (a, b) in [("x", "y"), ("width", "height")] {
            if let (Some(x), Some(y)) = (input.field(a), input.field(b))
                && !x.is_nullish()
                && !y.is_nullish()
            {
                let x = Self::decode_value(&x, cx).unwrap_or(UniformValue::Number(0.0));
                let y = Self::decode_value(&y, cx).unwrap_or(UniformValue::Number(0.0));
                return Some(UniformValue::List(vec![x, y]));
            }
        }
        ImageSource::decode(input, cx).map(UniformValue::Image)
    }
}

/// `{ name: value }`.
impl HostValue for Uniforms {
    fn decode<I: HostInput>(input: &I, cx: &mut HostCx) -> Option<Self> {
        if !input.is_object() || input.as_list().is_some() {
            return None;
        }
        let mut out = Vec::new();
        for (name, value) in input.entries() {
            if value.is_nullish() {
                continue;
            }
            match UniformValue::decode_value(&value, cx) {
                Some(v) => out.push((Arc::from(name.as_str()), v)),
                None => cx.warn(format!(
                    "uniforms: `{name}` has a value a shader does not take"
                )),
            }
        }
        Some(Uniforms(out))
    }

    fn ts_type(types: &mut TsTypes) -> String {
        types.declare("ShaderUniforms", |_| {
            "/** The values of a shader's uniforms, by name (`ShaderAsset`'s uniforms, typed per `.wgsl` file). */\n\
             export type ShaderUniforms = Record<string, ShaderUniformValue>;"
                .into()
        });
        "ShaderUniforms".into()
    }
}

/// The component [`crate::animation::AnimationProp::ShaderUniform`]
/// names for a shader's `nsh.progress` (its `progress` prop, animated).
pub const PROGRESS: u16 = u16::MAX;

/// A number the shader reads: a constant, or an animated value the UI
/// thread writes (`progress`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShaderNumber {
    /// A constant.
    Number(f32),
    /// An animated value.
    Animated(AnimationNode),
}

impl Default for ShaderNumber {
    fn default() -> Self {
        ShaderNumber::Number(0.0)
    }
}

impl ShaderNumber {
    /// Its value now: the constant, or 0 until the UI thread writes it.
    pub fn constant(self) -> f32 {
        match self {
            ShaderNumber::Number(n) => n,
            ShaderNumber::Animated(_) => 0.0,
        }
    }
}

impl From<f32> for ShaderNumber {
    fn from(n: f32) -> Self {
        ShaderNumber::Number(n)
    }
}

/// The value as it moves, written on the UI thread; the app keeps the
/// value while the shader reads it.
impl<A: crate::ui::Animated> From<&A> for ShaderNumber {
    fn from(a: &A) -> Self {
        ShaderNumber::Animated(a.animated_node().id())
    }
}

/// A number or an animated value (TS: `ShaderNumber`).
impl HostValue for ShaderNumber {
    fn decode<I: HostInput>(input: &I, _: &mut HostCx) -> Option<Self> {
        if let Some(n) = input.as_number() {
            return Some(ShaderNumber::Number(n as f32));
        }
        input.animated_node().map(ShaderNumber::Animated)
    }

    fn ts_type(_: &mut TsTypes) -> String {
        "ShaderNumber".into()
    }
}

/// An image a shader samples.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct ShaderImage {
    /// What it shows; `None` samples transparent.
    pub source: Option<ImageSource>,
    /// Where its `Image` uniform is in the buffer: the platform writes the
    /// image's size in points there once it loads (and the core already
    /// did when the source says it).
    pub offset: u32,
}

/// What a shader view draws ([`crate::MountItem::Shader`]).
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct ShaderFill {
    /// The program ([`ShaderProgram::id`]).
    pub program: u32,
    /// The uniforms' buffer, in WGSL's uniform layout
    /// ([`Reflection::uniforms`]).
    pub uniforms: Arc<[u8]>,
    /// The images, by slot.
    pub images: Vec<ShaderImage>,
    /// It reads `nsh.time`: it draws every frame while it shows.
    pub time: bool,
    /// The inputs it samples, bit [`ShaderInput::index`] each: its
    /// content (an effect's children) and what is behind it.
    pub inputs: u32,
    /// `nsh.progress`, until an animated value writes it.
    pub progress: f32,
    /// How far outside a pixel it samples its content (an effect's
    /// `maxSampleOffset`): the content is drawn that far beyond the view.
    pub max_offset: Points,
}

impl ShaderFill {
    /// It samples input `input`.
    pub fn samples(&self, input: ShaderInput) -> bool {
        self.inputs & (1 << input.index()) != 0
    }
}

/// The uniforms packed for a program: the buffer, the components animated
/// values write ([`crate::animation::AnimationProp::ShaderUniform`]) and
/// the images.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Packed {
    /// The buffer.
    pub bytes: Vec<u8>,
    /// Animated values and the component (`f32` index) they write from.
    pub bindings: Vec<(u16, AnimationNode)>,
    /// The images, by slot.
    pub images: Vec<ShaderImage>,
}

/// Packs `values` for a shader with `reflection`, warning about names it
/// does not have and values its uniforms do not take.
pub fn pack(reflection: &Reflection, values: &Uniforms, warn: &mut dyn FnMut(String)) -> Packed {
    let mut out = Packed {
        bytes: vec![0; reflection.uniforms_size as usize],
        ..Default::default()
    };
    for (name, _) in &values.0 {
        if reflection.uniform(name).is_none() {
            let names: Vec<&str> = reflection
                .uniforms
                .iter()
                .map(|u| u.name.as_str())
                .collect();
            warn(format!(
                "the shader has no uniform `{name}` (it has: {})",
                names.join(", ")
            ));
        }
    }
    for uniform in &reflection.uniforms {
        if let UniformType::Image(slot) = uniform.ty {
            let slot = slot as usize;
            if out.images.len() <= slot {
                out.images.resize(slot + 1, ShaderImage::default());
            }
            out.images[slot].offset = uniform.offset;
            // `Image.slot.x`.
            write_i32(&mut out.bytes, uniform.offset + 16, slot as i32);
        }
        let Some(value) = values
            .0
            .iter()
            .rev()
            .find(|(n, _)| **n == *uniform.name)
            .map(|(_, v)| v)
        else {
            continue;
        };
        match uniform.count {
            None => write_value(
                &mut out,
                uniform.ty,
                uniform.offset,
                value,
                &uniform.name,
                warn,
            ),
            Some(count) => {
                let UniformValue::List(items) = value else {
                    warn(format!("uniform `{}` is an array of {count}", uniform.name));
                    continue;
                };
                // An array of scalars may come flat: [a, b, ...] for array<vec4f, n> fills elements in turn.
                let per = uniform.ty.components() as usize;
                let elements: Vec<UniformValue> = if per > 1
                    && items
                        .iter()
                        .all(|v| matches!(v, UniformValue::Number(_) | UniformValue::Animated(_)))
                    && items.len() > count as usize
                {
                    items
                        .chunks(per)
                        .map(|c| UniformValue::List(c.to_vec()))
                        .collect()
                } else {
                    items.clone()
                };
                for (i, item) in elements.iter().take(count as usize).enumerate() {
                    write_value(
                        &mut out,
                        uniform.ty,
                        uniform.offset + uniform.stride * i as u32,
                        item,
                        &uniform.name,
                        warn,
                    );
                }
            }
        }
    }
    out
}

fn write_value(
    out: &mut Packed,
    ty: UniformType,
    offset: u32,
    value: &UniformValue,
    name: &str,
    warn: &mut dyn FnMut(String),
) {
    let wrong =
        |warn: &mut dyn FnMut(String)| warn(format!("uniform `{name}` is a `{}`", ty.wgsl()));
    match ty {
        UniformType::Image(slot) => match value {
            UniformValue::Image(source) => {
                if let Some(size) = source.size {
                    write_f32(&mut out.bytes, offset, size.width.0);
                    write_f32(&mut out.bytes, offset + 4, size.height.0);
                }
                out.images[slot as usize].source = Some(source.clone());
            }
            _ => wrong(warn),
        },
        UniformType::Mat2 | UniformType::Mat3 | UniformType::Mat4 => {
            let (columns, stride) = ty.columns().unwrap_or((2, 8));
            let UniformValue::List(items) = value else {
                return wrong(warn);
            };
            for (i, item) in items.iter().take((columns * columns) as usize).enumerate() {
                let (c, r) = (i as u32 / columns, i as u32 % columns);
                write_component(out, offset + c * stride + r * 4, false, item, name, warn);
            }
        }
        _ => {
            let n = ty.components();
            let int = ty.is_int();
            match value {
                UniformValue::Color(c) if n >= 3 && !int => {
                    let rgba = premultiplied(*c);
                    for (i, v) in rgba.iter().take(n as usize).enumerate() {
                        write_f32(&mut out.bytes, offset + 4 * i as u32, *v);
                    }
                }
                UniformValue::List(items) => {
                    for (i, item) in items.iter().take(n as usize).enumerate() {
                        write_component(out, offset + 4 * i as u32, int, item, name, warn);
                    }
                }
                // A number fills a vector (as WGSL's `vec4f(x)`).
                UniformValue::Number(v) => {
                    for i in 0..n {
                        write_number(&mut out.bytes, offset + 4 * i, int, *v);
                    }
                }
                UniformValue::Animated(node) if !int && (n == 1 || n == 4) => {
                    out.bindings.push(((offset / 4) as u16, *node));
                }
                // A string that is no color.
                UniformValue::Image(source) if n >= 3 && !int => {
                    warn(format!("uniform `{name}`: `{}` is not a color", source.uri))
                }
                _ => wrong(warn),
            }
        }
    }
}

fn write_component(
    out: &mut Packed,
    offset: u32,
    int: bool,
    value: &UniformValue,
    name: &str,
    warn: &mut dyn FnMut(String),
) {
    match value {
        UniformValue::Number(v) => write_number(&mut out.bytes, offset, int, *v),
        UniformValue::Animated(node) if !int => out.bindings.push(((offset / 4) as u16, *node)),
        _ => warn(format!("uniform `{name}`: a component is a number")),
    }
}

fn write_number(bytes: &mut [u8], offset: u32, int: bool, v: f32) {
    if int {
        write_i32(bytes, offset, v.round() as i32);
    } else {
        write_f32(bytes, offset, v);
    }
}

fn write_f32(bytes: &mut [u8], offset: u32, v: f32) {
    if let Some(slot) = bytes.get_mut(offset as usize..offset as usize + 4) {
        slot.copy_from_slice(&v.to_le_bytes());
    }
}

fn write_i32(bytes: &mut [u8], offset: u32, v: i32) {
    if let Some(slot) = bytes.get_mut(offset as usize..offset as usize + 4) {
        slot.copy_from_slice(&v.to_le_bytes());
    }
}

/// A color as a shader reads it: premultiplied sRGB, `0..=1`.
pub fn premultiplied(c: Color) -> [f32; 4] {
    let [r, g, b, a] = c.0.to_be_bytes().map(|v| v as f32 / 255.0);
    [r * a, g * a, b * a, a]
}

/// Writes an animated value into a uniforms' buffer at `component` (an
/// `f32` index; [`Packed::bindings`]): a number there, a color as its
/// four premultiplied components from there.
pub fn write_animated(bytes: &mut [u8], component: u16, value: AnimationValue) {
    let offset = component as u32 * 4;
    match value {
        AnimationValue::Number(n) => write_f32(bytes, offset, n),
        AnimationValue::Color(c) => {
            for (i, v) in premultiplied(Color(c)).iter().enumerate() {
                write_f32(bytes, offset + 4 * i as u32, *v);
            }
        }
    }
}

/// Writes the size of an image into a uniforms' buffer, once the
/// platform knows it.
pub fn write_image_size(bytes: &mut [u8], image: &ShaderImage, size: Size<Points>) {
    write_f32(bytes, image.offset, size.width.0);
    write_f32(bytes, image.offset + 4, size.height.0);
}

/// The `nsh` buffer ([`layout`]): the view's size, the screen's pixel
/// ratio, the time since the shader first drew (in seconds) and the
/// progress.
pub fn nsh_buffer(
    size: Size<Points>,
    scale: f32,
    time: f32,
    progress: Percentage,
) -> [u8; layout::NSH_SIZE as usize] {
    let mut out = [0u8; layout::NSH_SIZE as usize];
    for (offset, v) in [
        (layout::NSH_VIEW_SIZE, size.width.0),
        (layout::NSH_VIEW_SIZE + 4, size.height.0),
        (layout::NSH_SCALE, scale),
        (layout::NSH_TIME, time),
        (layout::NSH_PROGRESS, progress.0),
    ] {
        write_f32(&mut out, offset, v);
    }
    out
}

/// The frame buffer ([`layout`]) of a fill: it draws over the view
/// (`size`) and its images, by slot, are stretched over it, each as many
/// pixels in its own coordinates as it has (AGSL; none elsewhere).
pub fn frame_buffer(size: Size<Points>, images: &[(usize, Size<DevicePixels>)]) -> Vec<u8> {
    let view = Bounds::new(Point::default(), size);
    let inputs: Vec<_> = images
        .iter()
        .map(|&(slot, px)| (ShaderInput::Image(slot as u8), view, px))
        .collect();
    frame_buffer_of(view, &inputs)
}

/// The frame buffer ([`layout`]) of a drawing of `area` (points from the
/// view's top-left: an effect draws beyond its view) whose inputs lie at
/// their rects, each as many pixels as it has (AGSL reads them in those).
pub fn frame_buffer_of(
    area: Bounds<Points>,
    inputs: &[(ShaderInput, Bounds<Points>, Size<DevicePixels>)],
) -> Vec<u8> {
    let mut out = vec![0u8; layout::FRAME_SIZE as usize];
    let corners = |b: Bounds<Points>| [b.origin.x.0, b.origin.y.0, b.size.width.0, b.size.height.0];
    for (i, v) in corners(area).into_iter().enumerate() {
        write_f32(&mut out, layout::FRAME_AREA + 4 * i as u32, v);
    }
    for &(input, rect, px) in inputs {
        let at = layout::FRAME_INPUTS + layout::INPUT_STRIDE * input.index() as u32;
        for (i, v) in corners(rect).into_iter().enumerate() {
            write_f32(&mut out, at + layout::INPUT_RECT + 4 * i as u32, v);
        }
        write_f32(&mut out, at + layout::INPUT_PX, px.width.0 as f32);
        write_f32(&mut out, at + layout::INPUT_PX + 4, px.height.0 as f32);
    }
    out
}

/// Writes animated values ([`crate::animation::AnimationProp::ShaderUniform`])
/// into a uniforms' buffer, or `progress` for [`PROGRESS`].
pub fn write_args(bytes: &mut [u8], progress: &mut f32, values: &[(u16, AnimationValue)]) {
    for &(k, value) in values {
        if k == PROGRESS {
            *progress = value.number();
        } else {
            write_animated(bytes, k, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shader(source: &str) -> Shader {
        compile(source, &ShaderLimits::default()).unwrap()
    }

    fn f32_at(bytes: &[u8], component: usize) -> f32 {
        f32::from_le_bytes(bytes[component * 4..component * 4 + 4].try_into().unwrap())
    }

    fn values(values: Vec<(&str, UniformValue)>) -> Uniforms {
        Uniforms(values.into_iter().map(|(n, v)| (Arc::from(n), v)).collect())
    }

    #[test]
    fn uniforms_are_packed_in_wgsl_layout() {
        let s = shader(
            "struct U { a: f32, b: vec3f, c: vec2i, m: mat2x2f, k: array<vec4f, 2> }
             fn main(p: vec2f, u: U) -> vec4f { return vec4f(u.a + u.b.x + f32(u.c.x) + u.m[0].x + u.k[1].y); }",
        );
        let n = UniformValue::Number;
        let mut warnings = Vec::new();
        let packed = pack(
            s.reflection(),
            &values(vec![
                ("a", n(1.0)),
                // A color fills a vec3f with its premultiplied RGB.
                ("b", UniformValue::Color(Color(0xff00_0080))),
                // Integers are rounded.
                ("c", UniformValue::List(vec![n(2.4), n(-3.6)])),
                (
                    "m",
                    UniformValue::List(vec![n(1.0), n(2.0), n(3.0), n(4.0)]),
                ),
                // An array of vectors may come flat.
                (
                    "k",
                    UniformValue::List((1..=8).map(|i| n(i as f32)).collect()),
                ),
                ("nope", n(1.0)),
            ]),
            &mut |w| warnings.push(w),
        );
        // a: 0, b: 16 (vec3f aligns to 16), c: 32, m: 40, k: 64 (arrays
        // align to 16).
        let b = &packed.bytes;
        assert_eq!(f32_at(b, 0), 1.0);
        let a = 128.0 / 255.0;
        assert!((f32_at(b, 4) - a).abs() < 1e-6 && f32_at(b, 5) == 0.0 && f32_at(b, 6) == 0.0);
        assert_eq!(i32::from_le_bytes(b[32..36].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(b[36..40].try_into().unwrap()), -4);
        assert_eq!(
            [f32_at(b, 10), f32_at(b, 11), f32_at(b, 12), f32_at(b, 13)],
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            (16..24).map(|i| f32_at(b, i)).collect::<Vec<_>>(),
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("no uniform `nope`"), "{warnings:?}");
    }

    #[test]
    fn rust_sets_uniforms_by_name() {
        let s = shader(
            "struct U { a: f32, b: vec4f, c: vec2f }
             fn main(p: vec2f, u: U) -> vec4f { return u.b * u.a + vec4f(u.c, 0.0, 0.0); }",
        );
        let values = Uniforms::new()
            .set("a", 0.5)
            .set("b", crate::color::rgb(0x0000ff))
            .set("c", [3.0, 4.0]);
        let packed = pack(s.reflection(), &values, &mut |w| panic!("{w}"));
        let b = &packed.bytes;
        assert_eq!(f32_at(b, 0), 0.5);
        assert_eq!(
            [f32_at(b, 4), f32_at(b, 5), f32_at(b, 6), f32_at(b, 7)],
            [0.0, 0.0, 1.0, 1.0]
        );
        assert_eq!([f32_at(b, 8), f32_at(b, 9)], [3.0, 4.0]);
    }

    #[test]
    fn animated_values_are_bound_and_wrong_values_warned_about() {
        let s = shader(
            "struct U { r: f32, tint: vec4f, flag: i32 }
             fn main(p: vec2f, u: U) -> vec4f { return u.tint * u.r * f32(u.flag); }",
        );
        let node: AnimationNode = 7;
        let mut warnings = Vec::new();
        let packed = pack(
            s.reflection(),
            &values(vec![
                ("r", UniformValue::Animated(node)),
                ("tint", UniformValue::Animated(node)),
                // An integer is never animated.
                ("flag", UniformValue::Animated(node)),
            ]),
            &mut |w| warnings.push(w),
        );
        assert_eq!(packed.bindings, [(0, node), (4, node)]);
        assert_eq!(warnings, ["uniform `flag` is a `i32`"]);
        let mut bytes = packed.bytes.clone();
        write_animated(&mut bytes, 0, AnimationValue::Number(2.5));
        write_animated(&mut bytes, 4, AnimationValue::Color(0x0000_ffff));
        assert_eq!(
            [
                f32_at(&bytes, 0),
                f32_at(&bytes, 4),
                f32_at(&bytes, 6),
                f32_at(&bytes, 7)
            ],
            [2.5, 0.0, 1.0, 1.0]
        );
    }

    #[test]
    fn images_have_slots_and_their_known_sizes() {
        let s = shader(
            "struct U { a: Image, b: Image }
             fn main(p: vec2f, u: U) -> vec4f { return sample_image(u.a, p) + sample_image(u.b, p); }",
        );
        let source = ImageSource {
            uri: "asset:/a.png".into(),
            size: Some(Size::new(Points(20.0), Points(10.0))),
            scales: Default::default(),
        };
        let packed = pack(
            s.reflection(),
            &values(vec![("a", UniformValue::Image(source.clone()))]),
            &mut |w| panic!("{w}"),
        );
        assert_eq!(packed.images.len(), 2);
        assert_eq!(packed.images[0].source, Some(source));
        assert_eq!(packed.images[1].source, None);
        let (a, b) = (
            packed.images[0].offset as usize,
            packed.images[1].offset as usize,
        );
        // `Image { size: vec2f, slot: vec4i }`: the size the source knows,
        // the slot.
        assert_eq!(
            [
                f32_at(&packed.bytes, a / 4),
                f32_at(&packed.bytes, a / 4 + 1)
            ],
            [20.0, 10.0]
        );
        assert_eq!(
            i32::from_le_bytes(packed.bytes[b + 16..b + 20].try_into().unwrap()),
            1
        );
        // A size the platform learns later.
        let mut bytes = packed.bytes.clone();
        write_image_size(
            &mut bytes,
            &packed.images[1],
            Size::new(Points(3.0), Points(4.0)),
        );
        assert_eq!(
            [f32_at(&bytes, b / 4), f32_at(&bytes, b / 4 + 1)],
            [3.0, 4.0]
        );
    }

    #[test]
    fn the_nsh_and_frame_buffers_hold_the_view_and_its_inputs() {
        let nsh = nsh_buffer(
            Size::new(Points(100.0), Points(50.0)),
            2.0,
            1.5,
            Percentage(0.25),
        );
        for (offset, v) in [
            (layout::NSH_VIEW_SIZE, 100.0),
            (layout::NSH_VIEW_SIZE + 4, 50.0),
            (layout::NSH_SCALE, 2.0),
            (layout::NSH_TIME, 1.5),
            (layout::NSH_PROGRESS, 0.25),
        ] {
            assert_eq!(f32_at(&nsh, offset as usize / 4), v);
        }
        let frame = frame_buffer(
            Size::new(Points(100.0), Points(50.0)),
            &[(1, Size::new(DevicePixels(64), DevicePixels(32)))],
        );
        assert_eq!(f32_at(&frame, layout::FRAME_AREA as usize / 4 + 2), 100.0);
        let input = (layout::FRAME_INPUTS
            + layout::INPUT_STRIDE * ShaderInput::Image(1).index() as u32)
            as usize;
        assert_eq!(
            f32_at(&frame, (input + layout::INPUT_RECT as usize) / 4 + 3),
            50.0
        );
        assert_eq!(
            f32_at(&frame, (input + layout::INPUT_PX as usize) / 4),
            64.0
        );
    }
}
