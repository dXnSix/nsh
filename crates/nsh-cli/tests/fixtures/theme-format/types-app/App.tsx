// The theme types of `nsh` (`Theme`, `ThemePatch`, `ThemeDiagnostic`,
// `TokenKind`, `TokenDeclaration`, the typed `setModifier` and
// `modifiers()`) and of the app's `.nsh/types` (`ThemeModifiers`, `Tokens`
// and `Roles`, from `theme.json` and the UI sets' roles), type checked by
// `nsh check` (TS-TYPES). Each `@ts-expect-error` line must stay an error.
import {
  mount,
  modifier,
  modifiers,
  resetTheme,
  role,
  setModifier,
  setTheme,
  Text,
  token,
  tokens,
  updateTheme,
  View,
  type Roles,
  type Theme,
  type ThemeDiagnostic,
  type ThemeModifiers,
  type ThemePatch,
  type TokenDeclaration,
  type TokenKind,
  type Tokens,
} from "nsh";

// A document: tokens of each kind's values or references, a modifier, roles.
const corner: TokenDeclaration = { kind: "number", value: 6 };
const theme: Theme = {
  name: "Theme types",
  tokens: {
    brand: { kind: "color", value: "#123456", doc: "The brand's color." },
    corner,
    link: { kind: "color", value: "{brand}" },
    body: { kind: "typography", value: { fontSize: 14, lineHeight: "20px" } },
  },
  modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { brand: "#abcdef" } } } },
  resolutionOrder: ["colorMode"],
  roles: { primary: "brand" },
};
// @ts-expect-error A document has no `cssVars`: `nsh theme convert` converts an old theme.
const old: Theme = { cssVars: { light: { brand: "#123456" } } };

// `TokenKind` and `TokenDeclaration.kind` are the value types (B10).
const kinds: TokenKind[] = ["color", "number", "typography", corner.kind];
// @ts-expect-error A radius is a `number`: `radius` is no value type.
const radius: TokenKind = "radius";

// A patch: the document's tokens' values or references, role mappings.
const patch: ThemePatch = { values: { brand: "#654321", corner: 10, link: "{brand}" }, roles: { primary: "link" } };
// @ts-expect-error A patch has `values` and `roles`, and declares no token.
const declaring: ThemePatch = { tokens: { brand: { kind: "color", value: "#654321" } } };

// What the writes return.
function apply(): ThemeDiagnostic[] {
  const said: ThemeDiagnostic[] = [...setTheme(theme), ...updateTheme(patch), ...setTheme(null)];
  resetTheme(["brand", "primary"]);
  return said.filter((d) => d.severity === "error" && d.code !== "legacy-format" && d.pointer.startsWith("/"));
}

// The generated types: `Tokens` and `TokenKinds` of `theme.json`, `Roles`
// of the sets, `ThemeModifiers` of its modifiers.
const brand: Tokens["brand"] = token("brand");
const cornerValue: number = token("corner");
const cornerToken: number = tokens.get("corner").value;
const primary: Roles["primary"] | undefined = role("primary");
const roundness: number | undefined = role("radius");
// @ts-expect-error `radius` reads a number, not a color.
const radiusColor: string | undefined = role("radius");
const mode: ThemeModifiers["colorMode"] | undefined = modifier("colorMode");

setModifier("colorMode", "dark");
// @ts-expect-error `colorMode` has the contexts `light` and `dark`.
setModifier("colorMode", "sepia");

// `modifiers()` lists entries typed by `ThemeModifiers`: each one's name and
// context are what `setModifier` takes.
function pickDefaults() {
  for (const axis of modifiers()) {
    const name: keyof ThemeModifiers = axis.name;
    setModifier(axis.name, axis.default);
    void name;
  }
}

mount(() => (
  <View>
    <Text>{`${brand} ${cornerValue} ${cornerToken} ${primary} ${roundness} ${mode} ${kinds.join(",")}`}</Text>
    <Text>{`${apply().length} ${String(pickDefaults())} ${old.name} ${radius} ${declaring.values} ${radiusColor}`}</Text>
  </View>
));
