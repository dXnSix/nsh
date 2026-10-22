//! nsh-shader's tests: the profile's rules and reflection, over a corpus of fills, effects,
//! backdrops and transitions (`corpus/`).

mod corpus;
mod profile;

use std::path::PathBuf;

use nsh_shader::{Limits, Shader, compile};

/// The corpus: `(name, source)`, sorted.
fn corpus() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/shader/corpus");
    let mut out: Vec<(String, String)> = std::fs::read_dir(dir)
        .expect("the corpus")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "wgsl"))
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[track_caller]
fn shader(source: &str) -> Shader {
    compile(source, &Limits::default())
        .unwrap_or_else(|e| panic!("{}", e.render("test.wgsl", source)))
}

/// The error compiling `source`, rendered.
#[track_caller]
fn error(source: &str) -> nsh_shader::Error {
    match compile(source, &Limits::default()) {
        Ok(_) => panic!("compiled:\n{source}"),
        Err(e) => e,
    }
}
