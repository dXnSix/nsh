//! `<Shader>`: the shader drawn with Direct3D 11 into a texture of the
//! view's size, which Direct2D draws over its background and under its
//! children (`docs/dev/shaders.md` "Windows").
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
    geometry::{DevicePixels, Percentage, Points, Size},
    shader::{self, ProgramCode, ShaderFill, ShaderInput, ShaderProgram},
};
use windows::{
    Win32::Graphics::{
        Direct2D::{
            Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT},
            D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_LINEAR,
            ID2D1Bitmap1,
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
    args: Vec<(u16, AnimationValue)>,
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
}

impl ShaderState {
    fn stale(&mut self) {
        if let Some(t) = self.target.get_mut() {
            t.fresh = false;
        }
    }
}

/// Sets up a shader view.
pub(crate) fn create(view: &NshView) {
    view.state_mut().shader = Some(ShaderState::default());
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
            for &(k, value) in &state.args {
                shader::write_animated(&mut state.uniforms, k, value);
            }
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
            for &(k, value) in values {
                shader::write_animated(&mut state.uniforms, k, value);
            }
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

/// Draws a shader view's picture (rendering it first if it is stale).
pub(crate) fn draw(cx: &mut DrawCx<'_>, state: &ViewState) {
    let Some(shader) = state.shader.as_ref() else {
        return;
    };
    let Some(fill) = &shader.fill else { return };
    let (w, h) = (state.frame.size.width.0, state.frame.size.height.0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let Some(program) = shaders(cx.gfx, fill.program) else {
        return;
    };
    let size = (
        (w * cx.scale).round().max(1.0) as u32,
        (h * cx.scale).round().max(1.0) as u32,
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
    let Some(t) = target.as_mut() else { return };
    if !t.fresh {
        let time = shader.started.map_or(0.0, |s| s.elapsed().as_secs_f32());
        if let Err(e) = render(cx, t, &program, shader, (w, h), time) {
            log::warn!("<Shader>: {e}");
            return;
        }
        t.fresh = true;
    }
    // The fill follows the view's corners, as a background does.
    let radii = state.radii();
    let rounded = radii.iter().any(|&r| r > 0.0);
    if rounded {
        cx.push_clip_rounded(crate::geom::rect(0.0, 0.0, w, h), radii);
    }
    // SAFETY: a COM call on the live context with a bitmap of its device.
    unsafe {
        cx.dc.DrawBitmap(
            &t.bitmap,
            Some(&crate::render::rect_f(0.0, 0.0, w, h)),
            1.0,
            D2D1_INTERPOLATION_MODE_LINEAR,
            None,
            None,
        )
    };
    if rounded {
        cx.pop();
    }
}

fn make_target(gfx: &Gfx, size: (u32, u32), generation: u64) -> windows::core::Result<Target> {
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
        let mut view = None;
        gfx.d3d
            .CreateRenderTargetView(&texture, None, Some(&mut view))?;
        let surface: IDXGISurface = texture.cast()?;
        let bitmap = gfx.dc.CreateBitmapFromDxgiSurface(
            &surface,
            Some(&D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                // Pixels per DIP as the context draws: the bitmap covers the
                // view's points.
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
                colorContext: std::mem::ManuallyDrop::new(None),
            }),
        )?;
        Ok(Target {
            generation,
            size,
            view: view.ok_or_else(fail)?,
            bitmap,
            fresh: false,
            buffers: [None, None, None],
            images: Vec::new(),
            sampler: None,
        })
    }
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

fn render(
    cx: &mut DrawCx<'_>,
    t: &mut Target,
    program: &Program,
    shader: &ShaderState,
    (w, h): (f32, f32),
    time: f32,
) -> windows::core::Result<()> {
    let gfx = cx.gfx;
    let size = Size::new(Points(w), Points(h));
    let nsh = shader::nsh_buffer(size, cx.scale, time, Percentage(0.0));
    let images: Vec<(usize, Size<DevicePixels>)> = shader
        .images
        .iter()
        .enumerate()
        .filter_map(|(slot, f)| {
            let (w, h) = f.as_ref()?.size();
            Some((
                slot,
                Size::new(DevicePixels(w as i32), DevicePixels(h as i32)),
            ))
        })
        .collect();
    let frame = shader::frame_buffer(size, &images);
    t.buffers = [Some(buffer(gfx, &nsh)?), Some(buffer(gfx, &frame)?), None];
    if !shader.uniforms.is_empty() {
        t.buffers[2] = Some(buffer(gfx, &shader.uniforms)?);
    }
    // Each image's texture, made once per picture.
    let mut views = Vec::with_capacity(program.textures.len());
    for input in &program.textures {
        let ShaderInput::Image(slot) = *input else {
            views.push(None);
            continue;
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
