// `nsh/tizen` in a Tizen web app, without a browser or a TV:
// `node --test crates/nsh-platform-web/web/browser/tizen.test.mjs`. A fake
// TV shim (`globalThis.nshTizen`, `nsh-cli`'s `tizen_web/shim.js`) stands in
// for `tizen.*` and `webapis.*`, a fresh `vm` context for the app's realm,
// and a fake `keymap.label` for the runtime's keystroke parser.
import test from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import { createTizen } from "./tizen.js";

/** The app's realm: its own `Promise`, `Error`s and `Object`. */
const realm = vm.runInNewContext("({ Promise, Error, TypeError, RangeError, Object })");

/** The runtime's own module, as on a page that is no TV's. */
const native = { isAvailable: () => false };

/** `keymap.label`: throws a `TypeError` for a name of no key. */
const KNOWN = new Set(["enter", "left", "escape", "a", "f5", "ctrl+red", "plus"]);
const label = (name) => {
  if (!KNOWN.has(name.toLowerCase())) throw new TypeError(`invalid keystroke ${name}`);
  return name;
};

/** A TV shim that records what it is asked; `refuses` keys the TV keeps. */
function shim({ refuses = [], product = {}, preview = null } = {}) {
  const calls = [];
  return {
    calls,
    tv: true,
    keys: [],
    productInfo: () => ({ model: null, platformVersion: null, build: null, manufacturer: null, ...product }),
    registerKey(name) {
      if (refuses.includes(name)) throw new Error(`NotSupportedError: ${name}`);
      calls.push(["registerKey", name]);
    },
    unregisterKey(name) { calls.push(["unregisterKey", name]); },
    exit() { calls.push(["exit"]); },
    setPreviewData(json) {
      calls.push(["setPreviewData", json]);
      return preview ? Promise.reject(new Error(preview)) : Promise.resolve();
    },
  };
}

/** The error a promise rejects with. */
async function rejection(promise) {
  try {
    await promise;
  } catch (error) {
    return error;
  }
  assert.fail("resolved");
}

test("on a page that is no TV's the module is the runtime's own", () => {
  assert.equal(createTizen(native, undefined, realm, label), native);
  assert.equal(createTizen(native, { tv: false }, realm, label), native);
});

test("in a .wgt the module is available and reports the TV", async () => {
  const tv = shim({ product: { model: "QN65S95D", platformVersion: "8.0", manufacturer: "Samsung" } });
  const tizen = createTizen(native, tv, realm, label);
  assert.equal(tizen.isAvailable(), true);
  const pending = tizen.productInfo();
  assert.ok(pending instanceof realm.Promise, "a promise of the app's realm");
  const info = await pending;
  assert.equal(Object.getPrototypeOf(info), realm.Object.prototype);
  assert.deepEqual({ ...info }, { model: "QN65S95D", platformVersion: "8.0", build: null, manufacturer: "Samsung" });
});

test("keys go to the TV by Tizen's names", async () => {
  const tv = shim();
  const tizen = createTizen(native, tv, realm, label);
  for (const name of ["red", "ColorF0Red", "mediaplaypause", "medianext", "mediatrackprevious", "channelup", "info"]) {
    assert.equal(await tizen.registerKey(name), undefined);
  }
  await tizen.unregisterKey("blue");
  assert.deepEqual(tv.calls, [
    ["registerKey", "ColorF0Red"], ["registerKey", "ColorF0Red"], ["registerKey", "MediaPlayPause"],
    ["registerKey", "MediaTrackNext"], ["registerKey", "MediaTrackPrevious"], ["registerKey", "ChannelUp"],
    ["registerKey", "Info"], ["unregisterKey", "ColorF3Blue"],
  ]);
});

test("names are checked as natively before the TV hears them", async () => {
  const tv = shim({ refuses: ["Info"] });
  const tizen = createTizen(native, tv, realm, label);
  for (const name of ["", "reddish", "ctrl+red"]) {
    const error = await rejection(tizen.registerKey(name));
    assert.ok(error instanceof realm.TypeError, `${name}: ${error}`);
  }
  for (const name of ["enter", "left", "escape", "a", "f5", "plus"]) {
    const error = await rejection(tizen.unregisterKey(name));
    assert.ok(error instanceof realm.RangeError, `${name}: ${error}`);
  }
  assert.deepEqual(tv.calls, []);
  // The TV's answer.
  const error = await rejection(tizen.registerKey("info"));
  assert.ok(error instanceof realm.Error);
  assert.equal(error.code, "ERR_FEATURE_UNAVAILABLE_ON_PLATFORM");
  assert.match(error.message, /^nsh\/tizen registerKey: NotSupportedError: Info$/);
});

test("exitApp leaves the app at once", async () => {
  const tv = shim();
  await createTizen(native, tv, realm, label).exitApp();
  assert.deepEqual(tv.calls, [["exit"]]);
});

test("setPreviewData hands Smart Hub the data as JSON", async () => {
  const tv = shim();
  const data = { sections: [{ title: "Continue", tiles: [{ title: "Episode 3", action_data: "{\"id\":3}" }] }] };
  await createTizen(native, tv, realm, label).setPreviewData(data);
  assert.deepEqual(tv.calls, [["setPreviewData", JSON.stringify(data)]]);
  const refused = shim({ preview: "no preview" });
  const error = await rejection(createTizen(native, refused, realm, label).setPreviewData({}));
  assert.ok(error instanceof realm.Error);
  assert.equal(error.code, "ERR_FEATURE_UNAVAILABLE_ON_PLATFORM");
  // Data that is no JSON.
  const cycle = {};
  cycle.self = cycle;
  assert.ok(await rejection(createTizen(native, tv, realm, label).setPreviewData(cycle)) instanceof realm.TypeError);
});
