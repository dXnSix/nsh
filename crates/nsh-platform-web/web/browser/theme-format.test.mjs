// The web player reads and writes themes in the theme format
// (`docs/dev/theme-format.md`) with the same results as nsh-js: the calls
// and results of nsh-engine's `theme_format_ts` (`crates/nsh-engine/tests/
// engine/theme_format_ts.rs`), through the page's `browser/environment.js`
// and the module's ops 402 / 414 (`token`, `role`), 403 (the bundle's
// setup), 406 (`setTheme`), 409 (`updateTheme`), 410 (`resetTheme`), 411
// (a `ThemeScope`'s tokens), 415 to 417 (`setModifier`, `modifier`,
// `modifiers`) and 418 to 421 (`tokens.*`).
//
// `node --expose-gc --test crates/nsh-platform-web/web/browser/theme-format.test.mjs`.
// The module's tests need `NSH_BROWSER_WASM` (a `browser-tests` build of
// `nsh-platform-web` for wasm32: "Browser-host regression checks" in
// docs/dev/web.md), and skip without it. A turn ends as the engine ends one
// (`nsh_browser_test_turn`, `src/browser/fixtures.rs`): the writes of the
// turn reach a core with `nsh/base`'s sets, which resolves them once, and
// the tokens it resolved reach the page (`Host::set_tokens`), so a write's
// result is read after the turn, as in `theme_format_ts`.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { Values } from "./values.js";
import { createReactive } from "./reactive.js";
import { createEnvironment } from "./environment.js";

const path = process.env.NSH_BROWSER_WASM;
const module = path ? await WebAssembly.compile(await readFile(path)) : null;
const wasmOnly = { skip: !module };

/** The app's theme of `theme_format_ts` (`CONFIG`): primitives, semantic
 * tokens referring to them, one token of each value type, two modifiers
 * and two role mappings. */
const THEME = {
  name: "Test",
  tokens: {
    "gray-1": { kind: "color", value: "#fcfcfc" },
    "gray-12": { kind: "color", value: "#202020" },
    surface: { kind: "color", value: "{gray-1}", path: "surface.app", doc: "The app's background." },
    text: { kind: "color", value: "{gray-12}" },
    veil: { kind: "color", value: "#00000080" },
    space: { kind: "number", value: 12 },
    ratio: { kind: "number", value: 0.5 },
    "font-ui": { kind: "font", value: "Inter, system-ui, sans-serif" },
    label: { kind: "string", value: "OK" },
    beta: { kind: "boolean", value: true },
    fast: { kind: "duration", value: "0.15s" },
    ease: { kind: "easing", value: "ease-in" },
    body: { kind: "typography", value: { fontFamily: "{font-ui}", fontSize: 14, fontWeight: 400, lineHeight: "20px" } },
    lift: { kind: "shadow", value: [{ color: "#0000001a", y: 1, blur: 3 }] },
    hero: { kind: "gradient", value: { type: "linear", angle: 135, stops: [{ color: "#0090ff", position: 0 }, { color: "{gray-12}", position: 1 }] } },
    glass: { kind: "blur", value: { type: "background", radius: 16 } },
    edge: { kind: "border", value: { color: "{text}", width: 1, style: "dashed" } },
    press: { kind: "transition", value: { duration: "100ms", easing: [0.2, 0, 0, 1] } },
    settle: { kind: "transition", value: { duration: "200ms", easing: "ease-out" } },
  },
  modifiers: {
    colorMode: { default: "light", contexts: { light: {}, dark: { "gray-1": "#111111", "gray-12": "#eeeeee" } } },
    density: { default: "regular", contexts: { regular: {}, compact: { space: 8 } } },
  },
  resolutionOrder: ["density", "colorMode"],
  roles: { background: "surface", foreground: "text" },
};

/** `theme_format_ts`'s `with_overrides`: the app's `"themeOverrides"`. */
const OVERRIDES = { values: { label: "Yes" } };

/** An app on the module, as `runtime.js` starts one: the engine's first
 * tokens (the sets'), then the bundle's setup of the app's theme
 * (`{ document, overrides }`, run before any module of the app) and its turn,
 * unless `setup` is false. `turn()` ends a turn. */
async function start({ overrides = null, setup = true } = {}) {
  let bridge;
  const imports = {};
  for (const entry of WebAssembly.Module.imports(module)) (imports[entry.module] ??= {})[entry.name] = () => 0;
  imports.nsh_browser = { value: (...args) => bridge.value(...args) };
  const raw = (await WebAssembly.instantiate(module, imports)).exports;
  assert.equal(typeof raw.nsh_browser_test_turn, "function", "a browser-tests build");
  for (const name of Object.keys(raw)) if (/^nsh_(part|std|feature)_/.test(name)) raw[name]();
  const values = new Values();
  bridge = values.imports(() => raw.memory);
  const wasm = values.bind(raw);
  wasm.nsh_browser_initialize();
  let live = true;
  const invoke = (op, args) => values.boundary(() => values.with([args], ([id]) => values.result(wasm.nsh_browser_call(op, id))), () => raw.nsh_browser_trace());
  const reactive = createReactive(invoke, () => live, values);
  // The DOM's part (the appearance's root prop) is not the theme's.
  const env = createEnvironment(reactive, invoke, { rootProp() {} });
  // The engine's events, as `runtime.js` hands them on.
  invoke(400, [(name, args) => { if (name === "tokens") env.changedTokens(args[0]); }]);
  const turn = () => wasm.nsh_browser_test_turn();
  const setUp = () => {
    env.setup(null, JSON.stringify({ document: THEME, overrides }), null, null);
    turn();
  };
  turn();
  if (setup) setUp();
  const close = () => { if (live) { live = false; wasm.nsh_browser_dispose(); values.dispose(); } };
  return { api: env.api, env, reactive, turn, close };
}

/** What `f` throws, as `name: message`, or `"taken"`. */
const tried = f => {
  try { f(); return "taken"; } catch (e) { return `${e.name}: ${e.message}`; }
};

test("token() gives each value type in its canonical form", wasmOnly, async () => {
  const { api: { token }, close } = await start();
  try {
    const body = token("body") ?? {}, lift = token("lift") ?? [{}], hero = token("hero") ?? { stops: [] };
    const glass = token("glass") ?? {}, edge = token("edge") ?? {}, press = token("press") ?? { easing: [] };
    const settle = token("settle") ?? {};
    const shown = [
      `surface ${token("surface")}`,
      `veil ${token("veil")}`,
      `space ${token("space")} ${typeof token("space")}`,
      `ratio ${token("ratio")}`,
      `font ${token("font-ui")}`,
      `label ${token("label")}`,
      `beta ${token("beta")} ${typeof token("beta")}`,
      `fast ${token("fast")}`,
      `ease ${JSON.stringify(token("ease"))}`,
      `body ${body.fontFamily}|${body.fontSize}|${body.fontWeight}|${body.lineHeight}|${body.letterSpacing}`,
      `lift ${lift.length} ${lift[0].color} ${lift[0].x} ${lift[0].y} ${lift[0].blur}`,
      `hero ${hero.type} ${hero.angle} ${hero.stops.map(s => `${s.color}@${s.position}`).join(",")}`,
      `glass ${glass.type} ${glass.radius}`,
      `edge ${edge.color} ${edge.width} ${edge.style}`,
      `press ${press.duration} ${press.delay} ${press.easing.join(",")}`,
      `settle ${settle.duration} ${JSON.stringify(settle.easing)}`,
      `none ${token("nothing")}`,
    ].join("; ");
    assert.equal(shown,
      "surface #fcfcfc; veil #00000080; space 12 number; ratio 0.5; " +
      "font Inter, system-ui, sans-serif; label OK; beta true boolean; fast 150ms; " +
      'ease "ease-in"; body Inter, system-ui, sans-serif|14|400|20px|undefined; ' +
      "lift 1 #0000001a undefined 1 3; hero linear 135 #0090ff@0,#202020@1; " +
      "glass background 16; edge #202020 1 dashed; press 100ms undefined 0.2,0,0,1; " +
      'settle 200ms "ease-out"; none undefined');
  } finally { close(); }
});

test("a top-level read gives the app's document and overrides from the setup on", wasmOnly, async () => {
  const { api: { role, token, tokens }, env, turn, close } = await start({ setup: false });
  try {
    const read = () => [token("surface"), token("label"), role("background"), tokens.get("space")?.value, token("ease"),
      tokens.names().length].join(" ");
    // As the entry's module runs: its setup, then its top-level code,
    // before the core took the theme.
    env.setup(null, JSON.stringify({ document: THEME, overrides: OVERRIDES }), null, null);
    const top = read();
    turn();
    assert.deepEqual([top, read()], ["#fcfcfc Yes #fcfcfc 12 ease-in 19", "#fcfcfc Yes #fcfcfc 12 ease-in 19"]);
  } finally { close(); }
});

test("role() gives the token the document maps it onto, else the token of its name", wasmOnly, async () => {
  const { api: { role }, close } = await start();
  try {
    assert.equal(`background ${role("background")}; foreground ${role("foreground")}; primary ${role("primary")}`,
      "background #fcfcfc; foreground #202020; primary undefined");
  } finally { close(); }
});

test("the token dictionary gives kinds, values, names and entries", wasmOnly, async () => {
  const { api: { tokens }, close } = await start();
  try {
    const surface = tokens.get("surface") ?? {}, body = tokens.get("body") ?? { value: {} };
    const entry = tokens.entries().find(e => e.name === "surface") ?? {};
    assert.equal([
      `get ${surface.kind} ${surface.value}`,
      `body ${body.kind} ${body.value.fontSize}`,
      `missing ${tokens.get("nothing")}`,
      `kind ${tokens.kind("hero")} ${tokens.kind("nothing")}`,
      `grays ${tokens.names({ kind: "color", prefix: "gray" }).join(",")}`,
      `entry ${entry.kind} ${entry.path} ${entry.doc}`,
      `count ${tokens.names().length} ${tokens.entries().length}`,
    ].join("; "),
    "get color #fcfcfc; body typography 14; missing undefined; kind gradient undefined; " +
    "grays gray-1,gray-12; entry color surface.app The app's background.; count 19 19");
  } finally { close(); }
});

test("a reader runs again only for the names it read", wasmOnly, async () => {
  const { api: { token, updateTheme }, env, reactive, turn, close } = await start({ setup: false });
  try {
    const runs = { surface: 0, space: 0 };
    let dispose;
    // The entry's setup, then what renders in its turn.
    env.setup(null, JSON.stringify({ document: THEME, overrides: null }), null, null);
    reactive.root(stop => {
      dispose = stop;
      reactive.effect(() => { token("surface"); runs.surface += 1; });
      reactive.effect(() => { token("space"); runs.space += 1; });
    });
    // Once, with the app's document: the core's install of it changes
    // nothing.
    turn();
    const first = `${runs.surface} ${runs.space}`;
    updateTheme({ values: { space: 10 } });
    turn();
    // A primitive `surface` refers to: what read `surface` follows.
    updateTheme({ values: { "gray-1": "#ffffff" } });
    turn();
    // A token nothing here read.
    updateTheme({ values: { label: "Yes" } });
    turn();
    assert.equal(`${first} ${runs.surface} ${runs.space} ${token("surface")} ${token("space")}`, "1 1 2 2 #ffffff 10");
    dispose();
  } finally { close(); }
});

test("updateTheme returns what it leaves out, and the rest takes after the turn", wasmOnly, async () => {
  const { api: { token, updateTheme }, turn, close } = await start();
  try {
    const said = updateTheme({ values: { veil: "#ff0000", nothing: "#ffffff", space: "wide" } });
    const listed = said.map(d => `${d.severity} ${d.code} ${d.pointer}`).join(", ");
    // Read in the same turn: the values before it.
    const before = token("veil");
    turn();
    assert.equal(`${listed}; ${before}; veil ${token("veil")} space ${token("space")}`,
      "error undeclared-token /values/nothing, error bad-value /values/space; #00000080; veil #ff0000 space 12");
  } finally { close(); }
});

test("setTheme activates a document, returns its diagnostics, and setTheme(null) gives the built-in one back", wasmOnly, async () => {
  const { api: { setTheme, token, tokens }, turn, close } = await start();
  try {
    const said = setTheme({
      tokens: {
        surface: { kind: "color", value: "#123456" },
        bad: { kind: "dimension", value: 1 },
        wrong: { kind: "color", value: "white" },
      },
    });
    const errors = said.filter(d => d.severity === "error").map(d => `${d.code} ${d.pointer.split("/")[2]}`);
    turn();
    assert.equal(`${errors.join(", ")}; surface ${token("surface")} space ${token("space")} kind ${tokens.kind("bad")}`,
      "bad-kind bad, bad-value wrong; surface #123456 space undefined kind undefined");
    const back = setTheme(null);
    turn();
    assert.equal(`${back.length} ${token("surface")} ${token("space")}`, "0 #fcfcfc 12");
  } finally { close(); }
});

test("setTheme of an old cssVars theme keeps the active document", wasmOnly, async () => {
  const { api: { setTheme, token }, turn, close } = await start();
  try {
    const said = setTheme({ cssVars: { light: { surface: "#ff0000" } } });
    const listed = said.map(d => `${d.severity} ${d.code} "${d.pointer}" ${d.message.includes("nsh theme convert")}`).join(", ");
    turn();
    assert.equal(`${listed}; surface ${token("surface")}`, 'error legacy-format "" true; surface #fcfcfc');
  } finally { close(); }
});

test("setTheme and updateTheme throw a TypeError on anything but an object", wasmOnly, async () => {
  const { api: { setTheme, token, updateTheme }, turn, close } = await start();
  try {
    const out = [
      () => setTheme("{}"), () => setTheme(12), () => setTheme(true),
      () => updateTheme(null), () => updateTheme("{}"), () => updateTheme(undefined),
    ].map(tried);
    turn();
    const set = "TypeError: setTheme: expected a theme document (an object) or null";
    const update = "TypeError: updateTheme: expected a theme patch (an object)";
    assert.deepEqual(out, [set, set, set, update, update, update]);
    assert.equal(token("surface"), "#fcfcfc");
  } finally { close(); }
});

test("resetTheme removes names from the app's patch, themeOverrides' entries included", wasmOnly, async () => {
  const { api: { resetTheme, token, updateTheme }, turn, close } = await start({ overrides: OVERRIDES });
  try {
    const out = [`start ${token("label")}`];
    updateTheme({ values: { space: 10, veil: "#ff0000" } });
    turn();
    out.push(`updated ${token("space")} ${token("veil")}`);
    resetTheme(["space"]);
    turn();
    out.push(`one ${token("space")} ${token("veil")} ${token("label")}`);
    resetTheme();
    turn();
    out.push(`all ${token("space")} ${token("veil")} ${token("label")}`);
    out.push(tried(() => resetTheme("space")));
    assert.equal(out.join("; "),
      "start Yes; updated 10 #ff0000; one 12 #ff0000 Yes; all 12 #00000080 OK; " +
      "TypeError: resetTheme: expected token names (string[]) or nothing");
  } finally { close(); }
});

test("setModifier picks a context, and modifiers() lists them in resolutionOrder", wasmOnly, async () => {
  const { api: { modifier, modifiers, setModifier, token }, turn, close } = await start();
  try {
    const axes = () => modifiers().map(m => `${m.name}:${m.contexts.join("|")}:${m.default}:${m.selected}`).join(" ");
    const out = [`before ${axes()} ${modifier("colorMode")} ${token("surface")}`];
    setModifier("colorMode", "dark");
    turn();
    out.push(`after ${axes()} ${modifier("colorMode")} ${token("surface")} ${token("text")}`);
    assert.equal(out.join("; "),
      "before density:regular|compact:regular:regular colorMode:light|dark:light:light light #fcfcfc; " +
      "after density:regular|compact:regular:regular colorMode:light|dark:light:dark dark #111111 #eeeeee");
  } finally { close(); }
});

test("setModifier of what the active document lacks throws and changes nothing", wasmOnly, async () => {
  const { api: { modifier, setModifier, token }, turn, close } = await start();
  try {
    const out = [tried(() => setModifier("brand", "forest")), tried(() => setModifier("colorMode", "sepia"))];
    turn();
    out.push(`${modifier("colorMode")} ${token("surface")}`);
    assert.equal(out.join("; "),
      "TypeError: setModifier: the theme has no modifier `brand`; " +
      "TypeError: setModifier: the modifier `colorMode` has no context `sepia`; light #fcfcfc");
  } finally { close(); }
});

test("setModifier checks the document setTheme activated in the same turn", wasmOnly, async () => {
  const { api: { modifier, setModifier, setTheme, token }, turn, close } = await start();
  try {
    setTheme({
      tokens: { accent: { kind: "color", value: "#111111" } },
      modifiers: { brand: { default: "ocean", contexts: { ocean: { accent: "#0000ff" }, forest: { accent: "#00ff00" } } } },
      resolutionOrder: ["brand"],
    });
    const out = [tried(() => setModifier("brand", "forest")), tried(() => setModifier("colorMode", "dark"))];
    turn();
    assert.equal(`${out.join("; ")}; later ${token("accent")} ${modifier("brand")}`,
      "taken; TypeError: setModifier: the theme has no modifier `colorMode`; later #00ff00 forest");
  } finally { close(); }
});

test("setTheme keeps the picks the new document has, and overrides win over a picked context", wasmOnly, async () => {
  const { api: { modifier, setModifier, setTheme, token, updateTheme }, turn, close } = await start();
  try {
    updateTheme({ values: { "gray-1": "#abcdef" } });
    setModifier("colorMode", "dark");
    turn();
    assert.equal(`${token("surface")} ${token("text")}`, "#abcdef #eeeeee");
    const moded = {
      tokens: { surface: { kind: "color", value: "#fafafa" } },
      modifiers: { colorMode: { default: "light", contexts: { light: {}, dark: { surface: "#0a0a0a" } } } },
      resolutionOrder: ["colorMode"],
    };
    const out = [];
    setTheme(moded);
    turn();
    out.push(`kept ${modifier("colorMode")} ${token("surface")}`);
    setTheme({ tokens: { surface: { kind: "color", value: "#fafafa" } } });
    turn();
    out.push(`gone ${modifier("colorMode")} ${token("surface")}`);
    setTheme(moded);
    turn();
    out.push(`back ${modifier("colorMode")} ${token("surface")}`);
    assert.equal(out.join("; "), "kept dark #0a0a0a; gone undefined #fafafa; back light #fafafa");
  } finally { close(); }
});

test("a role-only patch reaches what read the role, once", wasmOnly, async () => {
  const { api: { role, token, updateTheme }, reactive, turn, close } = await start();
  try {
    let runs = 0, dispose;
    reactive.root(stop => { dispose = stop; reactive.effect(() => { role("background"); runs += 1; }); });
    const before = runs;
    const said = updateTheme({ roles: { background: "text" } });
    turn();
    assert.equal(`${said.length} ${role("background")} ${token("surface")} ${runs - before}`, "0 #202020 #fcfcfc 1");
    dispose();
  } finally { close(); }
});

test("a role mapped onto a token of the wrong type is returned and left out", wasmOnly, async () => {
  const { api: { role, updateTheme }, turn, close } = await start();
  try {
    const said = updateTheme({ roles: { background: "space", popover: "space" } });
    const listed = said.map(d => `${d.severity} ${d.code} ${d.pointer}`).join(", ");
    turn();
    assert.equal(`${listed}; ${role("background")} ${role("popover")}`,
      "error role-type /roles/background, error role-type /roles/popover; #fcfcfc undefined");
  } finally { close(); }
});

test("a ThemeScope reads its patch and its picks over the app's", wasmOnly, async () => {
  const { api: { role, token }, env, reactive, close } = await start();
  try {
    const shown = [];
    let dispose;
    reactive.root(stop => {
      dispose = stop;
      shown.push(`out ${token("surface")} ${token("text")} ${role("background")}`);
      env.inScope({ theme: { values: { surface: "#ff0000" } }, modifiers: { colorMode: "dark" } },
        () => shown.push(`in ${token("surface")} ${token("text")} ${role("background")}`));
      env.inScope({ theme: { roles: { background: "text" } } }, () => shown.push(`mapped ${role("background")}`));
    });
    assert.deepEqual(shown, ["out #fcfcfc #202020 #fcfcfc", "in #ff0000 #eeeeee #ff0000", "mapped #202020"]);
    dispose();
  } finally { close(); }
});

test("the web player has no cssVars theme left", async () => {
  // Its sources: the module's Rust and the page's JavaScript (not the
  // tests, which hand it an old theme to refuse).
  const crate = new URL("../../", import.meta.url);
  const files = [];
  const walk = async dir => {
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const url = new URL(entry.name + (entry.isDirectory() ? "/" : ""), dir);
      if (entry.isDirectory()) await walk(url);
      else if (/\.(rs|js|mjs)$/.test(entry.name) && !/\.test\.mjs$/.test(entry.name)) files.push(url);
    }
  };
  await walk(new URL("src/", crate));
  await walk(new URL("web/", crate));
  assert.ok(files.some(url => url.pathname.endsWith("/browser/environment.js")), "the page's sources");
  for (const url of files) {
    const text = await readFile(url, "utf8");
    assert.ok(!text.includes("cssVars"), `${url.pathname} names cssVars`);
  }
});
