// A case of the API's test suite (docs/dev/compatibility.md): the diff
// roles of `nsh/base`, `diff-added` and `diff-deleted`, as a theme sets
// them. A `DiffView`'s `+` and `−` draw the view's own green and red while
// the theme has no token of those names, the theme's once `setTheme` gives
// it them, and their own again with nsh.json's theme back. Its script
// checks each.
import { Text, View, cleanup, setTheme } from "nsh";
import { DiffView } from "nsh/base";

const diff = {
  cssVars: {
    light: { "diff-added": "#0f766e", "diff-deleted": "#be123c" },
    dark: { "diff-added": "#5eead4", "diff-deleted": "#fda4af" },
  },
};

function BaseDiffRoles() {
  // The next cases see nsh.json's theme.
  cleanup(() => setTheme(null));
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View style={{ width: 390, height: 120 }}>
        <DiffView
          oldValue={"keep\nold line\nkeep"}
          newValue={"keep\nnew line\nkeep"}
          layout="unified"
          indicators="classic"
          style={{ flex: 1 }}
        />
      </View>
      <View testID="diff" onPress={() => setTheme(diff)}>
        <Text>A theme with diff colors</Text>
      </View>
      <View testID="built" onPress={() => setTheme(null)}>
        <Text>The theme it was built with</Text>
      </View>
    </View>
  );
}

export default BaseDiffRoles;
