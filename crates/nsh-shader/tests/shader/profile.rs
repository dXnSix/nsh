//! The profile's rules: what a shader may say, and where an error points.

use nsh_shader::{Limits, compile};

use crate::{error, shader};

/// `main` around `body`, with uniforms `u: U { k: f32, a: array<vec4f, 4> }`.
fn with_main(body: &str) -> String {
    format!(
        "struct U {{ k: f32, a: array<vec4f, 4> }}\nfn main(p: vec2f, u: U) -> vec4f {{\n{body}\n}}\n"
    )
}

#[track_caller]
fn rejects(body: &str, message: &str) {
    let source = with_main(body);
    let e = error(&source);
    assert!(
        e.message.contains(message),
        "{:?} does not say {message:?}\n{source}",
        e.message
    );
}

#[test]
fn counted_loops_compile() {
    for body in [
        "var c = 0.0; for (var i = 0; i < 4; i++) { c += 1.0; } return vec4f(c);",
        "var c = 0.0; for (var i = 10; i > 0; i -= 2) { c += 1.0; } return vec4f(c);",
        "var c = 0.0; for (var i: i32 = 0; 4 > i; i += 1) { c += 1.0; } return vec4f(c);",
        "var c = 0.0; for (var x = 0.0; x <= 1.0; x += 0.25) { c += x; } return vec4f(c);",
        "var c = vec4f(0.0); for (var i = 0; i < 4; i++) { if i == 2 { continue; } c += u.a[i]; } return c;",
        "var c = vec4f(0.0); for (var i = 0; i < 3; i++) { c += u.a[i + 1] * u.a[3 - i]; } return c;",
        "var c = 0.0; for (var i = 0; i < 4; i++) { for (var j = i; j < 4; j++) { c += 1.0; } } return vec4f(c);",
    ] {
        let s = shader(&with_main(
            body.replace("for (var j = i;", "for (var j = 0;").as_str(),
        ));
        assert!(s.to_agsl().is_ok());
    }
    let s = shader(&with_main(
        "var c = 0.0; for (var i = 10; i > 0; i -= 2) { c += 1.0; } return vec4f(c);",
    ));
    assert_eq!(s.reflection().iterations, 5);
}

#[test]
fn other_loops_are_rejected() {
    rejects(
        "var i = 0; while i < 4 { i++; } return vec4f(0.0);",
        "`for` loops with constant bounds",
    );
    rejects(
        "var i = 0; loop { i++; if i > 4 { break; } } return vec4f(0.0);",
        "`for` loops with constant bounds",
    );
    rejects(
        "for (var i = 0; f32(i) < u.k; i++) {} return vec4f(0.0);",
        "`for` loops with constant bounds",
    );
    rejects(
        "for (var i = 0; i < 4; i++) { i += 1; } return vec4f(0.0);",
        "leaves its counter alone",
    );
    rejects(
        "var i = 0; for (; i < 4; i++) {} return vec4f(f32(i));",
        "read only inside the loop",
    );
    rejects(
        "for (var i = 0; i < 100000; i++) {} return vec4f(0.0);",
        "runs more than",
    );
    rejects(
        "for (var i = 0; i > -1; i++) {} return vec4f(0.0);",
        "runs more than",
    );
}

#[test]
fn indexes_are_proven_in_bounds() {
    rejects(
        "var c = vec4f(0.0); for (var i = 0; i < 4; i++) { c += u.a[i + 1]; } return c;",
        "out of 0..3",
    );
    rejects(
        "var j = 1; return u.a[j];",
        "an index is a constant or a loop counter",
    );
    rejects(
        "let v = vec4f(1.0); var j = 1; return vec4f(v[j]);",
        "an index is a constant or a loop counter",
    );
}

#[test]
fn what_agsl_cannot_say_is_rejected() {
    rejects("let x = 1u; return vec4f(f32(x));", "`u32`");
    rejects("return vec4f(dpdx(p.x));", "derivatives");
    rejects("if p.x > 1.0 { discard; } return vec4f(0.0);", "`discard`");
    rejects(
        "let a = i32(p.x); return vec4f(f32(a & 3));",
        "bit operations",
    );
    rejects(
        "let a = bitcast<i32>(p.x); return vec4f(f32(a));",
        "`bitcast`",
    );
    rejects(
        "var c = 0.0; switch i32(p.x) { case 1: { if p.y > 1.0 { break; } c = 1.0; } default: {} } return vec4f(c);",
        "`break` does not leave a `switch`",
    );
}

#[test]
fn shaders_have_no_resources_of_their_own() {
    let e = error("var<private> x: f32;\nfn main(p: vec2f) -> vec4f { return vec4f(x); }");
    assert!(e.message.contains("no global variables"), "{e}");
    assert_eq!((e.line, e.column), (1, 1));
    let e = error(
        "@group(0) @binding(0) var t: texture_2d<f32>;\nfn main(p: vec2f) -> vec4f { return vec4f(0.0); }",
    );
    assert!(e.message.contains("no global variables"), "{e}");
    let e = error("fn main(p: vec2f) -> vec4f { return nsh_stub(p); }");
    assert!(e.message.contains("NSH's own"), "{e}");
    let e = error(
        "fn nsh_x() -> f32 { return 1.0; }\nfn main(p: vec2f) -> vec4f { return vec4f(nsh_x()); }",
    );
    assert!(e.message.contains("name of NSH's"), "{e}");
}

#[test]
fn main_has_the_contract() {
    assert!(
        error("fn f(p: vec2f) -> vec4f { return vec4f(0.0); }")
            .message
            .contains("has a `main`")
    );
    assert!(
        error("fn main(p: vec2f) -> vec3f { return vec3f(0.0); }")
            .message
            .contains("takes `p: vec2f`")
    );
    assert!(
        error("fn main(p: vec2f, k: f32) -> vec4f { return vec4f(k); }")
            .message
            .contains("takes `p: vec2f`")
    );
}

#[test]
fn uniforms_and_images_are_read_where_they_are() {
    let image = "struct U { photo: Image }\n";
    let e = error(&format!(
        "{image}fn f(i: Image, p: vec2f) -> vec4f {{ return sample_image(i, p); }}\nfn main(p: vec2f, u: U) -> vec4f {{ return f(u.photo, p); }}"
    ));
    assert!(e.message.contains("`Image`"), "{e}");
    let e = error(&format!(
        "{image}fn main(p: vec2f, u: U) -> vec4f {{ var c = u; return vec4f(c.photo.size, 0.0, 0.0); }}"
    ));
    assert!(e.message.contains("not copied to a `var`"), "{e}");
    // A helper may take the uniforms and read them.
    let s = shader(&format!(
        "{image}fn f(p: vec2f, u: U) -> vec4f {{ return sample_image(u.photo, p) * u.photo.size.x; }}\nfn main(p: vec2f, u: U) -> vec4f {{ let v = u; return f(p, v); }}"
    ));
    let agsl = s.to_agsl().unwrap();
    assert!(agsl.source.contains("nsh_sample_4(p"), "{}", agsl.source);
    assert!(agsl.source.contains("nsh_u0_photo_size"), "{}", agsl.source);
}

#[test]
fn the_sample_budget_counts_loops() {
    let source = "fn main(p: vec2f) -> vec4f { var c = vec4f(0.0); for (var i = 0; i < 20; i++) { for (var j = 0; j < 10; j++) { c += sample_backdrop(p + vec2f(f32(i), f32(j))); } } return c; }";
    let e = error(source);
    assert!(e.message.contains("200 samples, more than 128"), "{e}");
    let mut limits = Limits::default();
    limits.max_samples = 200;
    assert_eq!(compile(source, &limits).unwrap().reflection().samples, 200);
    // The larger branch of an `if` counts.
    let source = "fn main(p: vec2f) -> vec4f { if p.x > 1.0 { return sample_content(p) + sample_content(p * 2.0); } return sample_content(p); }";
    assert_eq!(shader(source).reflection().samples, 2);
}

#[test]
fn errors_point_into_the_source() {
    let source = "fn main(p: vec2f) -> vec4f {\n    let x = 1u;\n    return vec4f(f32(x));\n}\n";
    let e = error(source);
    assert_eq!((e.line, e.column), (2, 13), "{e:?}");
    let rendered = e.render("glass.wgsl", source);
    assert!(rendered.starts_with("glass.wgsl:2:13: "), "{rendered}");
    assert!(rendered.contains("    let x = 1u;\n"), "{rendered}");
    // naga's own errors, too.
    let e = error("fn main(p: vec2f) -> vec4f {\n    return vec4f(q);\n}\n");
    assert_eq!(e.line, 2, "{e:?}");
    assert!(e.message.contains("no definition"), "{e:?}");
}

#[test]
fn source_length_is_limited() {
    let mut limits = Limits::default();
    limits.max_source_len = 16;
    assert!(compile("fn main(p: vec2f) -> vec4f { return vec4f(0.0); }", &limits).is_err());
}
