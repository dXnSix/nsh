// A case of the API's test suite (docs/dev/compatibility.md): paint classes
// that mean what they mean in a browser, as the platform receives them.
// `transform-none` clears the 3D rotations and skews, `translate-none`,
// `scale-none` and `rotate-none` their own properties, `rotate-z-*` comes
// after `rotate-x-*`; `borderStyle: "none"` / `"hidden"` (and
// `border-none`) leave no border whatever the widths; an outline style
// class wins over the width's solid, and alone is 3 points wide; a border
// without a color is the text color around it; every drop shadow layer is
// a filter; `perspective-*` is the children's, seen from the center;
// `selection:bg-*` is the selection highlight.
import { Input, Text, View } from "nsh";

function App() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>Tailwind paint</Text>
      <View testID="transform-none" class="size-4 translate-x-[8px] rotate-12 scale-90 rotate-x-45 skew-x-6 transform-none" />
      <View testID="property-none" class="size-4 translate-x-[8px] rotate-12 scale-90 rotate-x-45 translate-none scale-none rotate-none" />
      <View testID="rotate-z" class="size-4 rotate-z-30 rotate-x-45 rotate-12" />
      <View testID="border-none" style={{ height: 4, borderWidth: 3, borderStyle: "none" }} />
      <View testID="border-hidden" style={{ height: 4, borderStyle: "hidden", borderTopWidth: 2 }} />
      <View testID="border-class" class="h-1 border-none border-[3px]" />
      <View testID="outline-dashed" class="h-1 outline-dashed outline-2" />
      <View testID="outline-none" class="h-1 outline-none outline-2" />
      <View testID="outline-medium" class="h-1 outline-dotted" />
      <View class="text-[#ff0000]">
        <View testID="current" class="h-1 border-2" />
      </View>
      <View testID="drop-shadow" class="h-1 drop-shadow" />
      <View testID="perspective" class="perspective-[400px] w-[200px] h-[200px]">
        <View testID="perspective-child" class="size-[100px] rotate-y-45" />
      </View>
      <Input testID="selection" class="selection:bg-[#ff6600]" />
    </View>
  );
}

export default App;
