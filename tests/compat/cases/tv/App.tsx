// The TV domain of the API's test apps (docs/dev/compatibility.md): each
// case is a module of its own, shown alone when the script taps
// `id:case-<case>` (a one-point view over the corner, out of the layout);
// the steps after it are that case's.
import { type JSX, View, mount, signal } from "nsh";
import ModDeviceTv from "./mod-device-tv";
import ModTizen from "./mod-tizen";
import PropsPlatformTv from "./props-platform-tv";

const cases: Record<string, () => JSX.Element> = {
  "mod-device-tv": ModDeviceTv,
  "mod-tizen": ModTizen,
  "props-platform-tv": PropsPlatformTv,
};
const shown = signal("");

mount(() => (
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
));
