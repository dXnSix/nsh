import { createShader, mount, Shader, shaderSupport, signal, Text, View } from "nsh";
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

const sizes = ["S", "M", "L"] as const;
const radii = { S: 36, M: 64, L: 96 };
const tints = ["lime", "sky", "rose"] as const;
const colors = { lime: "#c8f26d", sky: "#7ea8ff", rose: "#ff6f91" };

function App() {
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
        {desktop ? "Click a size or a color: springs move the uniforms" : "Tap a size or a color: springs move the uniforms"}
      </Text>

      <Split
        grow
        inspector={
          <>
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
            <Panel title="shaderSupport()">
              <Text style={{ fontSize: 13, color: ui().text }}>
                fill {String(support.fill)} · uniforms {glow.uniforms.join(", ")}
              </Text>
            </Panel>
            <Log title="changes" lines={events()} minHeight={92} />
          </>
        }
      >
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
      </Split>
    </View>
  );
}

mount(App);
