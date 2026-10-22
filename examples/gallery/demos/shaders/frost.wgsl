// Frosted glass: what is behind the view blurred as a Gaussian `radius`
// points wide (its sigma, as CSS's blur(radius)), lightened by `tint`.
//
// One pass can afford a few dozen samples: they lie on a golden-angle spiral
// over a disk of 2.5 sigma, weighted by the Gaussian, and each pixel turns and
// shifts its spiral by noise, so what the sampling misses shows as an even,
// fine grain instead of copies of what is behind and steps at its edges.
struct Uniforms {
  tint: vec4f,
  radius: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
  // Two noise values per device pixel (Dave Hoskins' hash without sine).
  var h = fract(vec3f(floor(p * nsh.scale).xyx) * vec3f(0.1031, 0.1030, 0.0973));
  h += dot(h, h.yzx + 33.33);
  let noise = fract((h.xx + h.yz) * h.zy);
  let turn = noise.x * 6.2831853;
  let extent = 2.5 * u.radius;
  var c = vec4f(0.0);
  var total = 0.0;
  for (var i = 0; i < 96; i++) {
    // Evenly over the disk: the area inside a sample grows with its index.
    let t = (f32(i) + noise.y) / 96.0;
    let a = f32(i) * 2.3999632 + turn;
    let d = sqrt(t) * extent;
    let w = exp(-0.5 * d * d / max(u.radius * u.radius, 0.0001));
    c += sample_backdrop(p + vec2f(cos(a), sin(a)) * d) * w;
    total += w;
  }
  c = c / total;
  return c * (1.0 - u.tint.a) + u.tint;
}
