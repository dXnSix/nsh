// An effect: red and blue split from green by `nsh.progress`.
fn main(p: vec2f) -> vec4f {
    let shift = vec2f(8.0 * nsh.progress, 0.0);
    let r = sample_content(p + shift);
    let g = sample_content(p);
    let b = sample_content(p - shift);
    return vec4f(r.r, g.g, b.b, max(max(r.a, g.a), b.a));
}
