---
title: nsh/tizen
description: "What Samsung TVs have: the TV's product information, the remote's keys, leaving the app, and isAvailable."
order: 12.4
---

# nsh/tizen

`nsh/tizen` holds what Samsung TVs (Tizen) have: the TV's [product information](#productinfo), the remote's
[keys](#registerkey-and-unregisterkey) the app takes from the TV, [leaving the app](#exitapp) and Smart Hub's
[preview](#setpreviewdata). The module can be imported on every platform, so one bundle runs everywhere:
`isAvailable()` tells you whether you are on a Samsung TV, and elsewhere the other calls reject with
`ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`. To lay out for a TV on any platform, use `platform.tv` and the `tv={{…}}`
props from `nsh`.

```tsx
import { exitApp, isAvailable, productInfo, registerKey, setPreviewData, unregisterKey } from "nsh/tizen";
import type { ProductInfo } from "nsh/tizen";
```

## isAvailable

```ts
function isAvailable(): boolean;
```

`true` in an app running natively on a Samsung TV; `false` elsewhere.

## productInfo

```ts
function productInfo(): Promise<ProductInfo>;

interface ProductInfo {
  model: string | null;           // the model's name
  platformVersion: string | null; // the Tizen version, such as "8.0"
  build: string | null;           // the firmware's build
  manufacturer: string | null;
}
```

What the TV says about itself; a field the TV does not report is `null`.

## registerKey and unregisterKey

```ts
function registerKey(key: string): Promise<void>;
function unregisterKey(key: string): Promise<void>;
```

The remote's color, media and channel keys and Info belong to the TV until an app takes them. `key` is the key's name
in [`keymap`](./nsh.md#keymap): `"red"`, `"green"`, `"yellow"`, `"blue"`, `"mediaplaypause"`, `"mediaplay"`,
`"mediapause"`, `"mediastop"`, `"mediarewind"`, `"mediafastforward"`, `"medianext"`, `"mediaprevious"`,
`"channelup"`, `"channeldown"`, `"info"`. The arrows, Enter and Back always reach the app. A name of no key rejects
with a `TypeError`, a key the TV does not give apps with a `RangeError`. In a native app the color, media and channel
keys are the app's from the start; `unregisterKey` gives one back to the TV, and `registerKey("info")` takes the Info
key too. A key stays the app's while the app is shown, as the TV hides and shows it.

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

Leaves the app. On its first screen, Back already asks the user whether to leave, as Samsung's guidelines ask of TV
apps; call `exitApp` from a menu item of your own.

## setPreviewData

```ts
function setPreviewData(data: unknown): Promise<void>;
```

Sets the app's Smart Hub preview, the tiles the TV shows for the app on its home screen, in Samsung's preview format.
A TV takes the preview from web apps; in a native app the call rejects with
`ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`.
