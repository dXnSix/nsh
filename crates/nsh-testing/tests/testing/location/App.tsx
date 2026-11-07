import { location, mount, navigate, Text, View } from "nsh";

function App() {
  const link = (label: string, go: () => void) => (
    <View onPress={go}>
      <Text>{label}</Text>
    </View>
  );
  return (
    <View style={{ flex: 1 }}>
      <Text>{`Location: ${location().url}`}</Text>
      <Text>{`Path: ${location().pathname} Query: ${location().search}`}</Text>
      {link("Teams", () => navigate("/teams"))}
      {link("Table", () => navigate("?tab=table", { history: "replace" }))}
    </View>
  );
}

mount(App);
