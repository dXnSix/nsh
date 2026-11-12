# nsh-platform-tizen

Native Shell on Samsung TVs (Tizen 8.0 and later): `libnsh.so`, the library a TPK's C# host calls with one
P/Invoke, `nsh_tizen_main(argc, argv)`. It runs NSH's GPUI backend (`nsh-platform-gpui`'s `AppView`) full screen on
[dXn.GpuiTizen](https://github.com/dXnSix/dXn.GpuiTizen)'s Tizen platform for GPUI, gives `AppView` the TV's
settings and the app's state as they change, makes the remote's Back NSH's back navigation and asks before leaving
the app from its first screen. Built with `--features dev`, it is the development runtime: the dev server's app with
hot updates, connected through `sdb reverse` or, failing that, `sdb forward`.

`host/` is the C# host (`Program.cs`, `Nsh.Tizen.Host.csproj`).

## Building

dXn.GpuiTizen is a git dependency pinned by `rev`; TV builds take gpui-fast and wgpu from their forks through a
local Cargo config (`[patch.crates-io]`), never committed. `docs/dev/tizen.md` has the details; with the
environment there (Git's `insteadOf` for the local checkouts while they are unpublished):

```sh
PATCH=<dXn.GpuiTizen>/artifacts-temp/gpui-fast-patch.toml
LOCK=<dXn.GpuiTizen>/artifacts-temp/locks/nsh-tizen/Cargo.lock
# Unit tests, on the desktop:
cargo test --manifest-path crates/gpui/Cargo.toml -p nsh-platform-tizen --features dev --lib \
  --config "$PATCH" --config "resolver.lockfile-path='$LOCK'"
# The TV libraries (release and dev), from dXn.GpuiTizen, with the symbol check:
cargo xtask cross --target armv7 --release --manifest-path <nsh>/crates/gpui/Cargo.toml -p nsh-platform-tizen \
  --with-dev-variant --check-symbols --entry nsh_tizen_main --config "$PATCH"
```

`--target` is `armv7` (32-bit ARM TVs), `x86_64` (the emulator) or `aarch64`.
