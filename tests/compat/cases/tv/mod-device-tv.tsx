// A case of the API's test suite (docs/dev/compatibility.md): a script run
// as on an Android TV (`platform android tv`): `platform` reports the TV
// idiom with the real OS, `platform.tv` is true and desktop behavior does
// not apply; `platform.select` looks for the OS's own key, then `tv`, then
// `mobile`. Its script checks each.
import { Text, View, platform } from "nsh";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`OS: ${platform.os}`}</Text>
      <Text>{`Idiom: ${platform.idiom}`}</Text>
      <Text>{`TV: ${platform.tv} Desktop: ${platform.desktop}`}</Text>
      <Text>{`Select: ${platform.select({ tv: "tv", mobile: "mobile", desktop: "desktop" })}`}</Text>
      <Text>{`Select OS: ${platform.select({ android: "android", tv: "tv" })}`}</Text>
      <Text>{`Select mobile: ${platform.select({ mobile: "mobile", desktop: "desktop" })}`}</Text>
    </View>
  );
}

export default App;
