// A case of the API's test suite (docs/dev/compatibility.md): the app's
// location, `/` headless; `navigate` resolves a URL against it and moves it
// at once, `history: "replace"` too, and throws a `TypeError` for another
// site's URL. Its script checks each.
import { Text, View, location, navigate, signal } from "nsh";

function App() {
  const refused = signal("none");
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`Location: ${location().url}`}</Text>
      <Text>{`Parts: ${location().pathname} ${location().search || "-"} ${location().hash || "-"}`}</Text>
      <Text>{`Refused: ${refused()}`}</Text>
      <View testID="teams" onPress={() => navigate("/teams/7")}>
        <Text>Teams</Text>
      </View>
      <View testID="table" onPress={() => navigate("../news?page=2#top", { history: "replace" })}>
        <Text>News</Text>
      </View>
      <View
        testID="other"
        onPress={() => {
          try {
            navigate("https://example.com/");
          } catch (e) {
            refused.set((e as Error).name);
          }
        }}
      >
        <Text>Other site</Text>
      </View>
    </View>
  );
}

export default App;
