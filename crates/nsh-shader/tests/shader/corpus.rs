//! The corpus through the checker and the reflection.

use nsh_shader::{Input, UniformType};

use crate::{corpus, shader};

#[test]
fn every_shader_fits_the_profile() {
    for (name, source) in corpus() {
        let shader = shader(&source);
        assert_eq!(shader.source(), source, "{name}");
    }
}

#[test]
fn reflection_says_what_a_shader_reads() {
    let all = corpus();
    let get = |name: &str| shader(&all.iter().find(|(n, _)| n == name).unwrap().1);

    let plasma = get("plasma");
    let r = plasma.reflection();
    assert!(r.time && !r.progress && r.inputs.is_empty() && r.uniforms.is_empty());

    let glass = get("glass");
    let r = glass.reflection();
    assert_eq!(r.inputs, [Input::Backdrop]);
    assert_eq!(r.samples, 75);
    assert_eq!(r.iterations, 5 * (1 + 15));
    assert!(!r.time);

    let crossfade = get("crossfade");
    let r = crossfade.reflection();
    assert_eq!(r.inputs, [Input::From, Input::To]);
    assert!(r.progress);

    let image = get("image_cover");
    let r = image.reflection();
    assert_eq!(r.inputs, [Input::Image(0)]);
    assert_eq!(r.uniforms[1].ty, UniformType::Image(0));
    assert_eq!((r.uniforms[1].offset, r.uniforms_size), (16, 48));

    let checker = get("checker");
    let r = checker.reflection();
    assert_eq!(r.uniforms[0].name, "colors");
    assert_eq!((r.uniforms[0].count, r.uniforms[0].stride), (Some(4), 16));
    assert_eq!(
        (r.uniforms[1].ty, r.uniforms[1].offset),
        (UniformType::F32, 64)
    );
}
