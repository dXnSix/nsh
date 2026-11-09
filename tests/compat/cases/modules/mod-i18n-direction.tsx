// A case of the API's test suite (docs/dev/compatibility.md):
// `nsh/i18n`'s `setLocale` lays the app out in the direction its language
// is written in: a row runs from the right in Arabic, from the left in
// English and in the device's language again, and a view whose `direction`
// is "ltr" keeps its own row left to right. Its script checks the boxes'
// frames.
import { Text, View } from "nsh";
import { setLocale } from "nsh/i18n";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View style={{ flexDirection: "row" }}>
        <View testID="first" style={{ width: 100, height: 20 }} />
        <View testID="second" style={{ width: 50, height: 20 }} />
      </View>
      <View style={{ flexDirection: "row", direction: "ltr" }}>
        <View testID="pinned" style={{ width: 100, height: 20 }} />
      </View>
      <View testID="arabic" onPress={() => setLocale("ar")}>
        <Text>Arabic</Text>
      </View>
      <View testID="english" onPress={() => setLocale("en")}>
        <Text>English</Text>
      </View>
      <View testID="device" onPress={() => setLocale(null)}>
        <Text>Device</Text>
      </View>
    </View>
  );
}

export default App;
