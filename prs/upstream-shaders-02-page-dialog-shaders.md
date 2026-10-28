# Upstream PR: core: Let pages and dialogs come and go through a shader

Draft of a pull request for `longbridge/nsh`. Not opened: the owner decides and posts it.

- Branch: `pr/shaders/02-page-dialog-shaders`, on `pr/shaders/01-effects-transitions`. Four
  commits: the page and dialog shaders (with a compat case), the web runtime's shader gate, a transition that runs
  backwards, and `docs/dev/shaders.md`. Open it after the 01 PR is merged (rebase onto `upstream/main` then: upstream
  squash-merges, so `git rebase --onto upstream/main pr/shaders/01-effects-transitions pr/shaders/02-page-dialog-shaders`).
- Source: the fork's `68f128636`, `7b7b91f0d`, `42e06ab42` and the `docs/dev` part of `deac47d86` on
  `sync/upstream-478737b1`, messages rewritten for upstream. One fix the fork's main lacks: its web gate looked for a
  key `shader` (an earlier `animation: { shader }` design), so a web app whose only shader is a `transitionShader`
  got a runtime without naga; here the bundler counts `transitionShader` (key or string) instead, with a test.
- Every platform PR of the series sits on this one.

- Checked locally on `ba289971`: as 01, plus `cargo test -p nsh-cli web`; core compat passes. The base domain stops at
  step 887 (a Toggle frame) on `main` itself; run with `base-transition-shader` moved first, its 11 steps pass and the
  run stops at the same Toggle step.

The body to paste starts below the line.

---

## Description

Pages and dialogs come with the platform's own motion (`animation`). With `ShaderEffect` and `ShaderTransition`
(#[01 PR]) an app can change a view through a shader; this PR lets a page's push and pop and a dialog's presentation
and dismissal go through one too: a wipe, an iris, a ripple.

- **`transitionShader`** on a `NavPage` (a screen) or a `Dialog` (a modal): `{ source, uniforms?, duration? }`. The
  lower side (the page under it, the surface without the dialog) is `sample_from`, the upper `sample_to`, and
  `nsh.progress` goes from 0 to 1 when it comes, from 1 to 0 when it goes (`ShaderView::start_transition_backwards`).
  The core compiles the source once and sends `MountItem::NavShader { id, shader }` with a `Transition` layer
  (350 ms by default); a platform that does not draw it keeps the page's or the dialog's `animation`.
- **Web runtime gate.** A web runtime links naga only for apps that may make a shader view
  (`nsh_feature_shader`). The bundler now counts the new elements' tags and `transitionShader` as shader use, and the
  CLI keeps the gate for imports of any of the five elements. The built-in cross-fade and page shaders compile
  through the runtime's compiler, so `shader::crossfade()` gives `None` in a runtime without one.
- **`docs/dev/shaders.md`**: how the core and the renderer carry effects, transitions and page shaders.

## Public API

- `nsh/base`: `NavPage` and `Dialog` prop `transitionShader?: { source: string; uniforms?: Record<string,
  ShaderUniform>; duration?: number }` (ms, 350).
- `nsh-core` (`ops`): `Prop::TransitionShader(Option<NavShaderSpec>)` (`#[prop(js = "transitionShader", on = SCREEN |
  MODAL)]`).
- `nsh-core` (`shader`): `NavShaderSpec`, `NavShader { layer, duration }`; `crossfade()` returns
  `Option<Shader>` (it is new in #[01 PR]).
- `nsh-core` (`mount`): `MountItem::NavShader { id, shader: Option<NavShader> }`.
- `nsh-engine` (`shader`): `ShaderView::start_transition_backwards`.

## How to Test

- `cargo test -p nsh-core -p nsh-js -p nsh-bundler --features nsh-bundler/bundle` (the bundler's
  `a_shader_tag_counts_as_a_shader` covers `transitionShader`).
- `cargo test -p nsh-engine --test engine shader` (`pages_and_dialogs_take_a_transition_shader`: a page's and a
  dialog's shader arrive as `NavShader` with their duration, reading `from` and `to`).
- `cargo test -p nsh-cli web`.
- `NSH_COMPAT_APP=base cargo xtask compat`: the new case `base-transition-shader` (pages and a dialog with a
  `transitionShader` push, pop, open and close as without one).

## Checklist

- [x] I have read the [CONTRIBUTING](../CONTRIBUTING.md) document and followed the guidelines.
- [x] Reviewed the changes in this PR and confirmed AI generated code (if any) is accurate.
- [x] Passed `cargo test`, `cargo +nightly fmt` and clippy for the crates it touches (clean on the changed crates; see the note above the line).
- [x] Changes to what apps use keep what released versions shipped and come with a case in `tests/compat/`
  (`cargo xtask compat`).
- [ ] Ran the related `examples/gallery` demos on iOS and Android: the platform PRs and the gallery PR do.
