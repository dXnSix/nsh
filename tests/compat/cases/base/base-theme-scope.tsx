// A case of the API's test suite (docs/dev/compatibility.md): a
// `ThemeScope` with a `theme` draws what it holds with its values of the
// tokens it sets, over the app's (nsh.json's `"theme"` sets `up` here):
// `token()` and the classes naming them follow, in the scope's appearance,
// a page's `headerClass` among them;
// a token it leaves alone stays the app's; a scope inside it starts from
// its values; without a theme it is drawn with the app's. Its script checks
// each.
import { Text, ThemeScope, type Theme, View, signal, token } from "nsh";
import { NavPage, NavStack, createNavStack } from "nsh/base";

const pages = createNavStack<string>(["page"]);

const violet: Theme = { cssVars: { light: { up: "#7c3aed" }, dark: { up: "#a78bfa" } } };
const pink: Theme = { cssVars: { light: { up: "#db2777" }, dark: { up: "#f472b6" } } };

function BaseThemeScope() {
  const theme = signal<Theme | undefined>(violet);
  return (
    <View style={{ flex: 1, paddingTop: 64 }}>
      <Text>{`outside up ${token("up")}`}</Text>
      <View testID="outside" class="h-4 bg-up" />
      <ThemeScope theme={theme()}>
        <Text>{`inside up ${token("up")} radius-card ${token("radius-card")}`}</Text>
        <View testID="inside" class="h-4 bg-up" />
        <NavStack state={pages} style={{ height: 40 }}>
          {() => (
            <NavPage {...{ testID: "scoped-page" }} title="Scoped page" headerClass="bg-up">
              <Text>A page in the scope</Text>
            </NavPage>
          )}
        </NavStack>
        <ThemeScope appearance="dark">
          <Text>{`dark up ${token("up")}`}</Text>
          <View testID="dark" class="h-4 bg-up" />
        </ThemeScope>
      </ThemeScope>
      <View testID="pink" onPress={() => theme.set(pink)}>
        <Text>Another theme</Text>
      </View>
      <View testID="none" onPress={() => theme.set(undefined)}>
        <Text>No theme</Text>
      </View>
    </View>
  );
}

export default BaseThemeScope;
