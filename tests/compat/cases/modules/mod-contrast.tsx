// A case of the API's test suite (docs/dev/compatibility.md): the user's
// contrast setting as the headless runtime reports it (no more contrast
// asked for), read through `highContrast()` and by a `contrast-more:`
// class, which leaves the panel's own background. Its script checks both.
import { Text, View, highContrast } from "nsh";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`High contrast: ${highContrast()}`}</Text>
      <View testID="contrast-panel" class="h-8 bg-white contrast-more:bg-black" />
    </View>
  );
}

export default App;
