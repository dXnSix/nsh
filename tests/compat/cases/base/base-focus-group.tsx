// A case of the API's test suite (docs/dev/compatibility.md): `FocusGroup`
// of `nsh/base`, a region the arrow keys enter at its `destinations` and,
// with `trap`, never leave, with no look of its own: its view carries the
// trap and the destinations (refs to its own children, found once they
// are mounted) as the platform receives them, and lays its children out
// as the app's style says. A press on the last item moves the destination
// to it and sets the trap.
import { Text, View, signal, type ViewRef } from "nsh";
import { FocusGroup } from "nsh/base";

const item = { width: 20, height: 20 };

function App() {
  const second = signal<ViewRef>();
  const third = signal<ViewRef>();
  const switched = signal(false);
  return (
    <View style={{ flex: 1, paddingTop: 20 }}>
      <FocusGroup
        testID="shelf"
        trap={switched()}
        destinations={[switched() ? third() : second()]}
        style={{ flexDirection: "row" }}
      >
        <View testID="first" onPress={() => {}} style={item} />
        <View testID="second" ref={second} onPress={() => {}} style={item} />
        <View testID="third" ref={third} onPress={() => switched.set(true)} style={item} />
      </FocusGroup>
      <Text>{`Trap: ${switched()}`}</Text>
    </View>
  );
}

export default App;
