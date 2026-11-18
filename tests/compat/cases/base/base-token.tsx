// A case of the API's test suite (docs/dev/compatibility.md): the design
// tokens of the app's theme (nsh.json `"theme"`, a theme document: nsh/base's
// default tokens with the app's own, `up` and `radius-card`). `token()` reads
// one at the contexts picked, in the theme format's form (a color as
// `#rrggbb`, a number); a class names one as Tailwind names a theme variable
// (`bg-up`, `bg-primary/50`, `text-up/50`, `rounded-(--radius-card)`). Its
// script checks each.
import { Text, View, token } from "nsh";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`token up ${token("up")}`}</Text>
      <Text>{`token primary ${token("primary")}`}</Text>
      <Text>{`token radius-card ${token("radius-card")}`}</Text>
      <View testID="up" class="h-4 bg-up rounded-(--radius-card)" />
      <View testID="primary" class="h-4 bg-primary/50" />
      <Text testID="faded" class="text-up/50">A token's color at half its alpha</Text>
    </View>
  );
}

export default App;
