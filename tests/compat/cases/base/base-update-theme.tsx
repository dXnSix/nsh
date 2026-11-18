// A case of the API's test suite (docs/dev/compatibility.md): `updateTheme`
// merges a theme patch into the app's overrides while it runs and keeps the
// others (nsh.json's `"theme"` is nsh/base's default tokens with `up` and
// `radius-card` here); two updates in a row both take. `token()` reads each
// change from the next turn on and classes naming the tokens follow; an
// override wins over the contexts picked. A patch never adds a token: a value
// for a name the theme lacks is left out and returned as a diagnostic, and
// one that does not fit its token replaces the override, which then reads
// the theme's value. `resetTheme` puts the named tokens, or all, back to
// nsh.json's. Its script checks each.
import { Text, type ThemeDiagnostic, View, cleanup, resetTheme, setColorScheme, setModifier, signal, token, updateTheme } from "nsh";

/** The errors of `said`, each as its code and pointer, or `none`. */
function errors(said: ThemeDiagnostic[]): string {
  const out = said.filter((d) => d.severity === "error").map((d) => `${d.code} ${d.pointer}`);
  return out.length ? out.join(", ") : "none";
}

/** Shows `scheme`, and the theme's values for it: the appearance alone changes no token. */
function show(scheme: "light" | "dark") {
  setColorScheme(scheme);
  setModifier("colorMode", scheme);
}

function BaseUpdateTheme() {
  const result = signal("none");
  // The next cases see nsh.json's theme at its defaults in the system's appearance.
  cleanup(() => {
    resetTheme();
    setModifier("colorMode", "light");
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`update up ${token("up")}`}</Text>
      <Text>{`update primary ${token("primary")}`}</Text>
      <Text>{`update radius-card ${token("radius-card")}`}</Text>
      <Text>{`update result ${result()}`}</Text>
      <View testID="updated" class="h-4 bg-up" />
      <View testID="update" onPress={() => result.set(`update ${errors(updateTheme({ values: { up: "#2563eb" } }))}`)}>
        <Text>Update a token</Text>
      </View>
      <View
        testID="two"
        onPress={() => {
          updateTheme({ values: { primary: "#dc2626" } });
          updateTheme({ values: { "radius-card": 20 } });
          result.set("two");
        }}
      >
        <Text>Two updates</Text>
      </View>
      <View testID="dark" onPress={() => show("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="light" onPress={() => show("light")}>
        <Text>Go light</Text>
      </View>
      <View testID="bad" onPress={() => result.set(`bad ${errors(updateTheme({ values: { down: "#16a34a" } }))}`)}>
        <Text>Update a token the theme lacks</Text>
      </View>
      <View testID="unfit" onPress={() => result.set(`unfit ${errors(updateTheme({ values: { up: "#zz" } }))}`)}>
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
