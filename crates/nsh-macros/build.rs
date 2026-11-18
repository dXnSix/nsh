//! Writes a theme document into `OUT_DIR`, as an app's build script that
//! generates its theme does, for the integration tests' `theme_keys!` with
//! a `$OUT_DIR/` path (`tests/macros/theme_format_keys.rs`). Nothing here
//! reaches the library.

/// The document the tests read as `$OUT_DIR/theme_format_keys.json`.
const DOCUMENT: &str = r##"{
  "tokens": {
    "generated-accent": { "kind": "color", "value": "#0090ff", "path": "generated.accent" },
    "generated-gap": { "kind": "number", "value": 8 }
  },
  "modifiers": {
    "density": { "default": "regular", "contexts": { "regular": {}, "compact": { "generated-gap": 4 } } }
  },
  "resolutionOrder": ["density"]
}
"##;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out = std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR for build scripts");
    let path = std::path::Path::new(&out).join("theme_format_keys.json");
    // Written only when it changed, so the tests aren't built again for
    // nothing.
    if std::fs::read_to_string(&path).ok().as_deref() != Some(DOCUMENT) {
        std::fs::write(&path, DOCUMENT).expect("OUT_DIR is writable");
    }
}
