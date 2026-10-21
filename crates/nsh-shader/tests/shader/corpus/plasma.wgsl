// An animated plasma: reads nsh.time, so it draws every frame.
fn main(p: vec2f) -> vec4f {
    let uv = p / nsh.size;
    let t = nsh.time;
    var v = sin(uv.x * 10.0 + t);
    v += sin((uv.y * 10.0 + t) * 0.5);
    v += sin((uv.x * 10.0 + uv.y * 10.0 + t) * 0.5);
    let c = uv * 10.0 + vec2f(sin(t / 3.0), cos(t / 2.0)) * 5.0;
    v += sin(sqrt(c.x * c.x + c.y * c.y + 1.0) + t);
    let rgb = vec3f(sin(v * 3.14159), sin(v * 3.14159 + 2.094), sin(v * 3.14159 + 4.188)) * 0.5 + 0.5;
    return vec4f(rgb, 1.0);
}
