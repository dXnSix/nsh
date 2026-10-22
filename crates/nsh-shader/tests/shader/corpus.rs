//! The corpus through the checker, the reflection and every writer.

use nsh_shader::{GlslVersion, Input, UniformType};

use crate::{corpus, shader};

#[test]
fn every_shader_compiles_for_every_platform() {
    for (name, source) in corpus() {
        let shader = shader(&source);
        let msl = shader
            .to_msl()
            .unwrap_or_else(|e| panic!("{name}: msl: {e}"));
        assert!(msl.source.contains(&msl.fragment), "{name}");
        for version in [GlslVersion::Es300, GlslVersion::Core330] {
            let glsl = shader
                .to_glsl(version)
                .unwrap_or_else(|e| panic!("{name}: glsl: {e}"));
            assert!(glsl.fragment.contains("void main()"), "{name}");
        }
        let hlsl = shader
            .to_hlsl()
            .unwrap_or_else(|e| panic!("{name}: hlsl: {e}"));
        assert!(
            !hlsl.d3d11.contains("nagaSamplerHeap") && !hlsl.d2d.contains("nagaSamplerHeap"),
            "{name}"
        );
        assert!(
            !hlsl.d2d.contains("space"),
            "{name}: shader model 5.0 has no register spaces"
        );
        let agsl = shader
            .to_agsl()
            .unwrap_or_else(|e| panic!("{name}: agsl: {e}"));
        assert!(
            !agsl.source.contains("while") && !agsl.source.contains("switch"),
            "{name}"
        );
        assert!(
            agsl.source.contains("half4 main(float2 nsh_coord)"),
            "{name}"
        );
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

#[test]
fn agsl_writes_counted_loops_as_for_loops() {
    let all = corpus();
    let frost = shader(&all.iter().find(|(n, _)| n == "frost").unwrap().1);
    let agsl = frost.to_agsl().unwrap();
    assert!(
        agsl.source.contains("for (int y = 0; y < 9; y++)"),
        "{}",
        agsl.source
    );
    assert!(
        agsl.source.contains("for (int x = 0; x < 9; x++)"),
        "{}",
        agsl.source
    );
    // Constant arrays are local arrays, indexed by the counter itself.
    assert!(
        agsl.source.contains("[x]") && agsl.source.contains("[y]"),
        "{}",
        agsl.source
    );
}

#[test]
fn agsl_flattens_the_uniforms() {
    let all = corpus();
    let checker = shader(&all.iter().find(|(n, _)| n == "checker").unwrap().1);
    let agsl = checker.to_agsl().unwrap();
    assert!(
        agsl.source.contains("uniform float4 nsh_u0_colors[4];"),
        "{}",
        agsl.source
    );
    let colors = agsl
        .uniforms
        .iter()
        .find(|u| u.name == "nsh_u0_colors")
        .unwrap();
    assert_eq!((colors.count, colors.stride, colors.offset), (4, 16, 0));
    // `switch` is an `if` chain; integer `%` a helper.
    assert!(agsl.source.contains("nsh_imod_int"), "{}", agsl.source);
}
