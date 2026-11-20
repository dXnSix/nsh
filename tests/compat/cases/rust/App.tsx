// The rust domain of the API's test apps (docs/dev/compatibility.md): an
// app with a Rust crate of its own (`nsh.json` `"rust"`), whose UI set
// ships files. Each case is a module of its own, shown alone when the
// script taps `id:case-<case>` (a one-point view over the corner, out of
// the layout); the steps after it are that case's.
import { type JSX, View, mount, signal } from "nsh";
import { Window } from "nsh/desktop";
import SetAssetImports from "./set-asset-imports";

const cases: Record<string, () => JSX.Element> = {
  "set-asset-imports": SetAssetImports,
};
const shown = signal("");

mount(() => <Window id="main">{() => (
  <>
    {() => {
      const Case = cases[shown()];
      return Case ? <Case /> : null;
    }}
    <View style={{ position: "absolute", left: 0, top: 0, flexDirection: "row" }}>
      {Object.keys(cases).map(name => (
        <View testID={`case-${name}`} style={{ width: 1, height: 1 }} onPress={() => shown.set(name)} />
      ))}
    </View>
  </>
)}</Window>);
