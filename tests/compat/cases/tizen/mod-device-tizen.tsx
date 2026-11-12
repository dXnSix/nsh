// A case of the API's test suite (docs/dev/compatibility.md): a script run
// as a native Tizen app (`platform tizen`): `platform.os` is `"tizen"` (an
// `OS` of its own, which `===` narrows to), in the TV idiom, never desktop;
// `platform.select` takes the `tizen` key first, then `tv`, then `mobile`,
// and `native` (Tizen is no browser). Its script checks each.
import { type OS, Text, View, platform } from "nsh";

const tizen: OS = "tizen";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`OS: ${platform.os} Name: ${platform.name}`}</Text>
      <Text>{`Tizen: ${platform.os === tizen}`}</Text>
      <Text>{`Idiom: ${platform.idiom}`}</Text>
      <Text>{`TV: ${platform.tv} Desktop: ${platform.desktop}`}</Text>
      <Text>{`Select: ${platform.select({ tizen: "tizen", web: "web", linux: "linux", tv: "tv" })}`}</Text>
      <Text>{`Select TV: ${platform.select({ linux: "linux", tv: "tv", desktop: "desktop" })}`}</Text>
      <Text>{`Select native: ${platform.select({ native: "native", default: "default" })}`}</Text>
    </View>
  );
}

export default App;
