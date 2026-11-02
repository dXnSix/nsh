// The browser runtime's environment without nsh.wasm:
// `node --test crates/nsh-platform-web/web/browser/environment.test.mjs`.
// A stand-in reactive graph records what the environment writes.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createEnvironment } from "./environment.js";

/** The environment on signals that count their writes (`writes`). */
function environment() {
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
  };
  const env = createEnvironment(reactive, () => undefined, { rootProp() {} });
  return { env, writes };
}

/** What the engine sends (`set_environment` in `src/browser/engine.rs`). */
const engine = (changes = {}) => ({
  window: { width: 390, height: 844 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
  titleBar: { height: 0, left: 0, right: 0 }, keyboard: { height: 0, duration: 250 },
  colorScheme: "light", fontScale: 1, rtl: false, reduceMotion: false, pixelRatio: 2, screenReader: false,
  locale: "de-AT", host: false, languages: ["de-AT", "en-US"], hourCycle: "h23", firstDayOfWeek: 1,
  measurementSystem: "metric", temperatureUnit: "celsius", timeZone: "Europe/Vienna", appState: "active",
  platform: { os: "web", version: "0.0", idiom: "desktop", desktop: true, appleKeyboard: false },
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
  const names = [...declared(source, "DEVICE_EXPORTS"), ...declared(source, "ACCESSIBILITY_EXPORTS")];
  assert.ok(names.includes("timeZone") && names.includes("reduceMotion"), names.join());
  for (const name of names) assert.ok(name in env.api, `the browser runtime has no \`${name}\``);
});

test("the user's languages, regional preferences, time zone and app state are the engine's", () => {
  const { env } = environment();
  // Before the engine reports, the defaults of the native environment's.
  assert.equal(env.api.timeZone(), "UTC");
  assert.deepEqual(env.api.languages(), ["en-US"]);
  assert.equal(env.api.hourCycle(), "h12");
  assert.equal(env.api.appState(), "active");
  env.update(engine());
  assert.equal(env.api.timeZone(), "Europe/Vienna");
  assert.deepEqual(env.api.languages(), ["de-AT", "en-US"]);
  assert.equal(env.api.hourCycle(), "h23");
  assert.equal(env.api.firstDayOfWeek(), 1);
  assert.equal(env.api.measurementSystem(), "metric");
  assert.equal(env.api.temperatureUnit(), "celsius");
  assert.equal(env.api.appState(), "active");
});

test("a part is written only when it changed", () => {
  const { env, writes } = environment();
  env.update(engine());
  writes.length = 0;
  // The same values, in new objects and arrays, as each report has them.
  env.update(engine());
  assert.deepEqual(writes, []);
  env.update(engine({ appState: "background" }));
  assert.deepEqual(writes, ["background"]);
  assert.equal(env.api.appState(), "background");
  // A language fewer is a change, though the ones left are the same.
  writes.length = 0;
  env.update(engine({ appState: "background", languages: ["de-AT"] }));
  assert.deepEqual(writes, [["de-AT"]]);
  writes.length = 0;
  env.update(engine({ appState: "inactive", timeZone: "Asia/Tokyo", languages: ["de-AT"] }));
  assert.deepEqual(writes, ["Asia/Tokyo", "inactive"]);
});
