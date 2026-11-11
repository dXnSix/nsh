// A case of the API's test suite (docs/dev/compatibility.md): a gradient
// token. The theme set here gives `background-image-hero` as a CSS gradient,
// one in the light appearance and one in the dark: the class `bg-hero`
// draws it and `token()` gives its CSS, in the appearance shown. Its script
// checks each.
import { Text, View, cleanup, setColorScheme, setTheme, signal, token } from "nsh";

const theme = {
  cssVars: {
    light: { "background-image-hero": "linear-gradient(135deg, #7c3aed, #db2777)" },
    dark: { "background-image-hero": "linear-gradient(135deg, #a78bfa, #f472b6)" },
  },
};

function BaseGradientTokens() {
  const result = signal("none");
  // The next cases see nsh.json's theme in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`gradients hero ${token("background-image-hero")}`}</Text>
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
      <View testID="dark" onPress={() => setColorScheme("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="light" onPress={() => setColorScheme("light")}>
        <Text>Go light</Text>
      </View>
    </View>
  );
}

export default BaseGradientTokens;
