// An app built for TVs, run as on a TV on a desktop host (`nsh.json`
// `dev.idiom`, `--idiom tv`): it shows what `platform` reports, and a card
// whose label comes from its `tv={{…}}` props (a `desktop={{…}}` label
// beside them does not apply). `App.test.tsx` checks both.
import { mount, platform, Text, View } from "nsh";

function App() {
  return (
    <View style={{ flex: 1, backgroundColor: "#ffffff", padding: 48, gap: 16 }}>
      <Text>{`Idiom: ${platform.idiom}`}</Text>
      <Text>{`TV: ${platform.tv} Desktop: ${platform.desktop}`}</Text>
      <Text>{`Select: ${platform.select({ tv: "tv", desktop: "desktop", mobile: "mobile" })}`}</Text>
      <View
        testID="card"
        tv={{ accessibilityLabel: "On a TV", style: { padding: 24 } }}
        desktop={{ accessibilityLabel: "On a desktop" }}
        style={{ backgroundColor: "#dde3ea", borderRadius: 8, padding: 12 }}
      >
        <Text>Card</Text>
      </View>
    </View>
  );
}

mount(App);
