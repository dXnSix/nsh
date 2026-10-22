---
title: Shader
description: "The Shader element: a view filled by a GPU shader written in WGSL, its children drawn over it."
order: 8.5
group: primitives
demo: el-shader
---

# Shader

A shader view is a view the GPU fills: gradients that drift, glows, noise, an image distorted by ripples, any look a
function of the pixel's position can describe. The app writes the shader in WGSL, the language of WebGPU; NSH checks
it when the app builds and when it is made, and each platform runs it on its own GPU API: Metal on iOS, Mac Catalyst
and macOS, AGSL runtime shaders on Android 13 and later and WebGL 2 on the
web. The view's children draw over the shader, and its styles (corners, borders, clipping, shadows) apply as on
any view.

```rust tabs
use std::sync::LazyLock;

use nsh_core::{
    shader::{ShaderLimits, ShaderSource, Uniforms},
    ui::prelude::*,
};

/// Made once: each `ShaderSource::new` checks the source again.
static GLOW: LazyLock<ShaderSource> = LazyLock::new(|| {
    ShaderSource::new(include_str!("glow.wgsl"), &ShaderLimits::default()).expect("a shader")
});

fn glow() -> impl IntoElement {
    shader(GLOW.clone())
        .uniforms(Uniforms::new().set("tint", rgb(0xf59e0b)).set("radius", 64.0))
        .h(px(200.0))
        .rounded_xl()
        .items_center()
        .justify_center()
        .child(text("Over the shader").text_color(white()))
}
```

```tsx tabs
import { mount, Shader, Text } from "nsh";
import glow from "./glow.wgsl";

function Glow() {
  return (
    <Shader
      src={glow}
      uniforms={{ tint: "#f59e0b", radius: 64 }}
      style={{ height: 200, borderRadius: 12, alignItems: "center", justifyContent: "center" }}
    >
      <Text style={{ color: "white" }}>Over the shader</Text>
    </Shader>
  );
}

mount(Glow);
```

```wgsl
// glow.wgsl
struct Uniforms {
  tint: vec4f,
  radius: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let d = length(p - nsh.size * 0.5);
  return u.tint * (1.0 - smoothstep(u.radius * 0.6, u.radius, d));
}
```

## Writing a shader

A shader is one function, `main`, that returns the color of a pixel:

- `p` is the pixel's position in points from the view's top-left corner; the result is a premultiplied color (its
  red, green and blue already multiplied by its alpha), each component from 0 to 1.
- The uniforms are the fields of the struct `main` takes as its second parameter (`fn main(p: vec2f) -> vec4f` takes
  none): `f32`, `i32`, vectors of them, square `f32` matrices, fixed arrays of those (`array<vec4f, 8>`: an array's
  elements are 16 bytes apart) and `Image`.
- `nsh` holds the view: `nsh.size` (its size in points), `nsh.scale` (pixels per point) and `nsh.time` (seconds since
  the shader first drew).
- `sample_image(u.field, p)` reads an `Image` uniform at a position in points, the image stretched over the view
  (`u.field.size` is its size in points, to fit it another way). Outside the image a sample is transparent.

NSH's WGSL is the part of the language every platform runs: loops are counted `for` loops with constant bounds, array
indexes are constants or loop counters, and a pixel takes at most 128 texture samples. Names starting with `nsh_` are
NSH's. A shader outside these rules is an error at build time, with its line and column in the `.wgsl` file.

## Making a shader

A `.wgsl` file the app imports is a shader, checked when the app builds. `.nsh/types` declares each file's uniforms,
so the type checker knows `uniforms`' names and the values each takes:

```tsx
import ripple from "./ripple.wgsl"; // ShaderAsset<{ center?: …; strength?: … }>

ripple.uniforms; // ["center", "strength"]: the names, in the struct's order
```

`createShader(source)` makes one from a string at run time (a shader the app downloads or builds). It throws a
`SyntaxError` naming the line and column of what NSH's WGSL does not take; the same source always gives the same
shader. In Rust, `ShaderSource::new(source, &limits)` returns the shader or the error.

```tsx
import { createShader } from "nsh";

const stripes = createShader(`
fn main(p: vec2f) -> vec4f {
  let on = step(0.5, fract((p.x + p.y) / 24.0));
  return vec4f(vec3f(on * 0.15), 1.0);
}`);
```

## Uniforms

`uniforms` gives each uniform its value by name; a uniform left out is 0.

| WGSL type | Value |
|---|---|
| `f32` | a number, or an [animated value](../modules/nsh-animated.md) |
| `i32` | a number (rounded) |
| `vec2f` | `[x, y]`, a point `{ x, y }`, a size `{ width, height }`, or a number for both |
| `vec3f`, `vec4f` | a list of numbers, or a CSS color (premultiplied RGB / RGBA); `vec4f` also an animated color |
| `vec2i` … `vec4i` | a list of numbers |
| `mat2x2f` … `mat4x4f` | a list of numbers, column by column |
| `array<T, N>` | a list of `T`'s values (a flat list of numbers fills vectors in turn) |
| `Image` | an image: an imported image, a URI (`https://…`, `asset:/…`) |

Animated values are written into the shader's buffer on the UI thread: a spring or a timing on them redraws the
shader every frame they move, with no work in the app's code per frame.

```rust tabs
let radius = animated_value(24.0);
radius.spring(96.0, Spring::default());

shader(GLOW.clone()).uniforms(Uniforms::new().set("tint", rgb(0x38bdf8)).set("radius", &radius))
```

```tsx tabs
import { createAnimatedValue } from "nsh/animated";

const radius = createAnimatedValue(24);
radius.spring(96);

<Shader src={glow} uniforms={{ tint: "#38bdf8", radius }} style={{ height: 200 }} />;
```

## When it draws

A shader draws when what it reads changes: its uniforms, an image that loaded, the view's size, an animated value as
it moves. A shader that reads `nsh.time` draws every frame while it is on screen.

## Props

A shader view takes the props of a [view](./view.md) (styles, children, press and gesture props, `onLayout`,
accessibility, `ref`, …) plus:

| Prop | Rust | Type | Description |
|---|---|---|---|
| `src` | `shader(source)` | `ShaderAsset<U>` | The shader: a `.wgsl` import or `createShader`'s result. |
| `uniforms` | `.uniforms(uniforms)` | `U` (the shader's uniforms) | The values of its uniforms, by name. |

## Support

`shaderSupport()` from `nsh` says what the platform draws: `{ fill, backdrop, effect, transition }`, each `"live"`,
`"snapshot"` or `false`. `fill` is `<Shader>`: `"live"` on iOS, Mac Catalyst, macOS, Android 13 and later and the
web. Where it is `false`, a shader view shows its children and its own styles, so a layout that puts
content over a shader reads the same everywhere.

```tsx
import { shaderSupport } from "nsh";

const fancy = shaderSupport().fill !== false;
```

## Limits

An app embedded in another one makes up to 64 shaders (`createShader` throws a `RangeError` beyond), and a pixel of
a shader takes up to 128 texture samples, which the host app sets with its
[limits](../docs/sandbox-and-capabilities.md#limits) (`samples`).

## Platform notes

- iOS, Mac Catalyst and macOS draw with Metal in a `CAMetalLayer` over the view's background; programs compile off the
  main thread.
- Android draws with a `RuntimeShader` (Android 13, API 33, and later) when the view draws.
- The web renders with WebGL 2 into a canvas in the view's element.
