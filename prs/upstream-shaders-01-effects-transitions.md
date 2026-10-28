# Upstream PR: core: Add ShaderEffect, ShaderTransition, ScreenEffect and ScreenTransition on Shader

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/01-effects-transitions`, one commit on `upstream/main` `ba289971`.
  The first of the shader series that replaces the fork's old `pr/shaders/*` (built on the fork's own GPU pipeline,
  removed when the fork took upstream's `Shader` from #517): every part now sits on upstream's `Shader`, its WGSL
  contract and its wgpu renderer.
- Series: `01-effects-transitions` (this) → `02-page-dialog-shaders` → one branch per platform on 02 (`03-macos`,
  `04-ios` on 03, `05-android`, `06-web`, `07-windows`, `08-linux`, `09-gpui`) → `10-gallery-docs` on 02. Open them in
  that order; each platform PR is independent of the others.
- Source: the fork's port on `sync/upstream-478737b1`, commit `bf22a9f47`, with the commit message rewritten for
  upstream and a compat case added.
- Until a platform PR lands, its platform ignores the new mount items: the children show as they are and transitions
  change at once. The layer of an effect or a transition still arrives as `MountItem::Shader`, which such a platform
  draws as it draws a fill, under the children, with every picture transparent (the GPUI PR, 09, filters those
  layers out, since GPUI keeps drawing fills only). If a reviewer wants 01 to stand alone, the same filter can go
  into each backend here.
- Rebased onto `upstream/main` `ba289971` before opening. Conflicts on the way: `Core::commit`'s end on `7dba85d4`
  (upstream's #669 `split_scoped_frames` runs first, then `mark_shader_inputs` and the transition starts spliced at the
  batch's front), and the `element` re-exports on `ba289971` (#674's `Svg`, `svg` kept beside the new names).

- Checked locally on `ba289971` (2026-10-28): clippy `-D warnings` of the Core workflow's crates plus `nsh-cli`; `cargo test -p nsh-core -p nsh-js -p nsh-bundler` and `-p nsh-engine --test engine shader`; nightly rustfmt on the changed files; `NSH_COMPAT_APP=core cargo xtask compat` with `nsh-compat`'s `follow-nsh-main` (its `main` does not build against NSH's): passes.

The body to paste starts below the line.

---

## Description

`Shader` draws a view's box with WGSL. Apps also want views drawn *through* a shader: a ripple or a dissolve over a
card, a frosted glass that reads what is behind it, a card that changes through a wipe, the whole screen dimmed or
warped for a moment. This PR adds that on `Shader`'s own program, layer and renderer, as four elements; the platforms
draw them in the PRs that follow, so this one changes nothing an app sees yet.

- **WGSL.** Four functions read pictures the platform takes, each taking a point `uv` of the box (as `shade`'s) and
  returning a straight-alpha color, transparent outside the picture: `sample_content(uv)` (an effect's children, a
  screen effect's surface), `sample_backdrop(uv)` (what is behind the view; any shader may read it), `sample_from(uv)`
  and `sample_to(uv)` (a transition's two sides). `struct Nsh` gains `progress` (0 to 1: an effect's `progress`, or
  how far a transition is) and `overflow` (`maxSampleOffset`, points the shader may draw beyond its box; `uv` then runs
  past 0 and 1). The pictures are bound at `@binding(2)` (a sampler) to `@binding(6)`; `Shader::inputs` reflects which
  of the functions a program calls, and a program that calls none keeps today's two bind group layouts.
- **Elements** (`nsh`, and Rust `nsh_core::element`): `ShaderEffect` / `shader_effect()` draws its children through
  the shader (they keep their presses, focus and accessibility); `ShaderTransition` / `shader_transition()` changes
  them through it when its `value` changes (not its first value), for `duration` ms (300), and without `source` runs
  `fallback` (`"fade"`, a built-in cross-fade, or `"none"`); `ScreenEffect` / `screen_effect()` and `ScreenTransition` /
  `screen_transition()` do the same for the whole surface and take no space.
- **Core.** The layer carries its role (`ShaderRole`), overflow and progress; an animated `progress` is one more
  canvas slot. A changed `value` queues `MountItem::ShaderTransition { id, duration }`, put first in its batch so the
  platform pictures `from` before the batch applies; after a commit, `MountItem::ShaderInput { id }` names the effects
  and transitions whose subtree the batch touched (and backdrop readers and screen layers when anything changed).
- **Renderer** (`nsh_engine::shader`): `ShaderView::set_input` uploads a picture, `start_transition` runs the
  progress, frames grow by the overflow.
- **Bundler.** The JSX compiler emits the four elements as host elements (`shadereffect`, `shadertransition`,
  `screeneffect`, `screentransition`), as it does `Shader`.

### The series

This is the first of ten PRs; the others follow as each one they sit on is merged (the fork's branches show them now):

1. this one: the elements, the WGSL inputs and the renderer (`pr/shaders/01-effects-transitions`);
2. pages' and dialogs' `transitionShader` (`pr/shaders/02-page-dialog-shaders`);
3. to 9. one per platform, each on 2 and independent of the others: macOS, iOS, Android, web, Windows, Linux, and GPUI
   keeping fills only (`pr/shaders/03-macos` … `09-gpui`);
10. the gallery's Shaders page and the website's docs (`pr/shaders/10-gallery-docs`).

Branches: https://github.com/dXnSix/nsh/branches/all?query=pr%2Fshaders

## Public API

- `nsh` (TSX): `ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition`, with `ShaderEffectProps`,
  `ShaderTransitionProps`, `ScreenEffectProps`, `ScreenTransitionProps`. Props besides `Shader`'s `source`,
  `uniforms` and `paused`: `progress?: number | AnimatedNode` (effects; `nsh.progress`), `maxSampleOffset?: number`
  (`ShaderEffect`, `ShaderTransition`; points), `value?: string | number | boolean`, `duration?: number` (ms, 300),
  `fallback?: "fade" | "none"` (transitions).
- `nsh-core` (`element`): `shader_effect()`, `shader_transition()`, `screen_effect()`, `screen_transition()`;
  `ShaderView::progress`, `animated_progress`, `max_sample_offset`, `value`, `duration`, `fallback`.
- `nsh-core` (`shader`): `ShaderInput` (`Content`, `Backdrop`, `From`, `To`), `ShaderInputs`, `Shader::inputs`,
  `ShaderRole` (`Fill`, `Effect`, `Transition`, `Screen`, `ScreenTransition`), `ShaderProgress`, `TransitionKey`,
  `PROGRESS_WORD`, `crossfade()`; `ShaderLayer::role`, `overflow`, `progress`, `with_role`, `with_overflow`,
  `with_progress`, `progress_slot`.
- `nsh-core` (`ops`): `Tag::ShaderEffect`, `ShaderTransition`, `ScreenEffect`, `ScreenTransition`;
  `Prop::ShaderProgress`, `MaxSampleOffset`, `TransitionValue`, `TransitionDuration`, `TransitionFallback`;
  `TransitionFallback` (`Fade`, `None`).
- `nsh-core` (`mount`): `MountItem::ShaderTransition { id, duration }`, `MountItem::ShaderInput { id }`,
  `MountItem::view`.
- `nsh-engine` (`shader`): `ShaderView::set_input`, `has_input`, `start_transition`, `transitioning`.

## Breaking Changes

Nothing apps use changes: `Shader`'s props, its WGSL (`shade`, `nsh.size`, `nsh.time`, `nsh.scale`, `uniforms`) and
what it draws stay as they are.

For backends: `nsh_core::shader::builtins` takes the progress and the overflow and returns 8 words (the uniform
buffer of `struct Nsh` is 32 bytes), and `MountItem` has two more variants, which every backend in this repository
ignores in this PR.

```diff
- pub fn builtins(size: Size<Points>, time: f32, scale: f32) -> [u32; 4]
+ pub fn builtins(size: Size<Points>, time: f32, scale: f32, progress: f32, overflow: Points) -> [u32; 8]
```

## Performance

A `Shader` that reads no picture builds the same pipeline and bind groups as before. The core tracks the views whose
layers read pictures (`Core::pictured`) and, after a commit, walks up from the batch's views only when there are any.

## How to Test

- `cargo test -p nsh-core -p nsh-js -p nsh-bundler --features nsh-bundler/bundle`
- `cargo test -p nsh-engine --test engine shader`: the effect's layer and its pictures' bind group layout, the
  transition's start first in its batch, its cross-fade fallback, an animated progress through its slot.
- `NSH_COMPAT_APP=core cargo xtask compat`: the new case `core-shader-effects` (layout, an effect's children keep
  their presses, transitions' children change with `value`). Run against `nsh-compat`'s `follow-nsh-main`
  (`5fc0229`): its `main` does not build against NSH's current `main` (`nsh_core::ui` and others are gone).
- The case claims no `covers`: `nsh-compat` reads which exports a bundle uses from its own list of the JSX
  compiler's primitives, which lacks the four tags, so such a claim would fail. Its steps check the elements.

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches (clean on the changed crates; see the note above the line).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the platform PRs and the gallery PR do.
