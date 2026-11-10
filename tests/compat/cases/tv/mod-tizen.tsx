// A case of the API's test suite (docs/dev/compatibility.md): `nsh/tizen`
// off a Samsung TV (the compat runner has no TV host): `isAvailable()` is
// false and every call rejects with ERR_FEATURE_UNAVAILABLE_ON_PLATFORM.
// Its script checks them.
import { Text, View, signal } from "nsh";
import { exitApp, isAvailable, productInfo, registerKey, setPreviewData, unregisterKey } from "nsh/tizen";

function outcome(call: () => Promise<unknown>) {
  const result = signal("pending");
  call().then(
    () => result.set("resolved"),
    (error: { code?: string }) => result.set(error?.code ?? "rejected"),
  );
  return result;
}

function App() {
  const product = outcome(() => productInfo());
  const register = outcome(() => registerKey("red"));
  const unregister = outcome(() => unregisterKey("red"));
  const exit = outcome(() => exitApp());
  const preview = outcome(() => setPreviewData({ sections: [] }));
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`Tizen: ${isAvailable()}`}</Text>
      <Text>{`Product: ${product()}`}</Text>
      <Text>{`Register: ${register()}`}</Text>
      <Text>{`Unregister: ${unregister()}`}</Text>
      <Text>{`Exit: ${exit()}`}</Text>
      <Text>{`Preview: ${preview()}`}</Text>
    </View>
  );
}

export default App;
