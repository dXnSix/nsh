//! NSH's prelude, as WGSL appended to the app's source.
//!
//! The app's source comes first, so naga's spans in it are the app's own; what follows declares
//! `nsh`, `Image` and the `sample_*` functions the source calls. The checker sees stubs
//! (`Glue::Check`).

use std::fmt::Write as _;

use crate::{INPUTS, Input};

/// Which prelude to append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glue {
    /// Stubs, no entry point: to find the app's `main` and its uniforms' struct.
    Signature,
    /// Stubs and a fragment entry point that loads the uniforms: what the checker and the AGSL
    /// writer read (naga validates the uniforms' layout against the uniform address space).
    Check,
}

/// What the glue needs to know about the app's shader.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Spec<'a> {
    /// The struct `main` takes its uniforms in, if it takes any.
    pub uniforms: Option<&'a str>,
}

// `Image`'s `slot` (its x) says which texture `sample_image` samples; a vector of four aligns
// `Image` to 16 bytes, as the uniform address space wants a struct field aligned.
const STRUCTS: &str = "
struct Nsh { size: vec2f, scale: f32, time: f32, progress: f32 }
struct Image { size: vec2f, slot: vec4i }
struct NshInput { rect: vec4f, px: vec4f }
";

/// The app's source followed by the prelude `glue` asks for.
pub(crate) fn source(user: &str, glue: Glue, spec: Spec<'_>) -> String {
    let mut out = String::with_capacity(user.len() + 4096);
    out.push_str(user);
    // A line comment at the end of the app's source must not swallow the prelude.
    out.push_str("\n// NSH prelude\n");
    out.push_str(STRUCTS);
    let _ = writeln!(
        out,
        "struct NshFrame {{ area: vec4f, origin: vec4f, inputs: array<NshInput, {INPUTS}> }}"
    );
    match glue {
        Glue::Signature | Glue::Check => stubs(&mut out, glue, spec),
    }
    out
}

const SAMPLERS: [(&str, Input); 4] = [
    ("sample_content", Input::Content),
    ("sample_backdrop", Input::Backdrop),
    ("sample_from", Input::From),
    ("sample_to", Input::To),
];

fn stubs(out: &mut String, glue: Glue, spec: Spec<'_>) {
    out.push_str("@group(0) @binding(0) var<uniform> nsh: Nsh;\n");
    for (name, _) in SAMPLERS {
        let _ = writeln!(
            out,
            "fn {name}(p: vec2f) -> vec4f {{ return nsh_stub(p); }}"
        );
    }
    out.push_str(
        "fn sample_image(image: Image, p: vec2f) -> vec4f { return nsh_stub(p) * image.size.x; }\n",
    );
    out.push_str("fn nsh_stub(p: vec2f) -> vec4f { return vec4f(p, p) * 0.0; }\n");
    if glue == Glue::Check {
        let call = match spec.uniforms {
            Some(ty) => {
                let _ = writeln!(out, "@group(0) @binding(2) var<uniform> nsh_u: {ty};");
                "main(p, nsh_u)"
            }
            None => "main(p)",
        };
        let _ = writeln!(
            out,
            "@fragment fn nsh_check(@location(0) p: vec2f) -> @location(0) vec4f {{ return {call}; }}"
        );
    }
}
