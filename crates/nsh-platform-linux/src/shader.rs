//! `<Shader>`: the shader drawn with OpenGL ES 3 into a texture GTK draws
//! as a `GdkGLTexture` over the view's background and under its children
//! (`docs/dev/shaders.md` "Linux").
//!
//! Shaders draw in a `GdkGLContext` of their own, made from the display
//! (contexts made from one display share their textures with GTK's
//! renderer), through libepoxy's function pointers (GTK links libepoxy)
//! with glow. Each texture carries a fence GTK waits on before it samples
//! it. Textures are pooled: GTK hands one back (the release function of
//! its `GdkGLTexture`) when nothing draws it any more. A view renders when
//! what it draws changes (fill, animated uniforms, images, size, scale),
//! and on every frame of its frame clock while its shader reads
//! `nsh.time`.
//!
//! The texture is drawn as `GtkGLArea` draws its own: GL's rows go bottom
//! up, so the snapshot flips it.

use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use glow::HasContext;
use gtk4::{self as gtk, gdk, glib, graphene, prelude::*};
use nsh_core::{
    animation::AnimationValue,
    geometry::{DevicePixels, Percentage, Points, Size},
    shader::{self, GlslProgram, ProgramCode, ShaderFill, ShaderInput, ShaderProgram},
};

use crate::{
    surface::Surface,
    view::{NshView, ViewState},
};

/// The GL context shaders draw with, and what it compiled.
struct Gl {
    context: gdk::GLContext,
    gl: glow::Context,
    /// Programs by id (`None`: it did not compile).
    programs: HashMap<u32, Option<Program>>,
    vertex_array: glow::VertexArray,
    buffers: [glow::Buffer; 3],
    framebuffer: glow::Framebuffer,
    /// A transparent texture for inputs without a picture.
    empty: glow::Texture,
}

struct Program {
    program: glow::Program,
    /// Sampler locations and the inputs they are.
    samplers: Vec<(Option<glow::UniformLocation>, ShaderInput)>,
}

thread_local! {
    /// The context (made on first use; `None` without GL ES 3).
    static GL: RefCell<Option<Option<Gl>>> = const { RefCell::new(None) };
    /// The programs' GLSL, compiled in the context on first use.
    static SOURCES: RefCell<HashMap<u32, Option<GlslProgram>>> = RefCell::default();
    /// Textures of views that are gone: images' and rendered ones.
    static ORPHANS: RefCell<(Vec<glow::Texture>, Vec<Pooled>)> = RefCell::default();
}

/// libepoxy's GL functions: `epoxy_<name>` is a pointer to `<name>` of
/// the current context.
fn loader() -> Option<impl FnMut(&str) -> *const c_void> {
    // SAFETY: opens the library GTK already loaded (never closed).
    let mut handle = unsafe {
        libc::dlopen(
            c"libepoxy.so.0".as_ptr(),
            libc::RTLD_LAZY | libc::RTLD_NOLOAD,
        )
    };
    if handle.is_null() {
        // SAFETY: as above, loading it if it was not.
        handle = unsafe { libc::dlopen(c"libepoxy.so.0".as_ptr(), libc::RTLD_LAZY) };
    }
    if handle.is_null() {
        return None;
    }
    Some(move |name: &str| {
        let Ok(symbol) = std::ffi::CString::new(format!("epoxy_{name}")) else {
            return std::ptr::null();
        };
        // SAFETY: libepoxy exports each GL function as a pointer variable
        // named `epoxy_<name>`.
        unsafe {
            let at = libc::dlsym(handle, symbol.as_ptr()) as *const *const c_void;
            if at.is_null() { std::ptr::null() } else { *at }
        }
    })
}

/// Runs `f` with the context current; `None` without GL ES 3.
fn with_gl<R>(display: &gdk::Display, f: impl FnOnce(&mut Gl) -> R) -> Option<R> {
    GL.with(|cell| {
        let mut cell = cell.borrow_mut();
        let gl = cell.get_or_insert_with(|| {
            make(display)
                .map_err(|e| {
                    log::warn!(
                        target: "js",
                        "<Shader>: OpenGL ES 3 is not available ({e}): shader views show their children only"
                    )
                })
                .ok()
        });
        let gl = gl.as_mut()?;
        gl.context.make_current();
        ORPHANS.with(|o| {
            let (textures, pool) = &mut *o.borrow_mut();
            // SAFETY: the context is current; they are its textures.
            unsafe {
                for t in textures.drain(..) {
                    gl.gl.delete_texture(t);
                }
                pool.retain_mut(|p| {
                    if !p.free.load(Ordering::Acquire) {
                        return true;
                    }
                    if let Some(fence) = p.fence.take() {
                        gl.gl.delete_sync(fence);
                    }
                    gl.gl.delete_texture(p.texture);
                    false
                });
            }
        });
        let r = f(gl);
        gdk::GLContext::clear_current();
        Some(r)
    })
}

fn make(display: &gdk::Display) -> Result<Gl, String> {
    let context = display.create_gl_context().map_err(|e| e.to_string())?;
    context.set_allowed_apis(gdk::GLAPI::GLES);
    context.set_required_version(3, 0);
    context.realize().map_err(|e| e.to_string())?;
    context.make_current();
    let loader = loader().ok_or("libepoxy is not loaded")?;
    // SAFETY: the loader returns the functions of the current context.
    let gl = unsafe { glow::Context::from_loader_function(loader) };
    // SAFETY: GL calls with the context current.
    let made = unsafe {
        let vertex_array = gl.create_vertex_array()?;
        let buffers = [
            gl.create_buffer()?,
            gl.create_buffer()?,
            gl.create_buffer()?,
        ];
        let framebuffer = gl.create_framebuffer()?;
        let empty = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(empty));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            1,
            1,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&[0; 4])),
        );
        sampled(&gl);
        Gl {
            context,
            gl,
            programs: HashMap::new(),
            vertex_array,
            buffers,
            framebuffer,
            empty,
        }
    };
    gdk::GLContext::clear_current();
    Ok(made)
}

/// The bound texture samples linearly, clamped, without mipmaps (the
/// default minification filter would make it incomplete).
///
/// # Safety
///
/// The context is current and a texture is bound.
unsafe fn sampled(gl: &glow::Context) {
    for (k, v) in [
        (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
        (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
        (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
        (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
    ] {
        // SAFETY: as the caller promised.
        unsafe { gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32) };
    }
}

/// A program for the views to come (compiled when one first draws it).
pub(crate) fn program(program: &ShaderProgram) {
    let glsl = match &program.code {
        ProgramCode::Glsl(glsl) => Some(glsl.clone()),
        _ => None,
    };
    SOURCES.with(|s| s.borrow_mut().insert(program.id, glsl));
}

/// Program `id` in the context, compiled on first use.
fn compiled(gl: &mut Gl, id: u32) -> Option<&Program> {
    if !gl.programs.contains_key(&id) {
        let source = SOURCES.with(|s| s.borrow().get(&id).cloned().flatten());
        let made = source.and_then(|source| {
            link(&gl.gl, &source)
                .map_err(|e| log::error!(target: "js", "<Shader>: OpenGL did not compile the shader: {e}"))
                .ok()
        });
        gl.programs.insert(id, made);
    }
    gl.programs.get(&id)?.as_ref()
}

fn link(gl: &glow::Context, source: &GlslProgram) -> Result<Program, String> {
    // SAFETY: GL calls with the context current.
    unsafe {
        let stage = |kind, text: &str| -> Result<glow::Shader, String> {
            let s = gl.create_shader(kind)?;
            gl.shader_source(s, text);
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) {
                let log = gl.get_shader_info_log(s);
                gl.delete_shader(s);
                return Err(log);
            }
            Ok(s)
        };
        let vertex = stage(glow::VERTEX_SHADER, &source.vertex)?;
        let fragment = stage(glow::FRAGMENT_SHADER, &source.fragment)?;
        let program = gl.create_program()?;
        gl.attach_shader(program, vertex);
        gl.attach_shader(program, fragment);
        gl.link_program(program);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(log);
        }
        for block in &source.blocks {
            if let Some(index) = gl.get_uniform_block_index(program, &block.name) {
                gl.uniform_block_binding(program, index, block.buffer);
            }
        }
        let samplers = source
            .samplers
            .iter()
            .map(|(name, input)| (gl.get_uniform_location(program, name), *input))
            .collect();
        Ok(Program { program, samplers })
    }
}

/// An image's pixels (RGBA, premultiplied) and size.
struct Pixels {
    width: i32,
    height: i32,
    rgba: Vec<u8>,
}

impl Pixels {
    /// Its size in points: images are as many points as pixels, as a
    /// canvas draws them.
    fn points(&self) -> Size<Points> {
        Size::new(Points(self.width as f32), Points(self.height as f32))
    }
}

/// What a shader view draws.
#[derive(Default)]
pub(crate) struct ShaderState {
    fill: Option<Arc<ShaderFill>>,
    /// The fill's uniforms with the animated values and the images' sizes
    /// written in.
    uniforms: Vec<u8>,
    args: Vec<(u16, AnimationValue)>,
    /// The images by slot, once loaded.
    images: Vec<Option<Rc<Pixels>>>,
    started: Option<Instant>,
    ticking: bool,
    cache: RefCell<Cache>,
}

#[derive(Default)]
struct Cache {
    /// The last picture, while it shows what the view draws now.
    picture: Option<gdk::Texture>,
    /// Rendered textures.
    pool: Vec<Pooled>,
    /// The images' textures in the context, by slot (and the pixels they
    /// hold).
    images: Vec<Option<(Rc<Pixels>, glow::Texture)>>,
}

impl Drop for Cache {
    /// Its textures go when the context is next current (those GTK still
    /// draws once it hands them back).
    fn drop(&mut self) {
        let textures = self
            .images
            .drain(..)
            .flatten()
            .map(|(_, t)| t)
            .collect::<Vec<_>>();
        let pool = std::mem::take(&mut self.pool);
        if textures.is_empty() && pool.is_empty() {
            return;
        }
        ORPHANS.with(|o| {
            let mut o = o.borrow_mut();
            o.0.extend(textures);
            o.1.extend(pool);
        });
    }
}

/// A texture shaders render into.
struct Pooled {
    texture: glow::Texture,
    size: (i32, i32),
    /// GTK handed it back.
    free: Arc<AtomicBool>,
    /// The fence of its last render.
    fence: Option<glow::Fence>,
}

impl ShaderState {
    fn stale(&mut self) {
        self.cache.get_mut().picture = None;
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
                    Some(source) => {
                        // Still loaded: its size goes into the new buffer.
                        if source.size.is_none()
                            && let Some(pixels) = &state.images[slot]
                        {
                            shader::write_image_size(&mut state.uniforms, image, pixels.points());
                        }
                    }
                    None => state.images[slot] = None,
                }
            }
        }
        state.fill = fill;
        state.started.get_or_insert_with(Instant::now);
        state.stale();
    }
    for (slot, uri) in load {
        let weak = view.downgrade();
        let key = uri.clone();
        crate::image::load_bitmap(surface, &uri, move |result| {
            let Some(view) = weak.upgrade() else { return };
            match result {
                Ok(texture) => loaded(&view, slot, &key, &texture),
                Err(e) => log::warn!(target: "js", "<Shader>: {e}"),
            }
        });
    }
    tick(view);
    view.changed();
}

/// The image of `slot` (`uri`'s) loaded.
fn loaded(view: &NshView, slot: usize, uri: &str, texture: &gdk::Texture) {
    let (width, height) = (texture.width(), texture.height());
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied);
    let (bytes, stride) = downloader.download_bytes();
    let row = width as usize * 4;
    let mut rgba = Vec::with_capacity(row * height as usize);
    for line in bytes.chunks(stride).take(height as usize) {
        rgba.extend_from_slice(&line[..row]);
    }
    let pixels = Pixels {
        width,
        height,
        rgba,
    };
    {
        let mut s = view.state_mut();
        let Some(state) = s.shader.as_mut() else {
            return;
        };
        let Some(image) = state
            .fill
            .as_ref()
            .and_then(|f| f.images.get(slot).cloned())
        else {
            return;
        };
        if image.source.as_ref().map(|s| &*s.uri) != Some(uri) {
            return;
        }
        // The size the source said, else the image's.
        if image.source.as_ref().and_then(|s| s.size).is_none() {
            shader::write_image_size(&mut state.uniforms, &image, pixels.points());
        }
        state.images[slot] = Some(Rc::new(pixels));
        state.stale();
    }
    view.changed();
}

/// Animated uniforms' values (`AnimationProp::ShaderUniform`).
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

/// Its size changed: it renders again.
pub(crate) fn resized(view: &NshView) {
    if let Some(state) = view.state_mut().shader.as_mut() {
        state.stale();
    }
    view.changed();
}

/// A shader that reads `nsh.time` draws on every frame of the view's
/// frame clock (which runs while the view is mapped).
fn tick(view: &NshView) {
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
    view.add_tick_callback(|view, _| {
        {
            let mut s = view.state_mut();
            let Some(state) = s.shader.as_mut() else {
                return glib::ControlFlow::Break;
            };
            if !state.fill.as_ref().is_some_and(|f| f.time) {
                state.ticking = false;
                return glib::ControlFlow::Break;
            }
            state.stale();
        }
        view.changed();
        glib::ControlFlow::Continue
    });
}

/// Draws a shader view's picture, rendering it first if it is stale (or
/// its scale changed).
pub(crate) fn draw(snapshot: &gtk::Snapshot, state: &ViewState, widget: &NshView) {
    let Some(shader) = state.shader.as_ref() else {
        return;
    };
    let Some(fill) = &shader.fill else { return };
    let (w, h) = (state.frame.size.width.0, state.frame.size.height.0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let scale = widget.scale_factor().max(1) as f32;
    let mut cache = shader.cache.borrow_mut();
    let pixels = (
        (w * scale).round().max(1.0) as i32,
        (h * scale).round().max(1.0) as i32,
    );
    if cache
        .picture
        .as_ref()
        .is_some_and(|p| (p.width(), p.height()) != pixels)
    {
        cache.picture = None;
    }
    if cache.picture.is_none() {
        let time = shader.started.map_or(0.0, |s| s.elapsed().as_secs_f32());
        let program = fill.program;
        cache.picture = with_gl(&widget.display(), |gl| {
            render(gl, &mut cache, shader, program, (w, h), scale, time)
        })
        .flatten();
    }
    let Some(picture) = &cache.picture else {
        return;
    };
    // The fill follows the view's corners, as a background does.
    let rounded = state.radii().iter().any(|&r| r > 0.0);
    if rounded {
        snapshot.push_rounded_clip(&state.rounded_bounds());
    }
    snapshot.save();
    snapshot.translate(&graphene::Point::new(0.0, h));
    snapshot.scale(1.0, -1.0);
    snapshot.append_texture(picture, &graphene::Rect::new(0.0, 0.0, w, h));
    snapshot.restore();
    if rounded {
        snapshot.pop();
    }
}

/// Renders the view into a texture of the pool.
fn render(
    gl: &mut Gl,
    cache: &mut Cache,
    shader: &ShaderState,
    program: u32,
    (w, h): (f32, f32),
    scale: f32,
    time: f32,
) -> Option<gdk::Texture> {
    let (pw, ph) = (
        (w * scale).round().max(1.0) as i32,
        (h * scale).round().max(1.0) as i32,
    );
    let compiled = compiled(gl, program)?;
    let (program, samplers) = (compiled.program, compiled.samplers.clone());
    let g = &gl.gl;
    let size = Size::new(Points(w), Points(h));
    let nsh = shader::nsh_buffer(size, scale, time, Percentage(0.0));
    let sizes: Vec<(usize, Size<DevicePixels>)> = shader
        .images
        .iter()
        .enumerate()
        .filter_map(|(slot, i)| {
            let i = i.as_ref()?;
            Some((
                slot,
                Size::new(DevicePixels(i.width), DevicePixels(i.height)),
            ))
        })
        .collect();
    let frame = shader::frame_buffer(size, &sizes);
    // SAFETY: GL calls with the context current; the textures and fences
    // are this context's.
    unsafe {
        // Of the textures GTK let go of, those of another size go.
        cache.pool.retain_mut(|p| {
            if !p.free.load(Ordering::Acquire) {
                return true;
            }
            if let Some(fence) = p.fence.take() {
                g.delete_sync(fence);
            }
            let keep = p.size == (pw, ph);
            if !keep {
                g.delete_texture(p.texture);
            }
            keep
        });
        let at = match cache
            .pool
            .iter()
            .position(|p| p.free.load(Ordering::Acquire))
        {
            Some(at) => at,
            None => {
                let texture = g.create_texture().ok()?;
                g.bind_texture(glow::TEXTURE_2D, Some(texture));
                g.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    pw,
                    ph,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                sampled(g);
                cache.pool.push(Pooled {
                    texture,
                    size: (pw, ph),
                    free: Arc::new(AtomicBool::new(true)),
                    fence: None,
                });
                cache.pool.len() - 1
            }
        };
        let texture = cache.pool[at].texture;
        g.bind_framebuffer(glow::FRAMEBUFFER, Some(gl.framebuffer));
        g.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        g.viewport(0, 0, pw, ph);
        g.disable(glow::BLEND);
        g.clear_color(0.0, 0.0, 0.0, 0.0);
        g.clear(glow::COLOR_BUFFER_BIT);
        g.use_program(Some(program));
        g.bind_vertex_array(Some(gl.vertex_array));
        for (i, data) in [&nsh[..], &frame[..], &shader.uniforms[..]]
            .into_iter()
            .enumerate()
        {
            if data.is_empty() {
                continue;
            }
            g.bind_buffer(glow::UNIFORM_BUFFER, Some(gl.buffers[i]));
            g.buffer_data_u8_slice(glow::UNIFORM_BUFFER, data, glow::DYNAMIC_DRAW);
            g.bind_buffer_base(glow::UNIFORM_BUFFER, i as u32, Some(gl.buffers[i]));
        }
        for (unit, (location, input)) in samplers.iter().enumerate() {
            let bound = match input {
                ShaderInput::Image(slot) => shader
                    .images
                    .get(*slot as usize)
                    .and_then(|p| p.as_ref())
                    .and_then(|p| image_texture(g, &mut cache.images, *slot as usize, p)),
                _ => None,
            };
            g.active_texture(glow::TEXTURE0 + unit as u32);
            g.bind_texture(glow::TEXTURE_2D, Some(bound.unwrap_or(gl.empty)));
            g.uniform_1_i32(location.as_ref(), unit as i32);
        }
        g.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        g.bind_framebuffer(glow::FRAMEBUFFER, None);
        g.active_texture(glow::TEXTURE0);
        let fence = g.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0).ok();
        g.flush();
        let pooled = &mut cache.pool[at];
        pooled.free.store(false, Ordering::Release);
        pooled.fence = fence;
        let free = pooled.free.clone();
        let built = gdk::GLTextureBuilder::new()
            .set_context(Some(&gl.context))
            .set_id(texture.0.get())
            .set_width(pw)
            .set_height(ph)
            .set_format(gdk::MemoryFormat::R8g8b8a8Premultiplied)
            .set_sync(fence.map(|f| f.0 as *const c_void))
            .build_with_release_func(move || free.store(true, Ordering::Release));
        Some(built)
    }
}

/// The texture of an image in the context, made once per picture.
///
/// # Safety
///
/// The context is current.
unsafe fn image_texture(
    g: &glow::Context,
    images: &mut Vec<Option<(Rc<Pixels>, glow::Texture)>>,
    slot: usize,
    pixels: &Rc<Pixels>,
) -> Option<glow::Texture> {
    if images.len() <= slot {
        images.resize(slot + 1, None);
    }
    if let Some((held, t)) = &images[slot]
        && Rc::ptr_eq(held, pixels)
    {
        return Some(*t);
    }
    // SAFETY: the context is current (the caller promised); `rgba` holds
    // `width` × `height` pixels.
    unsafe {
        if let Some((_, old)) = images[slot].take() {
            g.delete_texture(old);
        }
        let t = g.create_texture().ok()?;
        g.bind_texture(glow::TEXTURE_2D, Some(t));
        g.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            pixels.width,
            pixels.height,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&pixels.rgba)),
        );
        sampled(g);
        images[slot] = Some((pixels.clone(), t));
        Some(t)
    }
}
