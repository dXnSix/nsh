// A transition: the new side grows from the middle in a circle, its edge
// softened over `edge` points.
struct Uniforms {
  edge: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let c = nsh.size * 0.5;
  let r = length(nsh.size) * 0.5 * nsh.progress;
  let t = smoothstep(r - u.edge, r + u.edge, length(p - c));
  return mix(sample_to(p), sample_from(p), t);
}
