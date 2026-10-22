// A transition: the new screen fades in over the old.
fn main(p: vec2f) -> vec4f {
    return mix(sample_from(p), sample_to(p), nsh.progress);
}
