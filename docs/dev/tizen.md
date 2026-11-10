# Tizen (Samsung TVs)

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
| `lib.rs` | `nsh_tizen_main`, and for the hosts of apps with Rust crates of their own `main`, `run_rust` and `set_ui_sets` |
| `tizen.rs` (Tizen only) | `dxn_gpui_tizen::run` with the package's bundle (or the development runtime), the app framework's data and cache directories (`app_get_data_path`, `app_get_cache_path`, each with `nsh/` in it), with `dev` the system's log (dlog, tag `NSH`) |
| `app.rs` | The full-screen window holding `AppView`, and the exit question over it; the TV's settings and the app's state given to `AppView` before the app loads (its engine starts with them), the settings again as they change; Exit quits GPUI, which ends `ui_app_main` |
| `settings.rs` | `dxn_gpui_tizen::settings` (locale, time zone, font scale, screen reader, reduce motion) as the values of `AppView`'s setters, and the setters to call again when they change; the appearance is dark (Tizen 8.0 reports no light or dark setting) |
| `lifecycle.rs` | The platform's window activation (off while the app framework has the app paused, which the platform reports to GPUI with the window hidden) as `Lifecycle::Background` / `Active`, through `AppView::set_lifecycle` |
| `back.rs` | Samsung's rule for Back: NSH's back navigation first (a modal, a stack, `onBackPress`); when nothing takes it (`AppView::on_unhandled_back`), the app asks whether to leave, staying chosen |
| `prompt.rs` | How that question reads: its words in the TV's language (English, Chinese in its script, Korean, Japanese, Spanish, French, German, Portuguese, Russian; English for the others), its sizes at the TV's font scale, and for assistive technologies an alert dialog named by the question with its answers as buttons, the chosen one the focused node, so Voice Guide reads them |
| `keys.rs` | The remote's key names (dXn.GpuiTizen's `keys::NAMED_KEYS`) as NSH reads them: NSH keys (`nsh_platform_gpui::nsh_key`, `TV-1`'s `ColorRed`, `MediaPlayPause`, `Info`, ...), `back` (navigation), `exit` (the TV's) |
| `package.rs` | The app in the TPK's resources: `res/nsh/bundle.nshc` (with `capabilities.json` and `bundle.js.map`), `res/nsh/assets/`, `res/nsh/app.json` (name, id, appearance), as Linux's packages hold them |
| `dev.rs` (feature `dev`) | The development runtime, below |

The logic modules build on the desktops too, and their unit tests run there:

```sh
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --lib                 # app, keys, settings, back, prompt, lifecycle, package, host
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --features dev --lib  # and dev's connection policy
```

## The C# host

Tizen's app model starts a third-party native app as a .NET app: `crates/gpui/nsh-platform-tizen/host/` is the
program (`Program.cs`, one P/Invoke into `libnsh.so`'s `nsh_tizen_main`; no TizenFX) and its project on
`Tizen.NET.Sdk` (`Nsh.Tizen.Host.csproj`), the same as dXn.GpuiTizen's own host. It is built once into the runtime
files (`dotnet build -c Release`), so packaging an app needs no .NET SDK. Without a .NET SDK that has Tizen's
workload, the crate's `host::tests` check what MSBuild would refuse first: the project must be well-formed XML (no
`--` in its comments: MSB4025), on `Tizen.NET.Sdk`, building the `Nsh.Tizen.Host` executable the runtime files ship,
and the program must keep its one P/Invoke (`$PATCH` and `$LOCK` as in [Building](#building)):

```sh
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --lib host::tests \
  --config "$PATCH" --config "resolver.lockfile-path='$LOCK'"
```

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
HTTP at the reverse-mapped port. Logs go to the dev server while connected and to dlog otherwise.

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

## Not yet on Tizen

- Packaging (`nsh package --target tizen`), `nsh run --target tizen` and the `tizen` component come with the CLI's
  Tizen target.
