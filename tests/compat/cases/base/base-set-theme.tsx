// A case of the API's test suite (docs/dev/compatibility.md): `setTheme`
// makes another theme document the app's while it runs, in place of
// nsh.json's `"theme"` (nsh/base's default tokens with `up` and
// `radius-card` here): documents are never merged, so a token only nsh.json's
// has is gone. `token()` reads it from the next turn on and classes naming
// its tokens follow, at the contexts picked, a page's `headerClass` among
// them. A bad entry is left out and returned as a diagnostic, and the rest of
// the document is the app's; a theme of the old format is refused
// (`legacy-format`) and the theme in force stays; `null` gives nsh.json's
// back. Its script checks each.
import { Text, type Theme, type ThemeDiagnostic, View, cleanup, setColorScheme, setModifier, setTheme, signal, token } from "nsh";
import { NavPage, NavStack, createNavStack } from "nsh/base";

const pages = createNavStack<string>(["page"]);

const brand: Theme = {
  tokens: { up: { kind: "color", value: "#2563eb" } },
  modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { up: "#93c5fd" } } } },
  resolutionOrder: ["colorMode"],
};
// A dark value that is no color: that entry is left out, the rest applies.
const bad: Theme = {
  tokens: { up: { kind: "color", value: "#16a34a" } },
  modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { up: "#zz" } } } },
  resolutionOrder: ["colorMode"],
};
// Each keeps `up` and has one bad entry, but the first: a theme of the old format.
const malformed = [
  ["bad-structure", '{"cssVars":null}'], // the old format, which `nsh theme convert` converts
  ["bad-function", '{"tokens":{"up":{"kind":"color","value":"#16a34a"},"down":{"kind":"color","value":"rgb(255 0 0"}}}'],
  ["bad-length", '{"tokens":{"up":{"kind":"color","value":"#16a34a"},"down":{"kind":"number","value":"12px"}}}'],
  ["bad-suffix", '{"tokens":{"up":{"kind":"color","value":"#16a34a"},"down":{"kind":"color","value":"#dc2626garbage"}}}'],
] as const;

/** The errors of `said`, each as its code and pointer, or `none`. */
function errors(said: ThemeDiagnostic[]): string {
  const out = said.filter((d) => d.severity === "error").map((d) => (d.pointer ? `${d.code} ${d.pointer}` : d.code));
  return out.length ? out.join(", ") : "none";
}

/** Shows `scheme`, and the theme's values for it: the appearance alone changes no token. */
function show(scheme: "light" | "dark") {
  setColorScheme(scheme);
  setModifier("colorMode", scheme);
}

function BaseSetTheme() {
  const result = signal("none");
  // The next cases see nsh.json's theme at its defaults in the system's appearance.
  cleanup(() => {
    setTheme(null);
    setModifier("colorMode", "light");
    setColorScheme("system");
  });
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`theme up ${token("up")}`}</Text>
      <Text>{`theme primary ${token("primary")}`}</Text>
      <Text>{`theme radius-card ${token("radius-card")}`}</Text>
      <Text>{`theme result ${result()}`}</Text>
      <View testID="themed" class="h-4 bg-up" />
      <NavStack state={pages} style={{ height: 40 }}>
        {() => (
          <NavPage {...{ testID: "page" }} title="Themed page" headerClass="bg-up">
            <Text>A page whose header names a token</Text>
          </NavPage>
        )}
      </NavStack>
      <View testID="set" onPress={() => result.set(`set ${errors(setTheme(brand))}`)}>
        <Text>Set a theme</Text>
      </View>
      <View testID="dark" onPress={() => show("dark")}>
        <Text>Go dark</Text>
      </View>
      <View testID="light" onPress={() => show("light")}>
        <Text>Go light</Text>
      </View>
      <View testID="bad" onPress={() => result.set(`bad ${errors(setTheme(bad))}`)}>
        <Text>Set a theme with a bad value</Text>
      </View>
      {malformed.map(([id, json]) => (
        <View testID={id} onPress={() => result.set(`${id} ${errors(setTheme(JSON.parse(json)))}`)}>
          <Text>{id}</Text>
        </View>
      ))}
      <View
        testID="built"
        onPress={() => {
          setTheme(null);
          result.set("built");
        }}
      >
        <Text>The theme it was built with</Text>
      </View>
    </View>
  );
}

export default BaseSetTheme;
