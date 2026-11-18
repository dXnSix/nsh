// A case of the API's test suite (docs/dev/compatibility.md): `role()`
// gives what a theme role draws with, as `token()` gives a token. A role the
// theme maps onto nothing is the token of its name: `popover` and `radius`
// of nsh.json's theme, and nothing for `warning`, which it has no token of,
// until the app's overrides map it (`updateTheme`'s `roles`) onto one. A
// mapping onto a token of another name is what a class reads as a variable
// (`bg-(--success)`: nothing drawn until the theme maps `success`), and it
// follows the contexts picked; `resetTheme` takes a role's name. Its script
// checks each.
import { Text, View, cleanup, resetTheme, role, setColorScheme, setModifier, updateTheme } from "nsh";

function BaseRole() {
  // The next cases see nsh.json's theme at its defaults in the system's appearance.
  cleanup(() => {
    resetTheme();
    setModifier("colorMode", "light");
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`role popover ${role("popover")}`}</Text>
      <Text>{`role radius ${role("radius")}`}</Text>
      <Text>{`role warning ${role("warning") ?? "its own"}`}</Text>
      <View testID="themed" style={{ height: 16, backgroundColor: role("warning") ?? "#d97706" }} />
      <View testID="classed" class="h-4 bg-(--success)" />
      <View testID="warn" onPress={() => updateTheme({ roles: { warning: "destructive" } })}>
        <Text>A warning color</Text>
      </View>
      <View testID="map" onPress={() => updateTheme({ roles: { popover: "muted", success: "muted" } })}>
        <Text>Map the popover's role</Text>
      </View>
      <View
        testID="dark"
        onPress={() => {
          setColorScheme("dark");
          setModifier("colorMode", "dark");
        }}
      >
        <Text>Go dark</Text>
      </View>
      <View testID="unwarn" onPress={() => resetTheme(["warning"])}>
        <Text>The theme's own warning color</Text>
      </View>
    </View>
  );
}

export default BaseRole;
