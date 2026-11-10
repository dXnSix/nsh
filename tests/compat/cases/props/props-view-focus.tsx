// A case of the API's test suite (docs/dev/compatibility.md): where the
// arrow keys take the keyboard focus (a TV's remote), as the platform
// receives it: `autoFocus` and `preferredFocus` on views, and the routes
// `nextFocusUp` / `nextFocusDown` / `nextFocusLeft` / `nextFocusRight`,
// refs to other views (one rendered after the view that names it, through
// a signal its `ref` sets). Its script checks each.
import { View, signal, type ViewRef } from "nsh";

function App() {
  const play = signal<ViewRef>();
  let resume: ViewRef | undefined;
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View testID="resume" ref={resume} autoFocus nextFocusDown={play()} onPress={() => {}} style={{ height: 20 }} />
      <View
        testID="play"
        ref={play}
        preferredFocus
        nextFocusUp={resume}
        nextFocusLeft={resume}
        nextFocusRight={resume}
        onPress={() => {}}
        style={{ height: 20 }}
      />
      <View testID="plain" onPress={() => {}} style={{ height: 20 }} />
    </View>
  );
}

export default App;
