---
title: Tizen
description: 在三星电视（Tizen 8.0 及以上）和 Tizen 模拟器上原生运行 Rust 和 TSX 编写的 NSH 应用，打包成签名的 TPK，并在电视上测试。
order: 16.2
group: platforms
---

# Tizen

NSH 应用可以原生运行在三星电视（2024 年起的机型，Tizen 8.0）和 Tizen 模拟器上：一个 TPK，由其中很小的 .NET 宿主启动
NSH 的运行时，以电视的分辨率用 GPU 绘制应用，并接收遥控器的按键。TSX 应用和带有自己 Rust crate 的应用都和在其他平台上一样运行。
在电视上，`os()` 为 `Os::Tizen`（TSX 中 `platform.os` 为 `"tizen"`），处于电视形态（TV idiom）。

三星电视有两个目标：

| | `--target tizen`（本页） | `--target tizen-web` |
|---|---|---|
| 运行方式 | 原生，NSH 的运行时 | 在电视的浏览器引擎中（[Web](./web.md#三星电视)） |
| `platform.os` | `"tizen"` | `"web"` |
| 包 | 签名的 `.tpk` | 签名的 `.wgt` |

两者读取 `nsh.json` 中同一个 `tizen` 部分，并用同一个证书配置签名。

## 准备

- **Tizen SDK** 的命令行工具：`sdb`（设备桥）和 `tz`（打包和签名），从 `PATH` 或 SDK 中查找（`TIZEN_SDK`，否则
  `~/tizen-studio`）。
- 三星证书管理器（VS Code 的 Tizen 扩展，或 `tz`）中的**证书配置**：一个作者证书，以及列出你电视 DUID 的分发者证书。电视和模拟器只安装签名的包；
  在 `nsh.json` 的 `tizen.signing` 中或用 `--sign` 指定配置名。
- **电视打开开发者模式**：在 Apps 中输入 `12345`，填入你电脑的地址并打开开发者模式，重启电视，然后执行
  `sdb connect <电视的地址>`。`sdb devices` 会列出它。
- Tizen 运行时（组件 `tizen`）会在第一次有命令需要时自动添加（[安装](./installation.md#组件)）。

## 运行应用

```sh
nsh run App.tsx --target tizen        # TSX 应用
nsh run --target tizen                # Rust 应用，在其目录中执行
```

`nsh run` 会在电视上安装 NSH 的开发包（每台电视一次，运行时变化时再装一次），通过 `sdb`（`sdb reverse`，不行时用
`sdb forward`）把它连到开发服务器并启动：你编辑时应用会热更新，构建错误和运行时错误显示在应用上方，日志输出到你的终端。
连接了多台设备时，用 `--device <serial>` 选择一台（`sdb devices` 列出序列号）。Ctrl-C 会停止电视上的应用。

## 打包

```sh
nsh package App.tsx --target tizen                              # .nsh/out/<Name>.tpk，用于 32 位 ARM 电视
nsh package App.tsx --target tizen --arch armv7l,aarch64        # 每个架构一个包，外加商店上传文件
nsh package App.tsx --target tizen --sign my-profile --run      # 用另一个配置签名，安装并启动
```

包中有宿主、对应架构的 NSH 运行时、应用的 bundle 和资源、图标（512×423，三星要求的尺寸），以及由 `nsh.json` 生成的
`tizen-manifest.xml`。`nsh package` 用 Tizen SDK 的 `tz pack`（`-t tpk -s <profile>`，对包的 zip 重新签名）为它签名；没有证书配置时会停下并说明如何创建。电视和电视模拟器用 SDK 的 `tz install` 安装它，`--run` 和 `nsh run` 都这样做。

- **`--arch`** 接受 `armv7l`（32 位 ARM，电视；默认）、`aarch64` 和 `x86_64`（模拟器）。指定多个时，每个包的版本号会加上第四位
  （armv7l `.1`、aarch64 `.2`、x86_64 `.3`）以及其电视匹配的 CPU 特性，这是三星 Seller Office 对多架构上传的要求；电视的包（armv7l、aarch64）
  还会放进 `<package>-<version>.zip`，即上传文件本身。模拟器的包（x86_64）不放进上传文件，单独成包。
- **`--run`** 把与电视架构对应的包安装到已连接的电视上并启动。

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

每个键都有默认值。`package` 是包 ID，恰好 10 个字母和数字（默认由应用的 `id` 生成）；`appId` 是 `<package>.<name>`（默认取应用名中的字母和数字）；
`version` 是 Tizen 的 `x.y.z`（默认为应用的 `version`，否则 `1.0.0`）；`apiVersion` 是能安装该包的最低 Tizen 版本（`8.0`）；
`privileges` 是在网络权限之外申请的权限；`signing` 是证书配置。Tizen 不接受的值（长度不对的包 ID、属于另一个包的 `appId`、带字母的版本号）
会让命令停下并说明哪里不对。

## 测试

```sh
nsh test --target tizen                              # 在 sdb 列出的电视（或模拟器）上运行应用的测试
nsh test --target tizen --device 192.168.1.5:26101   # 有多台设备时选其中一台
```

测试在电视上用 NSH 的开发包打开应用（安装和连接方式与 `nsh run` 相同），测试步骤通过开发协议作用于应用。应用占满电视屏幕：在这里
`open` 的 `width` 和 `height` 会被拒绝。应用一次打开一个。

## 带 Rust crate 的应用

带有自己 crate 的应用（`nsh.json` 中的 `"rust"`）会得到自己的运行时库，用 `tizen` 组件附带的 Tizen 目标规格为每个架构构建。
除了 Tizen SDK，这还需要：

- 组件指定的 nightly Rust 工具链，带 `rust-src` 和 `llvm-tools`（`nsh` 会说明是哪一个）；
- clang；
- Tizen 8.0 rootstrap
  （`~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license TIZEN-8.0-NativeAppDevelopment-CLI`；
  或设置 `TIZEN_ROOTSTRAP_ARMV7`、`TIZEN_ROOTSTRAP_AARCH64`、`TIZEN_ROOTSTRAP_X86_64`）；
- SDK 的 GCC 9.2 工具链
  （`~/tizen-studio/package-manager/package-manager-cli.bin install --accept-license NativeToolchain-Gcc-9.2`；
  或设置 `TIZEN_GCC_TOOLCHAIN_<ARCH>`）。

`nsh` 像检查自己的运行时一样检查它构建的库：只能导出 `nsh_tizen_main`，只能导入 Tizen 有文档的 API（`tizen` 组件附带的列表）、glibc 和
`libgcc_s`；否则构建会停下并列出这些符号。

NSH 的 Tizen crate 不发布到 crates.io（运行时以构建好的 `tizen` 组件提供），所以这类应用需要基于 NSH 的源码检出构建
（`nsh init --template rust --nsh-source <dir>`）；应用的 crate 从 crates.io 获取 NSH 时，`--target tizen` 会停下并说明原因。

## 应用看到什么

- `platform.os` 为 `"tizen"`（`Os::Tizen`），形态为 `"tv"`（`platform.tv` 为 `true`），`platform.desktop` 为 `false`；
  `platform.select({ tizen })` 在 `tv` 之前选取值，`tv={{…}}` 属性生效（[平台相关代码](./platform-specific.md#电视)）。
- 遥控器：方向键移动焦点，Enter 按下，Back 在应用中返回（关闭最新的对话框、弹出最内层的栈，`onBackPress` 生效）；在第一个屏幕上没有可返回的内容时，
  应用会按三星的规则询问是否离开。颜色键、媒体键、频道键和 Info 以它们的 [`keymap`](../modules/nsh.md#媒体键与电视遥控器按键) 名称到达应用。
- [`nsh/tizen`](../modules/nsh-tizen.md)：电视的型号和版本，以及按名称注册的遥控器按键。
- 电视的语言、时区、文字大小和减少动态效果设置与在其他平台上一样传给应用；电视的外观为深色。切换到其他应用时应用进入后台（`AppState`），回来时重新变为活跃。

## 限制

- 零售电视没有 shell 和系统日志：应用的日志只能通过 `nsh run` 看到。
- Tizen 模拟器只能在 x86-64 电脑上运行，不支持 Apple 芯片。
- 运行和测试需要证书配置，在模拟器上也一样。
