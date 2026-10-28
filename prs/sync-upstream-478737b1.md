# Merge upstream/main 478737b1

Brings the fork's main up to upstream/main 478737b1 (126 commits) and moves the fork's shader effects and
transitions onto upstream's new `Shader` (#517), so NSH keeps one GPU pipeline: upstream's. Merge with a merge
commit. Branch `sync/upstream-478737b1`; the next fork tag is `fork-0.3.0-2`.

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

- What it brings: Shader (#517), Select, OtpInput, NumberInput, DatePicker, Calendar, ColorPicker, Combobox, Toggle
  and ToggleGroup, RichEditor tables, plot marks and Sankey, the port takeover (#428), the incremental web dev runtime
  (#429), Android display density changes (#431, the fork's own feature as upstream merged it), every web app on the
  browser's JavaScript engine (#603), one event object for every event (#558), listeners with `on(event, l)` (#612),
  file dialogs, window and menu APIs for Rust views (#629), screen color picking, the animation graph's speed-ups
  (#588, #589, #590: the fork's #88, #89, #90 as upstream merged them), fixes.
- It also fixes the Linux CI's intermittent SIGSEGV after the text tests: the fork's `text.rs` added app fonts to
  Fontconfig's shared configuration while other font maps read it; upstream gives each map a configuration of its
  own (`set_font_files`).
- Conflicts: the fork's environment accessors and font functions next to upstream's in `nsh_core::prelude` and the
  crate root; the density remount (#431) is upstream's and its tests replace the fork's copy; iOS surface: upstream's
  keyboard duration in milliseconds and screen color with the fork's app state; web: the fork's app state and time
  zone next to upstream's screen color; macOS decor: upstream's shape without the shadow
  fields, the fork's visible border sides; Android: upstream's `setDensity`; docs: both sides' words.

## After the merge: effects and transitions on upstream's Shader

- `core: Add ShaderEffect, ShaderTransition, ScreenEffect and ScreenTransition on Shader`: `sample_content`,
  `sample_backdrop`, `sample_from`, `sample_to`, `nsh.progress`, `nsh.overflow` (`maxSampleOffset`) in the WGSL
  contract; the four elements (TSX from `nsh`, Rust builders); roles on `ShaderLayer`; `MountItem::ShaderTransition`
  and `ShaderInput`; `nsh_engine::shader` pictures, transitions and overflow.
- `core: Let pages and dialogs come and go through a shader`: `transitionShader` on `NavPage` and `Dialog`,
  `MountItem::NavShader`.
- Upstream links naga into a web runtime only for apps that use shaders (`nsh_feature_shader`): the bundler counts
  the four elements' tags and a page's `animation: { shader }` as shader use, the built-in cross-fade and page
  shaders compile through the runtime's compiler.
- The platforms (one commit each) and the gallery's Shaders page: see below.
- `docs: Describe shader effects, transitions and page shaders`.

## Validation

(filled in when the platform commits land)
