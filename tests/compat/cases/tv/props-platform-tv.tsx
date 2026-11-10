// A case of the API's test suite (docs/dev/compatibility.md): a script run
// as on an Android TV (`platform android tv`), where `tv={{…}}` props apply
// (any prop an element takes, here a label and a style; a prop is set in
// one place, as one set both ways is the one set last) and `desktop={{…}}`
// ones do not, beside a view without them. Its script checks each.
import { Text, View } from "nsh";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View
        testID="card"
        tv={{ accessibilityLabel: "On a TV", style: { height: 120, opacity: 0.5 } }}
        desktop={{ windowDrag: true }}
      >
        <Text>Card</Text>
      </View>
      <View testID="plain" style={{ height: 60 }}>
        <Text>Plain</Text>
      </View>
    </View>
  );
}

export default App;
