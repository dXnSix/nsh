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

On Windows the tool listens on the loopback address (`tcp:127.0.0.1:<port>`), where any process of
the machine could connect, so it also makes a token for the run and gives it in `NSH_DESIGN_TOKEN`:
the app's hello carries it (`{"hello":{"token":"…"}}`), and the tool takes the connection only after
a hello with its token. The token is never logged, at any level. Without the variable (or with it
empty) the hello is `{"hello":{}}`, as on every platform before.

A tool that starts an app with Rust crates of its own optimized (`nsh run --release` or `nsh dev
--release` on macOS, Linux or Windows, with the socket set) waits for its builds, so they are quick
ones: the describing program builds together with the development host, in one `cargo build` with
the generated workspace's `design` profile (release without LTO, incremental) in `target/design`,
instead of a debug `describe` and a `release` host built apart. What packages ship keeps `release`
(`docs/dev/tooling.md`, the Rust apps' generated workspace).

## The connection

`nsh_core::runtime::design` keeps it, the same on every platform. One JSON object a line both ways:

| From | Message | What |
|---|---|---|
| host | `{"hello":{}}` | Once connected; `{"hello":{"token":…}}` when the tool gave the run a token (`NSH_DESIGN_TOKEN`). |
| host | `{"surface":{"id","width","height", <handle>}}` | A window of the app's own (named by `Window.open`'s `id`), whenever it is new or its handle or size changed. The handle is what the tool draws it with: `"context"`, its layer's `CAContext` id, on macOS (`CALayerHost`); `"hwnd"` on Windows, together with its shared DirectComposition surface handle `"surface"` and owning process id `"pid"` once its swap chain is made; `"wayland"`, the window's monotonic tag (`n` matching its Wayland toplevel title `"nsh-design:<n>"`), on Linux. |
| host | `{"surface":{"id","gone":true}}` | The window closed. |
| tool | `{"pointer":{"id","kind":"down\|drag\|up\|move","x","y"}}` | Input, delivered by the platform to window `id`, in points from its content's top left. |
| tool | `{"wheel":{"id","x","y","dx","dy"}}`, `{"key":{"id","down","key",…}}`, `{"resize":{"id","width","height"}}` | The same; a key as the DOM names it. |
| tool | anything else | The app's: `design.next()`. |
| app | `design.send(object)` | To the tool. |
| host | `{"hotUpdate":{"modules":[…]}}` | Dev builds (`inspect`): a hot update ran these modules again, named by their hot keys as stamps name them (`src/Card.tsx`, `@acme-ui/src/Card.tsx`), so the tool draws again what they show. A module with no JSX has no key and is not named; what shows it is in an importer that ran too, and is. |

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
- **Windows**: the handle is the window's `HWND` (`"hwnd"`), and every window of the app's own in design
  mode uses a composed swap chain backed by a cross-process DirectComposition surface handle
  (`DCompositionCreateSurfaceHandle` + `IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle`),
  whose numeric handle (`"surface"`) and owning process id (`"pid"`) are reported on the `surface` line
  once the swap chain is made (and again if recreated after device loss). A design tool duplicates
  `"surface"` from `"pid"` (`OpenProcess(PROCESS_DUP_HANDLE)` + `DuplicateHandle`) and binds it directly
  into its own DirectComposition tree (`IDCompositionDesktopDevice::CreateSurfaceFromHandle`), preserving
  premultiplied alpha with zero frame copies, or falls back to a DWM thumbnail on `"hwnd"`. The tool's
  endpoint is `tcp:127.0.0.1:<port>` and the run's token comes in `NSH_DESIGN_TOKEN` (above). The windows
  are kept out of sight by the window
  procedure (`design::window_message`), never by a timer, so no frame of one reaches a display at any
  moment:
  - the main window never shows (`SW_HIDE`: its `WM_WINDOWPOSCHANGING` drops `SWP_SHOWWINDOW`, and the
    host does not show it at launch);
  - a window of the app's own gets the tool-window and no-activate styles before it first shows (kept
    through `WM_STYLECHANGING`): no taskbar button, no place in the window switcher, and the foreground
    window does not change when one opens (the host runs as a background app, `run_in_background`);
  - every `WM_WINDOWPOSCHANGING` of one, the one that changes its size included, sets its position so
    that its whole rectangle lies beside the virtual screen (the union of all displays' rectangles), by a
    margin: left of it, else right of it, below it or above it, the first place whose origin stays within
    -32,768 to 32,767 (`design::out_of_sight`), the range Windows keeps a window's origin in. A window may
    be far larger than the displays (16,384 points at 300 % are 49,152 pixels; at 200 % 32,768, which no
    longer fits left of a display left of the main one), and its origin cut to -32,768 would bring it
    back onto them. `WM_DISPLAYCHANGE` places them all again. DWM still composes them, so their thumbnails stay
    live and their frames keep coming. The 50 ms tick only reports the windows to the tool;
  - a window lays out at its content's size up to the natural-size limit (16,384 points a side) at any
    display scale (`WM_GETMINMAXINFO` neither raises it to the system's smallest window nor cuts it to
    the displays' size), while what it presents, its swap chain or composition swap chain, is cut to the
    device's largest texture (`gfx::presentable`): its pictures are drawn apart (below), by `rect` where
    a picture would be larger, never read from the swap chain;
  - the host's own window shortcuts (F11's full screen, the dev session's reload on F5 and Ctrl+R) are
    not installed: every key the tool sends is the app's.

  Input goes through `crate::input` in the window's root (`app_window::enter`), as the test agent's
  does; the connection's thread hands it over with `UiThread::invoke`. A `key` is the key event a real
  press gives (`input::key`, with its modifiers and `repeat`, down and up), then, going down, the text
  `TranslateMessage` would make of it where nothing took the key and neither Control nor Alt is held
  (`text_input::char_input`): letters, digits and every printable ASCII punctuation character by the US
  layout's virtual keys (Shift's characters by the key that types them), Space, Enter, Tab, Backspace,
  Delete, Escape, the arrows, Home, End, PageUp, PageDown and F1 to F12; any other value of one
  character (any Unicode) arrives as that character, with no virtual key; any other name is logged at
  debug level and dropped. When the tool closes the connection the host exits.
- **Linux**: the tool runs its own Wayland compositor and starts the app against it with `NSH_DESIGN_WAYLAND=1`,
  `GDK_BACKEND=wayland`, and `WAYLAND_DISPLAY` pointing to the tool's socket. Each window of the app's own gets a
  monotonic number `n` (`1, 2, …`), sets its Wayland toplevel title to `"nsh-design:<n>"` (kept across attribute
  updates), stays undecorated and shadowless so its toplevel geometry matches its content size, and is reported with
  handle `"wayland": n`. Its windows are neither moved nor hidden (the tool's compositor owns their display), and the
  main window never shows; the host's Ctrl+R reload shortcut is not installed. Pointer, wheel, and keyboard input
  arrive directly over the Wayland seat rather than through the design socket (`resize` resizes the window; `pointer`,
  `wheel`, `key`, and `viewport` socket messages are ignored). Without `NSH_DESIGN_WAYLAND=1` (or without a Wayland
  display), Linux design mode logs once and reports no windows. When the tool closes the connection, the app stops with
  its process group.

Pictures (`design.settled`, `design.picture`, below) are a platform's to draw
(`nsh_core::runtime::design::PictureHooks`, given with `set_picture_hooks` after `connect`); macOS, Windows, and Linux
draw them, and the other platforms answer `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`:

- **Settling** follows the core's rule (`Settling`): quiet for 50 ms since the question (no batch applied, no
  animation that ends by itself running, no batch waiting for a frame; an animation that loops with no end does
  not count), given up at 1 s. The platform looks every 5 ms on its main thread, at the surface the window shows
  (on macOS one surface holds every window of the app, so a window is quiet when the app is).
- **macOS** draws in process (`crate::picture`): once no batch waits for a frame, and a frame more, Core
  Animation's `renderInContext:` of the window's content at the scale and the part asked for, into an sRGB bitmap,
  written to the tool's file. Neither the Screen Recording permission nor the window shown is needed. Two things an
  in-process rendering leaves out are drawn for it: the outer box shadows (`visual.rs`'s casters, found by their
  holder's name `nsh.shadows`: their shapes' shadows drawn into an image by their offset, radius, colour and
  opacity, the box cut out where the holder's mask cuts it, in their place while the picture is taken), and an
  animation that loops with no end stands at the start of the run it is in (`Animator::at_loop_starts`). What the
  picture changes in the layers is put back in the same Core Animation transaction, so the window shows nothing
  of it. The tests compare it with the window server's picture of the window (`synth::composited_image`): at most
  2 % of the pixels differ by more than 8 of 255 (`tests/design_pictures.rs`, `tests/design_picture_shadows.rs`).
- **Windows** settles on a count of the batches its surface applied (`Surface::applied`; batches are applied as
  they arrive, none waits for a frame) and on whether an animation that ends by itself or a platform motion runs
  (`Surface::animating`: the graph's animations that end, and the frame clients but those with only a view's
  keyframes repeating forever left, `Surface::cosmetic_frame`, as macOS's `cosmetic_frames`), looked at every 5 ms
  on the UI thread. It draws in process (`crate::picture`): once what
  the engine sent before the question is applied and a frame has followed, the window's root view is drawn with
  Direct2D's offscreen path (`render::picture`, the snapshot's drawing with a region and the window's root) at the
  scale and the part asked for, over transparent pixels (alpha 0 where nothing is drawn: no window background and
  no caption buttons), read back as premultiplied BGRA and written as RGBA rows. The outer box shadows are drawn
  as the window draws them; an animation that loops with no end stands at the start of the run it is in
  (`Animator::at_loop_starts`), and so does a view's keyframe `animation` repeating forever (`spin`, `ping`,
  `pulse`, `bounce`: `transition::Live::stand_at_loop_start`); the views show their animated values again once it
  is drawn. A picture
  larger than the device's largest bitmap is drawn in tiles. **The one rule that differs:** a picture on Windows
  leaves out composed components, a web view's page and a native component's own visual
  (`NativeComponent::composed`): the system composes them, and the offscreen drawing has none of them. Every
  other rule of the contract holds as on macOS, the codes included (a drawing Direct2D fails is
  `ERR_NSH_DESIGN_NO_WINDOW`, as a window macOS cannot draw is). Tests: `cargo test -p nsh-platform-windows
  --features dev --test design_windows` (below).
- **Linux** settles on a count of the batches its surface applied (`Surface::applied`) and on whether a finite
  animation or platform transition runs (`Surface::animating`: the graph's finishing animations, navigation
  transitions, and `frame_callbacks` with a finite deadline `Next::At`, leaving endless `Next::Frame` keyframes out),
  looked at every 5 ms on the GLib main loop. It draws in process (`crate::picture`): after draining pending batches
  and waiting a frame, endless animations stand at their first frame (`UiAnimation::at_loop_starts` and recursive
  `Live::stand_at_loop_start`), the window is allocated at its size, the window's root `NshView` is snapshotted into a
  `gtk::Snapshot` at the requested scale and region (bypassing `GtkWidgetPaintable`, which drops render nodes on queued
  draws), rendered offscreen via GSK's renderer (tiled past 4,096 px a side), and downloaded via
  `gdk::TextureDownloader` as premultiplied sRGB RGBA (`R8g8b8a8Premultiplied`) over transparent pixels. Outer box
  shadows are drawn directly by `decor` in GSK. Tests: `cargo test -p nsh-platform-linux --features dev --test
  design_wayland` (below).

## `nsh/design`

```ts
import { design } from "nsh/design";
design.active();                       // started by a tool
const message = await design.next();   // the tool's next message
design.send({ called: { id: "a" } });
const layers = await design.layers("board:", ".tool-module"); // dev builds
const detail = await design.detail(layerId);
design.setOverrides({ "src/Card.tsx:12:5": { class: "gap-4", replace: false, states: 0 } });
const { timedOut } = await design.settled("card:a");                    // dev builds a tool started
const picture = await design.picture("card:a", { scale: 2, path: "/tmp/tool/a.rgba" });
const part = await design.picture("card:a", { scale: 1, path: "/tmp/tool/b.rgba", rect: { x: 0, y: 0, width: 100, height: 50 } });
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

A slot marker component (one a component's view puts where an app's content for a slot goes, around that
content) calls `design_slot(name)` (`nsh_core::prelude`) in its view, so a design tool finds the slot even
while it is empty: the marker's host is the place of slot `name` of the component whose view drew it.
`name` is the JS name of the prop that fills the slot, a value or a binding, read when a tool asks (what
it reads it does not subscribe to). The marker draws what the slot should look like while empty in a
design tool (with `design_mode()`); without dev tools (`inspect`) `design_slot` does nothing.

```rust
#[component]
fn Mark(props: MarkProps) -> impl IntoElement {
    design_slot(props.name);
    view().children_each(props.children, |child, _| child)
}

#[component]
fn Chip(props: ChipProps) -> impl IntoElement {
    view()
        .child(mark().name("left").child(props.left.place(|s| s)))
        .child(text(props.text))
}
```

`layers(prefix, hide)` reads, with the dev tool hooks (`inspect`), the live tree under each view whose
test id starts with `prefix`, keyed by the rest of the test id, with each component instance between
the nodes it rendered and their parent (ids from `1 << 32`), leaving out instances whose key or call
site contains `hide`; only roots that changed since the last call, all after `setOverrides`, `null`
when nothing changed: `{ roots: { <root>: { nodes } }, gone }`. A layer is one of:

| Layer | Shape |
|---|---|
| A node | `{ id, parent, kind, name, class, testId, text, x, y, width, height, source, hidden, overridden, slot, place }`. `kind` is the node's (`view`, `text`, `list`, `cell`, …), `name` its title (`View`); `source` is `{ module, line, column }`, where it was written. |
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
- A slot marker (a component whose view calls `design_slot`) has `place`, its slot's name; every other
  layer has `place: null`. A marker the component's view draws is listed under the host, empty or
  filled, as the host it is (`kind: "host"`, no `source`), with the slot's holder and the app's content
  under it. A marker that a component nested in the view draws for a slot of its own is that
  component's, part of the view, and not listed. A marker the app writes is a layer as any of the
  app's, with its `place`. A slot the view does not make (behind a `when` that is off) has no marker.
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

With the `Chip` above, `<Chip text="A" left={<Text>L</Text>} />` and an empty `<Chip text="B" />`:

```text
host Chip
  host Mark      place: "left"
    view View    slot: "left"
      text Text  "L"
host Chip
  host Mark      place: "left"     (its frame: what the marker draws while empty)
```

The core's tree says the same (`dev_tree`): a host has `flags.host` and its component's export as
`component`, a node of the script's inside a component's view has `given: { host, slot }`, the
nearest host above it that the script made, and a marker's host has `place: { name, of }`, `of` the
host whose view drew it (`None` when the script made it). The core's nodes have odd ids and were written nowhere:
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

### Pictures

A tool that shows most of an app's windows as pictures, and runs few of them live, asks for them by the window's
`id` (`Window.open`'s). Dev builds with the dev tool hooks (`inspect`) only, in an app a design tool started; a
Rust component asks the same with `nsh_core::runtime::design::settled` and `picture` (`PictureOptions`, `Picture`).

- `settled(id)` resolves once the window has settled (above: quiet for 50 ms, at most 1 s), after what the app did
  before asking has been sent to the platform: `{ timedOut }`, true when it was still busy at 1 s.
- `picture(id, { scale, path, rect?, hide? })` puts the overrides of `setOverrides` on the window's layers and lets
  the next commit lay them out, reads the tree, has the platform draw the window's content (or `rect` of it, in the
  content's points from its top left) at `scale` pixels a point once the batches before are applied and a frame
  has followed, into the file at `path`, and resolves to `{ width, height, layers, modules }`:
  - the file: raw pixels, 4 bytes each (red, green, blue, alpha; alpha premultiplied; sRGB), rows from the top,
    no padding and nothing else, `width * height * 4` bytes, which the tool maps. The app writes that path only;
  - `layers`: every layer under the window's root in `layers()`' node shape (components, hosts, `slot`, `place`),
    leaving out instances whose key or call site contains `hide`. `layers()`' "since the last call" is not touched:
    a picture neither reports nor hides a change to `layers()`;
  - `modules`: the modules the layers were written in, and those of their component instances (where each was used
    and where its component is), sorted, named as stamps name them: what a tool compares with the `hotUpdate`
    notice's modules;
  - an animation that loops with no end stands at its first frame; one that ends by itself, as it is.

Each rejects with a `code` (`nsh_core::runtime::design::codes`):

| Code | When |
|---|---|
| `ERR_NSH_DESIGN_NOT_ACTIVE` | No design tool started the app, or the build has no dev tool hooks |
| `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM` | The platform draws no pictures |
| `ERR_INVALID_ARG_VALUE` | `scale` not above 0 and at most 16, `path` not absolute, a `rect` without a size or with numbers that are not finite, an option of the wrong type |
| `ERR_NSH_DESIGN_NO_WINDOW` | No window of the app's own is open with that `id` |
| `ERR_NSH_DESIGN_RECT_OUTSIDE` | `rect` is not within the window's content |
| `ERR_NSH_DESIGN_PATH` | The file cannot be written at `path` |

Tests: `cargo test -p nsh-engine --features inspect --test engine design_pictures` (the core's part, with a
stand-in platform that settles on the batches and the animation graph and writes the file: the settle rule, the
overrides laid out before the tree, the tree and modules, `rect`, a loop at its first frame, the codes, outside
design mode, the hot update's notice); on macOS `cargo test -p nsh-platform-macos --features dev --test
design_pictures` and `--test design_picture_shadows` (above); on Windows `cargo test -p nsh-platform-windows
--features dev --test design_windows`, which plays the tool over TCP against an app it starts in design mode with
real Win32 windows: the hello with and without a token (the app at NSH's most verbose log level, its log without
the token), pictures at scale 1 and 2, a `rect`, a `hide`, settling after a timer's batch, during a 300 ms
animation and a 300 ms style transition and given up at 1 s, an endless loop drawn twice alike at its first frame,
a view's endless keyframes (`animate-pulse`) settling at once and drawn twice alike at their first frame, an outer
shadow, each code;
every place a window's size change left it in checked in its message (`WH_CALLWNDPROCRET`), the windows' styles,
the hidden main window, the foreground window as a window opens, `WM_DISPLAYCHANGE`, a 16,384 x 1,000-point
window at a stood-in 150 % and 300 % that lies beside the displays, presents at most the largest texture and gives
a `rect` picture, the 150 % one placed right of the displays once a display stood in left of the origin; every
printable
ASCII key and a non-ASCII character typed into a field, Control and Alt chords typing nothing, each named key at a
key-logging view and an unlisted one dropped; a composed component placed whatever its own box; and a design
window's shared DirectComposition surface handle (`surface`, `pid`) duplicated into the tool's process and
opened with `IDCompositionDesktopDevice::CreateSurfaceFromHandle`; on Linux `cargo test -p nsh-platform-linux
--features dev --test design_wayland`, which runs a minimal Smithay Wayland compositor in process and starts the app
against it (`NSH_DESIGN_WAYLAND=1`, `GDK_BACKEND=wayland`): hello with and without a token (log free of the token),
no surfaces reported without `NSH_DESIGN_WAYLAND=1`, `"wayland"` handles matching `"nsh-design:<n>"` toplevel titles
and `wl_shm` frames, main window unmapped, `gone` on close, `resize` and ignored `viewport`, pictures at scale 1 and 2
with `rect`/`hide`/shadows/loops/keyframes, settling, all error codes, and real Wayland pointer input reaching the app.

### Composed components on Windows

Windows' renderer places a composed component (`NativeComponent::composed`: a component with a visual of its own)
while its clipping ancestors leave a non-empty clip, whatever its own box, unless its view clips its own content
(`DrawCx::placement_within`): its `Placement` keeps the matrix and that clip. A component whose visual draws past
its box (a canvas layer of a 1x1 view at a world's origin, which a pan takes out of the pane) therefore stays in
the window's tree; one whose view clips (`overflow: hidden`, rounded corners) is placed only while its box meets
the clip.
