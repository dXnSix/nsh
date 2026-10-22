// A case of the API's test suite (docs/dev/compatibility.md): a list in its
// parent's viewport (`viewport="parent"`): no scroll view of its own, laid
// out at its full length below a header, its rows rendered for the page's
// scroll view around it. Its script scrolls the page and checks the rows.
import { List, Scroll, Text, View } from "nsh";

const range = (n: number) => Array.from({ length: n }, (_, i) => i);

function App() {
  return (
    <Scroll testID="page" style={{ flex: 1 }}>
      <View style={{ height: 300 }}>
        <Text>Page header</Text>
      </View>
      <List viewport="parent" testID="rows" each={range(200)} estimatedItemSize={40}>
        {(item) => (
          <View testID={`row-${item()}`} style={{ height: 40 }}>
            <Text>Row {item()}</Text>
          </View>
        )}
      </List>
    </Scroll>
  );
}

export default App;
