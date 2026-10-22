import {
  createShader,
  ScreenEffect,
  ScreenTransition,
  Shader,
  ShaderEffect,
  ShaderTransition,
  shaderSupport,
  signal,
  Text,
  View,
} from "nsh";
import { createAnimatedValue, Easing } from "nsh/animated";
import { Dialog } from "nsh/base";
import logo from "../logo@2x.png";
import { Button, Caption, Card, colors, DemoPage, Legend, Segmented } from "../ui";
import aurora from "./shaders/aurora.wgsl";
import calm from "./shaders/calm.wgsl";
import frost from "./shaders/frost.wgsl";
import reveal from "./shaders/reveal.wgsl";
import ring from "./shaders/ring.wgsl";
import ripple from "./shaders/ripple.wgsl";
import wave from "./shaders/wave.wgsl";

// ---------------------------------------------------------------------------
// A `.wgsl` import that reads `nsh.time`: it draws every frame while it is
// shown, under the view's children.

function Aurora() {
  return (
    <Card icon="spark" title="Aurora" subtitle="A .wgsl import that reads nsh.time, under its children">
      <Shader
        src={aurora}
        uniforms={{ top: "#0b1026", bottom: "#1c2a4a", band: "#5ce1b0", speed: 0.8 }}
        style={{ height: 180, borderRadius: 12, overflow: "hidden", justifyContent: "flex-end", padding: 16 }}
      >
        <Text style={{ fontSize: 22, fontWeight: "700", color: "#ffffff" }}>Northern lights</Text>
        <Text style={{ fontSize: 13, color: "rgba(255,255,255,0.75)" }}>Text and views draw over the shader</Text>
      </Shader>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// A shader made at run time (`createShader`) whose radius and color are
// animated values: a spring moves them on the UI thread, without JS.

const spotlight = createShader(`
struct Uniforms { color: vec4f, radius: f32 }

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let d = length(p - nsh.size * 0.5);
  let core = 1.0 - smoothstep(u.radius * 0.55, u.radius, d);
  let halo = exp(-max(d - u.radius * 0.55, 0.0) / (u.radius * 0.6)) * 0.5;
  return u.color * clamp(core + halo, 0.0, 1.0);
}
`);

const levels = [
  { value: "dim", label: "Dim", radius: 28 },
  { value: "warm", label: "Warm", radius: 52 },
  { value: "bright", label: "Bright", radius: 76 },
] as const;
type Level = (typeof levels)[number]["value"];

function Spotlight() {
  const level = signal<Level>("warm");
  const index = () => levels.findIndex((l) => l.value === level());
  const radius = createAnimatedValue(levels[1].radius);
  const hue = createAnimatedValue(1);
  const color = hue.interpolate({ inputRange: [0, 1, 2], outputRange: ["#6c8cff", "#ffb35c", "#fff3c4"] });
  const choose = (l: Level) => {
    level.set(l);
    radius.spring(levels[index()].radius, { stiffness: 120, damping: 12 });
    hue.timing(index(), { duration: 400, easing: Easing.out(Easing.cubic) });
  };
  return (
    <Card icon="spark" title="Spotlight" subtitle="createShader with animated uniforms">
      <Shader src={spotlight} uniforms={{ color, radius }} style={{ height: 180, borderRadius: 12, backgroundColor: "#101114" }} />
      <Segmented options={levels} value={level()} onChange={choose} />
      <Caption label={`radius ${levels[index()].radius} pt`} note="a spring on the UI thread" />
    </Card>
  );
}

// ---------------------------------------------------------------------------
// An image a shader samples (`Image` uniforms), distorted by rings whose
// phase and strength are animated; it draws only while they change.

function Ripple() {
  const phase = createAnimatedValue(0);
  const strength = createAnimatedValue(0);
  const center = signal({ x: 90, y: 90 });
  const drops = signal(0);
  const drop = () => {
    center.set({ x: 40 + Math.random() * 220, y: 40 + Math.random() * 100 });
    drops.set(drops() + 1);
    phase.set(0);
    strength.set(18);
    phase.timing(48, { duration: 2600, easing: Easing.linear });
    strength.timing(0, { duration: 2600, easing: Easing.out(Easing.quad) });
  };
  // One drop as the card appears.
  setTimeout(drop, 500);
  return (
    <Card icon="image" title="Ripple" subtitle="An image sampled by a shader, rings animated">
      <Shader src={ripple} uniforms={{ photo: logo, center: center(), phase, strength }} style={{ height: 180, borderRadius: 12 }} />
      <View style={{ flexDirection: "row", alignItems: "center", gap: 12 }}>
        <Button label="Drop" icon="spark" onPress={drop} />
        <Text style={{ flex: 1, fontSize: 13, color: colors.muted }}>{`${drops()} ${drops() === 1 ? "drop" : "drops"}`}</Text>
      </View>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// A `<ShaderEffect>`: its children are the shader's content, bent by a wave
// that `progress` (animated) moves along. They stay what they are: the
// button inside still takes presses.

function Wave() {
  const progress = createAnimatedValue(0);
  const taps = signal(0);
  const run = () => {
    progress.set(0);
    progress.timing(1, { duration: 1800, easing: Easing.inOut(Easing.quad) });
  };
  return (
    <Card icon="spark" title="Wave" subtitle="ShaderEffect: children drawn through a shader">
      <ShaderEffect src={wave} uniforms={{ amount: 12 }} progress={progress} maxSampleOffset={14}>
        <View style={{ padding: 16, gap: 10, borderRadius: 12, backgroundColor: "#0ea5e9" }}>
          <Text style={{ fontSize: 20, fontWeight: "700", color: "#ffffff" }}>Through a shader</Text>
          <Text style={{ fontSize: 13, color: "rgba(255,255,255,0.85)" }}>The views keep their presses</Text>
          <View style={{ flexDirection: "row" }}>
            <Button label={`Tapped ${taps()}×`} onPress={() => taps.set(taps() + 1)} />
          </View>
        </View>
      </ShaderEffect>
      <View style={{ flexDirection: "row" }}>
        <Button label="Bend" icon="spark" onPress={run} />
      </View>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// A shader that samples what is behind its view (`sample_backdrop`):
// frosted glass over stripes and a title, its own children drawn over it.

function Frost() {
  return (
    <Card icon="spark" title="Frosted glass" subtitle="sample_backdrop: what is behind the view">
      <View style={{ height: 180, borderRadius: 12, overflow: "hidden" }}>
        <View style={{ position: "absolute", left: 0, top: 0, right: 0, bottom: 0, flexDirection: "row" }}>
          <View style={{ flex: 1, backgroundColor: "#f43f5e" }} />
          <View style={{ flex: 1, backgroundColor: "#facc15" }} />
          <View style={{ flex: 1, backgroundColor: "#22c55e" }} />
          <View style={{ flex: 1, backgroundColor: "#6366f1" }} />
        </View>
        <Text style={{ position: "absolute", left: 20, top: 72, fontSize: 26, fontWeight: "800", color: "#111111" }}>
          BEHIND THE GLASS
        </Text>
        <Shader
          src={frost}
          uniforms={{ tint: "rgba(255,255,255,0.25)", radius: 7 }}
          style={{ position: "absolute", left: 36, top: 30, right: 36, bottom: 30, borderRadius: 18, padding: 12 }}
        >
          <Text style={{ fontSize: 14, fontWeight: "600", color: "#111111" }}>Frosted backdrop</Text>
        </Shader>
      </View>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// A `<ShaderTransition>`: when its `value` changes, the children change
// through the shader, the old ones a picture (`sample_from`), the new ones
// live (`sample_to`).

const seasons = [
  { name: "Spring", note: "Blossom and rain", color: "#22c55e" },
  { name: "Summer", note: "Long light evenings", color: "#f59e0b" },
  { name: "Autumn", note: "Leaves and wind", color: "#ef4444" },
  { name: "Winter", note: "Frost and still air", color: "#3b82f6" },
];

function Reveal() {
  const index = signal(0);
  const season = () => seasons[index() % seasons.length];
  return (
    <Card icon="spark" title="Reveal" subtitle="ShaderTransition: children that change through a shader">
      <ShaderTransition src={reveal} uniforms={{ edge: 3 }} value={index()} duration={700}>
        <View style={{ height: 140, borderRadius: 12, padding: 16, justifyContent: "flex-end", backgroundColor: season().color }}>
          <Text style={{ fontSize: 24, fontWeight: "800", color: "#ffffff" }}>{season().name}</Text>
          <Text style={{ fontSize: 13, color: "rgba(255,255,255,0.85)" }}>{season().note}</Text>
        </View>
      </ShaderTransition>
      <View style={{ flexDirection: "row" }}>
        <Button label="Next season" icon="spark" onPress={() => index.set(index() + 1)} />
      </View>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// A dialog whose `animation` is a shader: what is behind it is the lower
// side (`sample_from`), the dialog the upper one (`sample_to`); it comes
// with `nsh.progress` from 0 to 1 and goes back from 1 to 0. Pages
// (`NavPage`) take one the same way.

function DialogReveal() {
  const open = signal(false);
  return (
    <Card icon="spark" title="Dialog through a shader" subtitle="animation={{ shader, uniforms, duration }}">
      <View style={{ flexDirection: "row" }}>
        <Button label="Open the dialog" icon="spark" onPress={() => open.set(true)} />
      </View>
      <Dialog
        open={open()}
        animation={{ shader: reveal, uniforms: { edge: 4 }, duration: 600 }}
        onOpenChange={(o) => open.set(o)}
        backdropStyle={{ backgroundColor: "rgba(0,0,0,0.45)" }}
        style={{ margin: "auto", width: 300, maxWidth: "90%", padding: 20, gap: 12, borderRadius: 18, backgroundColor: "#7c3aed" }}
      >
        <Text style={{ fontSize: 22, fontWeight: "800", color: "#ffffff" }}>Revealed</Text>
        <Text style={{ fontSize: 14, color: "rgba(255,255,255,0.85)" }}>It came through reveal.wgsl and goes back the same way.</Text>
        <View style={{ flexDirection: "row" }}>
          <Button label="Close" onPress={() => open.set(false)} />
        </View>
      </Dialog>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// The whole screen through a shader: a screen effect draws every page,
// dialog and overlay through calm.wgsl while it is on; a screen transition
// changes the whole screen through ring.wgsl when its value changes. Both
// take no space where they are declared, and go with the page.

const tints = ["#0ea5e9", "#f97316", "#22c55e", "#a855f7"];

function Screen() {
  const on = signal(false);
  const tint = signal(0);
  return (
    <Card icon="spark" title="The whole screen" subtitle="ScreenEffect and ScreenTransition: pages, dialogs, overlays">
      <ScreenEffect src={on() ? calm : null} />
      <ScreenTransition src={ring} value={tint()} duration={900} />
      <View style={{ height: 64, borderRadius: 12, backgroundColor: tints[tint() % tints.length] }} />
      <View style={{ flexDirection: "row", gap: 8 }}>
        <Button label={on() ? "Bring the color back" : "Calm the screen"} icon="spark" onPress={() => on.set(!on())} />
        <Button label="Ripple the screen" icon="spark" onPress={() => tint.set(tint() + 1)} />
      </View>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// What this platform draws: a view a shader fills shows its children alone
// where shaders do not draw.

function Support() {
  const support = shaderSupport();
  const say = (s: string | false) => (s === false ? "no" : s);
  return (
    <Card icon="system" title="Support" subtitle="shaderSupport(): what this platform draws">
      <Legend color={support.fill ? colors.up : colors.faint} label="Fill" value={say(support.fill)} />
      <Legend color={support.backdrop ? colors.up : colors.faint} label="Backdrop" value={say(support.backdrop)} />
      <Legend color={support.effect ? colors.up : colors.faint} label="Effect" value={say(support.effect)} />
      <Legend color={support.transition ? colors.up : colors.faint} label="Transition" value={say(support.transition)} />
      <Legend color={support.screenEffect ? colors.up : colors.faint} label="Screen effect" value={say(support.screenEffect)} />
      <Legend color={support.screenTransition ? colors.up : colors.faint} label="Screen transition" value={say(support.screenTransition)} />
      <Caption label={`uniforms of ripple.wgsl: ${ripple.uniforms.join(", ")}`} />
    </Card>
  );
}

export function ShaderDemo() {
  return <DemoPage cards={[Aurora, Spotlight, Ripple, Wave, Frost, Reveal, DialogReveal, Screen, Support]} />;
}
