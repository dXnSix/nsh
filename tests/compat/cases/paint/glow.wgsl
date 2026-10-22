// A shader of the paint domain's shader case: a soft disc of a color.
struct Uniforms {
  color: vec4f,
  radius: f32,
  center: vec2f,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let d = length(p - u.center);
  return u.color * (1.0 - smoothstep(u.radius - 1.0, u.radius, d));
}
