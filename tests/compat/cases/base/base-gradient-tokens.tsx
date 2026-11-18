// A case of the API's test suite (docs/dev/compatibility.md): a gradient
// token. The theme set here gives `background-image-hero` a linear gradient,
// and the `dark` context of its `colorMode` modifier another: the class
// `bg-hero` draws it and `token()` gives its type, angle and stops, at the
// context picked with the appearance. Its script checks each.
import { Text, type Theme, View, cleanup, setColorScheme, setModifier, setTheme, signal, token } from "nsh";

const hero = (from: string, to: string) => ({
  type: "linear" as const,
  angle: 135,
  stops: [{ color: from }, { color: to }],
});

const theme: Theme = {
  tokens: { "background-image-hero": { kind: "gradient", value: hero("#7c3aed", "#db2777") } },
  modifiers: {
    colorMode: {
      default: "light",
      contexts: { light: {}, dark: { "background-image-hero": hero("#a78bfa", "#f472b6") } },
    },
  },
  resolutionOrder: ["colorMode"],
};

/** Shows `scheme`, and the theme's values for it: the appearance alone changes no token. */
function show(scheme: "light" | "dark") {
  setColorScheme(scheme);
  setModifier("colorMode", scheme);
}

/** A gradient token as `type angle stop, stop`. */
function gradient(value: unknown): string {
  if (value == null || typeof value !== "object") return `${value}`;
  const g = value as { type: string; angle?: number; stops: { color: string }[] };
  return `${g.type} ${g.angle} ${g.stops.map((s) => s.color).join(", ")}`;
}

function BaseGradientTokens() {
  const result = signal("none");
  // The next cases see nsh.json's theme at its defaults in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setModifier("colorMode", "light");
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`gradients hero ${gradient(token("background-image-hero"))}`}</Text>
      <Text>{`gradients result ${result()}`}</Text>
      <View testID="hero" class="h-4 w-4 bg-hero" />
      <View
        testID="set"
        onPress={() => {
          setTheme(theme);
          result.set("set");
        }}
      >
        <Text>Set a theme with a gradient</Text>
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

export default BaseGradientTokens;
