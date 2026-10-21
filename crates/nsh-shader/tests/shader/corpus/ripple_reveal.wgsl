// A transition: a ripple runs over the old screen and leaves the new one.
fn main(p: vec2f) -> vec4f {
    let center = nsh.size * 0.5;
    let d = p - center;
    let r = length(d);
    let front = nsh.progress * length(nsh.size);
    let band = exp(-pow((r - front) / 40.0, 2.0));
    let offset = select(vec2f(0.0), d / r * sin(r * 0.08 - nsh.progress * 30.0) * 12.0 * band, r > 0.001);
    let t = smoothstep(front - 60.0, front, r);
    return mix(sample_to(p + offset), sample_from(p + offset), t);
}
