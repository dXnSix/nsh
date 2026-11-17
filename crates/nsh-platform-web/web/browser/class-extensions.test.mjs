// Class extensions (`#[class_extension(C)]`) in the web player: the methods
// `describe` (op 200) lists with a set's extensions are on the class's
// prototype whichever is described first, and a call goes to Rust as op
// 210 with the extension's and the method's ids. With the wasm built with
// `browser-tests` (`NSH_BROWSER_WASM`), the fixture extension's calls run
// in Rust as the class's own do.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createModules } from "./modules.js";
import { Values } from "./values.js";
import { createReactive } from "./reactive.js";
import { createDom } from "./dom.js";
import { createComponents } from "./components.js";

const path = process.env.NSH_BROWSER_WASM;

/** A class `Builder` (id 4) of set `app`. */
const builderSet = () => ({
  specifier: "app", modules: [], classes: [{ id: 4, name: "Builder", interface: false, members: [
    { id: 0, name: "constructor", kind: "constructor", promise: false },
    { id: 1, name: "addStorage", kind: "method", promise: false },
  ] }],
});

/** Extension 7 of class 4, in set `i18n`. */
const i18nSet = () => ({
  specifier: "i18n", modules: [], classes: [], extensions: [{ id: 7, class: 4, members: [
    { id: 0, name: "addI18n", length: 1, promise: false },
    { id: 1, name: "load", length: 2, promise: true },
  ] }],
});

/** `createModules` over a fake `invoke` answering `describe` from `describes`, recording the other calls. */
const fake = (describes) => {
  const values = new Values();
  const calls = [];
  const invoke = (op, args) => {
    if (op === 200) return describes.shift() ?? [];
    calls.push([op, args]);
    return op === 202 ? undefined : "from Rust";
  };
  let live = true;
  const modules = createModules(invoke, values, () => live, () => {});
  return { modules, calls, values, close() { live = false; modules.dispose(); values.dispose(); } };
};

/** What a test checks of a method's property. */
const shape = (target, name) => {
  const d = Object.getOwnPropertyDescriptor(target, name);
  return d && [typeof d.value, d.value.name, d.value.length, d.writable, d.configurable, d.enumerable];
};

test("an extension's methods are on its class's prototype from the start", () => {
  const f = fake([[builderSet(), i18nSet()]]);
  const Builder = f.modules.exports.get("app").Builder;
  assert.deepEqual(shape(Builder.prototype, "addI18n"), ["function", "addI18n", 1, true, true, false]);
  assert.deepEqual(shape(Builder.prototype, "load"), ["function", "load", 2, true, true, false]);
  // The extending set exports nothing of the class.
  assert.deepEqual(Object.keys(f.modules.exports.get("i18n")), []);
  f.close();
});

test("an extension described before its class waits for it, and a gate's refresh brings one later", () => {
  // The extension first; the class after a gate ran.
  let f = fake([[i18nSet()], [builderSet()]]);
  f.modules.refresh();
  let Builder = f.modules.exports.get("app").Builder;
  assert.deepEqual(shape(Builder.prototype, "addI18n"), ["function", "addI18n", 1, true, true, false]);
  f.close();
  // The class first; the extension after a gate ran.
  f = fake([[builderSet()], [i18nSet()]]);
  Builder = f.modules.exports.get("app").Builder;
  assert.equal(shape(Builder.prototype, "addI18n"), undefined);
  f.modules.refresh();
  assert.deepEqual(shape(Builder.prototype, "load"), ["function", "load", 2, true, true, false]);
  f.close();
});

test("a call goes to Rust as op 210 with the extension's and the method's ids", async () => {
  const f = fake([[builderSet(), i18nSet()]]);
  const Builder = f.modules.exports.get("app").Builder;
  const builder = new Builder();
  assert.equal(builder.addI18n("hr"), "from Rust");
  assert.deepEqual(f.calls.at(-1), [210, [7, 0, builder, ["hr"], 0]]);
  // A method returning a promise: the reply settles it.
  const loaded = builder.load(1, 2);
  assert.ok(loaded instanceof Promise);
  const [op, [ext, member, receiver, args, id]] = f.calls.at(-1);
  assert.deepEqual([op, ext, member, receiver, args], [210, 7, 1, builder, [1, 2]]);
  assert.ok(id > 0);
  f.modules.reply(id, true, "loaded");
  assert.equal(await loaded, "loaded");
  // An instance a member returns has them too: its prototype is the class's.
  const returned = f.values.wrap(99, 4);
  assert.equal(returned.addI18n, builder.addI18n);
  f.close();
});

test("dispose forgets the extensions that wait for their class", () => {
  let live = true;
  const describes = [[i18nSet()], [builderSet()]];
  const values = new Values();
  const modules = createModules((op) => (op === 200 ? describes.shift() ?? [] : undefined), values, () => live, () => {});
  modules.dispose();
  // After the dispose, the class arrives: the extension waiting before is gone.
  modules.refresh();
  const Builder = modules.exports.get("app").Builder;
  assert.equal(shape(Builder.prototype, "addI18n"), undefined);
  live = false;
  values.dispose();
});

async function wasm() {
  const module = await WebAssembly.compile(await readFile(path));
  let bridge;
  const imports = {};
  for (const entry of WebAssembly.Module.imports(module)) (imports[entry.module] ??= {})[entry.name] = () => 0;
  imports.nsh_browser = { value: (...args) => bridge.value(...args) };
  const raw = (await WebAssembly.instantiate(module, imports)).exports;
  for (const name of Object.keys(raw)) if (/^nsh_(part|std|feature)_/.test(name)) raw[name]();
  const values = new Values();
  bridge = values.imports(() => raw.memory);
  const wasm = values.bind(raw);
  wasm.nsh_browser_initialize();
  let live = true;
  const invoke = (op, args) => values.boundary(() => values.with([args], ([id]) => values.result(wasm.nsh_browser_call(op, id))), () => raw.nsh_browser_trace());
  const api = createReactive(invoke, () => live, values);
  const dom = createDom(invoke, values, api, () => live, () => {});
  const components = createComponents(invoke, values, api, dom);
  const modules = createModules(invoke, values, () => live, () => {}, components.make);
  const close = () => { modules.dispose(); if (live) { wasm.nsh_browser_dispose(); live = false; values.dispose(); } };
  return { modules, close };
}

test("the fixture extension's methods run in Rust and chain on the same object", { skip: !path }, async () => {
  const f = await wasm();
  const Box = f.modules.exports.get("test/browser").CallbackBox;
  assert.ok(Box, "build the wasm with --features browser-tests");
  assert.deepEqual(shape(Box.prototype, "chain"), ["function", "chain", 0, true, true, false]);
  const box = new Box(() => 5);
  assert.equal(box.chain(), box);
  assert.equal(box.chain().chain().call(), 5);
  // The extending set exports nothing of the class.
  assert.equal(f.modules.exports.get("test/browser-extension")?.CallbackBox, undefined);
  f.close();
});

test("a callback is called on the spot and an Err is thrown with its message", { skip: !path }, async () => {
  const f = await wasm();
  const Box = f.modules.exports.get("test/browser").CallbackBox;
  const box = new Box(() => 20);
  assert.equal(box.callWith((own) => own + 1), 21);
  assert.throws(() => box.refuse("not now"), (e) => e instanceof Error && e.message === "not now");
  assert.throws(() => Box.prototype.chain.call({}), (e) => e instanceof TypeError && /CallbackBox\.chain/.test(e.message));
  // The object still works after a throw.
  assert.equal(box.chain().call(), 20);
  f.close();
});

test("an async method returns a promise, which a wrong this rejects", { skip: !path }, async () => {
  const f = await wasm();
  const Box = f.modules.exports.get("test/browser").CallbackBox;
  const box = new Box(() => 3);
  const later = box.later();
  assert.ok(later instanceof Promise);
  // Accepted: its task waits for the core (none runs here), so the promise
  // stays pending, where a refused call would have rejected already.
  const state = await Promise.race([
    later.then(() => "resolved", () => "rejected"),
    new Promise((resolve) => setTimeout(() => resolve("pending"), 20)),
  ]);
  assert.equal(state, "pending");
  // Disposing the app rejects it.
  later.catch(() => {});
  const refused = Box.prototype.later.call({});
  assert.ok(refused instanceof Promise);
  await assert.rejects(refused, (e) => e instanceof TypeError && /CallbackBox\.later/.test(e.message));
  f.close();
});
