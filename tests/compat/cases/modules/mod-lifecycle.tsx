// A case of the API's test suite (docs/dev/compatibility.md): `Lifecycle`
// beside `lifecycle()`: `on("terminate")` returns a function that stops
// listening, and nothing arrives while the app runs. Its script checks both.
import { Lifecycle, Text, View, signal } from "nsh";

function App() {
  const ended = signal("no");
  const stops = signal("listening");
  const stop = Lifecycle.on("terminate", () => ended.set("yes"));
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`Terminated: ${ended()}`}</Text>
      <Text>{`Terminate listener: ${stops()}`}</Text>
      <View
        testID="stop-terminate"
        onPress={() => {
          stop();
          stops.set(typeof stop === "function" ? "stopped" : "no function");
        }}
      >
        <Text>Stop</Text>
      </View>
    </View>
  );
}

export default App;
