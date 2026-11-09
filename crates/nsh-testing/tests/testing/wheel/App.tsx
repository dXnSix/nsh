import { mount, Scroll, signal, Text, View } from "nsh";

// A page that scrolls, a root that hears every wheel, a canvas that takes
// the wheel (`none`), a stepper that keeps it from the page (`contain`)
// and a plain row: wheels over each.
function App() {
  const heard = signal<string[]>([]);
  const note = (s: string) => heard.update((h) => [...h, s]);
  const offset = signal(0);
  return (
    <View
      style={{ flex: 1 }}
      onWheel={(e) => note(`root ${e.delta.x},${e.delta.y}${e.precise ? " precise" : ""}`)}
    >
      <Scroll
        testID="page"
        style={{ height: 400 }}
        onScrollEnd={(e) => offset.set(e.offset.y)}
      >
        <View testID="row" style={{ height: 100 }} />
        <View
          testID="canvas"
          style={{ height: 100 }}
          wheelScroll="none"
          onWheel={(e) =>
            note(`canvas ${e.delta.y} ${e.position.x},${e.position.y}${e.modifiers.control ? " ctrl" : ""}`)
          }
        />
        <View
          testID="stepper"
          style={{ height: 100 }}
          wheelScroll="contain"
          onWheel={(e) => note(`stepper ${e.delta.y}`)}
        />
        <View style={{ height: 1000 }} />
      </Scroll>
      <Text>{"Heard: " + heard().join("|")}</Text>
      <Text>{"Offset: " + offset()}</Text>
    </View>
  );
}

mount(App);
