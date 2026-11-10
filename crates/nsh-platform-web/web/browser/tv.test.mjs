// The TV category in the browser runtime without a browser or nsh.wasm:
// `node --expose-gc --test crates/nsh-platform-web/web/browser/tv.test.mjs`.
// `platform.tv` and `select`'s `tv` key (`environment.js`), `tv={{…}}`
// props (`dom.js`), each held to what nsh-js and the engine do.
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

test("the engine tells the page's runtime whether it is a TV", async () => {
  // The field `environment.js` reads, from `Environment::tv` (nsh-core).
  const source = await readFile(new URL("../../src/browser/engine.rs", import.meta.url), "utf8");
  assert.match(source, /"tv":e\.tv\(\)/);
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
