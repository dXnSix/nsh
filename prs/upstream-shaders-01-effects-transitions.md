# Upstream PR: core: Add ShaderEffect, ShaderTransition, ScreenEffect and ScreenTransition on Shader

Draft of a pull request for `longbridge/nsh`, posted as [longbridge/nsh#682](https://github.com/longbridge/nsh/pull/682)
(2026-10-28) with the body below the line.

- Branch: `pr/shaders/01-effects-transitions`, on `upstream/main` `ba289971`: the commit `5fec8534a` as opened, and
  three fix commits for the review (upstream squash-merges): `f4409abd3` web, `ab027dda3` inputs, `4baebb737` removal,
  and `36b23d76a` for Windows: upstream's `redrawn` in `nsh-platform-windows/src/mount.rs` matches every `MountItem`
  and lacked the two new variants, so the Windows check failed on #682 from the start (`5fec8534a` too; the fork's
  Windows run was on 07, which covers them). 02 adds `NavShader` to the same match in its page/dialog commit; 07's
  `redrawn` is unchanged. Not checkable on macOS (`ring` needs Windows headers): upstream's Windows job decides.
  The first of the shader series that replaces the fork's old `pr/shaders/*` (built on the fork's own GPU pipeline,
  removed when the fork took upstream's `Shader` from #517): every part now sits on upstream's `Shader`, its WGSL
  contract and its wgpu renderer.
- Series: `01-effects-transitions` (this) → `02-page-dialog-shaders` → one branch per platform on 02 (`03-macos`,
  `04-ios`, `05-android`, `06-web`, `07-windows`, `08-linux`, `09-gpui`) → `10-gallery-docs` on 02. Open them in
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
- Checked again after the review fixes (2026-10-28), on 01 (`4baebb737`) and 02 (`9a7251639`): the same clippy,
  `cargo test` (1228 passed on each) and `nsh-engine --test engine shader` (4 and 5 passed); nightly rustfmt on the
  changed `.rs` files; `node --test crates/nsh-platform-web/web/nsh.test.mjs` on 01 and 06-web (27 passed). Compat
  (`NSH_COMPAT_APP=core`, `follow-nsh-main` `5fc0229`): the core domain is clean; the run still fails 15 `covers`
  claims of upstream's own base cases (Accordion, OtpInput, NumberInput, Slider, TextView, RichExtension), the same
  15 on the original `5fec8534a`, so not from this PR (the earlier "passes" above did not hold on rerun). No
  platform crate's Rust changed, so the platform clippy runs were not repeated.

## Review (2026-10-28)

huacnlee requested changes (review `5688690224`, "Reviewed by ChatGPT on behalf of @huacnlee", static inspection
only) with three inline comments; all three hold, and each has a fix commit with a regression test. 02–10 restacked
on the fixes (02's dev docs updated in its docs commit; 06-web's conflict on the built-ins upload resolved to its own
8-word upload, and the new nsh.test.mjs case adapted to its renderer).

Replies:

1. [P1] web built-ins (`r4405288608`, `shader.rs` vertex shader):
   > Right: on this PR alone a plain `Shader` on the web uploaded 16 bytes for a 32-byte block, and WebGL refuses the
   > draw. f4409abd3 uploads all eight words (progress and overflow at 0, as `shader::builtins` lays them out), and
   > `nsh.test.mjs` gains "a plain Shader uploads the whole built-ins block, progress and overflow at 0", which drives
   > `renderShader` on a recording GL and fails on the old upload. The web PR of the series later fills progress and
   > overflow in the same upload.

2. [P2] `sample_*` helpers (`r4405288617`, `reads_inputs`):
   > Agreed. ab027dda3 drops the name filter and reflects reachable usage instead: `Shader::inputs` is now the
   > picture textures `nsh_fragment` uses according to naga's `FunctionInfo`, which follows calls, so a helper of any
   > name counts and one `shade` never calls does not. `inputs_are_the_pictures_shade_reaches` covers a
   > `sample_custom` helper calling `sample_content`, and the unused-helper case.

3. [P2] removed child (`r4405288622`, `mark_shader_inputs`):
   > Right, the walk starts from each mount item's view, and a removed child has no parent (or no node) left by then.
   > 4baebb737 records the old parent in `unlink` while any shader reads pictures (`Core::unlinked_from`, cleared each
   > commit), and `mark_shader_inputs` walks up from those parents as well.
   > `an_effect_takes_its_picture_again_when_a_child_leaves` removes and releases an absolute child of a fixed-size
   > `ShaderEffect` and asserts its `ShaderInput`; it fails without the fix.

- 2026-10-28: 01–10 force-pushed (with lease) to the fork; the three replies posted (`r4405581783`, `r4405582099`,
  `r4405582387`); the PR body updated to the one below. Re-requesting huacnlee's review through the API was refused
  (404, no write access to the repository): re-request it from the PR's Reviewers box.
- 2026-10-28: the Windows fix pushed (owner's OK): 01 `36b23d76a`, 02 `e294b76e2`, 03–10 restacked on it.

- 2026-10-29: rebased onto `upstream/main` `8d790445` (#682 conflicted with `main`): `fa324f651`, the same five
  commits; conflicts only where both sides added at the end (`nsh.test.mjs`, `core/tests.rs`: both kept). The four
  elements' declarations now return `JSX.Element` (folded into the first commit): upstream's #593 dropped `YNode`, and
  its new `Rust application` step (`nsh check examples/counter -- --skipLibCheck false`, #684) type-checks the
  declarations. Checked locally: clippy `-D warnings` of the workspace (all targets), `cargo test -p nsh-core -p
  nsh-js -p nsh-bundler --features nsh-bundler/bundle` and `-p nsh-engine --test engine shader`, wasm32 clippy
  (`compiler`), `nsh check examples/counter -- --skipLibCheck false`, `node --test` of `nsh.test.mjs`, `scroll`,
  `mouse`, `local-time` (45 pass), nightly rustfmt on the changed files.
- 2026-10-31: one more commit, `de88f19b9` (`core: Pad the shader built-ins to 32 bytes, so GLES devices draw
  them`). `struct Nsh` was 24 bytes in WGSL while `builtins` and the renderers write 32; wgpu refuses a uniform struct
  whose size is no multiple of 16 where the device lacks `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`, so on the arm64
  Android emulator (swiftshader, GLES) every shader failed: "group 0 binding 0 has a size of 24" (found while fixing
  the fork's Android shaders test, dXnSix/nsh#94). `@size(12)` on `overflow` pads it, with no member apps could read;
  `the_built_ins_take_the_bytes_the_renderers_upload` fails on the old struct (24, not 32). The GLSL the web gets is
  unchanged. Checked locally: `cargo test -p nsh-core --features glsl --lib shader::` (8 pass), `-p nsh-engine
  --features shader --lib shader::` (5 pass, Metal), clippy `-D warnings` of both with those features, nightly
  rustfmt. On the arm64 API 35 emulator (`nsh-ci-35`, swiftshader, Vulkan off: wgpu on GLES), 05-android's gallery
  Shader page: before, all four shaders refused (`binding 0 has a size of 24`) and no frame drawn; after, Glow and
  Code rain draw, and the two whose own `Uniforms` are 8 and 4 bytes are refused on `binding 1`: the same rule for
  the app's struct, upstream's contract for a plain `Shader` since #517, not this PR's.
  02–10 restacked on it (previous heads `old-pre-pad/*` locally). The body below says it in "For backends" and "How
  to Test".

The body to paste starts below the line.

---

**Depends on:** no other PR; it applies to `main` as it is. The rest of the shader series depends on it (see "The
series" below), so it goes first.

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
  of them the fragment stage reaches (through `shade` and whatever it calls), and a program that reads none keeps
  today's two bind group layouts.
- **Elements** (`nsh`, and Rust `nsh_core::element`): `ShaderEffect` / `shader_effect()` draws its children through
  the shader (they keep their presses, focus and accessibility); `ShaderTransition` / `shader_transition()` changes
  them through it when its `value` changes (not its first value), for `duration` ms (300), and without `source` runs
  `fallback` (`"fade"`, a built-in cross-fade, or `"none"`); `ScreenEffect` / `screen_effect()` and `ScreenTransition` /
  `screen_transition()` do the same for the whole surface and take no space.
- **Core.** The layer carries its role (`ShaderRole`), overflow and progress; an animated `progress` is one more
  canvas slot. A changed `value` queues `MountItem::ShaderTransition { id, duration }`, put first in its batch so the
  platform pictures `from` before the batch applies; after a commit, `MountItem::ShaderInput { id }` names the effects
  and transitions whose subtree the batch touched or a view left (and backdrop readers and screen layers when anything
  changed).
- **Renderer** (`nsh_engine::shader`): `ShaderView::set_input` uploads a picture, `start_transition` runs the
  progress, frames grow by the overflow.
- **Web.** The WebGL renderer uploads the whole 32-byte `struct Nsh` for a plain `Shader`, progress and overflow
  at 0; the web PR fills them in.
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
buffer of `struct Nsh` is 32 bytes, its `overflow` padded, as GLES devices without
`BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` take only uniform structs of whole 16-byte rows; the web renderer here uploads
all of it), and `MountItem` has two more variants, which every backend in this repository ignores in this PR.

```diff
- pub fn builtins(size: Size<Points>, time: f32, scale: f32) -> [u32; 4]
+ pub fn builtins(size: Size<Points>, time: f32, scale: f32, progress: f32, overflow: Points) -> [u32; 8]
```

## Performance

A `Shader` that reads no picture builds the same pipeline and bind groups as before. The core tracks the views whose
layers read pictures (`Core::pictured`) and, after a commit, walks up from the batch's views only when there are any.

## How to Test

- `cargo test -p nsh-core -p nsh-js -p nsh-bundler --features nsh-bundler/bundle`: among them
  `inputs_are_the_pictures_shade_reaches` (a helper of any name, an unused helper),
  `an_effect_takes_its_picture_again_when_a_child_leaves` (a removed child marks its effect) and
  `the_built_ins_take_the_bytes_the_renderers_upload` (the WGSL struct and `builtins` agree on 32 bytes).
- `node --test crates/nsh-platform-web/web/nsh.test.mjs`: a plain `Shader` uploads the 32-byte built-ins block.
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
