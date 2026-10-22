// Liquid glass over what is behind the view: a squircle that refracts and blurs the backdrop
// (5 x 15 samples), with a tint and a rim light.
struct Uniforms {
    tint: vec4f,
    refraction: f32,
    blur: f32,
    rim: f32,
}

const E: f32 = 2.718281828459045;

fn squircle(p: vec2f, n: f32, r: f32) -> f32 {
    let a = abs(p);
    let numerator = pow(a.x, n) + pow(a.y, n) - pow(r, n);
    let denominator = n * sqrt(pow(a.x, 2.0 * n - 2.0) + pow(a.y, 2.0 * n - 2.0)) + 0.00001;
    return numerator / denominator;
}

fn refract_scale(x: f32) -> f32 {
    return 1.0 - 2.332 * pow(4.544 * E, -6.923 * x - 0.992);
}

fn blurred(p: vec2f, radius: f32) -> vec4f {
    var color = vec4f(0.0);
    var total = 0.0;
    for (var ring = 1; ring <= 5; ring++) {
        for (var k = 0; k < 15; k++) {
            let angle = f32(k) * 0.41887903 + f32(ring) * 0.7;
            let r = radius * f32(ring) / 5.0;
            let weight = exp(-f32(ring * ring) / 12.5);
            color += sample_backdrop(p + vec2f(cos(angle), sin(angle)) * r) * weight;
            total += weight;
        }
    }
    return color / total;
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let center = nsh.size * 0.5;
    let half_size = max(center, vec2f(1.0));
    let q = (p - center) / half_size;
    let dist = -squircle(q, 3.0, 1.0);
    let alpha = smoothstep(0.0, 2.0 / min(half_size.x, half_size.y), dist);
    if alpha <= 0.0 {
        return vec4f(0.0);
    }
    let scale = mix(1.0, pow(max(refract_scale(max(dist, 0.0)), 0.001), 1.779), u.refraction);
    let at = center + (p - center) * scale;
    var color = blurred(at, u.blur);
    color = vec4f(mix(color.rgb, u.tint.rgb, u.tint.a), color.a);
    let rim = smoothstep(0.06, 0.0, max(dist, 0.0)) * u.rim;
    return vec4f(color.rgb * (1.0 + rim), 1.0) * alpha;
}
