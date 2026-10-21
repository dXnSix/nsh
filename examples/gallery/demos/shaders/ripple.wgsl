// A ring spreading from `center` over an image and a grid: `phase` moves
// its front out (7 points per unit) and `strength` fades it, both animated
// on the UI thread.
struct Uniforms {
  photo: Image,
  center: vec2f,
  phase: f32,
  strength: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  let d = p - u.center;
  let r = max(length(d), 0.001);
  let k = r - u.phase * 7.0;
  let wave = sin(k * 0.22) * u.strength * exp(-k * k / 1200.0);
  let q = p + d / r * wave;

  // `sample_image` stretches the image over the view: fit it inside,
  // centered, at most 1.6 times its size.
  let fit = min(min(nsh.size.x / u.photo.size.x, nsh.size.y / u.photo.size.y) * 0.8, 1.6);
  let shown = u.photo.size * fit;
  let uv = (q - (nsh.size - shown) * 0.5) / shown;
  let image = sample_image(u.photo, uv * nsh.size);

  let line = max(step(0.92, fract(q.x / 18.0)), step(0.92, fract(q.y / 18.0)));
  let light = (0.07 + 0.08 * line) * (1.0 + wave * 0.06);
  let grid = vec4f(vec3f(light), 1.0);
  return image * (1.0 + wave * 0.03) + grid * (1.0 - image.a);
}
