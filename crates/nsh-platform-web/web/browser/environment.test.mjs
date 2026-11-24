// The browser runtime's environment without nsh.wasm:
// `node --test crates/nsh-platform-web/web/browser/environment.test.mjs`.
// A stand-in reactive graph records what the environment writes.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createEnvironment } from "./environment.js";

/** The environment on signals that count their writes (`writes`); `invoke`
 * stands in for the engine's calls. */
function environment(invoke = () => undefined, graph = {}) {
  const writes = [];
  const reactive = {
    signal(value) {
      const read = () => value;
      read.set = next => { writes.push(next); value = next; };
      read.update = update => read.set(update(value));
      return read;
    },
    untracked: fn => fn(),
    batch: fn => fn(),
    context: fn => fn,
    ...graph,
  };
  const env = createEnvironment(reactive, invoke, { rootProp() {} });
  return { env, writes };
}

/** The locale settings the engine sends, `changes` over Vienna's. */
const settings = (changes = {}) => ({
  languages: ["de-AT", "en-US"], timeZone: "Europe/Vienna", hourCycle: "h23", firstDayOfWeek: 1,
  measurementSystem: "metric", temperatureUnit: "celsius", ...changes,
});

/** What the engine sends (`set_environment` in `src/browser/engine.rs`). */
const engine = (changes = {}) => ({
  horizontalSizeClass: "compact", verticalSizeClass: "regular",
  window: { width: 390, height: 844 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
  titleBar: { height: 0, left: 0, right: 0 }, keyboard: { height: 0, duration: 250 },
  colorScheme: "light", fontScale: 1, rtl: false, reduceMotion: false, pixelRatio: 2, screenReader: false,
  locale: "de-AT", host: false, lifecycle: "active", shaders: true, wakeCount: 0, localeSettings: settings(),
  network: { online: true, kind: "wifi", expensive: false, constrained: false, cellularData: "unknown" },
  platform: { name: "web", os: "web", version: "0.0", idiom: "desktop", desktop: true, appleKeyboard: false },
  ...changes,
});

/** The names a native module of nsh-js exports (`const <list>: &[Item]` in `src/env.rs`). */
function declared(source, list) {
  const start = source.indexOf(`pub(crate) const ${list}: &[Item] = &[`);
  assert.ok(start >= 0, `env.rs has no ${list}`);
  const items = source.slice(start, source.indexOf("\n];\n", start));
  return [...items.matchAll(/\(\s*"(\w+)",\s*Export::/g)].map(m => m[1]);
}

test("the environment serves every export of nsh/device and nsh/accessibility", async () => {
  const source = await readFile(new URL("../../../nsh-js/src/env.rs", import.meta.url), "utf8");
  const { env } = environment();
  const modules = { "nsh/device": declared(source, "DEVICE_EXPORTS"), "nsh/accessibility": declared(source, "ACCESSIBILITY_EXPORTS") };
  assert.ok(modules["nsh/device"].includes("lifecycle") && modules["nsh/accessibility"].includes("reduceMotion"), JSON.stringify(modules));
  // `runtime.js` serves `env.api` as `nsh`'s and `env.modules` as the
  // namespaces of those names: each has exactly what `env.rs` declares,
  // the same values as `nsh`'s.
  assert.deepEqual(Object.keys(env.modules ?? {}).sort(), Object.keys(modules).sort());
  for (const [specifier, names] of Object.entries(modules)) {
    const namespace = env.modules[specifier];
    assert.deepEqual(Object.keys(namespace).sort(), [...names].sort(), `the browser runtime's \`${specifier}\``);
    for (const name of names) {
      assert.ok(name in env.api, `the browser runtime's \`nsh\` has no \`${name}\``);
      assert.notEqual(namespace[name], undefined, `${specifier}'s \`${name}\``);
      assert.equal(namespace[name], env.api[name], `${specifier}'s \`${name}\` is nsh's`);
    }
  }
});

test("the browser reports no system wakes", () => {
  const { env } = environment();
  assert.equal(env.api.wakeCount(), 0);
  env.update(engine({ lifecycle: "background" }));
  env.update(engine({ lifecycle: "active" }));
  assert.equal(env.modules["nsh/device"].wakeCount(), 0);
});

test("setTheme and updateTheme hand the theme to the engine and return its diagnostics", () => {
  const calls = [];
  const said = [{ severity: "error", code: "bad-value", pointer: "/values/brand", message: "`brand`: #zz is no color" }];
  const document = { tokens: { brand: { kind: "color", value: "#e11d48" } } };
  const patch = { values: { brand: "#zz" } };
  const { env, writes } = environment((op, args) => {
    calls.push([op, args]);
    if (op === 406 && args[0] === 12) throw new TypeError("setTheme: expected a theme document (an object) or null");
    if (op === 406) return [];
    if (op === 409) return said;
  });
  assert.deepEqual(env.api.setTheme(document), []);
  assert.deepEqual(env.api.setTheme(null), []);
  assert.deepEqual(env.api.updateTheme(patch), said);
  assert.deepEqual(calls, [[406, [document]], [406, [null]], [409, [patch]]]);
  // The core applies them with the turn's ops: what read a token follows
  // once the engine hands the tokens on (`changedTokens`), not before.
  assert.deepEqual(writes, []);
  // The engine's refusal reaches the app.
  assert.throws(() => env.api.setTheme(12), TypeError);
});

test("resetTheme hands the names to the engine", () => {
  const calls = [];
  const { env, writes } = environment((op, args) => {
    calls.push([op, args]);
    if (op === 410 && typeof args[0] === "string") throw new TypeError("resetTheme: expected token names (string[]) or nothing");
  });
  assert.equal(env.api.resetTheme(["brand"]), undefined);
  env.api.resetTheme();
  assert.deepEqual(calls, [[410, [["brand"]]], [410, [undefined]]]);
  assert.deepEqual(writes, []);
  assert.throws(() => env.api.resetTheme("brand"), TypeError);
});

test("setModifier hands the context to the engine, and modifier and modifiers read what it keeps", () => {
  const calls = [];
  const { env, writes } = environment((op, args) => {
    calls.push([op, args]);
    if (op === 415 && args[1] === "jungle") throw new TypeError("setModifier: the modifier `brand` has no context `jungle`");
    if (op === 416) return "forest";
    if (op === 417) return JSON.stringify([{ name: "brand", contexts: ["ocean", "forest"], default: "ocean", selected: "forest" }]);
  });
  env.api.setModifier("brand", "forest");
  assert.deepEqual(writes, []);
  assert.equal(env.api.modifier("brand"), "forest");
  assert.deepEqual(env.api.modifiers(), [{ name: "brand", contexts: ["ocean", "forest"], default: "ocean", selected: "forest" }]);
  assert.deepEqual(calls, [[415, ["brand", "forest"]], [416, ["brand"]], [417, []]]);
  assert.throws(() => env.api.setModifier("brand", "jungle"), TypeError);
  assert.deepEqual(writes, []);
});

test("setup hands the app's theme to the engine as the bundle ships it", () => {
  const calls = [];
  const { env } = environment((op, args) => calls.push([op, args]));
  const theme = JSON.stringify({ document: { tokens: { brand: { kind: "color", value: "#e11d48" } } }, overrides: null });
  env.setup(null, theme, null, null);
  assert.deepEqual(calls, [[403, [theme]]]);
});

test("what read a token runs again only when that token changed, as the engine says", () => {
  // Signals that record who read them (`reads`) and which were written.
  let reads = null;
  const written = new Set();
  const { env } = environment(op => (op === 417 || op === 420 ? "[]" : undefined), {
    signal(value) {
      const read = () => { reads?.add(read); return value; };
      read.set = next => { written.add(read); value = next; };
      read.update = update => read.set(update(value));
      return read;
    },
  });
  const tracked = fn => { reads = new Set(); fn(); const out = reads; reads = null; return out; };
  const a = tracked(() => env.api.token("a")), b = tracked(() => env.api.role("b"));
  const listed = tracked(() => env.api.tokens.names()), picked = tracked(() => env.api.modifiers());
  const ran = signals => [...signals].some(signal => written.has(signal));
  const change = changes => { written.clear(); env.changedTokens(JSON.stringify(changes)); return [ran(a), ran(b), ran(listed), ran(picked)]; };
  // `a`'s value changed (and names nothing read).
  assert.deepEqual(change({ names: ["a", "c"], whole: false, shape: false }), [true, false, false, false]);
  // A role's name changes with the token it resolves to.
  assert.deepEqual(change({ names: ["b"], whole: false, shape: false }), [false, true, false, false]);
  // The modifiers or the names changed: what lists them runs.
  assert.deepEqual(change({ names: [], whole: false, shape: true }), [false, false, true, true]);
  // Roles mapped anew, or most of the names read: every reader.
  assert.deepEqual(change({ names: [], whole: true, shape: false }), [true, true, false, false]);
  assert.deepEqual(change({ names: ["a", "b"], whole: false, shape: false }), [true, true, false, false]);
  // No word of what changed: everything.
  written.clear();
  env.changedTokens();
  assert.deepEqual([ran(a), ran(b), ran(listed), ran(picked)], [true, true, true, true]);
});

test("tokens.* reads the token dictionary from the engine", () => {
  const calls = [];
  const { env } = environment((op, args) => {
    calls.push([op, args]);
    if (op === 418) return JSON.stringify(args[0] === "radius-card" ? { kind: "number", value: 12 } : null);
    if (op === 419) return JSON.stringify(args[0] === "radius-card" ? "number" : null);
    if (op === 420) return JSON.stringify(["radius-card"]);
    if (op === 421) return JSON.stringify([{ name: "radius-card", kind: "number" }]);
  });
  assert.deepEqual(env.api.tokens.get("radius-card"), { kind: "number", value: 12 });
  assert.equal(env.api.tokens.get("nothing"), undefined);
  assert.equal(env.api.tokens.kind("radius-card"), "number");
  assert.equal(env.api.tokens.kind("nothing"), undefined);
  assert.deepEqual(env.api.tokens.names({ kind: "number" }), ["radius-card"]);
  assert.deepEqual(env.api.tokens.entries(), [{ name: "radius-card", kind: "number" }]);
  assert.deepEqual(calls.map(([op]) => op), [418, 418, 419, 419, 420, 421]);
  assert.deepEqual(calls[4][1], [{ kind: "number" }, null]);
});

test("the lifecycle is the engine's", () => {
  const { env } = environment();
  // Before the engine reports, the native environment's default.
  assert.equal(env.api.lifecycle(), "active");
  env.update(engine({ lifecycle: "background" }));
  assert.equal(env.api.lifecycle(), "background");
  env.update(engine({ lifecycle: "inactive" }));
  assert.equal(env.api.lifecycle(), "inactive");
});

test("the network is the engine's", () => {
  const { env, writes } = environment();
  // Before the engine reports, the native environment's default: unknown,
  // which counts as online.
  assert.deepEqual(env.api.network(), {
    online: true, kind: "unknown", expensive: false, constrained: false, cellularData: "unknown",
  });
  env.update(engine());
  assert.equal(env.modules["nsh/device"].network().kind, "wifi");
  const offline = { online: false, kind: "none", expensive: false, constrained: false, cellularData: "unknown" };
  env.update(engine({ network: offline }));
  assert.deepEqual(env.api.network(), offline);
  // The same route again writes nothing.
  const before = writes.length;
  env.update(engine({ network: { ...offline } }));
  assert.equal(writes.length, before);
});

test("whether shaders draw is the engine's", () => {
  const { env } = environment();
  // Before the engine reports, the native environment's default.
  assert.equal(env.api.shadersAvailable(), true);
  env.update(engine());
  assert.equal(env.api.shadersAvailable(), true);
  env.update(engine({ shaders: false }));
  assert.equal(env.api.shadersAvailable(), false);
});

test("the locale settings are the engine's", () => {
  const { env } = environment();
  // Before the engine reports, the native environment's defaults: en-US's
  // in UTC.
  assert.deepEqual(env.api.localeSettings(), {
    languages: ["en-US"], timeZone: "UTC", hourCycle: "h12", firstDayOfWeek: 7,
    measurementSystem: "us", temperatureUnit: "fahrenheit",
  });
  env.update(engine());
  assert.deepEqual(env.api.localeSettings(), settings());
});

test("an appearance Rust set reaches colorScheme with nothing sent back, and a reset the system's", () => {
  const sent = [];
  const reactive = {
    signal(value) { const read = () => value; read.set = next => { value = next; }; read.update = update => read.set(update(value)); return read; },
    untracked: fn => fn(), batch: fn => fn(), context: fn => fn,
  };
  const env = createEnvironment(reactive, () => undefined, { rootProp: (name, value) => sent.push([name, value]) });
  env.update(engine({ colorScheme: "dark" }));
  assert.equal(env.api.colorScheme(), "dark");
  // The engine's `Host::set_appearance`: the core already shows it.
  env.changedAppearance("light");
  assert.equal(env.api.colorScheme(), "light");
  // The system's changing does not undo it.
  env.update(engine({ colorScheme: "dark" }));
  assert.equal(env.api.colorScheme(), "light");
  env.changedAppearance("system");
  assert.equal(env.api.colorScheme(), "dark");
  env.update(engine({ colorScheme: "light" }));
  assert.equal(env.api.colorScheme(), "light");
  // What is no appearance is ignored.
  env.changedAppearance("blue");
  assert.equal(env.api.colorScheme(), "light");
  // Only the direction went to the root: no appearance back to the engine.
  assert.deepEqual(sent.filter(([name]) => name === "appearance"), []);
});

test("a part is written only when it changed", () => {
  const { env, writes } = environment();
  env.update(engine());
  writes.length = 0;
  // The same values, in new objects and arrays, as each report has them.
  env.update(engine());
  assert.deepEqual(writes, []);
  env.update(engine({ lifecycle: "background" }));
  assert.deepEqual(writes, ["background"]);
  writes.length = 0;
  env.update(engine({ lifecycle: "inactive", locale: "fr-FR" }));
  assert.deepEqual(writes, ["fr-FR", "inactive"]);
  writes.length = 0;
  const tokyo = settings({ timeZone: "Asia/Tokyo" });
  env.update(engine({ lifecycle: "inactive", locale: "fr-FR", localeSettings: tokyo }));
  assert.deepEqual(writes, [tokyo]);
  // The same settings, in a new object with a new array of languages.
  writes.length = 0;
  env.update(engine({ lifecycle: "inactive", locale: "fr-FR", localeSettings: settings({ timeZone: "Asia/Tokyo" }) }));
  assert.deepEqual(writes, []);
  // A language fewer is a change, though the ones left are the same.
  const fewer = settings({ timeZone: "Asia/Tokyo", languages: ["de-AT"] });
  env.update(engine({ lifecycle: "inactive", locale: "fr-FR", localeSettings: fewer }));
  assert.deepEqual(writes, [fewer]);
});

test("setup hands over what appInfo reports, as the build wrote it", () => {
  const { env } = environment();
  assert.deepEqual(env.api.appInfo(), { id: null, name: null, version: null, build: null, development: false });
  env.setup(null, null, null, null, JSON.stringify({ id: "com.example.notes", name: "Notes", version: "1.2.0", build: 4, development: true }));
  assert.deepEqual(env.api.appInfo(), { id: "com.example.notes", name: "Notes", version: "1.2.0", build: 4, development: true });
  // The entry's own setup, without the fifth argument, keeps it.
  env.setup(null, null, null, null);
  assert.equal(env.api.appInfo().version, "1.2.0");
});

test("what a ThemeScope renders reads the scope's appearance and tokens", () => {
  // A context as the graph's: the value of the nearest provider around
  // what reads it. The engine's part (`src/browser/mod.rs`): a slot per
  // scope, and a name for the tokens made in it.
  const provided = [];
  const asked = [];
  const made = [];
  let slots = 0;
  const { env } = environment((op, args) => {
    if (op === 402) asked.push(args);
    if (op === 412) return slots++;
    if (op === 411) { made.push(args); return 1048576 + args[0]; }
  }, {
    // A memo that runs once, as a graph's with nothing changing.
    computed: fn => { let made = false, value; return () => { if (!made) { made = true; value = fn(); } return value; }; },
    cleanup() {},
    context(fallback) {
      if (fallback !== undefined) return fallback;
      const read = () => provided.at(-1);
      read.Provider = props => { provided.push(props.value); try { return props.children; } finally { provided.pop(); } };
      return read;
    },
  });
  const read = () => { env.api.token("up"); return env.api.colorScheme(); };
  env.update(engine());
  // No scope: the surface's.
  assert.equal(read(), "light");
  // In a dark scope, and in a scope without an appearance inside it.
  const props = { appearance: "dark" };
  assert.deepEqual(env.inScope(props, () => [read(), env.inScope({}, read)]), ["dark", "dark"]);
  // A scope without one of its own is the surface's, whatever it turns.
  env.update(engine({ colorScheme: "dark" }));
  assert.equal(env.inScope({}, read), "dark");
  assert.equal(env.inScope({ appearance: "light" }, read), "light");
  // `role()` asks as `token()` does, with its own call.
  const roles = [];
  const { env: other } = environment((op, args) => { if (op === 414) roles.push(args); });
  other.update(engine({ colorScheme: "dark" }));
  other.api.role("popover");
  assert.deepEqual(roles, [["popover", true, null]]);
  // `token()` asks for the side the scope shows, of the surface's tokens.
  assert.deepEqual(asked, [["up", false, null], ["up", true, null], ["up", true, null], ["up", true, null], ["up", false, null]]);
  // A scope with a theme: the engine makes its tokens over the surface's,
  // a scope in it over those, and `token()` names the nearest scope's.
  asked.length = 0;
  const violet = { values: { up: "#7c3aed" } }, pink = { values: { up: "#db2777" } };
  env.inScope({ theme: violet }, () => { read(); env.inScope({}, read); env.inScope({ theme: pink }, read); });
  assert.deepEqual(made, [[4, violet, null, null], [6, pink, 1048580, null]]);
  assert.deepEqual(asked.map(args => args[2]), [1048580, 1048580, 1048582]);
  // A scope with modifiers has tokens of its own too: the engine picks
  // its contexts, with a theme or without one.
  made.length = 0;
  asked.length = 0;
  const forest = { brand: "forest" };
  env.inScope({ modifiers: forest }, read);
  env.inScope({ theme: violet, modifiers: forest }, read);
  assert.deepEqual(made, [[7, null, null, forest], [8, violet, null, forest]]);
  assert.deepEqual(asked.map(args => args[2]), [1048583, 1048584]);
});

test("platform.select checks platform.name before platform.os and reads a built-in OS key once", () => {
  const { env } = environment();
  assert.equal(env.api.platform.name, "web");
  let reads = 0;
  assert.equal(env.api.platform.select({ get web() { reads += 1; return undefined; }, desktop: "d", default: "-" }), "d");
  assert.equal(reads, 1);
  assert.equal(env.api.platform.select({ web: null, default: "-" }), null);
  assert.equal(env.api.platform.select(null), undefined);

  env.update(engine({
    platform: { name: "miniprogram", os: "ios", version: "18.0", idiom: "phone", desktop: false, appleKeyboard: true },
  }));
  assert.equal(env.api.platform.name, "miniprogram");
  assert.equal(env.api.platform.select({ miniprogram: "mp", ios: "i", mobile: "m", native: "n", default: "d" }), "mp");
  assert.equal(env.api.platform.select({ miniprogram: undefined, ios: "i", mobile: "m", default: "d" }), "i");
  assert.equal(env.api.platform.select({ web: "w", mobile: "m", default: "d" }), "m");
  assert.equal(env.api.platform.select({ web: "w", native: "n", default: "d" }), "n");
});


test("size classes follow the engine, with no write for a resize inside one class", () => {
  const { env, writes } = environment();
  env.update(engine());
  assert.equal(env.api.horizontalSizeClass(), "compact");
  assert.equal(env.api.verticalSizeClass(), "regular");
  writes.length = 0;
  env.update(engine({ window: { width: 599, height: 844 } }));
  assert.deepEqual(writes, [{ width: 599, height: 844 }]);
  env.update(engine({ window: { width: 844, height: 390 }, horizontalSizeClass: "regular", verticalSizeClass: "compact" }));
  assert.equal(env.api.horizontalSizeClass(), "regular");
  assert.equal(env.api.verticalSizeClass(), "compact");
});
