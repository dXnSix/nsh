// Where nodes were written and who rendered them, for dev tools (hot
// bundles only): the browser host's side of nsh-js `dev_source.rs`, with the
// same answers.
//
// The JSX compiler stamps hot bundles: each top-level statement with JSX
// registers an anchor, `anchor(module, statement, line)`, every time its
// module runs, and every element and component use carries `(anchor,
// lineDelta, column)`. A node keeps its stamp; its line is computed when
// asked, from its anchor's line now.
//
// A component use is pending while its component starts: a native
// component's host node takes it (`rt.hostElement`; a `Dialog` keeps it for
// the modal it makes when it opens), and so does the instance of a hot
// component, so nothing else made in its body gets it.
//
// Each render of a hot component is an instance (its `module:name` key, its
// call site and its props), provided to what it renders through the
// reactive graph's context operations (no operation of its own) and
// forgotten with the owner that rendered it. A node made under an instance
// keeps its id.

export function createDevSource(reactive) {
  // "module\0statement" -> anchor id; by id: { module, anchor, line }.
  const anchorIds = new Map();
  const anchors = [];
  // node -> { anchor, delta, column }.
  const stamps = new WeakMap();
  // node -> instance id.
  const nodeInstance = new WeakMap();
  // id -> { key, callSite, parent, props }.
  const instances = new Map();
  let lastInstance = 0;
  let pending = null;
  // The context key instances are provided under.
  const instanceKey = {};

  const stampOf = (anchor, delta, column) =>
    typeof anchor === "number" && typeof delta === "number" && typeof column === "number" && anchor >= 0 && anchor < anchors.length
      ? { anchor, delta, column: Math.max(0, column) }
      : null;
  const resolve = stamp => {
    const entry = stamp && anchors[stamp.anchor];
    return entry ? { module: entry.module, anchor: entry.anchor, line: Math.max(1, entry.line + stamp.delta), column: stamp.column } : null;
  };
  const currentInstance = () => {
    if (lastInstance === 0 || !reactive.lookup) return 0;
    const id = reactive.lookup(instanceKey, 0);
    return typeof id === "number" ? id : 0;
  };

  return {
    // `anchor(module, statement, line)`: the id of a statement's anchor, its
    // line now `line`.
    anchor(module, statement, line) {
      const key = `${module}\0${statement}`;
      let id = anchorIds.get(key);
      if (id === undefined) {
        id = anchors.length;
        anchors.push({ module: String(module), anchor: String(statement), line: 0 });
        anchorIds.set(key, id);
      }
      anchors[id].line = Math.max(1, Math.trunc(Number(line)) || 1);
      return id;
    },
    // A node just made: its stamp, when the call had one, and its instance.
    made(node, anchor, delta, column) {
      if (node === null || typeof node !== "object") return;
      const stamp = stampOf(anchor, delta, column);
      if (stamp) stamps.set(node, stamp);
      const instance = currentInstance();
      if (instance) nodeInstance.set(node, instance);
    },
    // A component use starts at `(anchor, delta, column)`: what to restore
    // when it returns (`undefined`: an unstamped use changes nothing).
    enterUse(anchor, delta, column) {
      const site = stampOf(anchor, delta, column);
      if (!site) return undefined;
      const before = pending;
      pending = site;
      return { before };
    },
    // The use returned: what was pending before it is again.
    leaveUse(entered) {
      if (entered) pending = entered.before;
    },
    // The pending site, taken (a native component's host node, or a
    // `Dialog`, which keeps it for its modal).
    takePending() {
      const site = pending;
      pending = null;
      return site;
    },
    // Stamps `node` with `site` (taken with `takePending`).
    stamp(node, site) {
      if (site && node !== null && typeof node === "object") stamps.set(node, site);
    },
    // A render of hot component `key` with `props`, used at the pending site
    // (taken: its body is not at its use's site).
    beginInstance(key, props) {
      const parent = currentInstance();
      const id = ++lastInstance;
      instances.set(id, { key, callSite: pending, parent, props });
      pending = null;
      return id;
    },
    // Provides instance `id` to what renders under the current owner.
    provideInstance(id) {
      reactive.provide?.(instanceKey, id);
    },
    endInstance(id) {
      instances.delete(id);
    },
    // Where `node` was written; `null` for text, nodes made without a stamp
    // and other values.
    nodeSource(node) {
      return resolve(node !== null && typeof node === "object" ? stamps.get(node) : null);
    },
    // The instances that rendered `node`, innermost first.
    nodeInstances(node) {
      const out = [];
      let next = node !== null && typeof node === "object" ? nodeInstance.get(node) ?? 0 : 0;
      while (next !== 0 && out.length < 1024) {
        const instance = instances.get(next);
        if (!instance) break;
        out.push({ id: next, key: instance.key, callSite: resolve(instance.callSite) });
        next = instance.parent;
      }
      return out;
    },
    // The props instance `id` was rendered with, read from their
    // descriptors: a getter is reported, never called.
    instanceProps(id) {
      const props = instances.get(id)?.props;
      if (props === null || typeof props !== "object") return [];
      return Object.keys(Object.getOwnPropertyDescriptors(props)).map(name => {
        const d = Object.getOwnPropertyDescriptor(props, name);
        if (d.get || d.set) return [name, { kind: "getter" }];
        const v = d.value;
        if (v === undefined) return [name, { kind: "undefined" }];
        if (v === null) return [name, { kind: "null" }];
        switch (typeof v) {
          case "boolean": return [name, { kind: "bool", value: v }];
          case "number": return [name, { kind: "number", value: v }];
          case "string": return [name, { kind: "text", value: v }];
          case "function": return [name, { kind: "function" }];
          default: return [name, { kind: "object" }];
        }
      });
    },
  };
}
