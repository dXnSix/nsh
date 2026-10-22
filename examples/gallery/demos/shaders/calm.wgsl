// The whole screen in grayscale, darker toward its edges.
fn main(p: vec2f) -> vec4f {
  let c = sample_content(p);
  let l = dot(c.rgb, vec3f(0.2126, 0.7152, 0.0722));
  let v = 1.0 - 0.55 * smoothstep(0.35, 0.95, length(p / nsh.size - 0.5) * 1.4);
  return vec4f(vec3f(l) * v, c.a);
}
