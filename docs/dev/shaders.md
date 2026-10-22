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
