// Frosted glass: what is behind the view, averaged over a ring of samples
// `radius` points around each pixel, lightened by `tint`.
struct Uniforms {
  tint: vec4f,
  radius: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  var c = vec4f(0.0);
  for (var i = 0; i < 16; i++) {
    let a = f32(i) * 0.3927;
    c += sample_backdrop(p + vec2f(cos(a), sin(a)) * u.radius);
  }
  c = c / 16.0;
  return c * (1.0 - u.tint.a) + u.tint;
}
