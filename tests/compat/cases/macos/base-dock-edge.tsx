// A case of the API's test suite (docs/dev/compatibility.md): `DockArea`
// of `nsh/base` on a desktop docking tool panels at the area's edges, as
// Visual Studio and Rider do. The left dock's only panel (`layers`) drags
// out to the right edge: the drop indicator covers what a right dock would
// take (`renderDropIndicator` told the `dock`), the release makes it, and
// the left dock it leaves stays closed and empty at its size. A panel's
// `placements` keep it where it may go: the document (`["center"]`) shows
// no indicator at an edge and does not move there, and the ref's
// `movePanel` refuses it too. Dragged back to the left edge, `layers`
// opens the emptied dock again at the size it kept.
import { Text, View, signal } from "nsh";
import { DockArea, type DockAreaLayout, type DockAreaRef, type DockNode, type DockRegion } from "nsh/base";

function describe(node: DockNode): string {
  if (node.type === "tabs") {
    return `[${node.panels.map((p, i) => (i === (node.activeIndex ?? 0) ? `${p}*` : p)).join(" ")}]`;
  }
  const sizes = node.sizes ?? [];
  const children = node.children.map((child, i) => {
    const size = sizes[i];
    return size == null ? describe(child) : `${describe(child)}:${Math.round(size)}`;
  });
  return `${node.axis === "horizontal" ? "h" : "v"}(${children.join(" ")})`;
}

function describeDock(name: string, dock: DockRegion | null | undefined): string {
  if (!dock) return `${name} none`;
  return `${name} ${describe(dock.layout)} ${dock.open === false ? "closed" : "open"} ${Math.round(dock.size ?? 0)}`;
}

export default function BaseDockEdge() {
  const layout = signal<DockAreaLayout>({
    center: {
      type: "split",
      axis: "horizontal",
      children: [
        { type: "tabs", panels: ["doc"] },
        { type: "tabs", panels: ["notes"] },
      ],
      sizes: [150, null],
    },
    left: { layout: { type: "tabs", panels: ["layers"] }, size: 120 },
  });
  let area: DockAreaRef | undefined;
  return (
    <View style={{ paddingTop: 20 }}>
      <View style={{ width: 390, height: 300 }}>
        <DockArea
          ref={r => (area = r)}
          compact={false}
          layout={layout()}
          onLayoutChange={({ layout: next }) => layout.set(next)}
          panels={[
            { id: "doc", placements: ["center"] },
            { id: "layers", placements: ["left", "right", "bottom"] },
          ]}
          class="flex-1"
          tabBarClass="flex-row h-[24px]"
          tabClass="w-[60px]"
          renderTab={tab => (
            <View testID={`tab-${tab.panel}`}>
              <Text>{`Tab ${tab.panel}`}</Text>
            </View>
          )}
          renderPanel={panel => <Text>{`Panel ${panel.id}`}</Text>}
          renderDropIndicator={drop => <Text>{drop.dock ? `Drop at ${drop.dock}` : `Drop ${drop.side ?? "into"}`}</Text>}
        />
      </View>
      <Text>{() => `Center ${describe(layout().center)}`}</Text>
      <Text>{() => describeDock("Left", layout().left)}</Text>
      <Text>{() => describeDock("Right", layout().right)}</Text>
      <View
        testID="move-doc"
        style={{ width: 20, height: 20 }}
        onPress={() => area?.groupOf("layers").then(group => group != null && area?.movePanel("doc", { group }))}
      />
    </View>
  );
}
