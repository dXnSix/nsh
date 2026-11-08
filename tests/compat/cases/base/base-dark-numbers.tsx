// A case of the API's test suite (docs/dev/compatibility.md): a number token
// with a dark value of its own. The theme set here gives `radius-card` 14px
// in the light appearance and 6px in the dark one: `token()` and a class
// naming it (`rounded-(--radius-card)`) follow the appearance shown, and
// `gap` (given once) is the same in both. Its script checks each.
import { Text, View, cleanup, setColorScheme, setTheme, signal, token } from "nsh";

const theme = {
  cssVars: { light: { "radius-card": "14px", gap: "4px" }, dark: { "radius-card": "6px" } },
};

function BaseDarkNumbers() {
  const result = signal("none");
  // The next cases see nsh.json's theme in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`numbers radius-card ${token("radius-card")}`}</Text>
      <Text>{`numbers gap ${token("gap")}`}</Text>
      <Text>{`numbers result ${result()}`}</Text>
      <View testID="card" class="h-4 w-4 rounded-(--radius-card)" />
      <View
        testID="set"
        onPress={() => {
          setTheme(theme);
          result.set("set");
        }}
      >
        <Text>Set a theme with a dark number</Text>
      </View>
      <View testID="dark" onPress={() => setColorScheme("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="light" onPress={() => setColorScheme("light")}>
        <Text>Go light</Text>
      </View>
    </View>
  );
}

export default BaseDarkNumbers;
