//! Shaders end to end: `.wgsl` imports and `createShader` checked in Rust,
//! `<Shader>`'s program sent once per engine, its uniforms packed into the
//! buffer the program reads, animated ones bound to their components, and
//! `shaderSupport()`.

use std::time::Duration;

use nsh_core::{
    ApproxTextEngine, MountBatch, MountItem, Size, ViewKind,
    animation::AnimationProp,
    geometry::px,
    shader::{ShaderFill, ShaderLanguage, ShaderSupport},
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
        context: Default::default(),
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
