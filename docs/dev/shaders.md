# Shaders

Apps write fragment shaders in WGSL; NSH checks them against a profile every platform runs, reflects them and
translates them into each platform's language (`crates/nsh-shader`). Nothing NSH ships is a shader of its own (no
built-in looks: "No styles in NSH"); the corpus in `crates/nsh-shader/tests/shader/corpus/` (fills, effects, a
glass, transitions) is test material and a reference for apps, not an API.

## The language

A shader is WGSL with one function the app writes:

```wgsl
struct Uniforms { tint: vec4f, radius: f32 }

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let d = length(p - nsh.size * 0.5);
    return u.tint * (1.0 - smoothstep(u.radius - 1.0, u.radius, d));
}
```

- `p` is the pixel's position **in points from the view's top-left**; the result is a **premultiplied sRGB** color.
  `fn main(p: vec2f) -> vec4f` when the shader takes no uniforms.
- **Uniforms** are the fields of the struct `main` takes (any name; `Uniforms` by convention): `f32`, `i32`, `vec2/3/4`
  of them, square `f32` matrices, fixed arrays of those (WGSL's uniform layout: an array's elements 16 bytes apart, so
  `array<vec4f, N>` rather than `array<f32, N>`), and `Image`. The struct is never a value: `u.field` reads a field
  where it is (`let v = u;` names it, a helper may take it as a parameter), and the shader neither copies it into a
  `var` nor makes one.
- **NSH's prelude**, appended to the source (so naga's spans stay the app's), declares `nsh` (`size`: the view's size in
  points, `scale`: pixels per point, `time`: seconds since the shader first drew, `progress`: an effect's or a
  transition's progress, 0 to 1) and the inputs: `sample_content(p)` (what the element draws, under an effect),
  `sample_backdrop(p)` (what is behind it), `sample_from(p)` / `sample_to(p)` (a transition's two sides) and
  `sample_image(u.field, p)` for an `Image` uniform, each taking a position in points like `p` (an image is stretched
  over the view; `u.field.size` is its size in points, for fitting it). Outside an input's bounds a sample is
  transparent. Names starting with `nsh_` are NSH's.

## The profile

Android's runtime shaders (AGSL, API 33) are GLSL ES 1.00 with its Appendix A, so the profile is the WGSL that AGSL can
say too, checked on naga's IR (`profile.rs`), the same on every platform:

- **Loops are counted `for` loops**: `for (var i = start; i < bound; i += step)` with a constant start, bound and step
  (`<`, `<=`, `>`, `>=`, `!=`; `i++`, `i--`, `+=`, `-=`; `i32` or `f32` counters), a body that leaves the counter
  alone and reads it only inside the loop. No `while` or `loop`. The checker runs each loop to count its iterations
  and the counter's range. (naga's IR has only `loop`; `loop_shape` recognizes the `for` in it, also written out as
  `loop { if !(i < n) { break; } ... continuing { i++; } }`.)
- **Indexes are constants or loop counters** with `+`, `-`, `*` (`a[i + 1]`, `a[2 * i]`), and the checker proves them
  in bounds from the counters' ranges (an index out of range is an error at build time; naga's writers still clamp,
  `BoundsCheckPolicy::Restrict`).
- **Not in the profile**: derivatives (`dpdx`, `fwidth`), `discard`, `u32` and bit operations, `bitcast`, `override`,
  pointers as parameters, arrays as results or inside structs (other than the uniforms'), arrays of arrays,
  non-square matrices, `break` out of a `switch`, the packing and bit functions, and global variables or resources
  of the shader's own (its inputs are `nsh`, its uniforms and the `sample_*` functions; constants are `const`).
- **Limits** (`Limits`, enforced in Rust when a shader compiles): `max_samples`, texture samples a pixel takes at most
  (default 128: a 75-sample glass fits), counting each `sample_*` call, multiplied by the loops around it, the costlier
  branch of an `if` (a branch that returns does not add what follows it); `max_iterations`, loop iterations a pixel
  runs (4096); `max_source_len` (64 KiB).

Errors carry their place in the app's source (`Error { message, span, line, column, notes }`; `Error::render` prints
`path:line:column`, the line and a caret), naga's parse and validation errors as well as the profile's.

## Reflection

`Shader::reflection()`: the uniforms (name, type, byte offset, array length and stride, in WGSL's uniform layout;
images with their slot), the size of their buffer, the inputs `main` samples, whether it reads `nsh.time` (the shader
draws every frame while it is shown; otherwise it draws only when something it reads changes) and `nsh.progress`, and
its cost (samples, iterations).

## Programs

A program reads three buffers, filled by the platform (`nsh_shader::layout`): `nsh` (size, scale, time, progress),
the frame (the area it draws, in points; per input its rect in points, flipped by a negative height, and its size in
its own pixels) and the app's uniforms. Each writer is a feature, so a runtime links only its own:

| Feature | Platforms | Writer | Program |
|---|---|---|---|
| `msl` | iOS, macOS | naga's MSL | `nsh_vertex` / `nsh_fragment` over a 4-vertex strip; buffers 0, 1, 2; textures 0...; an inline sampler (linear, clamped to zero) |
| `hlsl` | Windows | naga's HLSL, shader model 5.0 | a Direct2D effect's pixel shader (`NshD2dMain`, one constant buffer `b0`, each input at Direct2D's texel coordinates); Direct3D 11 shaders as the fallback (`b0`..`b2`, `t0`..., `s0`) |
| `glsl` | Linux, the web | naga's GLSL, ES 3.00 / 3.30 core | a vertex and a fragment shader; uniform blocks and samplers by name |
| `agsl` | Android | NSH's own (`agsl.rs`) | a `RuntimeShader` source: each uniform field a `uniform` of its own, each input a `uniform shader` |

naga's HLSL declares samplers as indexes into a Direct3D 12 sampler heap; `hlsl.rs` replaces that with one sampler at
`s0` for Direct3D 11 and Direct2D. The AGSL writer maps naga's IR to AGSL as naga's writers map it to theirs (the same
expressions kept in variables), and writes out what ES 1.00 lacks: counted loops as `for` loops (their counters read
in place, never through a variable: ES 1.00 indexes with loop indexes and constants only), `switch` as an `if` chain,
WGSL's integer `/` and `%` (by zero: `x` and 0), its truncating float `%`, `round` (halves to even), `trunc`, the
hyperbolic functions, integer `abs` / `min` / `max` / `clamp` / `sign` / `dot`, `transpose`, `determinant`, and
arrays element by element (constant arrays as local arrays).

## The element

`<Shader src uniforms>` (TSX, from `nsh`) and `shader(source).uniforms(..)` (Rust, `nsh_core::ui`) are a host element,
`Tag::Shader` / `ViewKind::Shader`: a view the shader fills under its children, with every style of a view.
`<ShaderEffect src uniforms progress maxSampleOffset>` and `shader_effect(source)` are another, `Tag::ShaderEffect` /
`ViewKind::ShaderEffect`: a view whose children draw through the shader. They are its content input
(`sample_content`); its own background and borders draw as any view's, under the shader's result. Without `src` the
children draw as they are. `maxSampleOffset` (points) is how far beyond a pixel the shader samples its content: the
children are drawn that much beyond the view's bounds for it (`ShaderFill::max_offset`; the frame's area and the
content input's rect start that far outside the view). `progress` (both elements) is `nsh.progress`: a number, or an
animated value bound as `AnimationProp::ShaderUniform(shader::PROGRESS)`, which the platforms write into the `nsh`
buffer instead of the uniforms. Any shader may sample what is behind its view (`sample_backdrop`): the surface's
drawing under the view, excluding the view, its subtree and what is above it.

`<ShaderTransition src uniforms value duration fallback maxSampleOffset>` and `shader_transition(source).value(..)`
are a third, `Tag::ShaderTransition` / `ViewKind::ShaderTransition`: a view whose children change through the shader.
When a commit changes its `value` (`TransitionKey`: a string, number or boolean; the first value starts nothing), the
core sends `MountItem::ShaderTransition { id, duration }` first in that batch (`Core::transition_starts`, spliced in at
the end of `commit`): the platform pictures the children as they are before the batch's other items, its
`sample_from`, and draws them through the shader from then on, their new drawing its `sample_to` (the content input of
an effect, kept current the same way), while `nsh.progress` goes linearly from 0 to 1 over `duration` (300 ms by
default); then the children draw as they are. A transition that starts while one runs starts from what shows. Without
`src` it runs `fallback`: `"fade"` (the default) is a cross-fade the core supplies (`shader::crossfade`, one program
per process), `"none"` sends no fill and no start. Where `shaderSupport().transition` is `false` the core starts none,
so the children change at once.

- **Making a shader.** `createShader(source)` (`nsh-js` `shader.rs`) compiles in Rust with the VM's limits
  (`max_shader_samples`) and returns a frozen `{ uniforms: [names], __shader: id }`; a VM keeps its shaders by id and
  by source, so the same source is the same shader (a component making its shader on each render, a module run again
  by hot module replacement) and counts once against `max_shaders`. Errors throw a `SyntaxError` with
  `Error::render`'s text. A `.wgsl` import is a module of the bundler (`plugin.rs`: checked with the default limits at
  build time, then `export default createShader(source)`); `.nsh/types` declares each file's uniforms
  (`types.rs` `shader_declarations`, `tooling.md`). Rust makes `ShaderSource::new(source, &limits)` once and keeps it.
  Ids are unique per process (`ShaderSource::next_id`): the platforms keep programs by id.
- **Uniforms.** The prop holds values by name (`Uniforms`, `UniformValue`: numbers, lists, colors, animated values,
  images; strings are colors, else image URIs). At commit the core packs them for the shader's reflection
  (`shader::pack`): WGSL's uniform layout, colors premultiplied, integers rounded, a number filling a vector, images by
  slot (their size written when the source knows it, the platform writes it once the image loads), and warns about
  names the shader lacks and values its types do not take. Animated values become bindings
  (`AnimationProp::ShaderUniform(component)`, an `f32` index of the buffer), sent with the view's other
  `AnimationBindings`: the platform's animator writes a number into one component, a color into four, premultiplied
  (`shader::write_animated`), and the shader draws again; JS runs nothing per frame.
- **Programs and fills.** The first time a view draws a shader, the core translates it into the platform's language
  (`ShaderProgram::translate`, `Environment::shaders.language`) on the engine thread and sends it once per engine
  (`MountItem::ShaderProgram`); each view then gets `MountItem::Shader { fill }` (`ShaderFill`: the program's id, the
  packed buffer, the images, whether it reads `nsh.time`) when it changes. A translation that fails is logged and the
  view draws its children only.
- **Inputs.** `ShaderFill::inputs` says which inputs a shader samples (`ShaderFill::samples`). Platforms that draw
  from a picture of an input take it again when it changes: the core sends `MountItem::ShaderInput { id }` after a
  commit that changed a view in an effect's subtree (the effect included) and, once anything changed, for every shader
  that samples the backdrop (`Core::mark_shader_inputs`, a set of the shaders with inputs, walked only while it is not
  empty). What changes on the UI thread (animations, scrolling, loads) the platform notices itself.
- **`shaderSupport()`** reads `Environment::shaders` (`ShaderSupport`: the language and, per kind, `Live`, `Snapshot`
  or `No`). Each platform sets it on its surface; a headless engine (`nsh test`, `ShaderLanguage::None`) says fills
  are live, sends programs (with no code) and fills, so tests check what a shader view gets.

The platforms fill three buffers per draw (`shader::nsh_buffer`, `shader::frame_buffer` with each loaded image's size
in pixels: an image without one samples transparent, `shader::write_image_size`) and bind each image as the program's
inputs say.

| Platform | Draws with | Where | Draws every frame |
|---|---|---|---|
| iOS, Mac Catalyst, macOS | Metal (`nsh-metal`: loaded with `dlopen`, programs compiled off the main thread) | a `CAMetalLayer` in the view's layer (`decor::SHADER`), shaped by the decoration as image content is | the macOS surface's frame callbacks / the iOS mounter's shader display link, while in a window |
| Android 13+ | `RuntimeShader` (AGSL) in `ShaderBox.onDraw` | the view's canvas, over its background | `postInvalidateOnAnimation` while shown |
| Windows | Direct3D 11 into a texture Direct2D draws (`HlslProgram::d3d11`, `D3DCompile`) | the view's drawing, before its text and children | each frame the surface draws |
| Linux | OpenGL ES 3 in a `GdkGLContext` of its own (glow over libepoxy) into a texture, a `GdkGLTexture` with a fence | the view's snapshot, flipped as `GtkGLArea`'s | the widget's tick callback |
| Web | WebGL 2 on one shared canvas, copied to a bitmap canvas in the view's element | the element, under its children | `requestAnimationFrame` |
| GPUI | (draws the children) | | |

Effects and backdrops, per platform:

| Platform | `effect` | `backdrop` | How |
|---|---|---|---|
| Android 13+ | live | live | an effect records its children into a `RenderNode` (`maxSampleOffset` beyond the view) whose `RenderEffect` is the runtime shader, its content input that node; a backdrop shader runs as the render effect of a node drawing `Backdrop.java`'s recording. A render effect gives one live input: an effect that samples both reads the backdrop as transparent |
| macOS | snapshot | snapshot | an effect's children live in a container (`NshView::ensure_container`) whose layer an empty mask hides while the shader draws (AppKit hit-tests views, so they keep their clicks); the shader's `CAMetalLayer` covers the view and `maxSampleOffset` beyond it, above the container (`decor::EFFECT`). The content is the container rendered with `renderInContext:` (without its mask) into a bitmap; the backdrop is the surface's root rendered at the view's place with the view at opacity 0. Pictures are taken on the next display frame after a `ShaderInput` or a UI-thread change (`shader::ui_changed`, from `NshView::changed` / `moved` / `redraw` and scrolling: a text drawn once it scrolls into sight is a change), all at once, and uploaded (`ShaderLayer::set_input`); layers due to draw are drawn first (`displayIfNeeded`). `renderInContext:` draws no Metal layer, video or web page; in-process screenshots hide effects' containers |
| iOS, Mac Catalyst | snapshot | snapshot | as macOS, with the decoration's content view (`Decor::set_effect`; it neither clips nor takes the decoration's mask for an effect) hidden by an empty mask: UIKit skips views at alpha 0 in hit testing, not masked ones. The backdrop is the surface's root view (never the host app's window). Marks come from `ShaderInput`, animations, scrolling and loaded images (`Mounter::ui_changed`, which finds the effects around a view by the views' tags); pictures are taken on the shader display link's next frame |
| Web | snapshot | snapshot | NSH paints the input itself from the elements into a 2D canvas (`nsh.js` `paintTree`: backgrounds, rounded corners, uniform borders, images by `object-fit`, canvases (other shaders' pictures), text runs at the positions NSH laid them out, clipping and opacity; no rotations, gradients or shadows; cross-origin images without CORS are left out, they would taint the picture). An effect's children are at `opacity: 0` (`.nsh-fx`; browsers hit-test them still) under its canvas (`.nsh-fx-out`, `maxSampleOffset` beyond the element); the backdrop is the surface painted up to the view (what comes after it is above it). A `MutationObserver` on the effect (on the root for backdrops) and capture-phase `scroll` / `load` listeners mark the pictures stale, taken on the next animation frame |
| Windows | live | live | drawn with the frame: an effect's children are drawn by Direct2D into a texture of the area (the view and `maxSampleOffset` beyond it; `shader::draw_effect`, another context of the device, so what is pushed on the window's stays), not into the window; a backdrop shader's input is what the frame drew so far under the area, copied into a texture before the view draws its own background (`shader::take_backdrop`; transparent in passes drawn offscreen, for a filtered or 3D-transformed view). Both textures are bound with the images; the window is drawn again whenever anything in it changes, so the inputs are always current (`ShaderInput` needs nothing). Web views inside an effect are not in its content |
| Linux | live | snapshot | an effect's children are drawn, whenever GTK draws the effect again (it or something in it queued a draw), into a render node of their own (`NshView::snapshot_children`, not into the frame's) that the window's renderer renders over the area (`render_texture`); the texture is downloaded and uploaded into the shaders' context. The backdrop is a picture: after the frame clock's `after-paint` that follows a mark (`ShaderInput`, `shader::ui_changed` from `NshView::changed` / `moved` and scrolling; a shader's own redraws, `NshView::redraw`, mark nothing), the surface's views are drawn anew from its root (`NshView::draw`, not GTK's render nodes of the last frame, which have the view in them) up to the view: `shader::skipped` leaves out the view and whatever draws after it, `shader::past` the borders and scroll indicators of the views around it. GTK's damage phase is left alone in that pass |
| others | `false` | `false` | an effect draws its children as they are; `sample_backdrop` reads transparent |

Transitions, per platform (each draws as its effect does while a transition runs, and its children as they are
otherwise):

| Platform | `transition` | `sample_from` | Progress |
|---|---|---|---|
| Android 13+ | live | the children recorded into a `RenderNode` and rendered once by a `HardwareRenderer` into an `ImageReader`'s hardware bitmap (`ShaderBox.startTransition`), an input shader of the runtime shader; `sample_to` is the live render effect input | `dispatchDraw`, from `SystemClock.uptimeMillis` |
| macOS | snapshot | the container rendered with `renderInContext:` (`shader::start_transition`), or the running transition's picture (`ShaderLayer::picture`) | a frame callback |
| iOS, Mac Catalyst | snapshot | as macOS, from the decoration's content view (`ShaderHost::start`; effects and transitions are decorated when created, so the content view exists without styles) | the shader display link (`Mounter::transitions`) |
| Web | snapshot | painted from the elements (`op_st`, `paintTree`), or the running transition's canvas | `requestAnimationFrame` |
| Windows | live | the children drawn by Direct2D into a third input texture outside the frame (`render::draw_offscreen`), or the target copied (`CopyFromBitmap`) | `Surface::on_frame` |
| Linux | live | the children rendered to a texture as an effect's content (`shader::start_transition`), or the last picture (flipped from GL's rows) | the view's tick callback |
| GPUI | `false` | | |

`nsh_metal::ShaderLayer` holds pictures of all four inputs (`set_input`, `clear_input`) and takes a transition's
progress (`set_progress`).

AGSL computes `p` as the pixel's coordinate over the scale plus `nsh_origin`, the frame area's origin, as the other
writers' vertex stages place `p` over the area: an effect drawn beyond its view starts outside it.

On every platform the fill follows the view's corners and draws above its background, gradients and inset shadows
and below its borders, as a background does (Android draws the borders itself after the fill, `Decor.bordersAbove`,
as its image views do).

In-process screenshots on macOS (`nsh test --target macos`) draw each shader again into a texture the CPU reads
(`ShaderLayer::picture`, shaped as the layer is), since `cacheDisplayInRect` does not draw a `CAMetalLayer`. Android's screenshots use
`PixelCopy`: a software canvas draws no `RuntimeShader`.

## Verification

`cargo test -p nsh-shader` runs the profile's cases and every corpus shader through all four writers. The platforms'
own compilers check the programs: `metal_compiles` (ignored by default: it needs Xcode's Metal toolchain) runs `xcrun
metal` for macOS and iOS; `hlsl_compiles` (Windows) runs `D3DCompile` at `ps_5_0` / `vs_5_0`. `NSH_SHADER_EXPORT=<dir>`
writes every program to `<dir>` for compilers outside the tests: AGSL through `new RuntimeShader(source)` on a device
or emulator (a dex run with `app_process`), GLSL ES 3.00 through WebGL 2 in a headless Chrome, GLSL 3.30 through
macOS's OpenGL; `NSH_SHADER_CORPUS=<dir>` adds a directory of someone else's `.wgsl` files (gl-transitions translated
from GLSL with naga: 119 of 120 fit the profile, the other declares a global variable; all 119 compile with every
compiler above).

Size, release with fat LTO, stripped, over an empty binary: the checker (naga's WGSL front end and validator, the
profile) is 0.94 MB on macOS arm64, 1.04 MB on Android arm64 and 0.79 MB on WebAssembly (`opt-level = "z"`, 0.24 MB
with brotli); the writers add MSL 0.27 MB, HLSL 0.33 MB, GLSL 0.19 MB (WebAssembly 0.14 MB, 0.04 MB with brotli) and
AGSL 0.10 MB.

The element: `nsh-core`'s `shader::tests` (packing, bindings, images, the buffers), `nsh-engine`'s `shader` tests (a
`.wgsl` import and `createShader` end to end: one program per shader, the packed fills, the bindings, errors and
`shaderSupport()`), the `paint` domain's `paint-shader` case in `tests/compat`, and the gallery's Shaders demo
(`examples/gallery/demos/ShaderDemo.tsx`: `nsh.time`, animated uniforms, an image). The platforms are checked by
running them: `nsh test <entry> --target macos|ios|android|web` with `app.screenshot(..)` steps.
