---
title: Tizen
description: Run Rust and TSX NSH apps natively on Samsung TVs (Tizen 8.0 and later) and the Tizen emulator, package them as signed TPKs, and test them on a TV.
order: 16.2
group: platforms
---

# Tizen

NSH apps run natively on Samsung TVs, the TVs of 2024 on (Tizen 8.0), and on the Tizen emulator: a TPK whose small
.NET host starts NSH's runtime, which draws the app with GPU rendering at the TV's resolution and takes the remote's
keys. TSX apps and apps with Rust crates of their own run as on the other platforms. On a TV, `os()` is `Os::Tizen`
(`platform.os` is `"tizen"` in TSX) in the TV idiom.

There are two Samsung TV targets:

| | `--target tizen` (this page) | `--target tizen-web` |
|---|---|---|
| How it runs | natively, NSH's runtime | in the TV's browser engine ([Web](./web.md#samsung-tvs)) |
| `platform.os` | `"tizen"` | `"web"` |
| Package | a signed `.tpk` | a signed `.wgt` |

Both read the same `nsh.json` `tizen` section and sign with the same certificate profile.

## Requirements

- **The Tizen SDK**'s command line tools: `sdb` (the device bridge) and `tz` (packaging and signing), found on `PATH` or
  in the SDK (`TIZEN_SDK`, else `~/tizen-studio`).
- **A certificate profile** from Samsung's Certificate Manager (the Tizen extension for VS Code, or `tz`): an author
  certificate and a distributor certificate that lists your TV's DUID. TVs and the emulator install signed packages
  only; name the profile in `nsh.json` `tizen.signing` or with `--sign`.
- **The TV in Developer mode**: in Apps, type `12345`, turn Developer mode on with your computer's address, restart the
  TV, then `sdb connect <the TV's address>`. `sdb devices` lists it.
- The Tizen runtime, the `tizen` component, is added the first time a command needs it ([Installation](./installation.md#components)).

## Running an app

```sh
nsh run App.tsx --target tizen        # a TSX app
nsh run --target tizen                # a Rust app, from its directory
```

`nsh run` installs NSH's development package on the TV (once per TV, and again when the runtime changes), connects it
to the dev server through `sdb` (`sdb reverse`, else `sdb forward`) and launches it: the app runs with hot updates as
you edit, build and runtime errors show over it, and its logs come to your terminal. With several devices connected,
pick one with `--device <serial>` (`sdb devices` lists the serials). Ctrl-C stops the app on the TV.

## Packaging

```sh
nsh package App.tsx --target tizen                              # .nsh/out/<Name>.tpk, for 32-bit ARM TVs
nsh package App.tsx --target tizen --arch armv7l,aarch64        # one package per architecture and the store upload
nsh package App.tsx --target tizen --sign my-profile --run      # signed with another profile, installed and launched
```

A package holds the host, NSH's runtime for the architecture, the app's bundle and assets, its icon (512×423, as Samsung
asks) and its `tizen-manifest.xml`, made from `nsh.json`. `nsh package` signs it by running the Tizen SDK's
`tz pack -t tpk -s <profile>`; without a profile it stops and says how to make one.

- **`--arch`** takes `armv7l` (32-bit ARM, the TVs; the default), `aarch64` and `x86_64` (the emulator). With several,
  each package's version gets a fourth number (armv7l `.1`, aarch64 `.2`, x86_64 `.3`) and the CPU feature its TVs
  match, as Samsung's Seller Office asks of a multi-architecture upload, and the packages also go into
  `<package>-<version>.zip`, the upload itself.
- **`--run`** installs the package of the TV's architecture on the connected TV and launches it.

## `nsh.json`

```json
"tizen": {
  "package": "AbCdE12345",
  "appId": "AbCdE12345.Counter",
  "version": "1.2.0",
  "apiVersion": "8.0",
  "privileges": ["http://developer.samsung.com/privilege/productinfo"],
  "signing": "my-profile"
}
```

Every key has a default. `package` is the package id, exactly 10 letters and digits (by default made from the app's
`id`); `appId` is `<package>.<name>` (by default the app's name, letters and digits); `version` is Tizen's `x.y.z` (by
default the app's `version`, else `1.0.0`); `apiVersion` is the lowest Tizen that installs the package (`8.0`);
`privileges` are asked for besides the internet's; `signing` is the certificate profile. Values Tizen refuses (a
package id of another length, an `appId` of another package, a version with letters) stop the command with what is
wrong.

## Testing

```sh
nsh test --target tizen               # the app's tests on the TV (or the emulator) sdb lists
```

The tests open the app on the TV in NSH's development package, installed as `nsh run` installs it, and their steps act
on it over the dev protocol. The app fills the TV's screen: `open`'s `width` and `height` are refused there. Apps open
one at a time.

## Apps with Rust crates

An app with crates of its own (`nsh.json` `"rust"`) gets its own runtime library, built for each architecture with the
Tizen target specs the `tizen` component ships. That needs, besides the Tizen SDK:

- the nightly Rust toolchain the component names, with `rust-src` and `llvm-tools` (`nsh` says which);
- clang;
- the Tizen 8.0 rootstrap
  (`~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license TIZEN-8.0-NativeAppDevelopment-CLI`;
  or set `TIZEN_ROOTSTRAP_ARMV7`, `TIZEN_ROOTSTRAP_AARCH64`, `TIZEN_ROOTSTRAP_X86_64`);
- the SDK's GCC 9.2 toolchain
  (`~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license NativeToolchain-Gcc-9.2`; or set
  `TIZEN_GCC_TOOLCHAIN_<ARCH>`);
- a checkout of [dXn.GpuiTizen](https://github.com/dXnSix/dXn.GpuiTizen), whose symbol check every TV library passes
  (it exports `nsh_tizen_main` alone and calls only Tizen's documented APIs): `DXN_GPUI_TIZEN`, else
  `dXn.GpuiTizen` next to the checkout of NSH.

NSH's Tizen crate is not published on crates.io (the runtime ships built, as the `tizen` component), so such apps build
against a checkout of NSH (`nsh init --template rust --nsh-source <dir>`); with crates that take NSH from crates.io,
`--target tizen` stops and says so.

## What the app sees

- `platform.os` is `"tizen"` (`Os::Tizen`), the idiom `"tv"` (`platform.tv` is `true`), `platform.desktop` is
  `false`; `platform.select({ tizen })` picks a value before `tv`, and `tv={{…}}` props apply
  ([Platform-specific code](./platform-specific.md#tvs)).
- The remote: the arrows move the focus, Enter presses, Back goes back in the app (the newest dialog closes, the
  innermost stack pops, `onBackPress` works); on the first screen, with nothing to go back to, the app asks whether to
  leave, as Samsung's rules ask. The color, media and channel keys and Info reach the app as their
  [`keymap`](../modules/nsh.md#media-and-tv-remote-keys) names.
- [`nsh/tizen`](../modules/nsh-tizen.md): the TV's model and version, and the remote's keys registered by name.
- The TV's language, time zone, text size and reduce-motion setting reach the app as on every platform; the TV's
  appearance is dark. Leaving the app for another one moves it to the background (`AppState`), coming back makes it
  active again.

## Limits

- Retail TVs have no shell and no system log: the app's logs reach you through `nsh run` only.
- The Tizen emulator runs on x86-64 computers only, not on Apple silicon.
- Running and testing need a certificate profile, also on the emulator.
