// The new screen in a circle growing from the middle, a ripple bending
// both sides at its edge.
fn main(p: vec2f) -> vec4f {
  let center = nsh.size * 0.5;
  let r = length(nsh.size) * 0.5 * nsh.progress;
  let d = length(p - center);
  let ring = exp(-pow((d - r) / 28.0, 2.0));
  let q = p - (p - center) / max(d, 1.0) * ring * 14.0;
  let t = smoothstep(r - 2.0, r + 2.0, d);
  return mix(sample_to(q), sample_from(q), t);
}
