// A checkerboard of four colors: integer math, `%`, `switch`, `select`.
struct Uniforms {
    colors: array<vec4f, 4>,
    cell: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let cell = vec2i(floor(p / max(u.cell, 1.0)));
    let index = abs(cell.x + cell.y * 3) % 4;
    var color = vec4f(0.0);
    switch index {
        case 0: { color = u.colors[0]; }
        case 1: { color = u.colors[1]; }
        case 2, 3: { color = select(u.colors[2], u.colors[3], index == 3); }
        default: {}
    }
    return color;
}
