# Tizen (Samsung TVs)

NSH has two Samsung TV targets. **`tizen`** (this page) runs apps natively: a TPK whose C# host starts
`libnsh.so`. **`tizen-web`** runs them in the TV's browser engine as a Tizen web app (`.wgt`), the web runtime with a
TV shim ([web.md](web.md), "Tizen web apps"). Both read the same `nsh.json` `tizen` section and sign with the Tizen
SDK's `tz`; `nsh run`, `nsh package` and `nsh test` take either as `--target`.

| | `tizen` | `tizen-web` |
| --- | --- | --- |
| Runtime | `nsh-platform-tizen` (GPUI on dXn.GpuiTizen), the `tizen` component | the web runtime (`nsh-platform-web`), the `web` component |
| `platform.os` | `"tizen"` (`Os::Tizen`) | `"web"` (`Os::Web`) |
| Idiom | TV (`platform.tv`) | TV, from its page |
| `nsh package` | a signed TPK (one per `--arch`, or the multi-architecture `.zip`) | a signed `.wgt` |
| `nsh run` | the development TPK over `sdb` (`sdb reverse`, else `sdb forward`) | a development `.wgt` opening the dev server's page at the LAN address |
| `nsh test` | the development TPK on the TV or the emulator, over the dev protocol | headless Chromium, the TV's page in the TV idiom |
| Apps with Rust crates | `.nsh/rust/tizen`, built with the component's target specs | the web runtime of the app's crates |

## What apps see

`platform.os` is `"tizen"` (Rust's `Os::Tizen`), the idiom `"tv"` (`platform.tv`), `platform.desktop` false;
`platform.select({ tizen })` picks a value before `tv`, and TV props (`tv={{…}}`) apply. The remote's keys arrive as
NSH keys (`TV-1`: the arrows, Enter, `BrowserBack`, the color and media keys, `Info`); Back is NSH's back
navigation, and from the first screen, with nothing taking it, the app asks before leaving (Samsung's rule).
`nsh/tizen` gives the product information and registers the remote's keys ([js-modules.md](js-modules.md)). The
TV's settings (locale, time zone, font scale, reduce motion) reach the app as on every platform; the appearance is
dark. Pause and resume are `Lifecycle::Background` and `Active`. A Tizen web app is the web in the TV idiom, with the
same `nsh/tizen` served from the TV's web APIs.

NSH apps run natively on Samsung TVs (Tizen 8.0 and later) through `crates/gpui/nsh-platform-tizen`: NSH's GPUI
backend (`nsh-platform-gpui`'s `AppView`) on [dXn.GpuiTizen](https://github.com/dXnSix/dXn.GpuiTizen)'s Tizen
platform for GPUI (`dxn_gpui_tizen`: `ui_app_main` and Ecore's main loop, one full-screen window on Ecore_Wl2, EGL
and GL ES, the remote's keys, the on-screen keyboard, the app's lifecycle, the TV's settings). A Tizen target is
`target_os = "linux"` with `target_vendor = "tizen"`; how `nsh-platform-gpui` builds for it is in
[gpui.md](gpui.md) ("Tizen"). The app reports `Os::Tizen` and the TV idiom.

## The crate

`nsh-platform-tizen` is a `cdylib` and an `rlib` with the library name `nsh`, like Android's: `libnsh.so`, the
library a TPK's C# host calls, exports one symbol, `nsh_tizen_main(argc, argv)`. It lives in the GPUI workspace
(`crates/gpui/`), as it needs GPUI. Its modules (`src/`):

| Module | What it does |
| --- | --- |
| `lib.rs` | `nsh_tizen_main` (feature `entry`, a default one), and for the libraries of apps with Rust crates of their own `main`, `run_rust` and `set_ui_sets` |
| `tizen.rs` (Tizen only) | `dxn_gpui_tizen::run` with the package's bundle (or the development runtime), the app framework's data and cache directories (`app_get_data_path`, `app_get_cache_path`, each with `nsh/` in it), with `dev` the system's log (dlog, tag `NSH`) |
| `app.rs` | The full-screen window holding `AppView`, and the exit question over it; the TV's settings and the app's state given to `AppView` before the app loads (its engine starts with them), the settings again as they change; Exit quits GPUI, which ends `ui_app_main` |
| `settings.rs` | `dxn_gpui_tizen::settings` (locale, time zone, font scale, screen reader, reduce motion) as the values of `AppView`'s setters, and the setters to call again when they change; the appearance is dark (Tizen 8.0 reports no light or dark setting) |
| `lifecycle.rs` | The platform's window activation (off while the app framework has the app paused, which the platform reports to GPUI with the window hidden) as `Lifecycle::Background` / `Active`, through `AppView::set_lifecycle` |
| `back.rs` | Samsung's rule for Back: NSH's back navigation first (a modal, a stack, `onBackPress`); when nothing takes it (`AppView::on_unhandled_back`), the app asks whether to leave, staying chosen |
| `prompt.rs` | How that question reads: its words in the TV's language (English, Chinese in its script, Korean, Japanese, Spanish, French, German, Portuguese, Russian; English for the others), its sizes at the TV's font scale, and for assistive technologies an alert dialog named by the question with its answers as buttons, the chosen one the focused node, so Voice Guide reads them |
| `keys.rs` | The remote's key names (dXn.GpuiTizen's `keys::NAMED_KEYS`) as NSH reads them: NSH keys (`nsh_platform_gpui::nsh_key`, `TV-1`'s `ColorRed`, `MediaPlayPause`, `Info`, ...), `back` (navigation), `exit` (the TV's) |
| `tv.rs` | The TV's side of `nsh/tizen` (`nsh_core::runtime::TizenHost`, in the view's engine context as `TizenServices`): `dxn_gpui_tizen::system_info::product_info`'s model, platform version, build and maker; the remote's keys by their Tizen names, checked on the engine's thread (`keygrab::validate`) and grabbed or released on the main thread (`keygrab::grab`, `release`, from a task `tv::serve` runs): the color, media and channel keys are grabbed from the start, Info on request |
| `package.rs` | The app in the TPK's resources: `res/nsh/bundle.nshc` (with `capabilities.json` and `bundle.js.map`), `res/nsh/assets/`, `res/nsh/app.json` (name, id, appearance), as Linux's packages hold them |
| `dev.rs` (feature `dev`) | The development runtime, below |

The logic modules build on the desktops too, and their unit tests run there:

```sh
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --lib                 # app, keys, settings, back, prompt, lifecycle, package, host, tv
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --features dev --lib  # and dev's connection policy
```

## The C# host

Tizen's app model starts a third-party native app as a .NET app: `crates/gpui/nsh-platform-tizen/host/` is the
program (`Program.cs`, one P/Invoke into `libnsh.so`'s `nsh_tizen_main`; no TizenFX) and its project on
`Tizen.NET.Sdk` (`Nsh.Tizen.Host.csproj`), the same as dXn.GpuiTizen's own host. It is built once into the runtime
files (`dotnet build -c Release`, by `cargo xtask tizen`), so packaging an app needs no .NET SDK. Without a .NET SDK
that has Tizen's workload, the crate's `host::tests` check what MSBuild would refuse first: the project must be
well-formed XML (no `--` in its comments: MSB4025), on `Tizen.NET.Sdk`, building the `Nsh.Tizen.Host` executable the
runtime files ship, and the program must keep its one P/Invoke (`$PATCH` and `$LOCK` as in [Building](#building)):

```sh
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --lib host::tests \
  --config "$PATCH" --config "resolver.lockfile-path='$LOCK'"
```

`nsh_tizen_main` is the crate's own with its feature `entry` (a default one): the prebuilt runtime's, which runs the
packaged bundle. The library of an app with Rust crates of its own ([below](#apps-with-rust-crates)) depends on the
crate without it and defines its own `nsh_tizen_main`, which gives the runtime the app's UI sets (`set_ui_sets`) and
runs it (`main`, or `run_rust` with a Rust app's root): every TV library exports that one symbol and nothing else.
The generated code asserts `nsh::ENTRY` (`cfg!(feature = "entry")`) is false, so the two entries never meet in one
library (they would be a duplicate symbol at the link anyway).

## The development runtime

With `--features dev` the library runs the dev server's app instead of a packaged bundle: its bundles with hot
updates, the status line and error overlay over the app, and the app's logs in the developer's terminal, as Android's
development host (`nsh-platform-gpui`'s dev client, `AppView::connect_dev_server_with`). A TV reaches the
developer's machine through `sdb`, not the network, so the client's connections come from `dev::Connector`
(`nsh_dev::DevTransport`):

- first `ws://127.0.0.1:8790/ws`, a fixed port on the TV that `nsh run` maps to the dev server with `sdb reverse`;
- after three refusals in a row it also listens on port 8791 of the TV's loopback (`sdbd` delivers the connections
  of `sdb forward` there; no other host on the TV's network reaches it), which `nsh run` maps from the machine with
  `sdb forward` and connects to, relaying to the dev server; from then on both are tried in turn, the one that
  worked last first.

Over `sdb forward` the bundles, hot updates and logs work; the app's `asset:/` files come from the dev server over
HTTP at the reverse-mapped port. Logs go to the dev server while connected and to dlog otherwise. Its hello names
`HostPlatform::Tizen` (`AppView::connect_dev_server_with`'s `platform`), whose install command `nsh dev` shows as
`nsh run --target tizen`; servers older than it read it as `Other`.

## Building

dXn.GpuiTizen is a git dependency of the GPUI workspace, pinned by `rev` (`crates/gpui/Cargo.toml`), until it is on
crates.io. Its TV builds take gpui-fast and wgpu from their forks: `[patch.crates-io]` over all nine `gpui-fast-*`
crates and the wgpu crates (`wgpu-hal` among them), the same set as dXn.GpuiTizen's. Until the forks are published
that patch is a local Cargo config, never committed: dXn.GpuiTizen's `cargo xtask gpui-fast-release --verify` writes
it to `artifacts-temp/gpui-fast-patch.toml` (paths into the gpui-fast fork's release workspace and `../wgpu`).

Publishing: `nsh-platform-tizen` is not published (`publish = false`); it ships built, as the CLI's `tizen` component.
`cargo publish` takes a git dependency from crates.io by its `version`, so the workspace entries of dXn.GpuiTizen's
crates carry one beside `git` and `rev` (`0.1.0`, theirs). A crate that `release-crate.yml` publishes and that
depends on them needs `dxn_gpui_tizen` (and `dxn_gpui_tizen_sys`) on crates.io at that version before the NSH
release that includes it; until then the GPUI workspace's publish step fails on it. `nsh-platform-gpui` is one: it
depends on `dxn_gpui_tizen` on Tizen (`cfg(target_vendor = "tizen")`, its videos), and `cargo publish` checks
target-specific dependencies too, so dXn.GpuiTizen goes to crates.io before the first NSH release with Tizen videos.

Before dXn.GpuiTizen is pushed, Git reaches it in the local checkout (Cargo cannot patch an unreachable git
source), so every Cargo command runs with:

```sh
export CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=3 \
  GIT_CONFIG_KEY_0=url.<dXn.GpuiTizen checkout>.insteadOf GIT_CONFIG_VALUE_0=https://github.com/dXnSix/dXn.GpuiTizen \
  GIT_CONFIG_KEY_1=url.<gpui-fast checkout>.insteadOf GIT_CONFIG_VALUE_1=https://github.com/dXnSix/gpui-fast \
  GIT_CONFIG_KEY_2=url.<wgpu checkout>.insteadOf GIT_CONFIG_VALUE_2=https://github.com/dXnSix/wgpu
```

dXn.GpuiTizen builds on Linux only against the forks (its EGL path uses gpui-fast's external-target renderer), so
the GPUI workspace leaves `nsh-platform-tizen` out of its `default-members`: a bare `cargo test` or `cargo clippy`
there (`gpui.yml`) builds the desktop crates alone, and the TV backend is built by name, with the patch.

The committed `crates/gpui/Cargo.lock` is resolved without the patch (it records the GitHub source of
dXn.GpuiTizen and the published gpui-fast). Builds and tests with the patch resolve into a lockfile kept outside the
checkout, so it never records the local paths:

```sh
PATCH=<dXn.GpuiTizen>/artifacts-temp/gpui-fast-patch.toml
LOCK=<dXn.GpuiTizen>/artifacts-temp/locks/nsh-tizen/Cargo.lock    # seeded from crates/gpui/Cargo.lock
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --lib \
  --config "$PATCH" --config "resolver.lockfile-path='$LOCK'"
cargo clippy --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --all-targets --features dev \
  --config "$PATCH" --config "resolver.lockfile-path='$LOCK'" -- -D warnings
```

TV libraries are cross-built from dXn.GpuiTizen's checkout with its target specs, the Tizen 8.0 rootstrap and the
SDK's GCC toolchain (its `docs/setup.md`), for `armv7` (32-bit ARM TVs), `x86_64` (the emulator) and `aarch64`.
`--with-dev-variant` builds the `dev` library beside the release one (in `<target dir>/dev-variant`), and
`--check-symbols` checks each, found by package whatever the library's name: exactly one
export, `nsh_tizen_main`, and only documented Tizen symbols and glibc imported. `cargo xtask cross` keeps its own
copy of the workspace's lockfile under `artifacts-temp/locks/` and passes `--config` on to Cargo:

```sh
cd <dXn.GpuiTizen>
cargo xtask cross --target armv7 --release --manifest-path <nsh>/crates/gpui/Cargo.toml -p nsh-platform-tizen \
  --with-dev-variant --check-symbols --entry nsh_tizen_main --config "$PATCH"
cargo xtask cross --target armv7 --clippy --manifest-path <nsh>/crates/gpui/Cargo.toml -p nsh-platform-tizen \
  --config "$PATCH" -- -D warnings
```

Once the forks and dXn.GpuiTizen are published, `crates/gpui/Cargo.toml` takes the patch as git lines and the
environment above goes.

## The runtime and the `tizen` component

`cargo xtask tizen` (`crates/xtask/src/tizen.rs`) builds the runtime into `target/tizen/`, as `cargo xtask android`
builds Android's, through dXn.GpuiTizen's cross driver: for each architecture it runs, in dXn.GpuiTizen's checkout,
`cargo xtask cross --target <armv7|aarch64|x86_64> --release --manifest-path <nsh>/crates/gpui/Cargo.toml
-p nsh-platform-tizen --check-symbols --entry nsh_tizen_main` (`--with-dev-variant` for both variants, or the dev one
alone with `--features dev` into `<target>/dev-variant`), then builds the C# host and strips the libraries with the
nightly's `llvm-strip` (`--strip-unneeded`: the export stays):

```text
target/tizen/{release,dev}/Nsh.Tizen.Host.dll (+ .deps.json)   the host
target/tizen/{release,dev}/<arch>/libnsh.so                    armv7l, aarch64, x86_64; stripped
target/tizen/{release,dev}/symbols/<arch>/libnsh.so            the same with its symbols
target/tizen/targets/<spec>.json, toolchain                    dXn.GpuiTizen's target specs and its nightly
```

dXn.GpuiTizen's checkout is `DXN_GPUI_TIZEN`, else `../dXn.GpuiTizen` next to this checkout. Until the forks are
published, the patch config goes on every command (`--config`, or `NSH_TIZEN_CARGO_CONFIG`, comma-separated), and
the environment of "Building" above:

```sh
DXN_GPUI_TIZEN=<dXn.GpuiTizen> cargo xtask tizen --config "$PATCH"                 # both variants, every arch
DXN_GPUI_TIZEN=<dXn.GpuiTizen> cargo xtask tizen --variant dev --arch armv7l --config "$PATCH"
cargo test -p xtask --bin xtask tizen::tests                                          # arguments, layout, dist
```

The cross builds write to `CARGO_TARGET_DIR` (made absolute), else this checkout's `target/`. An armv7l build of
both variants takes about 5 minutes on an Apple silicon Mac with a warm target directory (2m 11s release, 2m 31s dev);
stripping takes armv7l's release library from 59 MB to 41 MB (a TPK of `examples/counter`: 23 MB).

`cargo xtask dist` packs `runtime/tizen/` (both variants' host and stripped libraries for the three architectures,
and `targets/`) when dXn.GpuiTizen's checkout and the .NET SDK are there (`NSH_DIST_NO_TIZEN` leaves it out), and
`nsh component pack` makes it the `tizen` component (`nsh-tizen.tar.gz`, the same for every host). The CLI
(`runtime::tizen`) takes `NSH_RUNTIME/tizen/<variant>`, else the distribution's, adding the component the first time
a command needs it, else in a checkout `cargo xtask tizen --variant <variant> --arch <archs>`'s output.

## The CLI's Tizen target

`crates/nsh-cli/src/tizen.rs`; the `sdb` and `tz` command lines and the manifest and staging are pure functions,
tested by `cargo test -p nsh-cli --bin nsh tizen::tests`, and the commands through the binary with a fake runtime,
`tz` and `sdb` by `cargo test -p nsh-cli --test cli tizen::`.

- **`nsh.json` `tizen`** (`package::TizenConfig`, `tizen-web`'s): `package` (exactly 10 letters and digits, by
  default made from the app's `id`), `appId` (`<package>.<name>`), `version` (`x.y.z`; `x.y` gets `.0`),
  `apiVersion` (`8.0`), `privileges` (URIs, added to the internet's), `signing` (the certificate profile).
- **`nsh package --target tizen [--arch <a>,...] [--sign <profile>] [--run]`** stages
  `<out>/tizen/<name>-<arch>/` from the release runtime: `bin/Nsh.Tizen.Host.dll`, `lib/libnsh.so` for the
  architecture, `res/nsh/` (`bundle.nshc`, `capabilities.json`, `app.json`, `assets/`: `nsh-platform-tizen`'s
  `package.rs`), `shared/res/icon.png` (512×423) and `tizen-manifest.xml` (`tizen::Manifest`: a `dotnet`
  `ui-application` running the host on the `tv-samsung` profile, as dXn.GpuiTizen's
  `host/tizen-manifest.template.xml`), then signs it with `tz pack -t tpk -s <profile> <staging>` and moves the
  package `tz` wrote (inside or next to the staged tree) to `<out>/<name>.tpk`. Without a profile it refuses, saying
  how to make one. With several architectures each package's version gets the fourth number (armv7l 1, aarch64 2,
  x86_64 3) and the CPU feature (`platform.core.cpu.arch.armv7` / `armv8` / `x86_64`), and the packages go to
  `<out>/<package>-<version>.<n>-<arch>.tpk` and into `<out>/<package>-<version>.zip` for Seller Office
  (dXn.GpuiTizen's `xtask/src/package/` conventions). `--run` installs the TV's architecture's package and
  launches it.
- **`nsh run --target tizen [--device <serial>]`** finds the device (`sdb devices`; the only one, or `--device`) and
  what it is (`sdb capability`: `cpu_arch`, and `secure_protocol`, a retail TV, which has no shell), builds the
  development TPK (`NshDevTizn.NshDev`, one package for every app, apart from `tizen-web`'s `NshDevHost`) from
  `runtime/tizen/dev/` for its architecture into `.nsh/tizen/` and signs it with `nsh.json` `tizen.signing` when the
  runtime changed, installs it with `sdb install` when the device does not have this build (`.nsh/tizen/installed`:
  a digest of the runtime per serial), then stops what an earlier session left, maps the TV's port 8790 to the dev
  server with `sdb reverse` and launches it (`sdb shell 0 execute <app id>` on a TV, `app_launcher -s` on the
  emulator). When `sdb reverse` fails it maps a free local port to the TV's 8791 with `sdb forward` and relays each
  connection there to the dev server (the library listens after three refusals of 8790). The run ends stopping the
  app and removing the mappings, Ctrl-C too. An app of Rust only (`"root"`) is packaged (signed) and installed with
  `nsh package --run`'s path.
- **`nsh test --target tizen`** (`test_tizen.rs`) installs the development TPK as `nsh run` does, then for each
  `open` serves the build in a test session, maps 8790 to it and launches the runtime, whose GPUI test agent takes
  the steps; one app at a time (the port is fixed). `open`'s platform there is Linux's, unsized (the app fills the
  screen).

## Apps with Rust crates

As Android's library, an app with `"rust"` crates gets its own `libnsh.so` (`rust_app.rs`,
`RustApp::tizen_workspace`, beside `gpui_workspace`): `.nsh/rust/tizen`, a workspace of its own whose member
`tizen-lib` is a `cdylib` of `nsh-platform-tizen` (`default-features = false`: without its `entry`) and the app's
crates whose one export is its own `nsh_tizen_main` (`nsh::set_ui_sets` with `nsh/base` and their sets, then
`nsh::main`, or `nsh::run_rust(argc, argv, app_root)` for a Rust app's root), and whose root manifest carries the
forks' `[patch.crates-io]` lines (git: the
nine `gpui-fast-*` crates from dXnSix/gpui-fast `tizen-release`, the wgpu crates from dXnSix/wgpu `v30-tizen`, the same
set as dXn.GpuiTizen's patch config; `rust_app::TIZEN_PATCHES`). It is built for the requested architectures with
the component's target specs (`runtime/tizen/targets`), not dXn.GpuiTizen's checkout: `tizen::cross_command` runs
the nightly the specs name (`targets/toolchain`) with `-Zbuild-std=std,panic_abort -Zjson-target-spec`, clang for C
with the rootstrap as sysroot, and clang driving the GCC toolchain's GNU ld (dXn.GpuiTizen's `cross::plan`). The
machine needs that nightly (`rust-src`, `llvm-tools`), clang, the Tizen 8.0 rootstrap (`TIZEN_ROOTSTRAP_<ARCH>`,
else the SDK's) and the SDK's GCC 9.2 toolchain (`TIZEN_GCC_TOOLCHAIN_<ARCH>`, else the SDK's `tools/`; the hints
give dXn.GpuiTizen's `docs/setup.md` command, `package-manager-cli.bin install --accept-license <package>`). Each
architecture's library then goes through dXn.GpuiTizen's symbol check, as the runtime's does
(`tizen::symbol_check`: `cargo xtask check-symbols <lib> --entry nsh_tizen_main` in its checkout, `DXN_GPUI_TIZEN`,
else `dXn.GpuiTizen` next to nsh's checkout; without one the build is refused before it starts), and is stripped with
the nightly's `llvm-strip --strip-unneeded` into `.nsh/rust/tizen-{release,dev}/<arch>/libnsh.so` (`tizen::strip_command`;
the target directory keeps the symbols). `nsh-platform-tizen` is not published (it ships built, as the `tizen`
component), so such apps build against a checkout of nsh: `RustApp::tizen` refuses crates that take nsh from
crates.io, naming `nsh init --template rust --nsh-source <dir>`.

## Limits

- Nothing here has run on a TV or the emulator yet: the `sdb` commands (`0 execute`, `0 kill`, `sdb reverse`'s
  form), where `tz pack` writes a TPK made from a staged tree, and the multi-architecture packaging assumptions
  (feature keys, four-part versions) are dXn.GpuiTizen's, unverified until the owner's run.
- Retail TVs have no `sdb shell`: `0 execute` and `0 kill` are the TV's launch and stop commands; logs reach the
  developer through the dev server only.
- The development TPK is one package for every app; a TV's install record is the app's, so after another app's run
  with another nsh, `nsh run` may skip an install it needs (delete `.nsh/tizen/installed`).
- `nsh test --target tizen` runs one app at a time and cannot give each its own data directory.
