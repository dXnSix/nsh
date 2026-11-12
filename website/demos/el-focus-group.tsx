import { mount, signal, Text, View, type ViewRef } from "nsh";
import { FocusGroup } from "nsh/base";
import { focusRing, Log, sectionSurface, Split, ui } from "./palette";

// Two shelves of a TV's home screen, each a `FocusGroup`: the arrows enter
// a shelf at the tile last focused in it (its `destinations`), not at the
// tile nearest the focus, and the side menu, open, traps them. Where the
// arrows move the focus (the GPUI backend, TVs) that is how a remote goes;
// elsewhere the tiles are pressables that Tab reaches.
const SHELVES: { title: string; items: string[] }[] = [
  { title: "Continue watching", items: ["Tides", "Northern Lights", "The Orchard", "Glasswork"] },
  { title: "New this week", items: ["Harbor", "Field Notes", "Small Hours", "Saltwater", "Late Bloom"] },
];

function Shelf(props: { title: string; items: string[]; log: (line: string) => void }) {
  // The tile the shelf is entered at: the one focused last.
  const current = signal<ViewRef>();
  return (
    <View style={{ gap: 8 }}>
      <Text style={{ fontSize: 13, fontWeight: "600", color: ui().muted }}>{props.title}</Text>
      <FocusGroup destinations={[current()]} style={{ flexDirection: "row", flexWrap: "wrap", gap: 10 }}>
        {props.items.map((item) => {
          let tile: ViewRef | undefined;
          return (
            <View
              ref={tile}
              onPress={() => props.log(`${props.title}: play ${item}`)}
              onFocus={() => {
                current.set(tile);
                props.log(`${props.title}: ${item}`);
              }}
              style={{ ...sectionSurface(), width: 132, height: 76, padding: 10, justifyContent: "flex-end" }}
              focusStyle={focusRing(2)}
            >
              <Text style={{ fontSize: 13, color: ui().text }}>{item}</Text>
            </View>
          );
        })}
      </FocusGroup>
    </View>
  );
}

function App() {
  const menu = signal(false);
  const events = signal<string[]>([]);
  const log = (line: string) => events.set([line, ...events()].slice(0, 4));
  return (
    <View style={{ flex: 1, backgroundColor: ui().page, padding: 20, paddingTop: 24, gap: 14 }}>
      <Text style={{ fontSize: 22, fontWeight: "700", color: ui().text }}>focus group</Text>
      <Text style={{ color: ui().muted }}>Move with the arrow keys: each shelf is entered at the tile focused last</Text>
      <Split gap={14} inspector={<Log title="onFocus · onPress" lines={events()} minHeight={92} />}>
        <View style={{ flexDirection: "row", gap: 16 }}>
          <FocusGroup trap={menu()} style={{ ...sectionSurface(), padding: 8, gap: 4, width: 120 }}>
            {["Home", "Movies", "Series"].map((name, i) => (
              <View
                onPress={() => log(`menu: ${name}`)}
                preferredFocus={i === 0}
                style={{ paddingVertical: 8, paddingHorizontal: 10, borderRadius: 8, borderWidth: 1, borderColor: "transparent" }}
                focusStyle={focusRing()}
              >
                <Text style={{ fontSize: 13, color: ui().text }}>{name}</Text>
              </View>
            ))}
            <View
              onPress={() => menu.set(!menu())}
              style={{ paddingVertical: 8, paddingHorizontal: 10, borderRadius: 8, borderWidth: 1, borderColor: "transparent" }}
              focusStyle={focusRing()}
            >
              <Text style={{ fontSize: 12, color: ui().muted }}>{menu() ? "Unpin menu" : "Pin menu"}</Text>
            </View>
          </FocusGroup>
          <View style={{ flex: 1, gap: 16 }}>
            {SHELVES.map((shelf) => (
              <Shelf title={shelf.title} items={shelf.items} log={log} />
            ))}
          </View>
        </View>
      </Split>
    </View>
  );
}

mount(App);
