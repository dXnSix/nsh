# Merge upstream/main e87d5b5f

Brings the fork's main up to upstream/main e87d5b5f (60 commits) and moves the fork's shader effects and
transitions onto upstream's new `Shader` (#517), so NSH keeps one GPU pipeline: upstream's. Merge with a merge
commit. Branch `sync/upstream-e87d5b5f`; the next fork tag is `fork-0.3.0-2`.

## Before the merge

- `cli: Drop --strict-port, which upstream's port takeover replaces`: upstream's #428 keeps the requested port and
  replaces the previous development session on it, so `--strict-port` (and `NSH_STRICT_PORT`) has nothing left to do.
  Consumers that pass it (Gamify's `xtask web` and browser specs) drop it with their next NSH bump.
- `core: Remove the fork's shader pipeline, which upstream's Shader replaces`: upstream added a `Shader` element
  drawn by wgpu with its own `nsh-shader` crate. The fork's pipeline (#19, #20, #21, #22, #26: the WGSL profile and its
  MSL/HLSL/GLSL/AGSL writers, `nsh-metal`, the per-platform renderers, its `Shader`, `ShaderEffect`,
  `ShaderTransition`, `ScreenEffect`, `ScreenTransition`, `createShader`, `shaderSupport`) goes; files only it
  changed are upstream's again, the fork's other features stay.

## The merge

- What it brings: Shader (#517), Select, OtpInput, NumberInput, RichEditor tables, plot parts, the port takeover
  (#428), the incremental web dev runtime (#429), Android display density changes (#431, the fork's own feature as
  upstream merged it), the browser JavaScript engine on the web, one event object for every event (#558), fixes.
- It also fixes the Linux CI's intermittent SIGSEGV after the text tests: the fork's `text.rs` added app fonts to
  Fontconfig's shared configuration while other font maps read it; upstream gives each map a configuration of its
  own (`set_font_files`).
- Conflicts: the fork's environment accessors and font functions next to upstream's in `nsh_core::prelude` and the
  crate root; the density remount (#431) is upstream's and its tests replace the fork's copy; iOS surface: upstream's
  keyboard duration in milliseconds with the fork's app state; macOS decor: upstream's shape without the shadow
  fields, the fork's visible border sides; Android: upstream's `setDensity`; docs: both sides' words.

## After the merge: effects and transitions on upstream's Shader

- `core: Add ShaderEffect, ShaderTransition, ScreenEffect and ScreenTransition on Shader`: `sample_content`,
  `sample_backdrop`, `sample_from`, `sample_to`, `nsh.progress`, `nsh.overflow` (`maxSampleOffset`) in the WGSL
  contract; the four elements (TSX from `nsh`, Rust builders); roles on `ShaderLayer`; `MountItem::ShaderTransition`
  and `ShaderInput`; `nsh_engine::shader` pictures, transitions and overflow.
- `core: Let pages and dialogs come and go through a shader`: `transitionShader` on `NavPage` and `Dialog`,
  `MountItem::NavShader`.
- The platforms (one commit each) and the gallery's Shaders page: see below.
- `docs: Describe shader effects, transitions and page shaders`.

## Validation

(filled in when the platform commits land)
