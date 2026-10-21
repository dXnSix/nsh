// Frosted glass: a 9 x 9 Gaussian of the backdrop with constant weights.
const WEIGHTS = array<f32, 9>(0.0162, 0.0540, 0.1216, 0.1945, 0.2270, 0.1945, 0.1216, 0.0540, 0.0162);

struct Uniforms {
    radius: f32,
    tint: vec4f,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let step = u.radius / 4.0;
    var color = vec4f(0.0);
    for (var y = 0; y < 9; y++) {
        for (var x = 0; x < 9; x++) {
            let offset = vec2f(f32(x - 4), f32(y - 4)) * step;
            color += sample_backdrop(p + offset) * WEIGHTS[x] * WEIGHTS[y];
        }
    }
    return vec4f(mix(color.rgb, u.tint.rgb, u.tint.a), 1.0);
}
