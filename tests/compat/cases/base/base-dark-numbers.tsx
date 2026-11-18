// A case of the API's test suite (docs/dev/compatibility.md): a number token
// with a dark value of its own. The theme set here gives `radius-card` 14
// and the `dark` context of its `colorMode` modifier 6: picked with the dark
// appearance, `token()` and a class naming it (`rounded-(--radius-card)`)
// follow, and `gap` (given once) is the same in both. Its script checks
// each.
import { Text, type Theme, View, cleanup, setColorScheme, setModifier, setTheme, signal, token } from "nsh";

const theme: Theme = {
  tokens: { "radius-card": { kind: "number", value: 14 }, gap: { kind: "number", value: 4 } },
  modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { "radius-card": 6 } } } },
  resolutionOrder: ["colorMode"],
};

/** Shows `scheme`, and the theme's values for it: the appearance alone changes no token. */
function show(scheme: "light" | "dark") {
  setColorScheme(scheme);
  setModifier("colorMode", scheme);
}

function BaseDarkNumbers() {
  const result = signal("none");
  // The next cases see nsh.json's theme at its defaults in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setModifier("colorMode", "light");
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
      <View testID="dark" onPress={() => show("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="light" onPress={() => show("light")}>
        <Text>Go light</Text>
      </View>
    </View>
  );
}

export default BaseDarkNumbers;
