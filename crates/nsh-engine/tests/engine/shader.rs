//! Shaders end to end: `.wgsl` imports and `createShader` checked in Rust,
//! `<Shader>`'s program sent once per engine, its uniforms packed into the
//! buffer the program reads, animated ones bound to their components, and
//! `shaderSupport()`.

use std::time::Duration;

use nsh_core::{
    ApproxTextEngine, MountBatch, MountItem, Size, ViewKind,
    animation::AnimationProp,
    geometry::px,
    shader::{PROGRESS, ShaderFill, ShaderInput, ShaderLanguage, ShaderSupport},
};
use nsh_engine::{Bundle, Engine, EngineConfig, Environment};

use crate::ChannelSink;

const GLOW: &str = "struct Uniforms { color: vec4f, radius: f32, center: vec2f }
fn main(p: vec2f, u: Uniforms) -> vec4f {
  let d = length(p - u.center);
  return u.color * (1.0 - smoothstep(u.radius - 1.0, u.radius, d)) * (0.5 + 0.5 * sin(nsh.time));
}
";

/// The app (with `glow.wgsl` beside it), compiled by nsh and loaded as
/// bytecode, on a platform whose shaders fill views.
fn start(app: &str) -> (Engine, crossbeam_channel::Receiver<MountBatch>) {
    static APPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nsh-shader-{}-{}",
        std::process::id(),
        APPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("App.tsx");
    std::fs::write(&entry, app).unwrap();
    std::fs::write(dir.join("glow.wgsl"), GLOW).unwrap();
    let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
        entry,
        minify: false,
        split: false,
        ui_sets: vec![nsh_base::registry().into()],
    })
    .unwrap();
    let bytecode =
        nsh_js::compile_bytecode_with(&bundle.code, false, &[nsh_base::registry().into()]).unwrap();
    // SAFETY: compiled just now by this runtime.
    let app = unsafe { Bundle::bytecode(bytecode) };
    let (tx, rx) = crossbeam_channel::unbounded();
    let environment = Environment {
        shaders: ShaderSupport::fills(ShaderLanguage::None),
        ..Environment::default()
    };
    let config = EngineConfig {
        app: nsh_engine::App::Js(app),
        viewport: Size::new(px(390.0), px(844.0)),
        trace: false,
        network: None,
        websocket: None,
        filesystem: None,
        assets: None,
        environment,
        header_heights: (nsh_core::Points(44.0), nsh_core::Points(96.0)),
        limits: nsh_engine::Limits::default(),
        ui_sets: vec![nsh_base::registry()],
        runtime: None,
        data_dir: None,
        storage: None,
        clock: Default::default(),
        pinned: Default::default(),
        clipboard: None,
    };
    (
        Engine::start(config, || ApproxTextEngine, ChannelSink(tx)).unwrap(),
        rx,
    )
}

/// The items of the batches the app sends within `wait`.
fn items(rx: &crossbeam_channel::Receiver<MountBatch>, wait: Duration) -> Vec<MountItem> {
    let mut out = Vec::new();
    while let Ok(batch) = rx.recv_timeout(wait) {
        out.extend(batch.items);
    }
    out
}

fn f32_at(bytes: &[u8], component: usize) -> f32 {
    f32::from_le_bytes(bytes[component * 4..component * 4 + 4].try_into().unwrap())
}

const APP: &str = r##"import { mount, createShader, Shader, Text } from "nsh";
import { createAnimatedValue } from "nsh/animated";
import glow from "./glow.wgsl";

const plain = createShader("fn main(p: vec2f) -> vec4f { return vec4f(p / nsh.size, 0.5, 1.0); }");

function App() {
  const radius = createAnimatedValue(10);
  return (
    <>
      <Shader src={plain} style={{ height: 50 }} />
      <Shader src={glow} uniforms={{ color: "#ff000080", radius, center: { x: 20, y: 30 } }} style={{ height: 100 }}>
        <Text>over</Text>
      </Shader>
      <Shader src={glow} uniforms={{ color: "blue", radius: 4, center: [1, 2] }} style={{ height: 100 }} />
    </>
  );
}

mount(App);
"##;

#[test]
fn shader_views_draw_their_programs_with_their_uniforms() {
    let (_engine, rx) = start(APP);
    let items = items(&rx, Duration::from_millis(500));
    let shaders: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            MountItem::Create {
                id,
                kind: ViewKind::Shader,
            } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(shaders.len(), 3, "{items:?}");
    // One program per shader, however many views draw it.
    let programs: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            MountItem::ShaderProgram { program } => Some(program.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(programs.len(), 2, "{programs:?}");
    let fills: Vec<(u32, ShaderFill)> = items
        .iter()
        .filter_map(|i| match i {
            MountItem::Shader {
                id,
                fill: Some(fill),
            } => Some((*id, (**fill).clone())),
            _ => None,
        })
        .collect();
    assert_eq!(fills.len(), 3, "{fills:?}");
    let fill = |id| &fills.iter().find(|(i, _)| *i == id).expect("a fill").1;
    let (plain, glow, blue) = (fill(shaders[0]), fill(shaders[1]), fill(shaders[2]));
    assert!(plain.uniforms.is_empty());
    assert!(!plain.time);
    assert_eq!(glow.program, blue.program);
    assert_ne!(glow.program, plain.program);
    assert!(glow.time, "glow reads nsh.time");
    // `struct Uniforms { color: vec4f, radius: f32, center: vec2f }`: 32
    // bytes, the color premultiplied, the center at component 6 (a vec2f
    // aligns to 8 bytes); the animated radius is the platform's to write.
    assert_eq!(glow.uniforms.len(), 32);
    let a = 128.0 / 255.0;
    for (i, v) in [a, 0.0, 0.0, a, 0.0, 0.0, 20.0, 30.0]
        .into_iter()
        .enumerate()
    {
        assert!(
            (f32_at(&glow.uniforms, i) - v).abs() < 1e-4,
            "component {i}: {:?}",
            glow.uniforms
        );
    }
    for (i, v) in [0.0, 0.0, 1.0, 1.0, 4.0, 0.0, 1.0, 2.0]
        .into_iter()
        .enumerate()
    {
        assert!(
            (f32_at(&blue.uniforms, i) - v).abs() < 1e-4,
            "component {i}: {:?}",
            blue.uniforms
        );
    }
    // The animated radius is bound to its component, so animations change
    // it on the UI thread.
    let bound = items.iter().any(|i| {
        matches!(i, MountItem::AnimationBindings { id, bindings }
            if *id == shaders[1] && bindings.iter().any(|(p, _)| *p == AnimationProp::ShaderUniform(4)))
    });
    assert!(bound, "{items:?}");
}

const UPDATES: &str = r##"import { mount, Shader, signal } from "nsh";
import glow from "./glow.wgsl";

const radius = signal(1);
setTimeout(() => radius.set(7), 50);

mount(() => <Shader src={glow} uniforms={{ color: "red", radius: radius(), center: [0, 0] }} style={{ height: 10 }} />);
"##;

#[test]
fn a_new_uniform_value_sends_a_new_fill_and_no_new_program() {
    let (_engine, rx) = start(UPDATES);
    let items = items(&rx, Duration::from_millis(500));
    let programs = items
        .iter()
        .filter(|i| matches!(i, MountItem::ShaderProgram { .. }))
        .count();
    assert_eq!(programs, 1);
    let radii: Vec<f32> = items
        .iter()
        .filter_map(|i| match i {
            MountItem::Shader {
                fill: Some(fill), ..
            } => Some(f32_at(&fill.uniforms, 4)),
            _ => None,
        })
        .collect();
    assert_eq!(radii, [1.0, 7.0]);
}

const CHECKS: &str = r##"import { mount, createShader, shaderSupport, View } from "nsh";
import glow from "./glow.wgsl";

const out = [];
try {
  createShader("fn main(p: vec2f) -> vec4f { return undefined_thing; }");
  out.push("compiled");
} catch (e) {
  out.push(e.name, e.message.includes("undefined_thing") ? "names it" : e.message);
}
try {
  createShader(42);
} catch (e) {
  out.push(e.name);
}
out.push(JSON.stringify(glow.uniforms));
out.push(JSON.stringify(shaderSupport()));
console.error("shader-checks " + out.join(" | "));
mount(() => <View />);
"##;

#[test]
fn create_shader_checks_its_source_and_shader_support_says_what_draws() {
    crate::logs::capture();
    let (_engine, _rx) = start(CHECKS);
    let lines = crate::logs::wait_for("shader-checks");
    let line = lines.first().expect("the checks ran");
    assert!(
        line.contains("SyntaxError | names it | TypeError | "),
        "{line}"
    );
    assert!(line.contains(r#"["color","radius","center"]"#), "{line}");
    assert!(
        line.contains(r#"{"fill":"live","backdrop":false,"effect":false,"transition":false}"#),
        "{line}"
    );
}

const EFFECT: &str = r##"import { mount, createShader, ShaderEffect, Shader, signal, Text, View } from "nsh";
import { createAnimatedValue } from "nsh/animated";

const wave = createShader("fn main(p: vec2f) -> vec4f { return sample_content(p + vec2f(nsh.progress, 0.0)); }");
const frost = createShader("fn main(p: vec2f) -> vec4f { return sample_backdrop(p) * 0.5; }");
const label = signal("one");
setTimeout(() => label.set("two"), 50);

mount(() => {
  const progress = createAnimatedValue(0);
  return (
    <View>
      <ShaderEffect src={wave} progress={progress} maxSampleOffset={12}>
        <Text>{label()}</Text>
      </ShaderEffect>
      <ShaderEffect progress={0.25}>
        <Text>plain</Text>
      </ShaderEffect>
      <Shader src={frost} style={{ height: 10 }} />
    </View>
  );
});
"##;

#[test]
fn shader_effects_take_their_children_and_learn_when_they_change() {
    let (_engine, rx) = start(EFFECT);
    let items = items(&rx, Duration::from_millis(500));
    let effects: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            MountItem::Create {
                id,
                kind: ViewKind::ShaderEffect,
            } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(effects.len(), 2, "{items:?}");
    let backdrop = items
        .iter()
        .find_map(|i| match i {
            MountItem::Create {
                id,
                kind: ViewKind::Shader,
            } => Some(*id),
            _ => None,
        })
        .expect("the backdrop shader");
    let fill = |id| {
        items.iter().rev().find_map(|i| match i {
            MountItem::Shader { id: of, fill } if *of == id => Some(fill.clone()),
            _ => None,
        })
    };
    let wave = fill(effects[0]).flatten().expect("the wave's fill");
    assert!(wave.samples(ShaderInput::Content));
    assert!(!wave.samples(ShaderInput::Backdrop));
    assert_eq!(wave.max_offset, nsh_core::Points(12.0));
    // Without a shader an effect draws its children as they are.
    assert!(fill(effects[1]).flatten().is_none(), "{items:?}");
    assert!(
        fill(backdrop)
            .flatten()
            .expect("the frost's fill")
            .samples(ShaderInput::Backdrop)
    );
    // The animated progress is bound to `nsh.progress`.
    assert!(items.iter().any(|i| matches!(i, MountItem::AnimationBindings { id, bindings }
        if *id == effects[0] && bindings.iter().any(|(p, _)| *p == AnimationProp::ShaderUniform(PROGRESS)))));
    // The text in the effect changed: the effect's input did, and every
    // backdrop's (something in the surface changed).
    let marks = |id| {
        items
            .iter()
            .filter(|i| matches!(i, MountItem::ShaderInput { id: of } if *of == id))
            .count()
    };
    assert!(marks(effects[0]) >= 2, "{items:?}");
    assert!(marks(backdrop) >= 2, "{items:?}");
    assert_eq!(marks(effects[1]), 0);
}
