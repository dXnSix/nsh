// Pressables as stops of the keyboard focus on the desktops: a field and
// three pressables, the first two in a row laid out in reverse, so their
// order on screen is not their order in the tree, and a view that takes no
// presses, which is no stop. `App.test.tsx` checks that Tab and Shift+Tab
// go through the stops in tree order, that the arrows move to the nearest
// stop in their direction (the field keeps Left and Right) and that Return
// and Space press the focused one.
import { mount, signal, Text, View } from "nsh";
import { Input } from "nsh/base";

function App() {
  const focused = signal("none");
  const pressed = signal("none");
  const name = signal("");

  const stop = (label: string) => (
    <View
      testID={label.toLowerCase()}
      onPress={() => pressed.set(label)}
      onFocus={() => focused.set(label)}
      onBlur={() => focused.set("none")}
      style={{ backgroundColor: "#dde3ea", borderRadius: 8, paddingVertical: 10, paddingHorizontal: 16 }}
    >
      <Text style={{ fontSize: 16 }}>{label}</Text>
    </View>
  );

  return (
    <View style={{ flex: 1, backgroundColor: "#ffffff", padding: 24, gap: 12 }}>
      <Text>{`Focused: ${focused()}`}</Text>
      <Text>{`Pressed: ${pressed()}`}</Text>
      <Text>{`Name: ${name()}`}</Text>
      <Input
        testID="name"
        placeholder="Name"
        onFocus={() => focused.set("Name")}
        onBlur={() => focused.set("none")}
        onChange={({ value }) => name.set(value)}
        style={{ height: 36 }}
      />
      <View style={{ flexDirection: "row-reverse", justifyContent: "flex-end", gap: 12 }}>
        {stop("First")}
        {stop("Second")}
      </View>
      {stop("Third")}
      <View style={{ paddingVertical: 10 }}>
        <Text>Not a stop</Text>
      </View>
    </View>
  );
}

mount(App);
