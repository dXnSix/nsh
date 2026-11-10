---
title: FocusGroup
description: FocusGroup：方向键当作一个整体对待的区域，就像电视遥控器在一排排内容和菜单之间移动那样：焦点从它的 destinations 进入，设置 trap 后方向键不会让焦点离开它。它没有自己的外观。
group: base
demo: el-focus-group
---

# FocusGroup

在电视上以及 GPUI 后端中，方向键把键盘焦点移到该方向上最近的停靠点。`FocusGroup` 是方向键当作一个整体对待的区域：一排卡片、一行标签、一个侧边菜单。焦点按顺序从它的 `destinations` 进入分组（一排卡片中上次获得焦点的那张、选中的标签），而不是离焦点最近的停靠点；其中没有焦点能到达的视图时，从分组中设置了 `preferredFocus` 的停靠点进入，否则从最近的停靠点进入。设置 `trap` 后，方向键永远不会让焦点离开分组：打开的侧边菜单、画在页面中的对话框。在分组内部，方向键照常去往最近的停靠点。分组可以嵌套，trap 会把内部的分组一起困住。分组没有自己的外观，也不做自己的布局：它是一个由应用设置样式、包住子元素的视图。

```rust tabs
use nsh_base::focus_group;
use nsh_core::{element::FocusTarget, prelude::*};

fn shelf(items: &'static [&'static str]) -> impl IntoElement {
    // 进入这一排时停在哪张卡片：上次获得焦点的那张。
    let refs: Vec<ElementRef> = items.iter().map(|_| element_ref()).collect();
    let current = signal(0usize);
    let targets = refs.clone();
    focus_group()
        .destinations(move || vec![FocusTarget::from(&targets[current.get()])])
        .flex_row()
        .gap_3()
        .children(items.iter().zip(refs).enumerate().map(|(i, (item, r))| {
            view()
                .element_ref(&r)
                .on_press(move || play(item))
                .on_focus(move || current.set(i))
                .focus(|s| s.border_2())
                .child(text(*item))
        }))
}
```

```tsx tabs
import { mount, signal, Text, View, type ViewRef } from "nsh";
import { FocusGroup } from "nsh/base";

function Shelf(props: { items: string[] }) {
  // 进入这一排时停在哪张卡片：上次获得焦点的那张。
  const current = signal<ViewRef>();
  return (
    <FocusGroup destinations={[current()]} class="flex-row gap-3">
      {props.items.map((item) => {
        let tile: ViewRef | undefined;
        return (
          <View ref={tile} onPress={() => play(item)} onFocus={() => current.set(tile)} class="focus:border-2">
            <Text>{item}</Text>
          </View>
        );
      })}
    </FocusGroup>
  );
}

function App() {
  return (
    <View class="gap-6 p-6">
      <Shelf items={["Tides", "Northern Lights", "The Orchard"]} />
      <Shelf items={["Harbor", "Field Notes", "Small Hours"]} />
    </View>
  );
}

mount(App);
```

`FocusGroup` 来自 [`nsh/base`](../modules/nsh-base.md)，在 Rust 中是 `nsh-base` crate。在 Rust 中用 `focus_group()` 创建，每个属性有一个 setter（`.destinations(..)`、`.trap(..)`），另有有类型的样式方法和用于内容的 `.child(..)`。分组里的视图保留自己的焦点属性：`autoFocus`、`preferredFocus` 和 `nextFocus*` 路线（[方向键与电视](./index.md#方向键与电视)）；路线可以离开设置了 trap 的分组。

## 属性

| 属性 | Rust | 类型 | 默认值 | 说明 |
|---|---|---|---|---|
| `destinations` | `.destinations(..)` | `ViewRef[]` | `[]` | 方向键进入分组时停在哪些视图，按顺序：第一个本身是、或包含焦点能到达的停靠点的视图获得焦点。它们在每次提交时重新查找，所以可以是分组自己的子元素；Rust 应用传入 `FocusTarget::from(&element_ref)`。 |
| `trap` | `.trap(..)` | `boolean` | `false` | 方向键永远不会让焦点离开分组：分组内该方向上没有停靠点时，方向键不移动焦点。 |
| `children` | `.child(..)` | `JSX.Element` | 无 | 分组的停靠点，以及应用放在它们之间的其他内容。 |
| `class`、`style` | 样式方法 | `string`、`StyleProp<ViewStyle>` | 无 | 分组的布局和外观。 |

## 平台

分组的规则在方向键于视图之间移动焦点的地方生效：GPUI 后端和电视。在其他平台上它是一个普通视图，其中的停靠点和别处一样被到达（桌面和 Web 上用 Tab）。
