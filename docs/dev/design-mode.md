# Design mode

A design tool shows an app's components live on its own canvas: it starts the app as a
**design host**, which opens a window per thing the tool shows, keeps those windows out of sight, and
lets the tool draw them in its own window and drive them. The app's code runs in the app's process,
never in the tool's. Dev hosts only (each platform's `dev` feature); fork-only, as the dev tool hooks
are (`nsh-root/fork-changes/design-mode.md`).

## Starting one

The tool listens on a socket and starts the app with `NSH_DESIGN_SOCKET` set to it: a Unix socket's
path, or `tcp:<host>:<port>` (any platform, the only form on Windows). Usually from an entry of the
tool's own kept outside the app (`nsh run <tool's entry> --app <app>`), which imports the app's
modules (`import.meta.glob`) and opens their windows (`Window.open`).

## The connection

`nsh_core::runtime::design` keeps it, the same on every platform. One JSON object a line both ways:

| From | Message | What |
|---|---|---|
| host | `{"hello":{}}` | Once connected. |
| host | `{"surface":{"id","width","height", <handle>}}` | A window of the app's own (named by `Window.open`'s `id`), whenever it is new or its handle or size changed. The handle is what the tool draws it with: `"context"`, its layer's `CAContext` id, on macOS (`CALayerHost`); `"hwnd"` on Windows (a DWM thumbnail); `"xid"` under X11 (Composite). |
| host | `{"surface":{"id","gone":true}}` | The window closed. |
| tool | `{"pointer":{"id","kind":"down\|drag\|up\|move","x","y"}}` | Input, delivered by the platform to window `id`, in points from its content's top left. |
| tool | `{"wheel":{"id","x","y","dx","dy"}}`, `{"key":{"id","down","key",…}}`, `{"resize":{"id","width","height"}}` | The same; a key as the DOM names it. |
| tool | anything else | The app's: `design.next()`. |
| app | `design.send(object)` | To the tool. |

The connection's thread connects (the tool may start listening later), routes input to the platform
(`DesignHooks::input`), queues the rest for `next()`, and calls `DesignHooks::closed` when the tool
closes it: the platform stops the app (on macOS with the dev session's process group).

## The platform's part

Each platform's `design.rs` connects at start (before any window opens) and on a timer lists the
app's own windows for `report`, which tells the tool what changed:

- **macOS**: a window opened in design mode gets its `id` as its AppKit identifier. The main window
  stays hidden; the others are made unseen (transparent, click-through, above other windows, on every
  Space, out of the window cycle) so that their display links still tick. No Dock icon, no menu bar,
  no App Nap. Input is given to the view under the point straight (AppKit drops events routed to a
  window that ignores the mouse), through a run-loop source in the common modes so that a control's
  own mouse loop still gets the drags and the release. `NSH_DESIGN_ALPHA` shows the windows.
- **Windows**: the handle is the window's `HWND` (`"hwnd"`), which a tool draws with a DWM thumbnail
  (`DwmRegisterThumbnail`). The main window stays hidden; the app's own windows are moved off every
  display and out of the taskbar and switcher (tool windows that never activate), where DWM still
  composes them. Input goes through `crate::input` in the window's root (`app_window::enter`), as the
  test agent's does; the connection's thread hands it over with `UiThread::invoke`. When the tool
  closes the connection the host exits.
- **Linux**: under X11 the handle is the window's X11 id (`"xid"`), which a tool draws with the
  Composite extension; Wayland lets no other process draw a window, so nothing is reported there
  (logged once). The main window stays hidden; the app's own windows are moved off every display
  (`XMoveWindow`) and out of the taskbar and pager, still mapped so the compositor keeps their
  pictures. GDK's X11 functions and libX11's are looked up at run time, as `xtest.rs` does. Pointer
  and wheel input go through `crate::input::simulate` in the window's root; keys type text, Enter
  and Backspace into the focused widget (`crate::synth`).

## `nsh/design`

```ts
import { design } from "nsh/design";
design.active();                       // started by a tool
const message = await design.next();   // the tool's next message
design.send({ called: { id: "a" } });
const layers = await design.layers("board:", ".tool-module"); // dev builds
const detail = await design.detail(layerId);
design.setOverrides({ "src/Card.tsx:12:5": { class: "gap-4", replace: false, states: 0 } });
```

A Rust component asks the same with `design_mode()` (`nsh_core::prelude`): true in an app a design
tool started, once the platform's design host has connected (`design::connect`, before the app
starts), false in every other app. It never changes while the app runs, so it is a plain read:
nothing subscribes, it works outside the reactive runtime too, and an app no tool started pays one
load for it. A view uses it for what only someone designing with the component should see:

```rust
#[component]
fn Section(props: SectionProps) -> impl IntoElement {
    view()
        .child(props.children)
        // An empty section is a place to drop content in a design tool, nothing in the app.
        .child(when(move || design_mode() && props.children.get().is_empty(), || text("Empty")))
}
```

The variable alone does not set it: a host without design mode (a release host, a platform with no
design host) leaves `design_mode()` and `design.active()` false.

`layers(prefix, hide)` reads, with the dev tool hooks (`inspect`), the live tree under each view whose
test id starts with `prefix`, keyed by the rest of the test id, with each component instance between
the nodes it rendered and their parent (ids from `1 << 32`), leaving out instances whose key or call
site contains `hide`; only roots that changed since the last call, all after `setOverrides`, `null`
when nothing changed: `{ roots: { <root>: { nodes } }, gone }`. A layer is one of:

| Layer | Shape |
|---|---|
| A node | `{ id, parent, kind, name, class, testId, text, x, y, width, height, source, hidden, overridden, slot }`. `kind` is the node's (`view`, `text`, `list`, `cell`, …), `name` its title (`View`); `source` is `{ module, line, column }`, where it was written. |
| A component written in TSX | `{ id, parent, kind: "component", name, key, source }`: an instance of a hot component, no node. `source` is where it was used. |
| A view hosting a Rust component | A node with `kind: "host"` and the component's JS export as `name` (`Button`). `source` is where the app wrote it. |
| A registered native view (`<native>`) | A node with `kind: "native"` and its registered name. |

### Rust components

A Rust component (`#[component]`) renders on a host view that TSX makes; its view's nodes are the
core's. A layer tree is what the app wrote, so:

- The host is the component (`kind: "host"`). What its view draws inside the host is not listed,
  the components that view nests neither.
- What the app gave it is listed under the host, with its subtree, in the order the view placed it
  and wherever the view (or a component it nests) placed it:
  - its JSX children, as the layers they are;
  - for each prop the app fills with content, the node that holds it, whose `slot` is the prop's JS
    name: a named slot's holder (a view), the list of a rows prop (`Items`), what a render function
    rendered. The holder is the glue's, so it has no `source`; the app's own nodes are under it.
    Every other layer has `slot: null`.
- A row a component's list wraps (a `cell` under that list) lists the app's row only, not the
  wrapper the component draws around it.
- A host in a slot or among the children is the app's, a layer like any other.

```tsx
<Card header={<Text>Head</Text>} footer={<Button title="OK" />}>
  <Text>Body</Text>
</Card>
```

```text
host Card
  view View      slot: "header"
    text Text    "Head"
  text Text      "Body"
  view View      slot: "footer"
    host Button
```

The core's tree says the same (`dev_tree`): a host has `flags.host` and its component's export as
`component`, and a node of the script's inside a component's view has `given: { host, slot }`, the
nearest host above it that the script made. The core's nodes have odd ids and were written nowhere:
`dev_source` and `dev_instances` answer nothing for them.

`detail(id)` is a layer in full. For a node: `{ style, width, height, padding, border, margin,
states, events, role, label, source, component }`; for a component instance: `{ props: [{ name,
value }] }`. A host's `component` is what its Rust component takes, from its spec, and what the app
passes it (`null` for any other node):

```ts
component: {
  name: string;                       // the JS export
  props: {
    name: string;                     // its JS name
    kind: "value" | "renderer" | "slot" | "children" | "items";
    type: string | null;              // TypeScript
    optional: boolean;
    value: PropValue | null;          // what the app passes; null: left out
  }[];
  events: { name: string; type: string | null; value: PropValue | null }[];  // name: "onPress"
}
// As an instance's props: read from the props object, a getter never called.
type PropValue =
  | { kind: "string" | "number" | "boolean"; value: string | number | boolean }
  | { kind: "getter" | "function" | "object" | "null" | "undefined" };
```

- `value` is read from the props object the app's JSX passes, without running anything: a literal
  (`title="Save"`) is its value, an expression (`disabled={busy()}`) a getter, a handler a function.
  The current value of an expression is not read. A rows prop is passed as a list's props, so its
  `value` is that of `each`.
- Hot bundles keep the props of the uses they stamp, until the owner that rendered the host goes.
  Without one (a release bundle, the browser host, a Rust app) every `value` is `null`.
- `type` is the spec's TypeScript, which only a build with the text of declarations carries
  (`nsh-core`'s `declarations`: `nsh` itself, not an app's host): `null` there. The names, kinds and
  `optional` are always there; the types are in the app's `.nsh/types`.

`setOverrides` keys classes and states by where an element was written, so an
element made again (a list's row, an `If`, a hot reload) gets them again, which a node's own override
does not survive. Modules are named there as the development bundle's stamps name them, the same on
every machine (`native-pipeline.md`): a library's module outside the app by its package
(`@acme-ui/src/Card.tsx:12:5`), so a tool may keep the keys in files people share.
