// A case of the API's test suite (docs/dev/compatibility.md): a shadow token.
// The theme set here gives `shadow-raised` one layer, and the `dark` context
// of its `colorMode` modifier another: the class `shadow-raised` draws it and
// `token()` gives its layers, at the context picked with the appearance. Its
// script checks each.
import { Text, type Theme, View, cleanup, setColorScheme, setModifier, setTheme, signal, token } from "nsh";

const theme: Theme = {
  tokens: { "shadow-raised": { kind: "shadow", value: [{ color: "#0000004d", y: 2, blur: 4 }] } },
  modifiers: {
    colorMode: {
      default: "light",
      contexts: { light: {}, dark: { "shadow-raised": [{ color: "#ffffff33", y: 4, blur: 8 }] } },
    },
  },
  resolutionOrder: ["colorMode"],
};

/** Shows `scheme`, and the theme's values for it: the appearance alone changes no token. */
function show(scheme: "light" | "dark") {
  setColorScheme(scheme);
  setModifier("colorMode", scheme);
}

/** A shadow token's layers as `x y blur spread color`, the members at their default (0) left out of the token. */
function layers(value: unknown): string {
  if (!Array.isArray(value)) return `${value}`;
  return value.map((l) => `${l.x ?? 0} ${l.y ?? 0} ${l.blur ?? 0} ${l.spread ?? 0} ${l.color}`).join(", ");
}

function BaseShadowTokens() {
  const result = signal("none");
  // The next cases see nsh.json's theme at its defaults in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setModifier("colorMode", "light");
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`shadows raised ${layers(token("shadow-raised"))}`}</Text>
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
      <View testID="dark" onPress={() => show("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="light" onPress={() => show("light")}>
        <Text>Go light</Text>
      </View>
    </View>
  );
}

export default BaseShadowTokens;
