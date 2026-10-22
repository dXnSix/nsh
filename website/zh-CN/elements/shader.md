---
title: Shader
description: Shader 元素：由用 WGSL 编写的 GPU 着色器填充的视图，子元素绘制在它上面。
order: 8.5
group: primitives
demo: el-shader
---

# Shader

着色器视图是由 GPU 填充的视图：缓缓流动的渐变、光晕、噪点、被涟漪扭曲的图片，凡是能用像素位置的函数描述的效果都可以。应用用 WebGPU 的语言 WGSL 编写着色器；NSH 在应用构建时和着色器创建时检查它，每个平台用自己的 GPU API 运行它：iOS、Mac Catalyst 和 macOS 上是 Metal，Android 13 及以上是 AGSL 运行时着色器，Windows 上是 Direct3D 11，Linux 上是 OpenGL ES 3，Web 上是 WebGL 2。视图的子元素绘制在着色器之上，视图的样式（圆角、边框、裁剪、阴影）与任何视图一样生效。

```rust tabs
use std::sync::LazyLock;

use nsh_core::{
    shader::{ShaderLimits, ShaderSource, Uniforms},
    ui::prelude::*,
};

/// 只创建一次：每次 `ShaderSource::new` 都会重新检查源码。
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

## 编写着色器

着色器就是一个函数 `main`，它返回一个像素的颜色：

- `p` 是像素相对视图左上角的位置，单位为点；返回值是预乘颜色（红、绿、蓝已乘以 alpha），每个分量在 0 到 1 之间。
- uniform 是 `main` 第二个参数所取结构体的字段（`fn main(p: vec2f) -> vec4f` 不取 uniform）：`f32`、`i32`、它们的向量、`f32` 方阵、这些类型的定长数组（`array<vec4f, 8>`：数组元素间隔 16 字节）以及 `Image`。
- `nsh` 描述这个视图：`nsh.size`（以点为单位的尺寸）、`nsh.scale`（每点的像素数）、`nsh.time`（着色器第一次绘制以来的秒数）和 `nsh.progress`（元素的 `progress`）。
- `sample_image(u.field, p)` 在以点为单位的位置读取一个 `Image` uniform，图片铺满整个视图（`u.field.size` 是它以点为单位的尺寸，可用来按其他方式适配）。图片之外的采样是透明的。

NSH 的 WGSL 是每个平台都能运行的那部分语言：循环是常量边界的计数 `for` 循环，数组下标是常量或循环计数器，一个像素最多进行 128 次纹理采样。以 `nsh_` 开头的名字属于 NSH。不符合这些规则的着色器会在构建时报错，并给出它在 `.wgsl` 文件中的行和列。

## 创建着色器

应用导入的 `.wgsl` 文件就是一个着色器，在应用构建时检查。`.nsh/types` 声明每个文件的 uniform，类型检查器因此知道 `uniforms` 的名字以及每个取什么值：

```tsx
import ripple from "./ripple.wgsl"; // ShaderAsset<{ center?: …; strength?: … }>

ripple.uniforms; // ["center", "strength"]：按结构体中的顺序排列的名字
```

`createShader(source)` 在运行时用字符串创建着色器（应用下载或拼出的着色器）。遇到 NSH 的 WGSL 不接受的内容时，它会抛出 `SyntaxError` 并指出行和列；同样的源码总是得到同一个着色器。在 Rust 中，`ShaderSource::new(source, &limits)` 返回着色器或错误。

```tsx
import { createShader } from "nsh";

const stripes = createShader(`
fn main(p: vec2f) -> vec4f {
  let on = step(0.5, fract((p.x + p.y) / 24.0));
  return vec4f(vec3f(on * 0.15), 1.0);
}`);
```

## Uniform

`uniforms` 按名字给每个 uniform 赋值；没写的 uniform 为 0。

| WGSL 类型 | 值 |
|---|---|
| `f32` | 数字，或[动画值](../modules/nsh-animated.md) |
| `i32` | 数字（四舍五入） |
| `vec2f` | `[x, y]`、点 `{ x, y }`、尺寸 `{ width, height }`，或同时用于两个分量的一个数字 |
| `vec3f`、`vec4f` | 数字列表，或 CSS 颜色（预乘的 RGB / RGBA）；`vec4f` 也可以是动画颜色 |
| `vec2i` … `vec4i` | 数字列表 |
| `mat2x2f` … `mat4x4f` | 数字列表，按列排列 |
| `array<T, N>` | `T` 的值组成的列表（扁平的数字列表依次填满各个向量） |
| `Image` | 图片：导入的图片，或 URI（`https://…`、`asset:/…`） |

动画值在 UI 线程上写入着色器的缓冲区：对它们执行 spring 或 timing 时，着色器在它们变化的每一帧重绘，应用代码每帧不需要做任何事。

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

## 何时绘制

着色器在它读取的内容变化时绘制：它的 uniform、加载完成的图片、视图的尺寸、正在变化的动画值。读取 `nsh.time` 的着色器在屏幕上时每一帧都绘制。

## 效果：子元素经过着色器

`<ShaderEffect>` 让它的子元素经过着色器绘制：子元素绘制的内容是着色器的输入 `sample_content(p)`，显示的是着色器的结果。子元素仍然是视图：照常布局、接收触摸和点击、被读屏软件读出。`maxSampleOffset` 是着色器在一个像素之外采样它们的最远距离，单位为点（涟漪的范围、模糊的半径）：子元素为此在视图边界之外多绘制这么远。没有 `src` 时，子元素按原样绘制。

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

`progress` 是着色器读到的 `nsh.progress`：一个数字，或由 UI 线程写入的动画值。两种元素都接受它。

## 视图背后的内容

`sample_backdrop(p)` 读取窗口在视图背后绘制的内容：它下面的视图，不包括视图本身和它包含的内容。采样它的 `<Shader>` 可以做出磨砂玻璃、着色面板、透镜：

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

## 属性

着色器视图接受[视图](./view.md)的属性（样式、子元素、按压和手势属性、`onLayout`、无障碍、`ref` 等），另外还有：

| 属性 | Rust | 类型 | 说明 |
|---|---|---|---|
| `src` | `shader(source)` | `ShaderAsset<U>` | 着色器：`.wgsl` 导入或 `createShader` 的结果。 |
| `uniforms` | `.uniforms(uniforms)` | `U`（着色器的 uniform） | 按名字给出的 uniform 的值。 |
| `progress` | `.progress(progress)` | `number \| AnimatedNode` | 着色器读到的 `nsh.progress`。 |

着色器效果接受同样的属性（`src` 可省略：`shader_effect(Some(source))`，`None` 表示没有），另外还有：

| 属性 | Rust | 类型 | 说明 |
|---|---|---|---|
| `maxSampleOffset` | `.max_sample_offset(px(…))` | `number` | 着色器在一个像素之外采样内容的最远距离，单位为点。 |

## 支持情况

`nsh` 的 `shaderSupport()` 说明平台绘制什么：`{ fill, backdrop, effect, transition }`，每项为 `"live"`、`"snapshot"` 或 `false`。`fill` 对应 `<Shader>`：在 iOS、Mac Catalyst、macOS、Android 13 及以上、Windows、Linux 和 Web 上为 `"live"`。`effect` 对应 `<ShaderEffect>`，`backdrop` 对应采样视图背后内容的着色器：在 Android 13 及以上为 `"live"`。`fill` 为 `false` 时，着色器视图显示它的子元素和自身样式，因此把内容放在着色器之上的布局在所有平台上读起来都一样；`effect` 为 `false` 时，效果按原样显示子元素；`backdrop` 为 `false` 时，`sample_backdrop` 读到透明。

```tsx
import { shaderSupport } from "nsh";

const fancy = shaderSupport().fill !== false;
```

## 限制

嵌入在其他应用中的应用最多创建 64 个着色器（超出时 `createShader` 抛出 `RangeError`），着色器的一个像素最多进行 128 次纹理采样，宿主应用通过它的[限制](../docs/sandbox-and-capabilities.md#限制)（`samples`）设置这一项。

## 平台说明

- iOS、Mac Catalyst 和 macOS 用 Metal 在视图背景之上的 `CAMetalLayer` 中绘制；程序在主线程之外编译。
- Android 在视图绘制时用 `RuntimeShader`（Android 13，API 33 及以上）绘制。效果把它作为子元素绘制内容的渲染效果运行，背景着色器把它作为视图背后内容的渲染效果运行：两者都实时读取输入。那里的着色器读取一个实时输入：内容或背景。
- Windows 用 Direct3D 11 渲染到一张由 Direct2D 绘制的纹理。
- Linux 用 OpenGL ES 3 渲染到一张由 GTK 绘制的纹理。
- Web 用 WebGL 2 渲染到视图元素中的一个 canvas。
