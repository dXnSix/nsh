// A case of the API's test suite (docs/dev/compatibility.md): `role()`
// gives what a theme role draws with, as `token()` gives a token. This app
// has no theme type, so a role is the token of its name: shadcn's `popover`
// in each appearance, and nothing for `warning` until the theme has a token
// of that name; a theme's `roles` maps a role onto a token of another name.
// Its script checks each.
import { Text, View, cleanup, resetTheme, role, setColorScheme, updateTheme } from "nsh";

function BaseRole() {
  // The next cases see nsh.json's theme in the system's appearance.
  cleanup(() => {
    resetTheme();
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`role popover ${role("popover")}`}</Text>
      <Text>{`role radius ${role("radius")}`}</Text>
      <Text>{`role warning ${role("warning") ?? "its own"}`}</Text>
      <View testID="themed" style={{ height: 16, backgroundColor: role("warning") ?? "#d97706" }} />
      <View testID="warn" onPress={() => updateTheme({ cssVars: { light: { warning: "#b45309" }, dark: { warning: "#fbbf24" } } })}>
        <Text>A warning color</Text>
      </View>
      <View
        testID="map"
        onPress={() => updateTheme({ cssVars: { light: { surface: "#fafafa" }, dark: { surface: "#262626" } }, roles: { popover: "surface" } })}
      >
        <Text>Map the popover's role</Text>
      </View>
      <View testID="dark" onPress={() => setColorScheme("dark")}>
        <Text>Go dark</Text>
      </View>
    </View>
  );
}

export default BaseRole;
