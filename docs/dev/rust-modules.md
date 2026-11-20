# Several Rust crates in one app

Status: implemented (`crates/nsh-cli/src/rust_app.rs`; `docs/dev/tooling.md`, "Apps with Rust components of their
own").

An app used to have at most one Rust crate (`nsh.json` `"rust": "<crate dir>"`, a string), with one
`#[native_module("<specifier>")]` entry, so everything Rust it offered was under one import specifier. Splitting an
app's native code (charts, storage, a shared crate of company components) meant one crate that depended on the
others and registered them all under its specifier.

## Design

- `"rust"` takes a crate or a list of them: `"rust": ["native/charts", "native/storage"]`. Each is a library crate
  with its own `#[native_module("<specifier>")]` (one per crate, as before), so each is its own import:
  `import { Chart } from "app/charts"`, `import { Store } from "app/storage"`. A string is a list of one.
- `rust_app.rs` writes one generated workspace with all of them: the describe program, the headless runners and every
  host and runner (iOS, Android, Linux, macOS, Windows, the web, the GPUI workspace, a Rust app's runners) depend on
  each crate and pass all their registries (`Description::of`, `headless::runner`, `set_ui_sets`, `ui_sets!`) in the
  list's order, after `nsh_base::registry()`, served beside the runtime's modules (the describe program describes
  the app's sets only).
- The generated code links each crate under a name of its own, `app_<package>` (`-` as `_`), and makes its set in a
  function of its own, `set_app_<package>()`, whose body glob-imports that crate: the fixed-name entry
  `__nsh_native_module` of each crate is found in its own scope, and one crate without an entry fails the build with
  what to add on the line of that crate (the file's one import of nsh's stand-in is what the glob shadows).
- Checked when the workspace is written (`rust_app::crates`): a crate listed twice, two crates with the same package
  name (Cargo takes `-` and `_` as one), or two whose `#[native_module("…")]` has the same specifier (read from the
  crate's `[lib] path` or `src/lib.rs`) fail with the two paths and what to change. That reading is early and
  friendly; the authority is the description the crates' describe program prints, in which `nsh` refuses two sets
  with one specifier, or one with a specifier of nsh's own (`rust_app::check_specifiers`), however the entry was
  written. `Registry::merged` already refuses two sets registering
  components or modules of the same name when an engine starts.
- Each specifier gets its declarations (`.nsh/types/<specifier>.d.ts`) and its assets (`asset:/@<package>/...`,
  already per package), as both come from the description of its set.
- `nsh dev` / `nsh run` watch every listed crate and the local crates they depend on by path, transitively
  (`RustApp::watched`; nsh's own crates and dev-dependencies are left out): a change to any of them describes the app
  again and restarts the session, as with one crate.
- Crates may depend on each other, or on crates not listed (a shared crate of components that another crate's
  components use), through plain Cargo path dependencies; only the ones listed in `"rust"` export to TS.
- A Rust app's `"root"` is a component of the first crate listed.
- The generated binaries are named after the first crate's package and a digest of all the crates' packages
  (`nsh-app-charts-ef5aa537-describe`), so they do not clash with the crates' own binaries in the app's `target/`, and
  their names stay short however many crates an app lists (Windows' linker cannot open paths over 260 characters).

## A library's set

A library of components (a crate several apps build on) has a UI set of its own under a fixed specifier
(`acme/ui`), so every app, and the library's own files (its examples, previews and docs), import its components by
one name. The library makes its set in a plain function, and the crate of each app using it serves the set from its
entry: nothing about the library is listed in `nsh.json`, and the app names it once, in Cargo.

```rust
// The library's crate, `acme-ui`: no `#[native_module]` of its own.
pub fn set() -> Registry {
    let mut registry = Registry::ui_set("acme/ui");
    registry.register::<Chip>().register::<Card>();
    registry
}

// An app's crate (`nsh.json` `"rust"`).
#[native_module("app/native")]
fn native(registry: &mut Registry) {
    registry.register::<Gauge>().serve(acme_ui::set());
}
```

- `Registry::serve` (nsh-core `element/component.rs`) serves a set wherever the set serving it is served, under its
  own specifier; `Registry::submodule` is the other kind, a set under a path of the set's own (`nsh/base/plot`). It
  panics for a set without a specifier, or one whose specifier is the serving set's or a path under it.
- `Registry::sets` lists a set, its submodules, then the sets it serves, each with theirs. Two crates of one app built
  on one library both serve its set: `Registry::served_sets` flattens a list of sets with each specifier once. A set
  whose specifier is listed already is left out when it exports the same (components, modules and classes, by name
  and Rust type: each crate made it with the library's function), and refused, naming the sets serving both, when it
  is another (two versions of one library). Everything that takes a list of sets goes through it:
  `Registry::merged`, `Description::of`, nsh-js's module table, the engine's fonts, nsh-testing, the platforms'
  development hosts, the web runtime and the CLI.
- The description (format 3) lists every set once, each with `entry`, the index of the crate whose entry serves it
  (the first, when two do). `rust_app::read_description` takes each crate's sets by it: the crate's own set first,
  imported by the crate's specifier, then its submodules and the sets it serves, which the generated code links with
  the crate (the hosts' `set_<crate>()` returns the whole set; the web runtime's gate of the crate,
  `nsh_ui_set_<specifier>`, registers them, so an app importing a served set keeps that crate's gate:
  `web::app_set_gates`). `check_specifiers` refuses a set imported as one of nsh's modules, two sets of one
  specifier, and a crate with no set of its own: one another crate serves too (list the library's crate in `"rust"`
  or serve its set, not both). Submodules of an app's crate are described the same way.

## Why a list of crates

The alternative, several `#[native_module]` entries in one crate, needs a collection of entries the generated code
can find (it finds one entry per crate by a fixed name), and keeps everything in one compilation unit. A crate per
specifier is Rust's own unit of reuse: a crate of shared components can be listed by several apps, versioned and
tested on its own.
