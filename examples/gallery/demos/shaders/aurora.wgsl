// Three bands of light drifting over a vertical gradient. It reads
// `nsh.time`, so it draws every frame while it is shown.
struct Uniforms {
  top: vec4f,
  bottom: vec4f,
  band: vec4f,
  speed: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let uv = p / nsh.size;
  let t = nsh.time * u.speed;
  var light = 0.0;
  for (var i = 0; i < 3; i++) {
    let f = f32(i);
    let y = 0.3 + 0.16 * f + 0.07 * sin(uv.x * (3.0 + 1.7 * f) + t * (0.5 + 0.35 * f));
    light += (0.55 - 0.12 * f) * exp(-abs(uv.y - y) * (16.0 - 4.0 * f));
  }
  let base = mix(u.top, u.bottom, uv.y);
  return min(base + u.band * light * base.a, vec4f(base.a));
}
