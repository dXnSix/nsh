//! Metal on Apple platforms (iOS, macOS): NSH's shader programs
//! (`nsh_core::shader`, in MSL) drawn into a `CAMetalLayer` the platform
//! puts under a shader view's children (`docs/dev/shaders.md`).
//!
//! Metal is loaded on first use (`dlopen`), as WebKit and AVFoundation
//! are, so an app without shaders does not load it at launch
//! (`docs/dev/pitfalls.md`): every call goes through the Objective-C
//! runtime (`msg_send!` on `AnyObject`); `CAMetalLayer` is QuartzCore's,
//! which every app links.
//!
//! A program compiles off the main thread (Metal's library, then the
//! render pipeline, the GPU compiler's work: tens of milliseconds the first
//! time) and is kept by id on the main thread's [`Gpu`]; a view whose
//! program is not ready draws nothing, and the platform draws it again
//! when [`Gpu::set_on_ready`]'s callback names the program.
//!
//! A layer draws only while fewer than [`MAX_IN_FLIGHT`] of its drawables
//! wait to be shown: `nextDrawable` blocks the main thread (up to a second)
//! when the system holds them all, which it does while the window is
//! occluded. A draw it skips leaves the layer stale; it draws when a
//! drawable is shown ([`Gpu::set_on_presented`]).

#![cfg(target_vendor = "apple")]

use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    ptr::NonNull,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Instant,
};

use nsh_core::{
    animation::AnimationValue,
    geometry::{DevicePixels, Percentage, Points, Size},
    shader::{self, ProgramCode, ShaderFill, ShaderInput, ShaderProgram},
};
use objc2::{
    encode::{Encode, Encoding, RefEncode},
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_core_foundation::{CGFloat, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSError, NSString};

type Id = Retained<AnyObject>;

/// `MTLPixelFormatBGRA8Unorm`: the layer's drawables.
const BGRA8_UNORM: usize = 80;
/// `MTLPixelFormatRGBA8Unorm`: images.
const RGBA8_UNORM: usize = 70;
/// `MTLPrimitiveTypeTriangleStrip`.
const TRIANGLE_STRIP: usize = 4;
/// `MTLLoadActionClear`.
const LOAD_CLEAR: usize = 2;
/// `MTLStoreActionStore`.
const STORE: usize = 1;
/// Bytes `set…Bytes:` takes; more goes into a buffer.
const MAX_INLINE_BYTES: usize = 4096;
/// Drawables a layer has waiting to be shown at most.
pub const MAX_IN_FLIGHT: usize = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct ClearColor {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

// SAFETY: `MTLClearColor`'s layout and encoding.
unsafe impl Encode for ClearColor {
    const ENCODING: Encoding = Encoding::Struct("?", &[f64::ENCODING; 4]);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Triple(usize, usize, usize);

// SAFETY: `MTLOrigin`'s and `MTLSize`'s layout and encoding.
unsafe impl Encode for Triple {
    const ENCODING: Encoding = Encoding::Struct("?", &[usize::ENCODING; 3]);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Region {
    origin: Triple,
    size: Triple,
}

// SAFETY: `MTLRegion`'s layout and encoding.
unsafe impl Encode for Region {
    const ENCODING: Encoding = Encoding::Struct("?", &[Triple::ENCODING, Triple::ENCODING]);
}

// SAFETY: a pointer to a `Region` is encoded as such.
unsafe impl RefEncode for Region {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

/// `MTLCreateSystemDefaultDevice`, once Metal is loaded.
type CreateDevice = unsafe extern "C" fn() -> *mut AnyObject;

fn create_device() -> Option<CreateDevice> {
    static LOADED: OnceLock<Option<usize>> = OnceLock::new();
    let f = (*LOADED.get_or_init(|| {
        let path = c"/System/Library/Frameworks/Metal.framework/Metal";
        // SAFETY: loads a system framework; the handle is never closed.
        let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY) };
        if handle.is_null() {
            log::error!("shaders: cannot load Metal");
            return None;
        }
        // SAFETY: a symbol of the framework just loaded.
        let f = unsafe { libc::dlsym(handle, c"MTLCreateSystemDefaultDevice".as_ptr()) };
        (!f.is_null()).then_some(f as usize)
    }))?;
    // SAFETY: `MTLCreateSystemDefaultDevice`'s signature.
    Some(unsafe { std::mem::transmute::<usize, CreateDevice>(f) })
}

fn class(name: &std::ffi::CStr) -> Option<&'static AnyClass> {
    AnyClass::get(name)
}

/// A drawable that reports no presentation is not counted.
fn in_flight_without_handler(in_flight: &std::sync::atomic::AtomicUsize) {
    in_flight.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
}

/// A Metal object handed to another thread: devices, libraries and
/// pipeline states are thread-safe.
struct Shared<T>(T);

// SAFETY: only Metal's thread-safe objects (devices, libraries, pipeline
// states) and plain data are wrapped.
unsafe impl<T> Send for Shared<T> {}

impl<T> Shared<T> {
    /// The value (a method, so a closure captures the whole wrapper).
    fn into_inner(self) -> T {
        self.0
    }
}

/// A program ready to draw.
struct Pipeline {
    state: Id,
    /// The inputs its fragment function samples, by texture index.
    textures: Vec<ShaderInput>,
}

enum Program {
    Compiling,
    Ready(Rc<Pipeline>),
    Failed,
}

/// The main thread's Metal device, its queue and the programs it compiled.
pub struct Gpu {
    device: Id,
    queue: Id,
    programs: RefCell<HashMap<u32, Program>>,
    on_ready: RefCell<Option<OnReady>>,
    on_presented: RefCell<Option<OnPresented>>,
    /// A transparent texture for inputs that have none.
    empty: Id,
}

type OnReady = Rc<dyn Fn(u32)>;
type OnPresented = Rc<dyn Fn()>;

thread_local! {
    static GPU: RefCell<Option<Option<Rc<Gpu>>>> = const { RefCell::new(None) };
}

/// The main thread's GPU (made on first use, which loads Metal); `None`
/// where Metal is missing.
pub fn gpu() -> Option<Rc<Gpu>> {
    GPU.with(|g| g.borrow_mut().get_or_insert_with(Gpu::new).clone())
}

impl Gpu {
    fn new() -> Option<Rc<Self>> {
        let create = create_device()?;
        // SAFETY: Metal's function; it returns a retained device or nil.
        let device = unsafe { Retained::from_raw(create()) }?;
        // SAFETY: `MTLDevice`'s API.
        let queue: Id = unsafe { msg_send![&*device, newCommandQueue] };
        let empty = texture(&device, 1, 1, &[0, 0, 0, 0])?;
        Some(Rc::new(Self {
            device,
            queue,
            programs: RefCell::default(),
            on_ready: RefCell::default(),
            on_presented: RefCell::default(),
            empty,
        }))
    }

    /// Calls `f` with a program's id when it is ready to draw.
    pub fn set_on_ready(&self, f: impl Fn(u32) + 'static) {
        *self.on_ready.borrow_mut() = Some(Rc::new(f));
    }

    /// Calls `f` after a layer's drawable was shown: the stale layers draw
    /// ([`ShaderLayer::is_stale`]).
    pub fn set_on_presented(&self, f: impl Fn() + 'static) {
        *self.on_presented.borrow_mut() = Some(Rc::new(f));
    }

    /// Compiles `program` (once per id) off the main thread.
    pub fn add_program(&self, program: &ShaderProgram) {
        let id = program.id;
        if self.programs.borrow().contains_key(&id) {
            return;
        }
        let ProgramCode::Msl(msl) = &program.code else {
            self.programs.borrow_mut().insert(id, Program::Failed);
            return;
        };
        self.programs.borrow_mut().insert(id, Program::Compiling);
        let work = Shared((
            self.device.clone(),
            msl.source.clone(),
            msl.vertex.clone(),
            msl.fragment.clone(),
            msl.textures.clone(),
        ));
        let queue = dispatch2::DispatchQueue::global_queue(
            dispatch2::GlobalQueueIdentifier::QualityOfService(
                dispatch2::DispatchQoS::UserInitiated,
            ),
        );
        queue.exec_async(move || {
            let (device, source, vertex, fragment, textures) = work.into_inner();
            let result = Shared(
                compile(&device, &source, &vertex, &fragment).map(|state| (state, textures)),
            );
            dispatch2::DispatchQueue::main().exec_async(move || {
                let result = result.into_inner();
                let Some(gpu) = GPU.with(|g| g.borrow().clone().flatten()) else {
                    return;
                };
                gpu.finish(id, result);
            });
        });
    }

    fn finish(&self, id: u32, result: Result<(Id, Vec<ShaderInput>), String>) {
        let program = match result {
            Ok((state, textures)) => Program::Ready(Rc::new(Pipeline { state, textures })),
            Err(e) => {
                log::error!(target: "js", "<Shader>: Metal could not compile the shader: {e}");
                Program::Failed
            }
        };
        let ready = matches!(program, Program::Ready(_));
        self.programs.borrow_mut().insert(id, program);
        let on_ready = self.on_ready.borrow().clone();
        if ready && let Some(f) = on_ready {
            f(id);
        }
    }

    fn pipeline(&self, id: u32) -> Option<Rc<Pipeline>> {
        match self.programs.borrow().get(&id) {
            Some(Program::Ready(p)) => Some(p.clone()),
            _ => None,
        }
    }
}

/// The program's library, functions and render pipeline state.
fn compile(device: &AnyObject, source: &str, vertex: &str, fragment: &str) -> Result<Id, String> {
    let describe = |e: Retained<NSError>| e.localizedDescription().to_string();
    // SAFETY: `MTLDevice`'s, `MTLLibrary`'s and the descriptor's API; nil
    // options compile with the defaults.
    unsafe {
        let library: Result<Id, Retained<NSError>> = msg_send![
            device,
            newLibraryWithSource: &*NSString::from_str(source),
            options: std::ptr::null::<AnyObject>(),
            error: _
        ];
        let library = library.map_err(describe)?;
        let function = |name: &str| -> Result<Id, String> {
            let f: Option<Id> =
                msg_send![&*library, newFunctionWithName: &*NSString::from_str(name)];
            f.ok_or_else(|| format!("no function `{name}`"))
        };
        let (vertex, fragment) = (function(vertex)?, function(fragment)?);
        let descriptor: Id = msg_send![
            class(c"MTLRenderPipelineDescriptor").ok_or("Metal is missing")?,
            new
        ];
        let _: () = msg_send![&*descriptor, setVertexFunction: &*vertex];
        let _: () = msg_send![&*descriptor, setFragmentFunction: &*fragment];
        let attachments: Id = msg_send![&*descriptor, colorAttachments];
        let color: Id = msg_send![&*attachments, objectAtIndexedSubscript: 0usize];
        let _: () = msg_send![&*color, setPixelFormat: BGRA8_UNORM];
        let state: Result<Id, Retained<NSError>> =
            msg_send![device, newRenderPipelineStateWithDescriptor: &*descriptor, error: _];
        state.map_err(describe)
    }
}

/// A 2D texture of `width` × `height` premultiplied RGBA pixels.
fn texture(device: &AnyObject, width: usize, height: usize, rgba: &[u8]) -> Option<Id> {
    // SAFETY: `MTLTextureDescriptor`'s and `MTLTexture`'s API; `rgba` holds
    // `width * height * 4` bytes.
    unsafe {
        let descriptor: Id = msg_send![
            class(c"MTLTextureDescriptor")?,
            texture2DDescriptorWithPixelFormat: RGBA8_UNORM,
            width: width,
            height: height,
            mipmapped: false
        ];
        let texture: Option<Id> = msg_send![device, newTextureWithDescriptor: &*descriptor];
        let texture = texture?;
        let region = Region {
            origin: Triple(0, 0, 0),
            size: Triple(width, height, 1),
        };
        let _: () = msg_send![
            &*texture,
            replaceRegion: region,
            mipmapLevel: 0usize,
            withBytes: rgba.as_ptr().cast::<c_void>(),
            bytesPerRow: width * 4
        ];
        Some(texture)
    }
}

/// The pixels of `image` as premultiplied sRGB RGBA, top row first.
fn pixels(image: &CGImage) -> (usize, usize, Vec<u8>) {
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGImageByteOrderInfo,
        kCGColorSpaceSRGB,
    };
    let (w, h) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    let mut out = vec![0u8; w * h * 4];
    // SAFETY: `out` holds `w * 4 * h` bytes and outlives the context.
    unsafe {
        let Some(space) = CGColorSpace::with_name(Some(kCGColorSpaceSRGB)) else {
            return (0, 0, Vec::new());
        };
        let Some(context) = CGBitmapContextCreate(
            out.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0,
        ) else {
            return (0, 0, Vec::new());
        };
        CGContext::draw_image(
            Some(&context),
            CGRect::new(CGPoint::ZERO, CGSize::new(w as f64, h as f64)),
            Some(image),
        );
    }
    (w, h, out)
}

/// What a shader view draws, in a `CAMetalLayer` the platform puts in its
/// layer, under its children.
pub struct ShaderLayer {
    layer: Id,
    fill: Option<Arc<ShaderFill>>,
    /// The fill's uniforms with the animated values written in.
    uniforms: Vec<u8>,
    /// Animated values, written again into a new fill's uniforms.
    args: Vec<(u16, AnimationValue)>,
    /// The images, by slot.
    textures: Vec<Option<(Id, Size<DevicePixels>)>>,
    /// Its size in points and its scale.
    size: (f64, f64),
    scale: f64,
    /// When it first drew (`nsh.time`).
    started: Option<Instant>,
    /// Its drawables presented and not shown yet.
    in_flight: Arc<std::sync::atomic::AtomicUsize>,
    /// A draw was skipped: it draws when a drawable is shown.
    stale: bool,
}

impl ShaderLayer {
    /// A new layer, or `None` without Metal (the view shows its children
    /// only).
    pub fn new() -> Option<Self> {
        let gpu = gpu()?;
        // SAFETY: `CAMetalLayer`'s API (QuartzCore).
        let layer: Id = unsafe {
            let layer: Id = msg_send![class(c"CAMetalLayer")?, new];
            let _: () = msg_send![&*layer, setDevice: &*gpu.device];
            let _: () = msg_send![&*layer, setPixelFormat: BGRA8_UNORM];
            let _: () = msg_send![&*layer, setFramebufferOnly: true];
            let _: () = msg_send![&*layer, setOpaque: false];
            let _: () = msg_send![&*layer, setAnchorPoint: CGPoint::new(0.0, 0.0)];
            // Shaders' colors are sRGB: the layer says so, so the system
            // shows them as other views' colors.
            if let Some(space) = objc2_core_graphics::CGColorSpace::with_name(Some(
                objc2_core_graphics::kCGColorSpaceSRGB,
            )) {
                let space = &*space as *const objc2_core_graphics::CGColorSpace;
                let _: () = msg_send![&*layer, setColorspace: space];
            }
            layer
        };
        Some(Self {
            layer,
            fill: None,
            uniforms: Vec::new(),
            args: Vec::new(),
            textures: Vec::new(),
            size: (0.0, 0.0),
            scale: 1.0,
            started: None,
            in_flight: Arc::default(),
            stale: false,
        })
    }

    /// The `CAMetalLayer` (a `CALayer`) to add to the view's layer.
    pub fn layer(&self) -> &AnyObject {
        &self.layer
    }

    /// The view's size in points and its scale: the layer covers it.
    pub fn set_frame(&mut self, width: f64, height: f64, scale: f64) -> bool {
        if self.size == (width, height) && self.scale == scale {
            return false;
        }
        self.size = (width, height);
        self.scale = scale;
        let pixels = CGSize::new(
            (width * scale).round().max(1.0),
            (height * scale).round().max(1.0),
        );
        // SAFETY: `CALayer`'s and `CAMetalLayer`'s API.
        unsafe {
            let _: () = msg_send![&*self.layer, setFrame: CGRect::new(CGPoint::ZERO, CGSize::new(width, height))];
            let _: () = msg_send![&*self.layer, setContentsScale: scale as CGFloat];
            let _: () = msg_send![&*self.layer, setDrawableSize: pixels];
        }
        true
    }

    /// What it draws now; the URIs of its images to load (slot, URI), new
    /// ones only.
    pub fn set_fill(&mut self, fill: Option<Arc<ShaderFill>>) -> Vec<(usize, Arc<str>)> {
        let old = self.fill.take();
        let mut load = Vec::new();
        if let Some(fill) = &fill {
            self.uniforms = fill.uniforms.to_vec();
            for &(k, value) in &self.args {
                shader::write_animated(&mut self.uniforms, k, value);
            }
            self.textures.resize(fill.images.len(), None);
            for (slot, image) in fill.images.iter().enumerate() {
                let before = old
                    .as_ref()
                    .and_then(|o| o.images.get(slot))
                    .and_then(|i| i.source.as_ref());
                match &image.source {
                    Some(source) if before.map(|b| &b.uri) != Some(&source.uri) => {
                        self.textures[slot] = None;
                        load.push((slot, source.uri.clone()));
                    }
                    None => self.textures[slot] = None,
                    _ => {}
                }
            }
        } else {
            self.uniforms.clear();
            self.textures.clear();
        }
        self.fill = fill;
        load
    }

    /// Animated uniforms' values (every bound one, each frame): whether
    /// any changed.
    pub fn set_args(&mut self, values: &[(u16, AnimationValue)]) -> bool {
        if self.args == values {
            return false;
        }
        self.args.clear();
        self.args.extend_from_slice(values);
        if self.fill.is_some() {
            for &(k, value) in values {
                shader::write_animated(&mut self.uniforms, k, value);
            }
        }
        true
    }

    /// The image of `slot` loaded, `points` its size in points; `uri` names
    /// the source it is of (a newer one may be loading).
    pub fn set_image(
        &mut self,
        slot: usize,
        uri: &str,
        image: &CGImage,
        points: (f64, f64),
    ) -> bool {
        let Some(fill) = self.fill.clone() else {
            return false;
        };
        let Some(entry) = fill.images.get(slot) else {
            return false;
        };
        if entry.source.as_ref().map(|s| &*s.uri) != Some(uri) {
            return false;
        }
        let Some(gpu) = gpu() else { return false };
        let (w, h, rgba) = pixels(image);
        if w == 0 || h == 0 {
            return false;
        }
        let pixels = Size::new(DevicePixels(w as i32), DevicePixels(h as i32));
        self.textures[slot] = texture(&gpu.device, w, h, &rgba).map(|t| (t, pixels));
        // The size the source said first, else the image's.
        if entry.source.as_ref().and_then(|s| s.size).is_none() {
            shader::write_image_size(&mut self.uniforms, entry, size(points.0, points.1));
        }
        true
    }

    /// The loaded images by slot and their sizes in pixels: the frame
    /// buffer places each over the view (without, its samples are
    /// transparent).
    fn images(&self) -> Vec<(usize, Size<DevicePixels>)> {
        self.textures
            .iter()
            .enumerate()
            .filter_map(|(slot, t)| Some((slot, t.as_ref()?.1)))
            .collect()
    }

    /// Whether it draws every frame (it reads `nsh.time`).
    pub fn draws_every_frame(&self) -> bool {
        self.fill.as_ref().is_some_and(|f| f.time)
    }

    /// The program it draws, if any.
    pub fn program(&self) -> Option<u32> {
        self.fill.as_ref().map(|f| f.program)
    }

    /// Whether a draw was skipped since it last drew.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Draws it into a new drawable, now. Nothing when it has no fill, its
    /// program is not ready or it has no size; later (stale) while its
    /// drawables wait to be shown.
    pub fn draw(&mut self) {
        use std::sync::atomic::Ordering;
        if self.in_flight.load(Ordering::Acquire) >= MAX_IN_FLIGHT {
            self.stale = true;
            return;
        }
        self.stale = false;
        let (width, height) = self.size;
        let Some(fill) = &self.fill else { return };
        let Some(gpu) = gpu() else { return };
        let Some(pipeline) = gpu.pipeline(fill.program) else {
            return;
        };
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let time = self
            .started
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f32();
        let nsh = shader::nsh_buffer(
            size(width, height),
            self.scale as f32,
            time,
            Percentage(0.0),
        );
        let frame = shader::frame_buffer(size(width, height), &self.images());
        // SAFETY: Metal's API; the bytes outlive the calls that copy them
        // (`set…Bytes:` copies, a buffer is made from them).
        unsafe {
            let drawable: Option<Id> = msg_send![&*self.layer, nextDrawable];
            let Some(drawable) = drawable else { return };
            let target: Id = msg_send![&*drawable, texture];
            let Some(pass_class) = class(c"MTLRenderPassDescriptor") else {
                return;
            };
            let pass: Id = msg_send![pass_class, renderPassDescriptor];
            let attachments: Id = msg_send![&*pass, colorAttachments];
            let color: Id = msg_send![&*attachments, objectAtIndexedSubscript: 0usize];
            let _: () = msg_send![&*color, setTexture: &*target];
            let _: () = msg_send![&*color, setLoadAction: LOAD_CLEAR];
            let _: () = msg_send![&*color, setStoreAction: STORE];
            let clear = ClearColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.0,
            };
            let _: () = msg_send![&*color, setClearColor: clear];
            let buffer: Option<Id> = msg_send![&*gpu.queue, commandBuffer];
            let Some(buffer) = buffer else { return };
            let encoder: Option<Id> =
                msg_send![&*buffer, renderCommandEncoderWithDescriptor: &*pass];
            let Some(encoder) = encoder else { return };
            let _: () = msg_send![&*encoder, setRenderPipelineState: &*pipeline.state];
            let bytes = |stage: &str, data: &[u8], index: usize| {
                if data.is_empty() {
                    return;
                }
                if data.len() <= MAX_INLINE_BYTES {
                    let ptr = data.as_ptr().cast::<c_void>();
                    match stage {
                        "vertex" => {
                            let _: () = msg_send![&*encoder, setVertexBytes: ptr, length: data.len(), atIndex: index];
                        }
                        _ => {
                            let _: () = msg_send![&*encoder, setFragmentBytes: ptr, length: data.len(), atIndex: index];
                        }
                    }
                } else {
                    let b: Option<Id> = msg_send![
                        &*gpu.device,
                        newBufferWithBytes: data.as_ptr().cast::<c_void>(),
                        length: data.len(),
                        options: 0usize
                    ];
                    let Some(b) = b else { return };
                    match stage {
                        "vertex" => {
                            let _: () = msg_send![&*encoder, setVertexBuffer: &*b, offset: 0usize, atIndex: index];
                        }
                        _ => {
                            let _: () = msg_send![&*encoder, setFragmentBuffer: &*b, offset: 0usize, atIndex: index];
                        }
                    }
                }
            };
            bytes("vertex", &frame, shader_index::FRAME);
            bytes("fragment", &nsh, shader_index::NSH);
            bytes("fragment", &frame, shader_index::FRAME);
            bytes("fragment", &self.uniforms, shader_index::UNIFORMS);
            for (index, input) in pipeline.textures.iter().enumerate() {
                let texture = match input {
                    ShaderInput::Image(slot) => self
                        .textures
                        .get(*slot as usize)
                        .cloned()
                        .flatten()
                        .map(|(t, _)| t),
                    _ => None,
                }
                .unwrap_or_else(|| gpu.empty.clone());
                let _: () = msg_send![&*encoder, setFragmentTexture: &*texture, atIndex: index];
            }
            let _: () = msg_send![&*encoder, drawPrimitives: TRIANGLE_STRIP, vertexStart: 0usize, vertexCount: 4usize];
            let _: () = msg_send![&*encoder, endEncoding];
            // Counted until shown (on a thread of Metal's; the main thread
            // draws the stale layers then).
            self.in_flight.fetch_add(1, Ordering::AcqRel);
            let in_flight = self.in_flight.clone();
            let shown = block2::RcBlock::new(move |_drawable: NonNull<AnyObject>| {
                in_flight.fetch_sub(1, Ordering::AcqRel);
                dispatch2::DispatchQueue::main().exec_async(|| {
                    let Some(gpu) = GPU.with(|g| g.borrow().clone().flatten()) else {
                        return;
                    };
                    let f = gpu.on_presented.borrow().clone();
                    if let Some(f) = f {
                        f();
                    }
                });
            });
            // The drawable's class may forward the message (the simulator's
            // does), which objc2's check of the signature cannot see.
            let sel = objc2::sel!(addPresentedHandler:);
            let responds: bool = msg_send![&*drawable, respondsToSelector: sel];
            if responds {
                // SAFETY: `addPresentedHandler:` takes one block.
                let send: unsafe extern "C-unwind" fn(
                    &AnyObject,
                    objc2::runtime::Sel,
                    &block2::Block<dyn Fn(NonNull<AnyObject>)>,
                ) = std::mem::transmute(objc2::ffi::objc_msgSend as unsafe extern "C-unwind" fn());
                send(&drawable, sel, &shown);
            } else {
                in_flight_without_handler(&self.in_flight);
            }
            let _: () = msg_send![&*buffer, presentDrawable: &*drawable];
            let _: () = msg_send![&*buffer, commit];
        }
    }

    /// What it shows as a picture (for in-process screenshots and the
    /// pictures of screens that leave, which do not draw a `CAMetalLayer`):
    /// drawn again into a texture the CPU reads. `None` without a fill.
    pub fn picture(&mut self) -> Option<(usize, usize, Vec<u8>)> {
        let (width, height) = self.size;
        let fill = self.fill.clone()?;
        let gpu = gpu()?;
        let pipeline = gpu.pipeline(fill.program)?;
        let (pw, ph) = (
            (width * self.scale).round() as usize,
            (height * self.scale).round() as usize,
        );
        if pw == 0 || ph == 0 {
            return None;
        }
        let time = self
            .started
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f32();
        let nsh = shader::nsh_buffer(
            size(width, height),
            self.scale as f32,
            time,
            Percentage(0.0),
        );
        let frame = shader::frame_buffer(size(width, height), &self.images());
        // SAFETY: Metal's API, as `draw`'s; the target is shared storage
        // (readable once the command buffer completed).
        unsafe {
            let descriptor: Id = msg_send![
                class(c"MTLTextureDescriptor")?,
                texture2DDescriptorWithPixelFormat: BGRA8_UNORM,
                width: pw,
                height: ph,
                mipmapped: false
            ];
            // `MTLTextureUsageRenderTarget`.
            let _: () = msg_send![&*descriptor, setUsage: 4usize];
            let target: Option<Id> =
                msg_send![&*gpu.device, newTextureWithDescriptor: &*descriptor];
            let target = target?;
            let pass: Id = msg_send![class(c"MTLRenderPassDescriptor")?, renderPassDescriptor];
            let attachments: Id = msg_send![&*pass, colorAttachments];
            let color: Id = msg_send![&*attachments, objectAtIndexedSubscript: 0usize];
            let _: () = msg_send![&*color, setTexture: &*target];
            let _: () = msg_send![&*color, setLoadAction: LOAD_CLEAR];
            let _: () = msg_send![&*color, setStoreAction: STORE];
            let buffer: Id = msg_send![&*gpu.queue, commandBuffer];
            let encoder: Id = msg_send![&*buffer, renderCommandEncoderWithDescriptor: &*pass];
            let _: () = msg_send![&*encoder, setRenderPipelineState: &*pipeline.state];
            let set = |data: &[u8], index: usize, vertex: bool| {
                if data.is_empty() {
                    return;
                }
                let b: Option<Id> = msg_send![&*gpu.device, newBufferWithBytes: data.as_ptr().cast::<c_void>(), length: data.len(), options: 0usize];
                let Some(b) = b else { return };
                if vertex {
                    let _: () =
                        msg_send![&*encoder, setVertexBuffer: &*b, offset: 0usize, atIndex: index];
                } else {
                    let _: () = msg_send![&*encoder, setFragmentBuffer: &*b, offset: 0usize, atIndex: index];
                }
            };
            set(&frame, shader_index::FRAME, true);
            set(&nsh, shader_index::NSH, false);
            set(&frame, shader_index::FRAME, false);
            set(&self.uniforms, shader_index::UNIFORMS, false);
            for (index, input) in pipeline.textures.iter().enumerate() {
                let texture = match input {
                    ShaderInput::Image(slot) => self
                        .textures
                        .get(*slot as usize)
                        .cloned()
                        .flatten()
                        .map(|(t, _)| t),
                    _ => None,
                }
                .unwrap_or_else(|| gpu.empty.clone());
                let _: () = msg_send![&*encoder, setFragmentTexture: &*texture, atIndex: index];
            }
            let _: () = msg_send![&*encoder, drawPrimitives: TRIANGLE_STRIP, vertexStart: 0usize, vertexCount: 4usize];
            let _: () = msg_send![&*encoder, endEncoding];
            let _: () = msg_send![&*buffer, commit];
            let _: () = msg_send![&*buffer, waitUntilCompleted];
            let mut bgra = vec![0u8; pw * ph * 4];
            let region = Region {
                origin: Triple(0, 0, 0),
                size: Triple(pw, ph, 1),
            };
            let _: () = msg_send![
                &*target,
                getBytes: NonNull::new(bgra.as_mut_ptr()).expect("pixels").as_ptr().cast::<c_void>(),
                bytesPerRow: pw * 4,
                fromRegion: region,
                mipmapLevel: 0usize
            ];
            // BGRA to RGBA.
            for px in bgra.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
            Some((pw, ph, bgra))
        }
    }
}

/// The buffer indexes of [`nsh_core::shader::MslProgram`].
mod shader_index {
    pub(crate) const NSH: usize = nsh_core::shader::MslProgram::NSH as usize;
    pub(crate) const FRAME: usize = nsh_core::shader::MslProgram::FRAME as usize;
    pub(crate) const UNIFORMS: usize = nsh_core::shader::MslProgram::UNIFORMS as usize;
}

/// A size in points from AppKit's / UIKit's.
fn size(width: f64, height: f64) -> Size<Points> {
    Size::new(Points(width as f32), Points(height as f32))
}
