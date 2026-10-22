// A linear gradient from `start` to `end` along `angle` (radians, 0 points right).
struct Uniforms {
    start: vec4f,
    end: vec4f,
    angle: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let dir = vec2f(cos(u.angle), sin(u.angle));
    let half_extent = abs(dir.x) * nsh.size.x * 0.5 + abs(dir.y) * nsh.size.y * 0.5;
    let t = clamp(dot(p - nsh.size * 0.5, dir) / (2.0 * half_extent) + 0.5, 0.0, 1.0);
    return mix(u.start, u.end, t);
}
