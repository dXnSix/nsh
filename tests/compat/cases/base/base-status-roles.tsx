// A case of the API's test suite (docs/dev/compatibility.md): the status
// roles of `nsh/base`, `success` and `warning`, as a theme sets them. A
// `TextView`'s tip and warning alerts draw their own colors while the theme
// has no token of those names, the theme's once `setTheme` gives it a
// document with them, and their own again with nsh.json's theme back. Its
// script checks each.
import { Text, type Theme, View, cleanup, setTheme } from "nsh";
import { TextView } from "nsh/base";

const ALERTS = `> [!TIP]
> Tip text

> [!WARNING]
> Warning text`;

const status: Theme = {
  tokens: { success: { kind: "color", value: "#15803d" }, warning: { kind: "color", value: "#b45309" } },
  modifiers: {
    colorMode: { default: "light", contexts: { light: {}, dark: { success: "#4ade80", warning: "#fcd34d" } } },
  },
  resolutionOrder: ["colorMode"],
};

function BaseStatusRoles() {
  // The next cases see nsh.json's theme.
  cleanup(() => setTheme(null));
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <TextView markdown={ALERTS} />
      <View testID="status" onPress={() => setTheme(status)}>
        <Text>A theme with status colors</Text>
      </View>
      <View testID="built" onPress={() => setTheme(null)}>
        <Text>The theme it was built with</Text>
      </View>
    </View>
  );
}

export default BaseStatusRoles;
