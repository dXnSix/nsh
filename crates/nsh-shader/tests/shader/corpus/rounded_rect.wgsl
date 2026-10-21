// A rounded rect with a border, antialiased over one pixel.
struct Uniforms {
    fill: vec4f,
    border: vec4f,
    radius: f32,
    width: f32,
}

fn rounded_rect(p: vec2f, half_size: vec2f, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2f(radius);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let aa = 1.0 / nsh.scale;
    let d = rounded_rect(p - nsh.size * 0.5, nsh.size * 0.5 - vec2f(aa), u.radius);
    let inside = 1.0 - smoothstep(-aa, aa, d);
    let inner = 1.0 - smoothstep(-aa, aa, d + u.width);
    return mix(u.border, u.fill, inner) * inside;
}
