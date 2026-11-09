// A case of the API's test suite (docs/dev/compatibility.md): a mouse
// button other than the primary one pressed over a view (`onAuxPress`):
// the button the handler gets, heard by the nearest listener around the
// view under the pointer. The handler shows what it gets as text. Its
// script checks each.
import { Text, View, signal } from "nsh";

function App() {
  const pressed = signal("none");
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View
        testID="tab"
        style={{ height: 60 }}
        onAuxPress={(e) => pressed.set(`${pressed()} ${e.pressedButton}`)}
      >
        <View testID="label" style={{ height: 20 }} />
      </View>
      <Text>Aux: {pressed()}</Text>
    </View>
  );
}

export default App;
