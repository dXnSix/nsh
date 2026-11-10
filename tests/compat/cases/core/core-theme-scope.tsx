// A case of the API's test suite (docs/dev/compatibility.md): `ThemeScope`,
// a view whose content is shown in an appearance of its own. In it
// `colorScheme()` and `dark:` classes follow the scope, nested scopes the
// nearest one with an appearance, and the app's own appearance leaves a
// scope as it says. Its script checks each.
import { Text, ThemeScope, View, colorScheme, setColorScheme, signal } from "nsh";

function App() {
  const scheme = signal<"light" | "dark" | undefined>("dark");
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`Outside: ${colorScheme()}`}</Text>
      <View testID="outside" class="bg-white dark:bg-black" style={{ width: 40, height: 40 }} />
      <ThemeScope appearance={scheme()} testID="scope" class="bg-white dark:bg-black">
        <Text>{`Inside: ${colorScheme()}`}</Text>
        <View testID="inside" class="bg-white dark:bg-black" style={{ width: 40, height: 40 }} />
        <ThemeScope>
          <Text>{`Nested: ${colorScheme()}`}</Text>
        </ThemeScope>
      </ThemeScope>
      <View testID="app-dark" onPress={() => setColorScheme("dark")}>
        <Text>The app goes dark</Text>
      </View>
      <View testID="scope-light" onPress={() => scheme.set("light")}>
        <Text>The scope goes light</Text>
      </View>
      <View testID="scope-none" onPress={() => scheme.set(undefined)}>
        <Text>The scope follows</Text>
      </View>
      <View testID="app-system" onPress={() => setColorScheme("system")}>
        <Text>The app follows the system</Text>
      </View>
    </View>
  );
}

export default App;
