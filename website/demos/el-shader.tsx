import { createShader, mount, Shader, ShaderEffect, shaderSupport, signal, Text, View } from "nsh";
import { createAnimatedValue, Easing } from "nsh/animated";
import { desktop, Log, Panel, Segmented, size, Split, ui } from "./palette";

// A glow that breathes with `nsh.time`, its size and color animated
// uniforms: a spring and a timing move them on the UI thread.
const glow = createShader(`
struct Uniforms {
  color: vec4f,
  radius: f32,
  rings: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let d = length(p - nsh.size * 0.5);
  let breathe = 1.0 + 0.06 * sin(nsh.time * 2.0);
  let r = u.radius * breathe;
  let core = 1.0 - smoothstep(r * 0.55, r, d);
  let halo = exp(-max(d - r * 0.55, 0.0) / (r * 0.7)) * 0.55;
  let ring = 0.5 + 0.5 * cos(d / max(r, 1.0) * 6.2832 * u.rings - nsh.time * 3.0);
  let light = clamp(core + halo * mix(1.0, ring, min(u.rings, 1.0)), 0.0, 1.0);
  return u.color * light;
}
`);

// `<ShaderEffect>`: its children are the content, bent by a wave that
// `nsh.progress` moves along; they keep their presses.
const wave = createShader(`
struct Uniforms { amount: f32 }

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let shift = sin(p.y * 0.09 + nsh.progress * 6.2832) * u.amount * sin(nsh.progress * 3.1416);
  return sample_content(p + vec2f(shift, 0.0));
}
`);

// Frosted glass: what is behind the view, averaged around each pixel.
const frost = createShader(`
fn main(p: vec2f) -> vec4f {
  var c = vec4f(0.0);
  for (var i = 0; i < 16; i++) {
    let a = f32(i) * 0.3927;
    c += sample_backdrop(p + vec2f(cos(a), sin(a)) * 7.0);
  }
  return c / 16.0 * 0.75 + vec4f(0.25);
}
`);

const kinds = ["fill", "effect", "backdrop"] as const;
const sizes = ["S", "M", "L"] as const;
const radii = { S: 36, M: 64, L: 96 };
const tints = ["lime", "sky", "rose"] as const;
const colors = { lime: "#c8f26d", sky: "#7ea8ff", rose: "#ff6f91" };

function App() {
  const kind = signal<(typeof kinds)[number]>("fill");
  const progress = createAnimatedValue(0);
  const presses = signal(0);
  const sized = signal<(typeof sizes)[number]>("M");
  const tinted = signal<(typeof tints)[number]>("lime");
  const rings = signal(0);
  const events = signal<string[]>([]);
  const radius = createAnimatedValue(radii.M);
  const hue = createAnimatedValue(0);
  const color = hue.interpolate({ inputRange: [0, 1, 2], outputRange: [colors.lime, colors.sky, colors.rose] });
  const log = (line: string) => events.set([line, ...events()].slice(0, 4));
  const support = shaderSupport();
  return (
    <View style={{ flex: 1, backgroundColor: ui().page, padding: 20, paddingTop: 24, gap: 14 }}>
      <Text style={{ fontSize: 22, fontWeight: "700", color: ui().text }}>shader</Text>
      <Text style={{ color: ui().muted }}>
        {kind() === "fill"
          ? desktop
            ? "Click a size or a color: springs move the uniforms"
            : "Tap a size or a color: springs move the uniforms"
          : kind() === "effect"
            ? desktop
              ? "Click Bend to move the wave; the button inside still takes clicks"
              : "Tap Bend to move the wave; the button inside still takes taps"
            : "The glass reads what is behind it"}
      </Text>

      <Split
        grow
        inspector={
          <>
            <Segmented
              label="kind"
              options={kinds}
              value={kind()}
              onChange={(k) => {
                kind.set(k);
                log(k === "fill" ? "<Shader>" : k === "effect" ? "<ShaderEffect>" : "sample_backdrop");
              }}
            />
            <If when={kind() === "fill"}>
              <Segmented
                label="radius"
                options={sizes}
                value={sized()}
                onChange={(s) => {
                  sized.set(s);
                  radius.spring(radii[s], { stiffness: 120, damping: 12 });
                  log(`radius.spring(${radii[s]})`);
                }}
              />
              <Segmented
                label="color"
                options={tints}
                value={tinted()}
                onChange={(t) => {
                  tinted.set(t);
                  hue.timing(tints.indexOf(t), { duration: 450, easing: Easing.out(Easing.cubic) });
                  log(`color → ${colors[t]}`);
                }}
              />
              <Segmented
                label="rings"
                options={[0, 2, 4] as const}
                value={rings()}
                onChange={(n) => {
                  rings.set(n);
                  log(`uniforms.rings = ${n}`);
                }}
              />
            </If>
            <If when={kind() === "effect"}>
              <View
                onPress={() => {
                  progress.set(0);
                  progress.timing(1, { duration: 1800, easing: Easing.inOut(Easing.quad) });
                  log("progress.timing(1)");
                }}
                activeStyle={{ opacity: 0.6 }}
                style={{ alignSelf: "flex-start", height: 32, paddingHorizontal: 14, borderRadius: 8, justifyContent: "center", ...ui().raised }}
              >
                <Text style={{ fontSize: 13, fontWeight: "500", color: ui().text }}>Bend</Text>
              </View>
            </If>
            <Panel title="shaderSupport()">
              <Text style={{ fontSize: 13, color: ui().text }}>
                fill {String(support.fill)} · effect {String(support.effect)} · backdrop {String(support.backdrop)}
              </Text>
            </Panel>
            <Log title="changes" lines={events()} minHeight={92} />
          </>
        }
      >
        <If when={kind() === "fill"}>
          <Shader
            src={glow}
            uniforms={{ color, radius, rings: rings() }}
            style={{
              flex: 1,
              minHeight: desktop ? 260 : 240,
              borderRadius: 16,
              backgroundColor: "#101113",
              alignItems: "center",
              justifyContent: "flex-end",
              padding: 16,
            }}
          >
            <Text style={{ fontSize: size.detail, color: "rgba(255,255,255,0.7)" }}>Children draw over the shader</Text>
          </Shader>
        </If>
        <If when={kind() === "effect"}>
          <View style={{ flex: 1, minHeight: desktop ? 260 : 240, justifyContent: "center", padding: 16 }}>
            <ShaderEffect src={wave} uniforms={{ amount: 12 }} progress={progress} maxSampleOffset={14}>
              <View style={{ padding: 18, gap: 12, borderRadius: 16, backgroundColor: "#7ea8ff" }}>
                <Text style={{ fontSize: 22, fontWeight: "700", color: "#101113" }}>Through a shader</Text>
                <View
                  onPress={() => {
                    presses.update((n) => n + 1);
                    log(`pressed inside (${presses()})`);
                  }}
                  style={{ alignSelf: "flex-start", paddingHorizontal: 16, height: 40, justifyContent: "center", borderRadius: 10, backgroundColor: "#101113" }}
                >
                  <Text style={{ fontSize: size.detail, color: "#eceef1" }}>{`Pressed ${presses()}×`}</Text>
                </View>
              </View>
            </ShaderEffect>
          </View>
        </If>
        <If when={kind() === "backdrop"}>
          <View style={{ flex: 1, minHeight: desktop ? 260 : 240, borderRadius: 16, overflow: "hidden" }}>
            <View style={{ position: "absolute", left: 0, top: 0, right: 0, bottom: 0, flexDirection: "row" }}>
              <View style={{ flex: 1, backgroundColor: "#ff6f91" }} />
              <View style={{ flex: 1, backgroundColor: "#ffb547" }} />
              <View style={{ flex: 1, backgroundColor: "#c8f26d" }} />
              <View style={{ flex: 1, backgroundColor: "#7ea8ff" }} />
            </View>
            <View style={{ position: "absolute", left: 0, right: 0, top: 0, bottom: 0, alignItems: "center", justifyContent: "center" }}>
              <Text style={{ fontSize: 28, fontWeight: "800", color: "#101113" }}>BEHIND THE GLASS</Text>
            </View>
            <Shader
              src={frost}
              style={{ position: "absolute", left: 64, top: 32, right: 24, bottom: 32, borderRadius: 20, padding: 14 }}
            >
              <Text style={{ fontSize: size.detail, fontWeight: "600", color: "#101113" }}>Frosted glass</Text>
            </Shader>
          </View>
        </If>
      </Split>
    </View>
  );
}

mount(App);
