// An effect: blocks that grow with `nsh.progress`.
fn main(p: vec2f) -> vec4f {
    let block = max(1.0, round(nsh.progress * 24.0));
    let center = (floor(p / block) + 0.5) * block;
    return sample_content(center);
}
