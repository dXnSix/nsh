// Fractal value noise: five octaves in a counted loop.
struct Uniforms {
    color: vec4f,
    scale: f32,
}

fn hash(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2f(1.0, 0.0));
    let c = hash(i + vec2f(0.0, 1.0));
    let d = hash(i + vec2f(1.0, 1.0));
    return mix(mix(a, b, s.x), mix(c, d, s.x), s.y);
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    var q = p / u.scale;
    var amplitude = 0.5;
    var total = 0.0;
    for (var octave = 0; octave < 5; octave++) {
        total += noise(q) * amplitude;
        q *= 2.0;
        amplitude *= 0.5;
    }
    return u.color * total;
}
