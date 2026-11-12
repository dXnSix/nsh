---
title: FocusGroup
description: "FocusGroup: a region the arrow keys treat as one, as a TV's remote moves through shelves and menus: the focus enters it at its destinations, and with trap the arrows never leave it. It has no look of its own."
group: base
demo: el-focus-group
---

# FocusGroup

On a TV, in the GPUI backend and in browsers, the arrow keys move the keyboard focus to the nearest stop in their
direction. A `FocusGroup` is a region they treat as one: a shelf of tiles, a row of tabs, a side menu. The focus enters a group at
its `destinations`, in order (the tile focused last in a shelf, the selected tab), rather than at the stop nearest the
focus; without one in reach, at the group's `preferredFocus` stop, else the nearest. With `trap`, the arrows never take
the focus out of the group: an open side menu, a dialog drawn in the page. Inside a group the arrows go to the nearest
stop as usual. Groups nest, and a trap holds the groups inside it. The group has no look and lays out nothing of its
own: it is a view the app styles around its children.

```rust tabs
use nsh_base::focus_group;
use nsh_core::{element::FocusTarget, prelude::*};

fn shelf(items: &'static [&'static str]) -> impl IntoElement {
    // The tile the shelf is entered at: the one focused last.
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
  // The tile the shelf is entered at: the one focused last.
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

`FocusGroup` comes from [`nsh/base`](../modules/nsh-base.md), the `nsh-base` crate in Rust. In Rust, `focus_group()`
makes it, with a setter per prop (`.destinations(..)`, `.trap(..)`), the typed style methods and `.child(..)` for its
content. The views inside it keep their own focus props: `autoFocus`, `preferredFocus` and the `nextFocus*` routes
([Arrow keys and TVs](./index.md#arrow-keys-and-tvs)); a route may leave a group that traps.

## Props

| Prop | Rust | Type | Default | Description |
|---|---|---|---|---|
| `destinations` | `.destinations(..)` | `(ViewRef \| null \| undefined)[]` | `[]` | The views the arrows enter the group at, in order (an entry not set yet names nothing): the first that is, or holds, a stop the focus can reach takes it. They are found in every commit, so they may be the group's own children; a Rust app passes `FocusTarget::from(&element_ref)`. |
| `trap` | `.trap(..)` | `boolean` | `false` | The arrows never take the focus out of the group: an arrow with no stop that way inside it goes nowhere. |
| `children` | `.child(..)` | `JSX.Element` | none | The group's stops, and whatever the app lays out among them. |
| `class`, `style` | style methods | `string`, `StyleProp<ViewStyle>` | none | The group's layout and look. |

## Platforms

The group's rules apply where the arrow keys move the focus between views: the GPUI backend, the web (browsers, and
a TV's web app) and TVs. On the other platforms it is a plain view, and its stops are reached as everywhere else (Tab
on the desktops).
