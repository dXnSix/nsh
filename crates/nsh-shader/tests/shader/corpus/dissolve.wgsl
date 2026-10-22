// A transition: the old screen dissolves in noise.
struct Uniforms {
    grain: f32,
}

fn hash(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(12.9898, 78.233))) * 43758.5453);
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let n = hash(floor(p / max(u.grain, 1.0)));
    return select(sample_from(p), sample_to(p), n < nsh.progress);
}
