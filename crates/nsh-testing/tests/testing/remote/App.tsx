// A TV app's keys, as `app.keyboard.remote` presses a remote's: bindings
// of the arrows and of every media and TV key, a view the focus is on for
// OK, and a screen to go back from.
import { keymap, mount, signal, Text, View, type ViewRef } from "nsh";
import { createNavStack, NavPage, NavStack } from "nsh/base";

type Route = { name: "home" } | { name: "detail" };
const nav = createNavStack<Route>([{ name: "home" }]);

const KEYS = [
  "up",
  "down",
  "left",
  "right",
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

const pressed = signal<string[]>([]);

function Press(props: { label: string; onPress: () => void; ref?: (view: ViewRef) => void }) {
  return (
    <View ref={props.ref} onPress={props.onPress}>
      <Text>{props.label}</Text>
    </View>
  );
}

function Home() {
  const played = signal(0);
  let play: ViewRef | undefined;
  return (
    <NavPage title="Home">
      <View style={{ flex: 1 }}>
        <Text>{`Pressed: ${pressed().join(" ")}`}</Text>
        <Text>{`Played: ${played()}`}</Text>
        <Press label="Play" ref={(view) => (play = view)} onPress={() => played.set(played() + 1)} />
        <Press label="Focus play" onPress={() => play?.focus()} />
        <Press label="Open" onPress={() => nav.push({ name: "detail" })} />
      </View>
    </NavPage>
  );
}

function App() {
  for (const key of KEYS) {
    keymap.bind(key, ({ keystroke }) => pressed.set([...pressed(), keystroke]));
  }
  return (
    <NavStack state={nav} style={{ flex: 1 }}>
      {(route) =>
        route.name === "home" ? (
          <Home />
        ) : (
          <NavPage title="Detail">
            <Text>Detail body</Text>
          </NavPage>
        )
      }
    </NavStack>
  );
}

mount(App);
