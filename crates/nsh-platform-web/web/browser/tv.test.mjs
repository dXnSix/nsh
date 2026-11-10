// The TV category in the browser runtime without a browser:
// `node --expose-gc --test crates/nsh-platform-web/web/browser/tv.test.mjs`.
// `platform.tv` and `select`'s `tv` key (`environment.js`), `tv={{…}}`
// props (`dom.js`), and the page telling the player it is on a TV (`nsh.js`,
// `idiom: "tv"`), each held to what nsh-js and the engine do.
//
// Without a module the engine is a fake that answers a page's idiom with the
// environment the engine sends for it. With `NSH_BROWSER_WASM` (a
// `browser-tests` build of `nsh-platform-web` for wasm32: "Browser-host
// regression checks" in docs/dev/web.md) the module decides: the page's
// settings go to its `nsh_environment`, and the environment it sends the
// runtime at start (`nsh_browser_test_environment`) is the one checked.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { Values } from "./values.js";
import { createDom } from "./dom.js";
import { createEnvironment } from "./environment.js";

/** The environment on plain signals; `invoke` and the DOM do nothing. */
function environment() {
  const reactive = {
    signal(value) {
      const read = () => value;
      read.set = next => { value = next; };
      read.update = update => read.set(update(value));
      return read;
    },
    untracked: fn => fn(),
    batch: fn => fn(),
    context: fn => fn,
  };
  return createEnvironment(reactive, () => undefined, { rootProp() {} });
}

/** What the engine sends (`set_environment` in `src/browser/engine.rs`), with `platform`. */
const engine = platform => ({
  window: { width: 1920, height: 1080 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
  titleBar: { height: 0, left: 0, right: 0 }, keyboard: { height: 0, duration: 250 },
  colorScheme: "dark", fontScale: 1, rtl: false, reduceMotion: false, pixelRatio: 1, screenReader: false,
  locale: "en-US", host: false, lifecycle: "active",
  localeSettings: { languages: ["en-US"], timeZone: "UTC", hourCycle: "h12", firstDayOfWeek: 7,
    measurementSystem: "us", temperatureUnit: "fahrenheit" },
  platform: { os: "web", version: "0.0", appleKeyboard: false, ...platform },
});

const TV = { idiom: "tv", desktop: false, tv: true };

const path = process.env.NSH_BROWSER_WASM;
const module = path ? await WebAssembly.compile(await readFile(path)) : null;

/** The fake engine: the platform it reports for the page's `idiom`, as
 * `nsh_environment` reads it (anything unknown is a phone). */
function fakeEngine(settings) {
  const idiom = ["desktop", "pad", "tv"].includes(settings.idiom) ? settings.idiom : "phone";
  return engine({ idiom, desktop: idiom === "desktop", tv: idiom === "tv" });
}

/** The environment the engine sends the runtime of a page that told it
 * `settings` (`nsh_environment`), on the module or the fake. */
async function engineEnvironment(settings) {
  if (!module) return fakeEngine(settings);
  let bridge;
  const imports = {};
  for (const entry of WebAssembly.Module.imports(module)) (imports[entry.module] ??= {})[entry.name] = () => 0;
  imports.nsh_browser = { value: (...args) => bridge.value(...args) };
  const raw = (await WebAssembly.instantiate(module, imports)).exports;
  assert.equal(typeof raw.nsh_browser_test_environment, "function", "a browser-tests build");
  const values = new Values();
  bridge = values.imports(() => raw.memory);
  const wasm = values.bind(raw);
  wasm.nsh_browser_initialize();
  try {
    const bytes = new TextEncoder().encode(JSON.stringify(settings));
    const ptr = raw.nsh_alloc(bytes.length);
    new Uint8Array(raw.memory.buffer, ptr, bytes.length).set(bytes);
    raw.nsh_environment(ptr, bytes.length);
    return values.result(wasm.nsh_browser_test_environment());
  } finally {
    wasm.nsh_browser_dispose();
    values.dispose();
  }
}

test("platform.tv follows the engine, and select looks for tv after the OS's key and before desktop / mobile", () => {
  const env = environment();
  const { platform } = env.api;
  // Before the engine's environment: a desktop browser, not a TV.
  assert.equal(platform.tv, false);
  assert.equal(platform.select({ tv: "tv", desktop: "desktop" }), "desktop");

  env.update(engine(TV));
  assert.equal(platform.tv, true);
  assert.equal(platform.desktop, false);
  assert.equal(platform.idiom, "tv");
  assert.equal(platform.select({ web: "web", tv: "tv", mobile: "mobile" }), "web");
  assert.equal(platform.select({ tv: "tv", desktop: "desktop", mobile: "mobile" }), "tv");
  assert.equal(platform.select({ desktop: "desktop", mobile: "mobile" }), "mobile");
  assert.equal(platform.select({ tv: "tv" }), "tv");
  assert.equal(platform.select({ default: "default" }), "default");

  // Not a TV: the `tv` key is passed over.
  env.update(engine({ idiom: "phone", desktop: false, tv: false }));
  assert.equal(platform.tv, false);
  assert.equal(platform.select({ tv: "tv", mobile: "mobile" }), "mobile");
  assert.equal(platform.select({ tv: "tv" }), undefined);
  env.update(engine({ idiom: "desktop", desktop: true, tv: false }));
  assert.equal(platform.select({ tv: "tv", desktop: "desktop" }), "desktop");
});

test("a page's TV idiom reaches the engine, which tells the runtime it is a TV, and tv props apply", async () => {
  const data = await engineEnvironment({ idiom: "tv" });
  assert.equal(data.platform.idiom, "tv");
  assert.equal(data.platform.tv, true);
  assert.equal(data.platform.desktop, false);
  assert.equal(data.window.width, 1920);

  // The runtime takes it as `runtime.js` does: `platform` for the
  // environment, and the DOM's platform for platform props.
  const env = environment();
  env.update(data);
  assert.equal(env.api.platform.tv, true);
  assert.equal(env.api.platform.idiom, "tv");
  const values = new Values();
  const calls = [];
  const dom = createDom((op, args) => calls.push([op, args]), values, { getOwner: () => null }, () => true, () => {});
  dom.setPlatform(env.api.platform);
  const view = values.makeNode(2);
  dom.rt.prop(view, "tv", { style: { padding: 24 } });
  dom.rt.prop(view, "desktop", { cursor: "pointer" });
  assert.deepEqual(calls.filter(([op]) => op === 303).map(([, [, name, value]]) => [name, value]), [["style", { padding: 24 }]]);

  // A desktop browser's page is no TV.
  const desktop = await engineEnvironment({ idiom: "desktop" });
  assert.deepEqual([desktop.platform.idiom, desktop.platform.tv, desktop.platform.desktop], ["desktop", false, true]);
});

test("tv props set their props on a TV only, and unset the keys a new object no longer has", () => {
  const values = new Values();
  const calls = [];
  const dom = createDom((op, args) => calls.push([op, args]), values, { getOwner: () => null }, () => true, () => {});
  const platform = { os: "web", desktop: false, tv: true };
  dom.setPlatform(platform);
  const view = values.makeNode(2);
  const props = () => calls.filter(([op]) => op === 303).map(([, [, name, value]]) => [name, value]);

  // On a TV: the `tv` object and the `web` one, not the desktop's.
  dom.rt.prop(view, "tv", { style: { padding: 24 }, class: "text-2xl" });
  dom.rt.prop(view, "desktop", { cursor: "pointer" });
  dom.rt.prop(view, "web", { testID: "web" });
  assert.deepEqual(props(), [["style", { padding: 24 }], ["class", "text-2xl"], ["testID", "web"]]);

  // A key the new object no longer has is unset; a handler listens as the element's.
  calls.length = 0;
  dom.rt.prop(view, "tv", { style: { padding: 32 }, onPress: () => {} });
  assert.deepEqual(props(), [["class", undefined], ["style", { padding: 32 }]]);
  assert.deepEqual(calls.filter(([op]) => op === 305).map(([, [, name, on]]) => [name, on]), [["onPress", true]]);
  calls.length = 0;
  dom.rt.prop(view, "tv", null);
  assert.deepEqual(props(), [["style", undefined]]);

  // Not a TV (a phone, a desktop browser): `tv` applies nothing.
  for (const other of [{ os: "web", desktop: false, tv: false }, { os: "web", desktop: true, tv: false }]) {
    calls.length = 0;
    dom.setPlatform(other);
    dom.rt.prop(view, "tv", { style: { padding: 8 } });
    assert.deepEqual(props(), []);
  }
});

// nsh.js's player, from its source: `Player` is not exported.
const source = await readFile(new URL("../nsh.js", import.meta.url), "utf8");
const { Player } = await import(`data:text/javascript,${encodeURIComponent(`${source}\nexport { Player };`)}`);

/** A player on a fake engine with the page's `options`; `environment()` is
 * the real one, on a stand-in document. */
function fakePlayer(options) {
  const player = Object.create(Player.prototype);
  Object.assign(player, {
    options,
    hostEvents: [],
    reducedMotion: { matches: false },
    safeArea: () => [0, 0, 0, 0],
    keyboardHeight: () => 0,
    updateRootClasses() {},
    wasm: {},
  });
  return player;
}

/** Runs `fn` with the globals `environment()` reads standing in. */
function onPage(fn) {
  const names = ["document", "window", "getComputedStyle", "matchMedia", "devicePixelRatio"];
  const saved = Object.fromEntries(names.map(name => [name, globalThis[name]]));
  Object.assign(globalThis, {
    devicePixelRatio: 1,
    document: { documentElement: { getAttribute: () => null }, visibilityState: "visible", hasFocus: () => true },
    window: globalThis,
    getComputedStyle: () => ({ fontSize: "16px" }),
    // A mouse: the pointer heuristic alone would say "desktop".
    matchMedia: () => ({ matches: true, addEventListener() {} }),
  });
  try { return fn(); } finally {
    for (const [name, value] of Object.entries(saved)) {
      if (value === undefined) delete globalThis[name]; else globalThis[name] = value;
    }
  }
}

test("a page tells the player it is on a TV, and the engine reads the TV idiom", async () => {
  // The player sends the page's idiom, not the one the pointer suggests.
  const started = [];
  const player = fakePlayer({ idiom: "tv" });
  onPage(() => player.begin(1920, environment => { started.push(JSON.parse(environment)); return -1; }));
  assert.equal(started.length, 1);
  assert.equal(started[0].idiom, "tv");
  assert.equal(player.idiom, "tv");
  // What the player sends is what the engine reads: a TV.
  const sent = await engineEnvironment(started[0]);
  assert.deepEqual([sent.platform.idiom, sent.platform.tv, sent.platform.desktop], ["tv", true, false]);
  // A TV's system UI (dialogs) is not the desktop's.
  assert.equal(player.desktopUI, false);
  // Without it, the pointer decides.
  const measured = fakePlayer({});
  onPage(() => measured.begin(1920, () => -1));
  assert.equal(measured.idiom, "desktop");

  // Every idiom the player documents is one the engine reads and reports
  // back (`nsh_environment`, `Environment::reported_idiom`); an unknown one
  // is a phone.
  const documented = source.match(/`idiom` \(([^)]*)\)/)?.[1] ?? "";
  for (const idiom of ["phone", "pad", "desktop", "tv"]) {
    assert.ok(documented.includes(`"${idiom}"`), `nsh.js documents "${idiom}"`);
    assert.equal((await engineEnvironment({ idiom })).platform.idiom, idiom);
  }
  assert.equal((await engineEnvironment({ idiom: "watch" })).platform.idiom, "phone");
});
