// A case of the API's test suite (docs/dev/compatibility.md): shaders. A
// `.wgsl` import and `createShader` are checked shaders whose uniforms'
// names the app reads; a source that is not WGSL of NSH's profile throws a
// SyntaxError; `shaderSupport()` says what the platform draws; `<Shader>`
// fills a view under its children, with constant and animated uniforms;
// `<ShaderEffect>` draws its children through a shader (or as they are
// without one), and a shader reads what is behind it; `<ShaderTransition>`
// changes its children through a shader when its `value` changes (a
// cross-fade without one); `<ScreenEffect>` and `<ScreenTransition>` draw
// the whole surface through a shader and take no space where they are.
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
import { createAnimatedValue } from "nsh";
import glow from "./glow.wgsl";

const plain = createShader("fn main(p: vec2f) -> vec4f { return vec4f(p / nsh.size, 0.5, 1.0); }");
const wave = createShader("fn main(p: vec2f) -> vec4f { return sample_content(p + vec2f(sin(p.y * 0.1) * nsh.progress * 8.0, 0.0)); }");
const frost = createShader("fn main(p: vec2f) -> vec4f { return sample_backdrop(p) * 0.6 + vec4f(0.4); }");
const wipe = createShader("fn main(p: vec2f) -> vec4f { return select(sample_from(p), sample_to(p), p.x < nsh.size.x * nsh.progress); }");
const tint = createShader("fn main(p: vec2f) -> vec4f { let c = sample_content(p); return vec4f(c.rgb * (1.0 - 0.2 * nsh.progress), c.a); }");

function invalid() {
  try {
    createShader("fn main(p: vec2f) -> vec4f { return nowhere; }");
    return "compiled";
  } catch (e) {
    return `${(e as Error).name}: ${(e as Error).message.includes("nowhere") ? "names it" : "says nothing"}`;
  }
}

function App() {
  const radius = createAnimatedValue(12);
  const page = signal(1);
  const support = shaderSupport();
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>Shaders</Text>
      <Text>{`glow: ${glow.uniforms.join(", ")}`}</Text>
      <Text>{`plain: ${plain.uniforms.length} uniforms`}</Text>
      <Text>{`invalid: ${invalid()}`}</Text>
      <Text>{`fill ${String(support.fill)}, backdrop ${String(support.backdrop)}, effect ${String(support.effect)}`}</Text>
      <Shader testID="plain" src={plain} style={{ height: 20 }} />
      <Shader
        testID="glow"
        src={glow}
        uniforms={{ color: "#ff000080", radius, center: { x: 20, y: 20 } }}
        style={{ height: 40 }}
      >
        <Text>Over the shader</Text>
      </Shader>
      <ShaderEffect testID="wave" src={wave} progress={radius} maxSampleOffset={8} style={{ padding: 8 }}>
        <Text>Through the shader</Text>
      </ShaderEffect>
      <ShaderEffect testID="plain-effect">
        <Text>No shader, as it is</Text>
      </ShaderEffect>
      <Shader testID="frost" src={frost} style={{ height: 20 }} />
      <ShaderTransition testID="wipe" src={wipe} value={page()} duration={200} maxSampleOffset={0}>
        <Text>{`Page ${page()}`}</Text>
      </ShaderTransition>
      <ShaderTransition testID="fade" value={page()} fallback="fade">
        <Text>{`Faded ${page()}`}</Text>
      </ShaderTransition>
      <View testID="next-page" onPress={() => page.set(page() + 1)} style={{ padding: 8 }}>
        <Text>Next page</Text>
      </View>
      <ScreenEffect src={tint} progress={0.5} />
      <ScreenTransition src={wipe} value={page()} duration={200} />
      <ScreenTransition value={page()} fallback="none" />
      <Text>{`screen effect ${String(support.screenEffect)}, screen transition ${String(support.screenTransition)}`}</Text>
    </View>
  );
}

export default App;
