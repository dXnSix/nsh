---
title: nsh/tizen
description: "三星电视具有的能力：电视的产品信息、遥控器按键、退出应用，以及 isAvailable。"
order: 12.4
---

# nsh/tizen

`nsh/tizen` 提供三星电视（Tizen）具有的能力：电视的[产品信息](#productinfo)、应用从电视取用的遥控器[按键](#registerkey-和-unregisterkey)、[退出应用](#exitapp)以及 Smart Hub 的[预览](#setpreviewdata)。这个模块在所有平台上都可以导入，因此一份 bundle 到处运行：`isAvailable()` 告诉你是否运行在三星电视上，在其他平台上其余调用以 `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` 拒绝。要在任意平台上为电视布局，请使用 `nsh` 的 `platform.tv` 和 `tv={{…}}` 属性。

```tsx
import { exitApp, isAvailable, productInfo, registerKey, setPreviewData, unregisterKey } from "nsh/tizen";
import type { ProductInfo } from "nsh/tizen";
```

## isAvailable

```ts
function isAvailable(): boolean;
```

在三星电视上运行的应用中为 `true`，无论是原生运行（`--target tizen`）还是作为 Tizen Web 应用（`--target tizen-web`，在电视浏览器引擎中运行的 `.wgt`）；其他地方为 `false`，包括浏览器中的 Web。

在 Tizen Web 应用中，这个模块调用电视的 Web API：`productInfo` 读取 `webapis.productinfo` 和 `tizen.systeminfo`，按键交给 `tizen.tvinputdevice`，`exitApp` 交给 `tizen.application`，`setPreviewData` 交给 `webapis.preview`。`nsh package --target tizen-web` 写出的包会申请它们所需的权限。

## productInfo

```ts
function productInfo(): Promise<ProductInfo>;

interface ProductInfo {
  model: string | null;           // 型号名称
  platformVersion: string | null; // Tizen 版本，例如 "8.0"
  build: string | null;           // 固件构建
  manufacturer: string | null;
}
```

电视报告的自身信息；电视没有报告的字段为 `null`。

## registerKey 和 unregisterKey

```ts
function registerKey(key: string): Promise<void>;
function unregisterKey(key: string): Promise<void>;
```

遥控器的颜色键、媒体键、频道键和 Info 键在应用取用之前属于电视。`key` 是 [`keymap`](./nsh.md#keymap) 中的按键名：`"red"`、`"green"`、`"yellow"`、`"blue"`、`"mediaplaypause"`、`"mediaplay"`、`"mediapause"`、`"mediastop"`、`"mediarewind"`、`"mediafastforward"`、`"medianext"`、`"mediaprevious"`、`"channelup"`、`"channeldown"`、`"info"`。方向键、确认键和返回键总是到达应用。不是按键名的值以 `TypeError` 拒绝，电视不交给应用的按键以 `RangeError` 拒绝。在原生应用中，颜色键、媒体键和频道键从一开始就属于应用；`unregisterKey` 把某个键还给电视，`registerKey("info")` 也会取用 Info 键。在 Tizen Web 应用中，这些键（包括 Info）从一开始都属于应用。在应用显示期间按键一直属于应用，电视隐藏和显示应用时也是如此。

```tsx
import { keymap } from "nsh";
import { isAvailable, registerKey } from "nsh/tizen";

if (isAvailable()) await registerKey("red");
keymap.bind("red", () => toggleSubtitles());
```

## exitApp

```ts
function exitApp(): Promise<void>;
```

立即退出应用。在应用的第一个界面上，返回键已经会先询问用户是否退出，这正是三星对电视应用的要求；你可以在自己的菜单项中调用 `exitApp`。

## setPreviewData

```ts
function setPreviewData(data: unknown): Promise<void>;
```

设置应用的 Smart Hub 预览，即电视在主屏幕上为应用显示的磁贴，格式为三星的预览格式。电视从 Web 应用获取预览：在 Tizen Web 应用中，`data` 以 JSON 形式交给 `webapis.preview.setPreviewData`，电视拒绝时 promise 以 `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` 拒绝。在原生应用中这个调用以 `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` 拒绝。
