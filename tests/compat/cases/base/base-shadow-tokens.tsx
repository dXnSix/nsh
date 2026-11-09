// A case of the API's test suite (docs/dev/compatibility.md): a shadow token.
// The theme set here gives `shadow-raised` as CSS `box-shadow`, one in the
// light appearance and one in the dark: the class `shadow-raised` draws it
// and `token()` gives its CSS, in the appearance shown. Its script checks
// each.
import { Text, View, cleanup, setColorScheme, setTheme, signal, token } from "nsh";

const theme = {
  cssVars: {
    light: { "shadow-raised": "0 2px 4px #0000004d" },
    dark: { "shadow-raised": "0 4px 8px #ffffff33" },
  },
};

function BaseShadowTokens() {
  const result = signal("none");
  // The next cases see nsh.json's theme in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`shadows raised ${token("shadow-raised")}`}</Text>
      <Text>{`shadows result ${result()}`}</Text>
      <View testID="raised" class="h-4 w-4 bg-white shadow-raised" />
      <View
        testID="set"
        onPress={() => {
          setTheme(theme);
          result.set("set");
        }}
      >
        <Text>Set a theme with a shadow</Text>
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

export default BaseShadowTokens;
