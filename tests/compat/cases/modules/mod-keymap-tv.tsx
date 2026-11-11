// A case of the API's test suite (docs/dev/compatibility.md): the media and
// TV remote keys of `nsh/keymap` on the headless runtime: their labels, a
// binding of each name (and of the web's names, which are aliases), and the
// keys pressed firing their bindings. Its script checks each.
import { Text, View, keymap, signal } from "nsh";

const NAMES = [
  "mediaplaypause",
  "mediaplay",
  "mediapause",
  "mediastop",
  "mediarewind",
  "mediafastforward",
  "medianext",
  "mediaprevious",
  "red",
  "green",
  "yellow",
  "blue",
  "channelup",
  "channeldown",
  "info",
];

function App() {
  const pressed = signal("none");
  const bound = signal(0);
  const aliases = signal("not bound");
  for (const name of NAMES) {
    keymap.bind(name, ({ keystroke }) => pressed.set(keystroke));
    bound.set(bound() + 1);
  }
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`TV bound: ${bound()}`}</Text>
      <Text>{`TV pressed: ${pressed()}`}</Text>
      <Text>{`TV label: ${keymap.label("mediaplaypause")}, ${keymap.label("red")}, ${keymap.label("channelup")}`}</Text>
      <Text>{`TV aliases: ${aliases()}`}</Text>
      <View
        testID="aliases"
        onPress={() => {
          const unbind = keymap.bind({ mediatracknext: () => {}, colorf0red: () => {} });
          unbind();
          aliases.set(typeof unbind);
        }}
      >
        <Text>Aliases</Text>
      </View>
    </View>
  );
}

export default App;
