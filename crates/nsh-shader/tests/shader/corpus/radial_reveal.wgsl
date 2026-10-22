// A transition: the new screen in a circle growing from `origin`, with a soft edge.
struct Uniforms {
    origin: vec2f,
    softness: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let corners = array<vec2f, 4>(vec2f(0.0), vec2f(nsh.size.x, 0.0), vec2f(0.0, nsh.size.y), nsh.size);
    var reach = 0.0;
    for (var i = 0; i < 4; i++) {
        reach = max(reach, distance(u.origin, corners[i]));
    }
    let edge = nsh.progress * (reach + u.softness);
    let t = 1.0 - smoothstep(edge - u.softness, edge, distance(p, u.origin));
    return mix(sample_from(p), sample_to(p), t);
}
