// A case of the API's test suite (docs/dev/compatibility.md): shaders. A
// `.wgsl` import and `createShader` are checked shaders whose uniforms'
// names the app reads; a source that is not WGSL of NSH's profile throws a
// SyntaxError; `shaderSupport()` says what the platform draws; `<Shader>`
// fills a view under its children, with constant and animated uniforms;
// `<ShaderEffect>` draws its children through a shader (or as they are
// without one), and a shader reads what is behind it.
import { createShader, Shader, ShaderEffect, shaderSupport, Text, View } from "nsh";
import { createAnimatedValue } from "nsh/animated";
import glow from "./glow.wgsl";

const plain = createShader("fn main(p: vec2f) -> vec4f { return vec4f(p / nsh.size, 0.5, 1.0); }");
const wave = createShader("fn main(p: vec2f) -> vec4f { return sample_content(p + vec2f(sin(p.y * 0.1) * nsh.progress * 8.0, 0.0)); }");
const frost = createShader("fn main(p: vec2f) -> vec4f { return sample_backdrop(p) * 0.6 + vec4f(0.4); }");

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
    </View>
  );
}

export default App;
