import { mount, signal, Text, View } from "nsh";
import { Input } from "nsh/base";

// A tab that closes on the middle button, a panel that hears the other
// buttons and a context menu, and a field: presses with each button.
function App() {
  const heard = signal<string[]>([]);
  const note = (s: string) => heard.update((h) => [...h, s]);
  return (
    <View style={{ flex: 1, paddingTop: 20 }}>
      <View
        testID="panel"
        style={{ height: 200 }}
        onContextMenu={() => note("menu")}
        onAuxPress={(e) =>
          note(`panel ${e.pressedButton} ${e.position.x},${e.position.y}${e.modifiers.shift ? " shift" : ""}`)
        }
      >
        <View testID="tab" style={{ height: 40 }} onPress={() => note("tab")}>
          <Text>Tab</Text>
        </View>
        <View
          testID="close"
          style={{ height: 40 }}
          onAuxPress={(e) => note(`close ${e.pressedButton}`)}
        >
          <Text>Closable</Text>
        </View>
        <Input testID="field" style={{ height: 40 }} />
      </View>
      <Text>{"Heard: " + heard().join("|")}</Text>
    </View>
  );
}

mount(App);
