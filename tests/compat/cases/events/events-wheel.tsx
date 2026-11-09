// A case of the API's test suite (docs/dev/compatibility.md): the mouse
// wheel or a touchpad over a view (`onWheel`): whether its deltas are
// precise, heard by the listeners around the view under the pointer. The
// handler shows what it gets as text. Its script checks each.
import { Text, View, signal } from "nsh";

function App() {
  const wheel = signal("none");
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View
        testID="canvas"
        style={{ height: 60 }}
        wheelScroll="none"
        onWheel={(e) => wheel.set(`${wheel()} ${e.precise ? "touchpad" : "notches"}`)}
      >
        <View testID="inner" style={{ height: 20 }} />
      </View>
      <Text>Wheel: {wheel()}</Text>
    </View>
  );
}

export default App;
