// import.meta.glob: the modules a pattern matches, imported lazily or up front.
import { signal, Text, View } from "nsh";

const lazy = import.meta.glob<{ name: string }>("./glob/*.ts");
const eager = import.meta.glob<string>("./glob/*.ts", { eager: true, import: "name" });

export default () => {
  const loaded = signal("");
  void Promise.all(Object.values(lazy).map((load) => load().then((m) => m.name))).then((names) =>
    loaded.set(names.join(",")),
  );
  return (
    <View>
      <Text>{`Glob keys: ${Object.keys(lazy).join(",")}`}</Text>
      <Text>{`Glob eager: ${Object.values(eager).join(",")}`}</Text>
      <Text>{`Glob loaded: ${loaded()}`}</Text>
    </View>
  );
};
