// An image that covers the view (keeps its aspect, crops), tinted.
struct Uniforms {
    tint: vec4f,
    photo: Image,
}

fn main(p: vec2f, u: Uniforms) -> vec4f {
    let view = nsh.size / max(nsh.size.x, nsh.size.y);
    let image = u.photo.size / max(u.photo.size.x, u.photo.size.y);
    let scale = max(view.x / image.x, view.y / image.y);
    let uv = (p / nsh.size - 0.5) * view / (image * scale) + 0.5;
    return sample_image(u.photo, uv * nsh.size) * u.tint;
}
