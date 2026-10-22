//! `<Shader>`: the shader drawn with Direct3D 11 into a texture of the
//! view's size, which Direct2D draws over its background and under its
//! children (`docs/dev/shaders.md` "Windows").
//!
//! A `<ShaderTransition>` draws as an effect while a transition runs
//! ([`start_transition`]): its children, drawn with the frame, are its
//! `sample_to`; before the batch that changes them they are drawn once
//! into another texture, its `sample_from` (or the running transition's
//! picture is copied there), and a frame callback moves `nsh.progress`
//! from 0 to 1.
//!
//! The inputs are live, drawn with the frame: a `<ShaderEffect>`'s children
//! are drawn by Direct2D into a texture (its content) instead of the
//! window, and what the frame drew behind a view so far is copied into
//! another (its backdrop) before the pass. Both cover the area the shader
//! draws: the view and an effect's `maxSampleOffset` beyond it.
//!
//! The program is `HlslProgram::d3d11` (`nsh_vertex` / `nsh_fragment`,
//! shader model 5.0), compiled with `D3DCompile` (`d3dcompiler_47`, part of
//! Windows) when it arrives, on the device Direct2D draws with: the texture
//! is a Direct2D bitmap of the same device. A view renders again when what
//! it draws changes (its fill, an animated uniform, an image, its size,
//! the device), and on every frame when its shader reads `nsh.time`.
//! Direct2D's pending drawing is flushed before the Direct3D pass; Direct2D
//! keeps the state of its own device context apart from the app's.

use std::{cell::RefCell, collections::HashMap, ffi::CString, rc::Rc, sync::Arc, time::Instant};

use nsh_core::{
    animation::AnimationValue,
    geometry::{Bounds, DevicePixels, Percentage, Point, Points, Size},
    shader::{self, ProgramCode, ShaderFill, ShaderInput, ShaderProgram},
};
use windows::{
    Win32::Graphics::{
        Direct2D::{
            Common::{D2D_POINT_2U, D2D_RECT_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT},
            D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
            D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_LINEAR, ID2D1Bitmap1, ID2D1Image,
        },
        Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, Fxc::D3DCompile, ID3DBlob, ID3DInclude},
        Direct3D11::{
            D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
            D3D11_BUFFER_DESC, D3D11_COMPARISON_NEVER, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            D3D11_SAMPLER_DESC, D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE_ADDRESS_CLAMP,
            D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_VIEWPORT, ID3D11Buffer,
            ID3D11PixelShader, ID3D11RenderTargetView, ID3D11SamplerState,
            ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader,
        },
        Dxgi::{
            Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
            IDXGISurface,
        },
    },
    core::{Interface, PCSTR},
};

use crate::{
    geom::Mat4,
    gfx::{self, Gfx},
    render::DrawCx,
    surface::Surface,
    view::{NshView, ViewState},
};

/// A program's shaders on the device.
struct Program {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    /// The inputs its pixel shader samples, by register.
    textures: Vec<ShaderInput>,
}

/// A program's compiled bytecode (kept to make its shaders again after a
/// device loss) and its shaders on the current device.
struct Compiled {
    vertex: Vec<u8>,
    pixel: Vec<u8>,
    textures: Vec<ShaderInput>,
    made: Option<(u64, Rc<Program>)>,
}

thread_local! {
    static PROGRAMS: RefCell<HashMap<u32, Option<Compiled>>> = RefCell::default();
}

/// A program for the views to come: compiled now.
pub(crate) fn program(program: &ShaderProgram) {
    let id = program.id;
    if PROGRAMS.with(|p| p.borrow().contains_key(&id)) {
        return;
    }
    let compiled = match &program.code {
        ProgramCode::Hlsl(hlsl) => {
            let stage = |entry: &str, target: &str| compile(&hlsl.d3d11, entry, target);
            match (
                stage(&hlsl.vertex, "vs_5_0"),
                stage(&hlsl.fragment, "ps_5_0"),
            ) {
                (Ok(vertex), Ok(pixel)) => Some(Compiled {
                    vertex,
                    pixel,
                    textures: hlsl.textures.clone(),
                    made: None,
                }),
                (Err(e), _) | (_, Err(e)) => {
                    log::error!(target: "js", "<Shader>: Direct3D could not compile the shader: {e}");
                    None
                }
            }
        }
        _ => None,
    };
    PROGRAMS.with(|p| p.borrow_mut().insert(id, compiled));
}

/// `source`'s `entry` compiled for `target`, or the compiler's messages.
fn compile(source: &str, entry: &str, target: &str) -> Result<Vec<u8>, String> {
    let entry = CString::new(entry).map_err(|e| e.to_string())?;
    let target = CString::new(target).map_err(|e| e.to_string())?;
    let mut blob: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    // `D3DCOMPILE_OPTIMIZATION_LEVEL3`.
    const OPTIMIZE: u32 = 1 << 15;
    // SAFETY: the source, entry point and target outlive the call; the
    // blobs are out parameters it fills.
    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None::<&ID3DInclude>,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(target.as_ptr().cast()),
            OPTIMIZE,
            0,
            &mut blob,
            Some(&mut errors),
        )
    };
    let bytes = |b: &ID3DBlob| {
        // SAFETY: a blob's buffer holds `GetBufferSize` bytes.
        unsafe {
            std::slice::from_raw_parts(b.GetBufferPointer().cast::<u8>(), b.GetBufferSize())
                .to_vec()
        }
    };
    match (result, blob) {
        (Ok(()), Some(blob)) => Ok(bytes(&blob)),
        (result, _) => Err(errors
            .map(|e| String::from_utf8_lossy(&bytes(&e)).into_owned())
            .unwrap_or_else(|| format!("{result:?}"))),
    }
}

/// Program `id`'s shaders on `gfx`'s device (made again after a loss).
fn shaders(gfx: &Gfx, id: u32) -> Option<Rc<Program>> {
    let generation = gfx::generation();
    PROGRAMS.with(|p| {
        let mut p = p.borrow_mut();
        let compiled = p.get_mut(&id)?.as_mut()?;
        if let Some((g, program)) = &compiled.made
            && *g == generation
        {
            return Some(program.clone());
        }
        let mut vertex = None;
        let mut pixel = None;
        // SAFETY: COM calls on the live device with bytecode D3DCompile made.
        unsafe {
            gfx.d3d
                .CreateVertexShader(&compiled.vertex, None, Some(&mut vertex))
                .ok()?;
            gfx.d3d
                .CreatePixelShader(&compiled.pixel, None, Some(&mut pixel))
                .ok()?;
        }
        let program = Rc::new(Program {
            vertex: vertex?,
            pixel: pixel?,
            textures: compiled.textures.clone(),
        });
        compiled.made = Some((generation, program.clone()));
        Some(program)
    })
}

/// What a shader view draws.
#[derive(Default)]
pub(crate) struct ShaderState {
    fill: Option<Arc<ShaderFill>>,
    /// The fill's uniforms with the animated values written in.
    uniforms: Vec<u8>,
    /// `nsh.progress`: the fill's, or its animated value.
    progress: f32,
    args: Vec<(u16, AnimationValue)>,
    /// A `<ShaderEffect>` or `<ShaderTransition>`: its children are the
    /// shader's content (or its `sample_to`).
    effect: bool,
    /// A `<ShaderTransition>`; when it runs, its start and duration.
    transition: bool,
    running: Option<(Instant, std::time::Duration)>,
    /// The images, by slot, once loaded.
    images: Vec<Option<Rc<crate::image::Frame>>>,
    started: Option<Instant>,
    /// A frame callback draws it (`nsh.time`).
    ticking: bool,
    target: RefCell<Option<Target>>,
}

/// The texture it renders into, the bitmap Direct2D draws it with, and
/// the device resources of its last render.
struct Target {
    generation: u64,
    size: (u32, u32),
    view: ID3D11RenderTargetView,
    bitmap: ID2D1Bitmap1,
    /// Rendered with what it shows now.
    fresh: bool,
    buffers: [Option<ID3D11Buffer>; 3],
    images: Vec<Option<(usize, ID3D11ShaderResourceView)>>,
    sampler: Option<ID3D11SamplerState>,
    /// The textures of the children (an effect's content, a transition's
    /// `sample_to`), the backdrop and a transition's `sample_from`, of the
    /// target's size.
    inputs: [Option<Input>; 3],
    /// The backdrop was copied in this frame ([`take_backdrop`]).
    backdrop: bool,
}

/// A texture an input is drawn (the content) or copied (the backdrop)
/// into, and the Direct2D bitmap of it.
#[derive(Clone)]
struct Input {
    view: ID3D11ShaderResourceView,
    target: ID3D11RenderTargetView,
    bitmap: ID2D1Bitmap1,
}

impl Target {
    /// The texture of input `at` (0: the children, 1: the backdrop, 2: a
    /// transition's `sample_from`).
    fn input(&mut self, gfx: &Gfx, at: usize, scale: f32) -> Option<Input> {
        if self.inputs[at].is_none() {
            self.inputs[at] = texture(gfx, self.size, scale, D2D1_BITMAP_OPTIONS_TARGET)
                .map(|(view, target, bitmap)| Input {
                    view,
                    target,
                    bitmap,
                })
                .map_err(|e| log::warn!("<Shader>: {e}"))
                .ok();
        }
        self.inputs[at].clone()
    }
}

impl ShaderState {
    fn stale(&mut self) {
        if let Some(t) = self.target.get_mut() {
            t.fresh = false;
        }
    }
}

/// Sets up a shader view; `effect`: a `<ShaderEffect>`, `transition`: a
/// `<ShaderTransition>`.
pub(crate) fn create(view: &NshView, effect: bool, transition: bool) {
    view.state_mut().shader = Some(ShaderState {
        effect: effect || transition,
        transition,
        ..ShaderState::default()
    });
}

impl ShaderState {
    /// The input its children are: an effect's content, a transition's
    /// `sample_to`.
    fn children(&self) -> ShaderInput {
        if self.transition {
            ShaderInput::To
        } else {
            ShaderInput::Content
        }
    }
}

/// A shader transition starts ([`nsh_core::MountItem::ShaderTransition`],
/// before the rest of its batch): what shows now is drawn into its
/// `sample_from`, and a frame callback moves `nsh.progress` from 0 to 1
/// over `duration`.
pub(crate) fn start_transition(
    surface: &Rc<Surface>,
    view: &NshView,
    duration: std::time::Duration,
) {
    let Ok(gfx) = gfx::get() else { return };
    let scale = surface.scale();
    let (area, from, running) = {
        let state = view.state();
        let Some(shader) = state.shader.as_ref() else {
            return;
        };
        let Some(fill) = shader.fill.as_ref() else {
            return;
        };
        if !shader.transition || shaders(&gfx, fill.program).is_none() {
            return;
        }
        let (w, h) = (state.frame.size.width.0, state.frame.size.height.0);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let o = fill.max_offset.0.max(0.0);
        let area = Bounds::new(
            Point::new(Points(-o), Points(-o)),
            Size::new(Points(w + 2.0 * o), Points(h + 2.0 * o)),
        );
        let size = (
            (area.size.width.0 * scale).round().max(1.0) as u32,
            (area.size.height.0 * scale).round().max(1.0) as u32,
        );
        let mut target = shader.target.borrow_mut();
        let generation = gfx::generation();
        if target
            .as_ref()
            .is_none_or(|t| t.generation != generation || t.size != size)
        {
            *target = make_target(&gfx, size, generation)
                .map_err(|e| log::warn!("<Shader>: {e}"))
                .ok();
        }
        let Some(t) = target.as_mut() else { return };
        let Some(from) = t.input(&gfx, 2, scale) else {
            return;
        };
        let running = shader.running.is_some();
        if running {
            // What shows: the transition as it was last drawn.
            // SAFETY: COM calls on bitmaps of the live device, of one size.
            if let Err(e) = unsafe { from.bitmap.CopyFromBitmap(None, &t.bitmap, None) } {
                log::debug!("<ShaderTransition>: no picture of the transition: {e}");
            }
        }
        (area, from, running)
    };
    if !running && let Ok(image) = from.bitmap.cast::<ID2D1Image>() {
        // The children as they are, at the picture's origin.
        let base = Mat4::new().translate(-area.origin.x.0, -area.origin.y.0);
        crate::render::draw_offscreen(surface, &image, scale, |cx| {
            view.with_children(|children| {
                for child in children {
                    let origin = {
                        let s = child.state();
                        view.state().child_origin(s.frame, s.shift)
                    };
                    crate::render::draw_view(cx, child, base, origin);
                }
            });
        });
    }
    {
        let mut s = view.state_mut();
        let Some(shader) = s.shader.as_mut() else {
            return;
        };
        shader.running = Some((Instant::now(), duration));
        shader.progress = 0.0;
        shader.stale();
    }
    view.changed();
    if running {
        // Its frame callback runs on.
        return;
    }
    let id = view.id();
    surface.on_frame(move |surface, _| {
        let Some(view) = surface.view(id) else {
            return false;
        };
        let runs = {
            let mut s = view.state_mut();
            let Some(shader) = s.shader.as_mut() else {
                return false;
            };
            let Some((start, duration)) = shader.running else {
                return false;
            };
            let t =
                (start.elapsed().as_secs_f32() / duration.as_secs_f32().max(f32::EPSILON)).min(1.0);
            shader.progress = t;
            if t >= 1.0 {
                // Its children draw as they are again.
                shader.running = None;
                if let Some(t) = shader.target.get_mut() {
                    t.inputs[2] = None;
                }
            }
            shader.stale();
            t < 1.0
        };
        view.changed();
        runs
    });
}

/// What `view` draws now (`None`: its children only); loads its images.
pub(crate) fn set(surface: &Rc<Surface>, view: &NshView, fill: Option<Arc<ShaderFill>>) {
    let mut load = Vec::new();
    {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        let old = state.fill.take();
        if let Some(fill) = &fill {
            state.uniforms = fill.uniforms.to_vec();
            state.progress = fill.progress;
            shader::write_args(&mut state.uniforms, &mut state.progress, &state.args);
            state.images.resize(fill.images.len(), None);
            for (slot, image) in fill.images.iter().enumerate() {
                let before = old
                    .as_ref()
                    .and_then(|o| o.images.get(slot))
                    .and_then(|i| i.source.as_ref());
                match &image.source {
                    Some(source) if before.map(|b| &b.uri) != Some(&source.uri) => {
                        state.images[slot] = None;
                        load.push((slot, source.uri.clone()));
                    }
                    None => state.images[slot] = None,
                    _ => {}
                }
            }
        }
        state.fill = fill;
        state.stale();
    }
    let id = view.id();
    for (slot, uri) in load {
        let weak = Rc::downgrade(surface);
        let key = uri.clone();
        crate::image::load_bitmap(surface, &uri, move |result| {
            let frame = match result {
                Ok(frame) => frame,
                Err(e) => {
                    log::warn!(target: "js", "<Shader>: {e}");
                    return;
                }
            };
            let Some(view) = weak.upgrade().and_then(|s| s.view(id)) else {
                return;
            };
            {
                let mut s = view.state_mut();
                let Some(state) = s.shader.as_mut() else {
                    return;
                };
                let Some(entry) = state
                    .fill
                    .as_ref()
                    .and_then(|f| f.images.get(slot).cloned())
                else {
                    return;
                };
                if entry.source.as_ref().map(|s| &s.uri) != Some(&key) {
                    return;
                }
                if entry.source.as_ref().and_then(|s| s.size).is_none() {
                    let (w, h) = frame.size();
                    shader::write_image_size(
                        &mut state.uniforms,
                        &entry,
                        Size::new(Points(w as f32), Points(h as f32)),
                    );
                }
                state.images[slot] = Some(frame);
                state.stale();
            }
            view.changed();
        });
    }
    tick(surface, view);
    view.changed();
}

/// Animated uniforms' values.
pub(crate) fn set_args(view: &NshView, values: &[(u16, AnimationValue)]) {
    {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        if state.args == values {
            return;
        }
        state.args.clear();
        state.args.extend_from_slice(values);
        if state.fill.is_some() {
            shader::write_args(&mut state.uniforms, &mut state.progress, values);
        }
        state.stale();
    }
    view.changed();
}

/// A shader that reads `nsh.time` renders on every frame while it lives.
fn tick(surface: &Rc<Surface>, view: &NshView) {
    {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        if state.ticking || !state.fill.as_ref().is_some_and(|f| f.time) {
            return;
        }
        state.ticking = true;
        state.started.get_or_insert_with(Instant::now);
    }
    let id = view.id();
    surface.on_frame(move |surface, _| {
        let Some(view) = surface.view(id) else {
            return false;
        };
        {
            let mut s = view.state_mut();
            let Some(state) = s.shader.as_mut() else {
                return false;
            };
            if !state.fill.as_ref().is_some_and(|f| f.time) {
                state.ticking = false;
                return false;
            }
            state.stale();
        }
        view.changed();
        true
    });
}

/// True when `state` is an effect drawing its children through its
/// shader ([`draw_effect`]): they are not drawn into the window.
pub(crate) fn effect(state: &ViewState) -> bool {
    state
        .shader
        .as_ref()
        .is_some_and(|s| s.effect && s.fill.is_some() && (!s.transition || s.running.is_some()))
}

/// Draws a shader view's picture (rendering it first if it is stale);
/// an effect draws with its children ([`draw_effect`]).
pub(crate) fn draw(cx: &mut DrawCx<'_>, state: &ViewState) {
    if state.shader.as_ref().is_none_or(|s| s.effect) {
        return;
    }
    let Some(area) = prepare(cx, state) else {
        return;
    };
    // The fill follows the view's corners, as a background does.
    let radii = state.radii();
    let rounded = radii.iter().any(|&r| r > 0.0);
    if rounded {
        let (w, h) = (state.frame.size.width.0, state.frame.size.height.0);
        cx.push_clip_rounded(crate::geom::rect(0.0, 0.0, w, h), radii);
    }
    finish(cx, state, area);
    if rounded {
        cx.pop();
    }
}

/// Draws an effect: `view`'s children into its content, then the shader
/// at `world` (set on the context). False when it draws nothing: the
/// children are then drawn as they are.
pub(crate) fn draw_effect(cx: &mut DrawCx<'_>, view: &NshView, world: Mat4) -> bool {
    let (area, content) = {
        let state = view.state();
        let Some(area) = prepare(cx, &state) else {
            return false;
        };
        let Some(shader) = state.shader.as_ref() else {
            return false;
        };
        let samples = shader
            .fill
            .as_ref()
            .is_some_and(|f| f.samples(shader.children()));
        let content = match samples {
            true => shader
                .target
                .borrow_mut()
                .as_mut()
                .and_then(|t| t.input(cx.gfx, 0, cx.scale)),
            false => None,
        };
        (area, content)
    };
    if let Some(content) = content
        && let Ok(image) = content.bitmap.cast::<ID2D1Image>()
    {
        // The children at the content's origin, drawn by another context:
        // what is pushed on this one stays as it is.
        let base = Mat4::new().translate(-area.origin.x.0, -area.origin.y.0);
        let composed = cx.composed.len();
        crate::decor::layer::draw_into(cx, &image, cx.scale, true, |inner| {
            view.with_children(|children| {
                for child in children {
                    let origin = {
                        let s = child.state();
                        view.state().child_origin(s.frame, s.shift)
                    };
                    crate::render::draw_view(inner, child, base, origin);
                }
            });
        });
        // Web views in it were placed in the content's coordinates.
        cx.composed.truncate(composed);
    }
    cx.set_transform(world);
    finish(cx, &view.state(), area);
    true
}

/// The area a shader view draws (points from its top-left), once its
/// program is ready and its target made; `None` when it draws nothing.
fn prepare(cx: &DrawCx<'_>, state: &ViewState) -> Option<Bounds<Points>> {
    let shader = state.shader.as_ref()?;
    let fill = shader.fill.as_ref()?;
    let (w, h) = (state.frame.size.width.0, state.frame.size.height.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    shaders(cx.gfx, fill.program)?;
    let o = if shader.effect {
        fill.max_offset.0.max(0.0)
    } else {
        0.0
    };
    let area = Bounds::new(
        Point::new(Points(-o), Points(-o)),
        Size::new(Points(w + 2.0 * o), Points(h + 2.0 * o)),
    );
    let size = (
        (area.size.width.0 * cx.scale).round().max(1.0) as u32,
        (area.size.height.0 * cx.scale).round().max(1.0) as u32,
    );
    let mut target = shader.target.borrow_mut();
    let generation = gfx::generation();
    if target
        .as_ref()
        .is_none_or(|t| t.generation != generation || t.size != size)
    {
        *target = make_target(cx.gfx, size, generation)
            .map_err(|e| log::warn!("<Shader>: {e}"))
            .ok();
    }
    target.as_ref().map(|_| area)
}

/// Renders the shader if its picture is stale (always when it reads an
/// input) and draws it over `area`.
fn finish(cx: &mut DrawCx<'_>, state: &ViewState, area: Bounds<Points>) {
    let Some(shader) = state.shader.as_ref() else {
        return;
    };
    let Some(fill) = &shader.fill else { return };
    let Some(program) = shaders(cx.gfx, fill.program) else {
        return;
    };
    let mut target = shader.target.borrow_mut();
    let Some(t) = target.as_mut() else { return };
    let live = shader.effect || fill.samples(ShaderInput::Backdrop);
    if !t.fresh || live {
        let backdrop = match std::mem::take(&mut t.backdrop) {
            true => t.inputs[1].clone(),
            false => None,
        };
        let children = match fill.samples(shader.children()) && shader.effect {
            true => t.inputs[0].clone(),
            false => None,
        };
        let from = match shader.running.is_some() && fill.samples(ShaderInput::From) {
            true => t.inputs[2].clone(),
            false => None,
        };
        let size = state.frame.size;
        let time = shader.started.map_or(0.0, |s| s.elapsed().as_secs_f32());
        // By input: the content, the backdrop, `sample_from`, `sample_to`.
        let inputs = match shader.transition {
            true => [None, backdrop, from, children],
            false => [children, backdrop, None, None],
        };
        if let Err(e) = render(cx, t, &program, shader, size, area, &inputs, time) {
            log::warn!("<Shader>: {e}");
            return;
        }
        t.fresh = true;
    }
    let (x, y) = (area.origin.x.0, area.origin.y.0);
    let (w, h) = (area.size.width.0, area.size.height.0);
    // SAFETY: a COM call on the live context with a bitmap of its device.
    unsafe {
        cx.dc.DrawBitmap(
            &t.bitmap,
            Some(&crate::render::rect_f(x, y, w, h)),
            1.0,
            D2D1_INTERPOLATION_MODE_LINEAR,
            None,
            None,
        )
    };
}

/// Copies what is behind a view whose shader reads it, before the view
/// draws anything: its own background is not behind it.
pub(crate) fn take_backdrop(cx: &mut DrawCx<'_>, state: &ViewState) {
    let Some(shader) = state.shader.as_ref() else {
        return;
    };
    let samples = shader
        .fill
        .as_ref()
        .is_some_and(|f| f.samples(ShaderInput::Backdrop));
    if !samples {
        return;
    }
    let Some(area) = prepare(cx, state) else {
        return;
    };
    let mut target = shader.target.borrow_mut();
    let Some(t) = target.as_mut() else { return };
    let Some(input) = t.input(cx.gfx, 1, cx.scale) else {
        return;
    };
    t.backdrop = copy_backdrop(cx, &input, area);
}

/// Copies what the pass drew so far under `area` (the view's points, at
/// the context's transform) into `input`, transparent where the target
/// has nothing (beyond the window, a pass drawn offscreen). False when
/// the copy failed.
fn copy_backdrop(cx: &mut DrawCx<'_>, input: &Input, area: Bounds<Points>) -> bool {
    // SAFETY: COM calls on the live device; the texture is not drawn with.
    unsafe {
        if let Ok(context) = cx.gfx.d3d.GetImmediateContext() {
            context.ClearRenderTargetView(&input.target, &[0.0, 0.0, 0.0, 0.0]);
        }
    }
    if !cx.backdrop {
        return true;
    }
    let Some((x, y, _, _)) = cx.transform.bounds(
        area.origin.x.0,
        area.origin.y.0,
        area.size.width.0,
        area.size.height.0,
    ) else {
        return false;
    };
    let scale = cx.scale;
    // Layers pushed reach the target when they pop: the copy is made while
    // they are closed.
    cx.flatten(|dc| {
        // SAFETY: COM calls on the live context, its target and the
        // input's bitmap; the pending drawing is flushed first.
        unsafe {
            if dc.Flush(None, None).is_err() {
                return false;
            }
            let Some(target) = dc
                .GetTarget()
                .ok()
                .and_then(|t| t.cast::<ID2D1Bitmap1>().ok())
            else {
                return false;
            };
            let window = target.GetPixelSize();
            let px = input.bitmap.GetPixelSize();
            let (x, y) = ((x * scale).round() as i64, (y * scale).round() as i64);
            let left = x.clamp(0, window.width as i64);
            let top = y.clamp(0, window.height as i64);
            let right = (x + px.width as i64).clamp(0, window.width as i64);
            let bottom = (y + px.height as i64).clamp(0, window.height as i64);
            if right <= left || bottom <= top {
                return true;
            }
            input
                .bitmap
                .CopyFromBitmap(
                    Some(&D2D_POINT_2U {
                        x: (left - x) as u32,
                        y: (top - y) as u32,
                    }),
                    &target,
                    Some(&D2D_RECT_U {
                        left: left as u32,
                        top: top as u32,
                        right: right as u32,
                        bottom: bottom as u32,
                    }),
                )
                .map_err(|e| log::debug!("<Shader>: backdrop not copied: {e}"))
                .is_ok()
        }
    })
}

/// A texture of `size` pixels Direct3D renders into and samples, and a
/// Direct2D bitmap of it (`scale` pixels per point, with `options`).
fn texture(
    gfx: &Gfx,
    size: (u32, u32),
    scale: f32,
    options: D2D1_BITMAP_OPTIONS,
) -> windows::core::Result<(
    ID3D11ShaderResourceView,
    ID3D11RenderTargetView,
    ID2D1Bitmap1,
)> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.0,
        Height: size.1,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    // SAFETY: COM calls on the live devices with valid descriptions.
    unsafe {
        let mut texture: Option<ID3D11Texture2D> = None;
        gfx.d3d.CreateTexture2D(&desc, None, Some(&mut texture))?;
        let texture = texture.ok_or_else(fail)?;
        let mut target = None;
        gfx.d3d
            .CreateRenderTargetView(&texture, None, Some(&mut target))?;
        let mut view = None;
        gfx.d3d
            .CreateShaderResourceView(&texture, None, Some(&mut view))?;
        let surface: IDXGISurface = texture.cast()?;
        let bitmap = gfx.dc.CreateBitmapFromDxgiSurface(
            &surface,
            Some(&D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0 * scale,
                dpiY: 96.0 * scale,
                bitmapOptions: options,
                colorContext: std::mem::ManuallyDrop::new(None),
            }),
        )?;
        Ok((view.ok_or_else(fail)?, target.ok_or_else(fail)?, bitmap))
    }
}

fn make_target(gfx: &Gfx, size: (u32, u32), generation: u64) -> windows::core::Result<Target> {
    // Pixels per DIP as the context draws: the bitmap is drawn over the
    // area's points.
    let (_, view, bitmap) = texture(gfx, size, 1.0, D2D1_BITMAP_OPTIONS_NONE)?;
    Ok(Target {
        generation,
        size,
        view,
        bitmap,
        fresh: false,
        buffers: [None, None, None],
        images: Vec::new(),
        sampler: None,
        inputs: [None, None, None],
        backdrop: false,
    })
}

fn fail() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL)
}

/// A constant buffer holding `data` (a multiple of 16 bytes).
fn buffer(gfx: &Gfx, data: &[u8]) -> windows::core::Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: data.len().div_ceil(16) as u32 * 16,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        ..Default::default()
    };
    let mut padded = data.to_vec();
    padded.resize(desc.ByteWidth as usize, 0);
    let init = D3D11_SUBRESOURCE_DATA {
        pSysMem: padded.as_ptr().cast(),
        ..Default::default()
    };
    let mut out = None;
    // SAFETY: a COM call on the live device; `padded` outlives it.
    unsafe { gfx.d3d.CreateBuffer(&desc, Some(&init), Some(&mut out))? };
    out.ok_or_else(fail)
}

/// The texture of an image's pixels (premultiplied BGRA).
fn image_view(
    gfx: &Gfx,
    frame: &crate::image::Frame,
) -> windows::core::Result<ID3D11ShaderResourceView> {
    let (width, height, data) = frame.pixels();
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let init = D3D11_SUBRESOURCE_DATA {
        pSysMem: data.as_ptr().cast(),
        SysMemPitch: width * 4,
        SysMemSlicePitch: 0,
    };
    // SAFETY: COM calls on the live device; `data` holds the rows.
    unsafe {
        let mut texture: Option<ID3D11Texture2D> = None;
        gfx.d3d
            .CreateTexture2D(&desc, Some(&init), Some(&mut texture))?;
        let mut view = None;
        gfx.d3d
            .CreateShaderResourceView(&texture.ok_or_else(fail)?, None, Some(&mut view))?;
        view.ok_or_else(fail)
    }
}

/// Renders the shader over `area` of a view of `size`, with the content's
/// the backdrop's and a transition's textures (`inputs`, by input index)
/// where it has them.
#[allow(clippy::too_many_arguments)]
fn render(
    cx: &mut DrawCx<'_>,
    t: &mut Target,
    program: &Program,
    shader: &ShaderState,
    size: Size<Points>,
    area: Bounds<Points>,
    inputs: &[Option<Input>; 4],
    time: f32,
) -> windows::core::Result<()> {
    let gfx = cx.gfx;
    let nsh = shader::nsh_buffer(size, cx.scale, time, Percentage(shader.progress));
    let view = Bounds::new(Point::default(), size);
    let px = Size::new(DevicePixels(t.size.0 as i32), DevicePixels(t.size.1 as i32));
    let mut rects: Vec<(ShaderInput, Bounds<Points>, Size<DevicePixels>)> = shader
        .images
        .iter()
        .enumerate()
        .filter_map(|(slot, f)| {
            let (w, h) = f.as_ref()?.size();
            Some((
                ShaderInput::Image(slot as u8),
                view,
                Size::new(DevicePixels(w as i32), DevicePixels(h as i32)),
            ))
        })
        .collect();
    for (input, texture) in [
        ShaderInput::Content,
        ShaderInput::Backdrop,
        ShaderInput::From,
        ShaderInput::To,
    ]
    .into_iter()
    .zip(inputs)
    {
        if texture.is_some() {
            rects.push((input, area, px));
        }
    }
    let frame = shader::frame_buffer_of(area, &rects);
    t.buffers = [Some(buffer(gfx, &nsh)?), Some(buffer(gfx, &frame)?), None];
    if !shader.uniforms.is_empty() {
        t.buffers[2] = Some(buffer(gfx, &shader.uniforms)?);
    }
    // Each image's texture, made once per picture.
    let mut views = Vec::with_capacity(program.textures.len());
    for input in &program.textures {
        let slot = match *input {
            ShaderInput::Image(slot) => slot,
            input => {
                views.push(inputs[input.index()].as_ref().map(|i| i.view.clone()));
                continue;
            }
        };
        let slot = slot as usize;
        let Some(frame) = shader.images.get(slot).cloned().flatten() else {
            views.push(None);
            continue;
        };
        let key = Rc::as_ptr(&frame) as usize;
        if t.images.len() <= slot {
            t.images.resize(slot + 1, None);
        }
        if t.images[slot].as_ref().is_none_or(|(k, _)| *k != key) {
            t.images[slot] = Some((key, image_view(gfx, &frame)?));
        }
        views.push(t.images[slot].as_ref().map(|(_, v)| v.clone()));
    }
    if t.sampler.is_none() {
        let desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let mut sampler = None;
        // SAFETY: a COM call on the live device with a valid description.
        unsafe { gfx.d3d.CreateSamplerState(&desc, Some(&mut sampler))? };
        t.sampler = sampler;
    }
    // SAFETY: COM calls on the live devices; Direct2D's pending drawing is
    // flushed first, and its context keeps its own Direct3D state.
    unsafe {
        cx.dc.Flush(None, None)?;
        let context = gfx.d3d.GetImmediateContext()?;
        context.OMSetRenderTargets(Some(&[Some(t.view.clone())]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: t.size.0 as f32,
            Height: t.size.1 as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]));
        context.ClearRenderTargetView(&t.view, &[0.0, 0.0, 0.0, 0.0]);
        context.IASetInputLayout(None);
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        context.VSSetShader(&program.vertex, None);
        context.PSSetShader(&program.pixel, None);
        context.VSSetConstantBuffers(0, Some(&t.buffers));
        context.PSSetConstantBuffers(0, Some(&t.buffers));
        if !views.is_empty() {
            context.PSSetShaderResources(0, Some(&views));
        }
        context.PSSetSamplers(0, Some(std::slice::from_ref(&t.sampler)));
        context.Draw(4, 0);
        context.OMSetRenderTargets(None, None);
        if !views.is_empty() {
            let none: Vec<Option<ID3D11ShaderResourceView>> = vec![None; views.len()];
            context.PSSetShaderResources(0, Some(&none));
        }
    }
    Ok(())
}
