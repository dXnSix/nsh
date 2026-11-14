// Stamps and instances on the browser host (`dev_source.js`), with the
// answers nsh-js gives for the same calls (`crates/nsh-js/src/dev_source.rs`'s
// tests): `node --test crates/nsh-platform-web/web/browser/dev_source.test.mjs`.
// A small reactive graph with owners and context stands in for the engine's.
import test from "node:test";
import assert from "node:assert/strict";
import { createDom } from "./dom.js";
import { createHot } from "./hot.js";
import { createDevSource } from "./dev_source.js";

// Owners with context and cleanups, computed values run at once: enough for
// instances provided by context and forgotten with their owner.
function fakeReactive() {
  let owner = { parent: null, context: new Map(), cleanups: [] };
  const run = (fn, child) => {
    const outer = owner;
    owner = child;
    try { return fn(); } finally { owner = outer; }
  };
  const lookup = (key, fallback) => {
    for (let o = owner; o; o = o.parent) if (o.context.has(key)) return o.context.get(key);
    return fallback;
  };
  const provide = (key, value) => owner.context.set(key, value);
  const cleanup = fn => { owner.cleanups.push(fn); return fn; };
  const child = () => ({ parent: owner, context: new Map(), cleanups: [] });
  const dispose = o => { for (const fn of o.cleanups.splice(0)) fn(); };
  const computed = fn => { const value = run(fn, child()); return () => value; };
  const untracked = fn => fn();
  // A scope of its own, disposed with `dispose()`.
  const scope = fn => { const o = child(); const value = run(fn, o); return { value, dispose: () => dispose(o) }; };
  const signal = value => { const read = () => value; read.set = next => { value = next; }; return read; };
  return { lookup, provide, cleanup, computed, untracked, signal, batch: fn => fn(), scope, getOwner: () => owner };
}

// A renderer whose nodes are plain objects with even ids, as the engine
// hands out.
function setup() {
  const reactive = fakeReactive();
  const values = { nodeIds: new WeakMap(), nodes: new Map(), signals: new WeakSet(), memos: new WeakSet() };
  let next = 2;
  const dom = createDom((op, args) => {
    if (op === 300) { const node = values.makeNode(next); next += 2; return node; }
    if (op === 301) { const node = values.makeNode(next); next += 2; return node; }
    return undefined;
  }, values, reactive, () => true, () => {});
  const devSource = createDevSource(reactive);
  dom.setDevSource(devSource);
  const hot = createHot(reactive, error => { throw error; }, devSource).hot;
  const id = node => values.nodeIds.get(node);
  return { reactive, values, dom, rt: dom.rt, hot, devSource, id };
}

const at = (module, anchor, line, column) => ({ module, anchor, line, column });

test("stamped elements say where they were written; text and unstamped nodes say nothing", () => {
  const { dom, rt, id } = setup();
  const a0 = rt.anchor("src/App.tsx", "App", 3);
  const a1 = rt.anchor("src/App.tsx", "Footer", 12);
  const view = rt.element("view", a0, 2, 7);
  const footer = rt.element("text", a1, 0, 30);
  const label = rt.text("hi");
  const plain = rt.element("view");
  assert.deepEqual(dom.nodeSource(id(view)), at("src/App.tsx", "App", 5, 7));
  assert.deepEqual(dom.nodeSource(id(footer)), at("src/App.tsx", "Footer", 12, 30));
  assert.equal(dom.nodeSource(id(label)), null);
  assert.equal(dom.nodeSource(id(plain)), null);
  assert.equal(dom.nodeSource(99_998), null);
});

test("an anchor run again moves its nodes", () => {
  const { dom, rt, id } = setup();
  const a0 = rt.anchor("m", "List", 4);
  const row = rt.element("view", a0, 3, 9);
  assert.deepEqual(dom.nodeSource(id(row)), at("m", "List", 7, 9));
  // The module runs again with the statement two lines lower: same anchor.
  assert.equal(rt.anchor("m", "List", 6), a0);
  assert.deepEqual(dom.nodeSource(id(row)), at("m", "List", 9, 9));
});

test("a native component's node takes its use's site; a hot component's body does not", () => {
  const { dom, rt, hot, id } = setup();
  const a0 = rt.anchor("main", "Rows", 2);
  const a1 = rt.anchor("main", "App", 20);
  // A native component: its node is made without a stamp.
  const List = () => rt.element("list");
  let unstamped, made;
  const Rows = hot.component("main", "Rows", () => {
    const v = rt.element("view", a0, 1, 10);
    // Made in the body, as a placeholder is: no site.
    made = rt.element("view");
    rt.component(List, {}, a0, 2, 5);
    unstamped = rt.component(List, {});
    return v;
  }, "rows");
  let lists = [];
  const NativeList = () => { const l = rt.element("list"); lists.push(l); return l; };
  const App = hot.component("main", "App", () => {
    const v = rt.element("view", a1, 1, 10);
    rt.component(Rows, {}, a1, 2, 7);
    rt.component(NativeList, {}, a1, 3, 5);
    rt.component(NativeList, {}, a1, 4, 5);
    return v;
  }, "app");
  rt.component(App, {});
  assert.equal(dom.nodeSource(id(made)), null);
  assert.equal(dom.nodeSource(id(unstamped)), null);
  assert.deepEqual(dom.nodeSource(id(lists[0])), at("main", "App", 23, 5));
  assert.deepEqual(dom.nodeSource(id(lists[1])), at("main", "App", 24, 5));
});

test("nodes name the instances that rendered them, innermost first, and props are read without getters", () => {
  const { dom, rt, hot, id } = setup();
  const a0 = rt.anchor("main", "Card", 2);
  const a1 = rt.anchor("main", "App", 9);
  let card, inner, renders = 0;
  const Card = hot.component("main", "Card", () => {
    card = rt.element("view", a0, 1, 10);
    // A nested, anonymous component folds into Card.
    inner = rt.component(() => rt.element("text"), {});
    return card;
  }, "card");
  const App = hot.component("main", "App", () => {
    const v = rt.element("view", a1, 1, 10);
    rt.component(Card, {
      title: "Hello", count: 3, on: true, none: null, onPress() {},
      get children() { renders++; return rt.element("text"); },
    }, a1, 2, 7);
    return v;
  }, "app");
  rt.component(App, {});
  const chain = dom.nodeInstances(id(card));
  // As nsh-js: Card used at App's anchor + 2, App mounted (no site).
  assert.deepEqual(chain.map(i => [i.key, i.callSite]), [
    ["main:Card", at("main", "App", 11, 7)],
    ["main:App", null],
  ]);
  assert.equal(dom.nodeInstances(id(inner))[0].key, "main:Card");
  assert.deepEqual(dom.instanceProps(chain[0].id), [
    ["title", { kind: "text", value: "Hello" }],
    ["count", { kind: "number", value: 3 }],
    ["on", { kind: "bool", value: true }],
    ["none", { kind: "null" }],
    ["onPress", { kind: "function" }],
    ["children", { kind: "getter" }],
  ]);
  assert.equal(renders, 0);
});

test("an instance goes with its owner", () => {
  const { dom, rt, hot, reactive, id } = setup();
  const a0 = rt.anchor("main", "Card", 2);
  let card;
  const Card = hot.component("main", "Card", () => (card = rt.element("view", a0, 1, 10)), "card");
  const shown = reactive.scope(() => rt.component(Card, { title: "x" }));
  const instance = dom.nodeInstances(id(card))[0].id;
  assert.notEqual(dom.instanceProps(instance).length, 0);
  shown.dispose();
  assert.deepEqual(dom.instanceProps(instance), []);
});

test("kept() lists the kept top-level values by module and name", () => {
  const { hot } = setup();
  const count = hot.keep("main", "count", "s1", { value: 3 });
  hot.keep("main", "nav", "s2", { routes: ["home"] });
  const kept = hot.kept();
  assert.deepEqual(Object.keys(kept), ["main:count", "main:nav"]);
  assert.equal(kept["main:count"], count);
});
