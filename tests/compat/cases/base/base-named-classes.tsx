// A case of the API's test suite (docs/dev/compatibility.md): a number token
// of a Tailwind 4 namespace is a named class, as a theme variable is in
// Tailwind. nsh.json's `"theme"` sets `radius-card` (14px), so `rounded-card`
// and its corner forms are that radius; Tailwind's own
// `rounded-lg` keeps its meaning. Its script checks each.
import { View } from "nsh";

function BaseNamedClasses() {
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <View testID="named" class="h-4 w-4 rounded-card" />
      <View testID="corner" class="h-4 w-4 rounded-tl-card" />
      <View testID="tailwind" class="h-4 w-4 rounded-lg" />
    </View>
  );
}

export default BaseNamedClasses;
