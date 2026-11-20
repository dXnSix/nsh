# Design mode

A design tool (RayX Sketch) shows an app's components live on its own canvas: it starts the app as a
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
| host | `{"surface":{"id","width","height", <handle>}}` | A window of the app's own (named by `Window.open`'s `id`), whenever it is new or its handle or size changed. The handle is what the tool draws it with: `"context"`, its layer's `CAContext` id, on macOS (`CALayerHost`). |
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

`layers(prefix, hide)` reads, with the dev tool hooks (`inspect`), the live tree under each view whose
test id starts with `prefix`, keyed by the rest of the test id, with each component instance between
the nodes it rendered and their parent (ids from `1 << 32`), leaving out instances whose key or call
site contains `hide`; only roots that changed since the last call, all after `setOverrides`, `null`
when nothing changed. `setOverrides` keys classes and states by where an element was written, so an
element made again (a list's row, an `If`, a hot reload) gets them again, which a node's own override
does not survive.
