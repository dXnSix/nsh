// A case of the API's test suite (docs/dev/compatibility.md): `updateTheme`
// changes some of the app's tokens while it runs and keeps the others
// (nsh.json's `"theme"` sets `up` and `radius-card` here, over shadcn's
// `neutral`); two updates in a row both take. `token()` reads each change at
// once and classes naming the tokens follow, in each appearance; an update
// with a value NSH cannot read throws a `TypeError` and changes nothing.
// `resetTheme` puts the named tokens, or all, back to nsh.json's. Its script
// checks each.
import { Text, View, cleanup, resetTheme, setColorScheme, signal, token, updateTheme } from "nsh";

function BaseUpdateTheme() {
  const result = signal("none");
  // The next cases see nsh.json's theme in the system's appearance.
  cleanup(() => {
    resetTheme();
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`update up ${token("up")}`}</Text>
      <Text>{`update primary ${token("primary")}`}</Text>
      <Text>{`update radius-card ${token("radius-card")}`}</Text>
      <Text>{`update result ${result()}`}</Text>
      <View testID="updated" class="h-4 bg-up" />
      <View
        testID="update"
        onPress={() => {
          updateTheme({ cssVars: { light: { up: "#2563eb" }, dark: { up: "#93c5fd" } } });
          result.set(`update ${token("up")}`);
        }}
      >
        <Text>Update a token</Text>
      </View>
      <View
        testID="two"
        onPress={() => {
          updateTheme({ cssVars: { light: { primary: "#dc2626" } } });
          updateTheme({ cssVars: { theme: { "radius-card": "20px" } } });
          result.set("two");
        }}
      >
        <Text>Two updates</Text>
      </View>
      <View testID="dark" onPress={() => setColorScheme("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="system" onPress={() => setColorScheme("system")}>
        <Text>Follow the system</Text>
      </View>
      <View
        testID="bad"
        onPress={() => {
          try {
            updateTheme({ cssVars: { light: { up: "#16a34a" }, dark: { up: "#zz" } } });
            result.set("taken");
          } catch (e) {
            result.set(`refused ${(e as Error).name}`);
          }
        }}
      >
        <Text>Update with a bad value</Text>
      </View>
      <View
        testID="reset"
        onPress={() => {
          resetTheme(["up", "radius-card"]);
          result.set("reset");
        }}
      >
        <Text>Reset two tokens</Text>
      </View>
      <View
        testID="all"
        onPress={() => {
          resetTheme();
          result.set("all");
        }}
      >
        <Text>Reset every token</Text>
      </View>
    </View>
  );
}

export default BaseUpdateTheme;
