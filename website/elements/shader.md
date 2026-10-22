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
and macOS, AGSL runtime shaders on Android 13 and later, Direct3D 11 on Windows, OpenGL ES 3 on Linux and WebGL 2 on
the web. The view's children draw over the shader, and its styles (corners, borders, clipping, shadows) apply as on
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
- `nsh` holds the view: `nsh.size` (its size in points), `nsh.scale` (pixels per point), `nsh.time` (seconds since
  the shader first drew) and `nsh.progress` (the element's `progress`).
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

## Effects: children through a shader

`<ShaderEffect>` draws its children through a shader: what they draw is the shader's input, `sample_content(p)`, and
the shader's result is what shows. The children stay views: they lay out, take touches and clicks, and read to
screen readers as usual. `maxSampleOffset` is how far beyond a pixel the shader samples them, in points (a ripple's
reach, a blur's radius): the children are drawn that much beyond the view's bounds for it. Without `src` the
children draw as they are.

```rust tabs
static WAVE: LazyLock<ShaderSource> = LazyLock::new(|| {
    ShaderSource::new(include_str!("wave.wgsl"), &ShaderLimits::default()).expect("a shader")
});

let progress = animated_value(0.0);
progress.timing(1.0, 2000.0, Easing::Linear);

shader_effect(Some(WAVE.clone()))
    .uniforms(Uniforms::new().set("amount", 8.0))
    .progress(&progress)
    .max_sample_offset(px(12.0))
    .child(card())
```

```tsx tabs
import { ShaderEffect } from "nsh";
import { createAnimatedValue, Easing } from "nsh/animated";
import wave from "./wave.wgsl";

const progress = createAnimatedValue(0);
progress.timing(1, { duration: 2000, easing: Easing.linear });

<ShaderEffect src={wave} uniforms={{ amount: 8 }} progress={progress} maxSampleOffset={12}>
  <Card />
</ShaderEffect>;
```

```wgsl
// wave.wgsl
struct Uniforms { amount: f32 }

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let shift = sin(p.y * 0.09 + nsh.progress * 6.2832) * u.amount;
  return sample_content(p + vec2f(shift, 0.0));
}
```

`progress` is what the shader reads as `nsh.progress`: a number, or an animated value the UI thread writes. Both
elements take it.

## What is behind a view

`sample_backdrop(p)` reads what the window draws behind the view: the views under it, without the view itself and
what it contains. A `<Shader>` that samples it makes frosted glass, tinted panes, lenses:

```wgsl
fn main(p: vec2f) -> vec4f {
  var c = vec4f(0.0);
  for (var i = 0; i < 16; i++) {
    let a = f32(i) * 0.3927;
    c += sample_backdrop(p + vec2f(cos(a), sin(a)) * 7.0);
  }
  return c / 16.0 * 0.8 + vec4f(0.2);
}
```

## Transitions: children that change through a shader

`<ShaderTransition>` changes its children through a shader when its `value` changes: a picture of what they drew
before is the shader's `sample_from(p)`, what they draw now its `sample_to(p)`, and `nsh.progress` goes from 0 to 1
over `duration` milliseconds (300 by default). Then the children show as they are. The new children are views from
the start: they lay out, take touches and clicks and update while the transition runs. A transition that starts while
another runs starts from what shows. Without `src` it runs its `fallback`: a cross-fade (`"fade"`, the default) or
nothing (`"none"`).

```rust tabs
static REVEAL: LazyLock<ShaderSource> = LazyLock::new(|| {
    ShaderSource::new(include_str!("reveal.wgsl"), &ShaderLimits::default()).expect("a shader")
});

let page = signal(0.0);

shader_transition(Some(REVEAL.clone()))
    .value(move || TransitionKey::from(page.get()))
    .duration(Duration::from_millis(500))
    .child(dynamic(move || card(page.get())))
```

```tsx tabs
import { ShaderTransition, signal } from "nsh";
import reveal from "./reveal.wgsl";

const page = signal(0);

<ShaderTransition src={reveal} value={page()} duration={500}>
  <Card n={page()} />
</ShaderTransition>;
```

```wgsl
// reveal.wgsl: the new side grows from the middle in a circle.
fn main(p: vec2f) -> vec4f {
  let c = nsh.size * 0.5;
  let r = length(nsh.size) * 0.5 * nsh.progress;
  let edge = smoothstep(r - 2.0, r + 2.0, length(p - c));
  return mix(sample_to(p), sample_from(p), edge);
}
```

## Props

A shader view takes the props of a [view](./view.md) (styles, children, press and gesture props, `onLayout`,
accessibility, `ref`, …) plus:

| Prop | Rust | Type | Description |
|---|---|---|---|
| `src` | `shader(source)` | `ShaderAsset<U>` | The shader: a `.wgsl` import or `createShader`'s result. |
| `uniforms` | `.uniforms(uniforms)` | `U` (the shader's uniforms) | The values of its uniforms, by name. |
| `progress` | `.progress(progress)` | `number \| AnimatedNode` | What the shader reads as `nsh.progress`. |

A shader effect takes the same, with an optional `src` (`shader_effect(Some(source))`, `None` for none), plus:

| Prop | Rust | Type | Description |
|---|---|---|---|
| `maxSampleOffset` | `.max_sample_offset(px(…))` | `number` | How far beyond a pixel the shader samples its content, in points. |

A shader transition takes the props of a view, `uniforms` and `maxSampleOffset` as an effect does, an optional `src`
(`shader_transition(Some(source))`), plus:

| Prop | Rust | Type | Description |
|---|---|---|---|
| `value` | `.value(value)` | `string \| number \| boolean` | What its children show: when it changes, they change through the shader. |
| `duration` | `.duration(Duration)` | `number` | How long a transition runs, in milliseconds (300). |
| `fallback` | `.fallback(TransitionFallback)` | `"fade" \| "none"` | What it runs without `src`: a cross-fade (the default) or nothing. |

## Support

`shaderSupport()` from `nsh` says what the platform draws: `{ fill, backdrop, effect, transition }`, each `"live"`,
`"snapshot"` or `false`. `fill` is `<Shader>`: `"live"` on iOS, Mac Catalyst, macOS, Android 13 and later, Windows,
Linux and the web. `effect` is `<ShaderEffect>` and `backdrop` a shader that samples what is behind it: `"live"` on
Android 13 and later and Windows, and `"snapshot"` on iOS, Mac Catalyst, macOS and the web: there the shader reads a picture of its input, taken again when it
changes. On Linux `effect` is `"live"` and `backdrop` `"snapshot"`. `transition` is `<ShaderTransition>`: `"live"`
on Android 13 and later, Windows and Linux, `"snapshot"` on iOS, Mac Catalyst, macOS and the web; where it is
`false`, the children change at once. Where `fill` is `false`, a shader view shows its children and its own styles, so a layout that
puts content over a shader reads the same everywhere; where `effect` is `false`, an effect shows its children as they
are, and where `backdrop` is `false`, `sample_backdrop` reads transparent.

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
  main thread. There an effect and a backdrop read a picture of their input, taken again on the next frame after it
  changes; shaders, videos and web views inside an effect or behind a backdrop read as transparent there. A
  transition's two sides are pictures too: the children before the change, and as they are, taken again as they
  change.
- Android draws with a `RuntimeShader` (Android 13, API 33, and later) when the view draws. An effect runs it as the
  render effect of its children's drawing, a backdrop shader as the render effect of what is behind the view: both
  read their input live. A shader there reads one live input, its content or its backdrop. A transition reads its new
  children live and a picture of the old ones, taken when it starts.
- Windows renders with Direct3D 11 into a texture Direct2D draws. An effect's children and what is behind a backdrop
  are drawn into textures with each frame, so both read their input live, as does a transition's new side; its old
  side is drawn once when it starts.
- Linux renders with OpenGL ES 3 into a texture GTK draws. An effect renders its children with each frame it draws,
  as does a transition its new side (its old side is rendered once when it starts); a backdrop reads a picture of
  what is behind the view, taken again on the next frame after it changes.
- The web renders with WebGL 2 into a canvas in the view's element. An effect and a backdrop read a picture NSH paints
  of the elements (backgrounds, rounded corners, borders, images, canvases and text), taken again on the next frame
  after they change, and so does a transition of its two sides.
