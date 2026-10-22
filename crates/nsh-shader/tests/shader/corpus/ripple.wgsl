// An effect: the content rippling out from `center`.
struct Uniforms {
    center: vec2f,
    amplitude: f32,
    wavelength: f32,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let d = p - u.center;
    let r = length(d);
    let wave = sin(r / u.wavelength * 6.28318 - nsh.time * 6.0) * u.amplitude * exp(-r / 200.0);
    let offset = select(vec2f(0.0), d / r * wave, r > 0.001);
    return sample_content(p + offset);
}
