// The arrow keys' focus moves of the page (`nsh.js` `arrowFocus`) on a
// fake DOM: `node --expose-gc --test crates/nsh-platform-web/web/browser/focus.test.mjs`.
//
// Without a module the engine is a fake that records the page's description
// of its stops (what `nsh_focus_move` gets) and answers the stop each case
// expects, so the tests check what the page tells the core and how it
// applies the answer. With `NSH_BROWSER_WASM` (any build of
// `nsh-platform-web` for wasm32: "Browser-host regression checks" in
// docs/dev/web.md) the same cases run on the module's `nsh_focus_move` and
// `nsh_focus_reveal`, so `nsh_core::focus` decides. The decisions
// themselves are tested on the host: `cargo test -p nsh-platform-web --test
// text focus_moves`.

import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const source = await readFile(new URL("../nsh.js", import.meta.url), "utf8");
const { Player } = await import(`data:text/javascript,${encodeURIComponent(`${source}\nexport { Player };`)}`);

const path = process.env.NSH_BROWSER_WASM;
const module = path ? await WebAssembly.compile(await readFile(path)) : null;

/** The module's exports, its imports stubbed (the focus exports call none). */
async function realWasm() {
  const imports = {};
  for (const entry of WebAssembly.Module.imports(module)) (imports[entry.module] ??= {})[entry.name] = () => 0;
  return (await WebAssembly.instantiate(module, imports)).exports;
}

/** A fake engine: records each `nsh_focus_move` request and answers `answer()`. */
function fakeWasm(answer) {
  const memory = { buffer: new ArrayBuffer(1 << 16) };
  let next = 8;
  const requests = [];
  return {
    requests,
    memory,
    nsh_alloc(len) {
      const ptr = next;
      next += len;
      return ptr;
    },
    nsh_dealloc() {},
    nsh_key: () => -1,
    nsh_focus_move(ptr, len) {
      requests.push(JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len))));
      return answer() ?? -1;
    },
    // Centered vertically, as little as shows it horizontally (no margin
    // here: the module's adds the core's).
    nsh_focus_reveal(start, len, viewport, vertical) {
      if (start >= 0 && start + len <= viewport) return NaN;
      if (vertical) return start - (viewport - len) / 2;
      return start < 0 ? start : start + len - viewport;
    },
  };
}

/** An element of the fake DOM: a view when it has an `id`. */
class El {
  constructor({ id, rect = [0, 0, 0, 0], kind, stop, field, props, shown = true, horizontal, classes = [] } = {}, children = []) {
    if (id !== undefined) this._id = id;
    this._kind = kind;
    this._props = props;
    this._horizontal = horizontal;
    this._shown = shown;
    this.rect = rect;
    this.attributes = new Map(stop ? [["tabindex", "0"]] : []);
    this.tabIndex = stop ? 0 : -1;
    this._pressable = !!stop;
    this.classes = new Set(classes);
    this.classList = { contains: (name) => this.classes.has(name) };
    this.children = [];
    this.parentNode = null;
    if (field) {
      this._field = new El({ classes: ["nsh-field"], rect });
      Object.assign(this._field, { disabled: false, readOnly: false, ...field });
      this.add(this._field);
    }
    for (const child of children) this.add(child);
    this.scrollLeft = 0;
    this.scrollTop = 0;
    this.clientLeft = 0;
    this.clientTop = 0;
  }
  add(child) {
    child.parentNode = this;
    this.children.push(child);
    return child;
  }
  get parentElement() {
    return this.parentNode;
  }
  get offsetWidth() {
    return this.rect[2];
  }
  get offsetHeight() {
    return this.rect[3];
  }
  get clientWidth() {
    return this.rect[2];
  }
  get clientHeight() {
    return this.rect[3];
  }
  getAttribute(name) {
    return this.attributes.get(name) ?? null;
  }
  getBoundingClientRect() {
    // A scroll view's content moves by its ancestors' offsets.
    let [x, y] = this.rect;
    for (let p = this.parentNode; p; p = p.parentNode) {
      x -= p.scrollLeft;
      y -= p.scrollTop;
    }
    const [, , width, height] = this.rect;
    return { left: x, top: y, width, height, right: x + width, bottom: y + height };
  }
  checkVisibility() {
    for (let el = this; el; el = el.parentNode) if (el._shown === false) return false;
    return true;
  }
  contains(el) {
    for (; el; el = el.parentNode) if (el === this) return true;
    return false;
  }
  closest(selector) {
    const names = selector.split(",").map((s) => s.trim().replace(/^\./, ""));
    for (let el = this; el; el = el.parentNode) if (names.some((n) => el.classes.has(n))) return el;
    return null;
  }
  focus(options) {
    this.focusOptions = options;
    document.activeElement = this;
  }
  blur() {
    if (document.activeElement === this) document.activeElement = document.body;
  }
}

/** A player over `root` (the root view, 800 × 600) on the module or a fake answering `answer`. */
async function setup(children, answer = () => null) {
  const root = new El({ id: 1, rect: [0, 0, 800, 600] }, children);
  const page = new El({}, [root]);
  globalThis.document = { body: new El({}), activeElement: null };
  document.activeElement = document.body;
  globalThis.getSelection = () => null;
  const player = Object.create(Player.prototype);
  const wasm = module ? await realWasm() : fakeWasm(answer);
  const scrolls = [];
  Object.assign(player, {
    root,
    wasm,
    // The glide's offsets land at once.
    scrollSmoothly(el, dx, dy, smooth) {
      scrolls.push([el._id, dx, dy, smooth]);
      el.scrollLeft += dx;
      el.scrollTop += dy;
    },
    keyFocus() {},
    setState() {},
  });
  void page;
  return { player, root, wasm, scrolls, requests: wasm.requests ?? [] };
}

/** A key event of `key` on the page, which records whether it was prevented. */
function key(key, extra = {}) {
  return {
    key, code: key, repeat: false, isComposing: false, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false,
    defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; },
    stopPropagation() {},
    ...extra,
  };
}

/** A pressable stop at `[x, y, width, height]`. */
const button = (id, rect, extra = {}) => new El({ id, rect, stop: true, ...extra });

/** Presses `name` with the focus where it is; the view that has the focus after. */
function press(player, name) {
  const e = key(name);
  player.keyDown(e);
  return { e, focused: document.activeElement?._id ?? player.viewOf(document.activeElement)?._id };
}

test("Left and Right move along a row of stops, and the key scrolls at its end", async () => {
  const row = [button(10, [0, 0, 100, 40]), button(11, [120, 0, 100, 40]), button(12, [240, 0, 100, 40])];
  const answers = [11, 12, null];
  const { player, requests } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, row)], () => answers.shift());
  row[0].focus();
  let { e, focused } = press(player, "ArrowRight");
  assert.equal(focused, 11);
  assert.ok(e.defaultPrevented);
  // Moved without the browser's own scrolling, with the ring.
  assert.deepEqual(row[1].focusOptions, { focusVisible: true, preventScroll: true });
  ({ focused } = press(player, "ArrowRight"));
  assert.equal(focused, 12);
  // Nothing to the right: the key goes on to scrolling (none here).
  ({ e, focused } = press(player, "ArrowRight"));
  assert.equal(focused, 12);
  assert.equal(e.defaultPrevented, false);
  if (!module) {
    // What the core got: the stops in DOM order with their rectangles, the
    // focused one, the root's layers.
    const [first] = requests;
    assert.equal(first.key, "right");
    assert.equal(first.focused, 10);
    assert.deepEqual(first.modals, []);
    assert.deepEqual(first.stops.map((s) => [s.id, s.frame, s.visible, s.scope, s.layer, s.popup, s.menu, s.editable]), [
      [10, [0, 0, 100, 40], true, 1, 2, null, false, false],
      [11, [120, 0, 100, 40], true, 1, 2, null, false, false],
      [12, [240, 0, 100, 40], true, 1, 2, null, false, false],
    ]);
    assert.deepEqual(first.layers, [[1, [{ id: 2, frame: [0, 0, 800, 600], presses: false }]]]);
  }
});

test("Up and Down take the nearest stop in an overlapping column", async () => {
  const stops = [button(10, [0, 0, 100, 40]), button(11, [240, 0, 100, 40]), button(20, [0, 60, 100, 40]), button(21, [240, 60, 100, 40])];
  const answers = [21, 11, 10];
  const { player, requests } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, stops)], () => answers.shift());
  stops[1].focus();
  assert.equal(press(player, "ArrowDown").focused, 21);
  assert.equal(press(player, "ArrowUp").focused, 11);
  // Nothing focused: Down enters the first stop in sight.
  document.activeElement = document.body;
  assert.equal(press(player, "ArrowDown").focused, 10);
  if (!module) {
    assert.deepEqual(requests.map((r) => [r.key, r.focused]), [["down", 11], ["up", 21], ["down", null]]);
  }
});

test("the newest modal that shows is the scope; a leaving one is not", async () => {
  const pageStop = button(10, [0, 0, 100, 40]);
  const older = [button(40, [300, 200, 100, 40]), button(41, [420, 200, 100, 40])];
  const newer = button(50, [300, 300, 100, 40]);
  const modal = (id, stops) => new El({ id, kind: "modal", rect: [0, 0, 800, 600] }, [new El({ id: id + 1, rect: [0, 0, 800, 600] }, stops)]);
  const first = modal(30, older);
  const second = modal(35, [newer]);
  second._leaving = true;
  const answers = [41, null];
  const { player, requests } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, [pageStop]), first, second], () => answers.shift());
  older[0].focus();
  assert.equal(press(player, "ArrowRight").focused, 41);
  // Left of the modal's first stop is the page's: out of reach.
  older[0].focus();
  const { e, focused } = press(player, "ArrowLeft");
  assert.equal(focused, 40);
  assert.equal(e.defaultPrevented, false);
  if (!module) {
    const [request] = requests;
    assert.deepEqual(request.modals, [[30, true], [35, false]]);
    assert.deepEqual(request.stops.map((s) => [s.id, s.scope, s.layer]), [[10, 1, 2], [40, 30, 31], [41, 30, 31], [50, 35, 36]]);
    assert.deepEqual(request.layers.map(([scope, layers]) => [scope, layers.map((l) => l.id)]), [[1, [2]], [30, [31]], [35, [36]]]);
  }
});

test("a backdrop over the page takes the stops under it out of reach", async () => {
  const pageStop = button(10, [100, 300, 100, 40]);
  const backdrop = new El({ id: 3, rect: [0, 0, 800, 600], stop: false });
  // It closes the sheet when pressed.
  backdrop._pressable = true;
  const panel = [button(60, [100, 450, 100, 40]), button(61, [300, 450, 100, 40])];
  const answers = [null, 61];
  const { player, requests } = await setup([
    new El({ id: 2, rect: [0, 0, 800, 600] }, [pageStop]),
    backdrop,
    new El({ id: 4, rect: [0, 420, 800, 180] }, panel),
  ], () => answers.shift());
  panel[0].focus();
  // The page's stop above is under the backdrop.
  assert.equal(press(player, "ArrowUp").focused, 60);
  assert.equal(press(player, "ArrowRight").focused, 61);
  if (!module) {
    assert.deepEqual(requests[0].layers, [[1, [
      { id: 2, frame: [0, 0, 800, 600], presses: false },
      { id: 3, frame: [0, 0, 800, 600], presses: true },
      { id: 4, frame: [0, 420, 800, 180], presses: false },
    ]]]);
    assert.deepEqual(requests[0].stops.map((s) => [s.id, s.layer]), [[10, 2], [60, 4], [61, 4]]);
  }
});

test("the stop an arrow focuses comes into sight: its scroll view moves by the reveal delta", async () => {
  const items = [0, 1, 2, 3, 4, 5].map((i) => button(100 + i, [0, i * 100, 300, 40]));
  const list = new El({ id: 5, kind: "scroll", rect: [0, 0, 300, 200] }, items);
  list.scrollHeight = 600;
  list.scrollWidth = 300;
  const answers = [101, 102, 103];
  const { player, scrolls, requests } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, [list])], () => answers.shift());
  items[0].focus();
  // In sight already: no scroll.
  assert.equal(press(player, "ArrowDown").focused, 101);
  assert.deepEqual(scrolls, []);
  // Out of sight below: centered in the viewport, as the core reveals it.
  assert.equal(press(player, "ArrowDown").focused, 102);
  assert.deepEqual(scrolls, [[5, 0, 120, true]]);
  // Half out of sight below, where the scroll put it.
  assert.equal(press(player, "ArrowDown").focused, 103);
  assert.deepEqual(scrolls, [[5, 0, 120, true], [5, 0, 100, true]]);
  assert.equal(list.scrollTop, 220);
  if (!module) {
    // Out of sight is still a stop the arrows reach.
    assert.deepEqual(requests[1].stops.map((s) => [s.id, s.visible]),
      [[100, true], [101, true], [102, false], [103, false], [104, false], [105, false]]);
    // After the scroll the list's stops are where it shows them.
    assert.deepEqual(requests[2].stops.slice(2, 4).map((s) => [s.frame, s.visible]),
      [[[0, 80, 300, 40], true], [[0, 180, 300, 40], true]]);
  }
});

test("a text field keeps its arrows, and an arrow focuses a field as a stop", async () => {
  const before = button(10, [0, 0, 100, 40]);
  const input = new El({ id: 11, rect: [120, 0, 200, 40], field: {} });
  const answers = [11];
  const { player, requests } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, [before, input])], () => answers.shift());
  before.focus();
  assert.equal(press(player, "ArrowRight").focused, 11);
  assert.equal(document.activeElement, input._field);
  assert.deepEqual(input._field.focusOptions, { preventScroll: true });
  // In the field the arrows move its caret: nothing asks the core, nothing is prevented.
  for (const name of ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]) {
    const { e } = press(player, name);
    assert.equal(document.activeElement, input._field);
    assert.equal(e.defaultPrevented, false);
  }
  if (!module) {
    assert.equal(requests.length, 1);
    assert.deepEqual(requests[0].stops.map((s) => [s.id, s.editable]), [[10, false], [11, true]]);
  }
});

test("modifiers and Shift leave the arrows to shortcuts and selection", async () => {
  const row = [button(10, [0, 0, 100, 40]), button(11, [120, 0, 100, 40])];
  const { player, requests } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, row)], () => 11);
  row[0].focus();
  for (const mod of ["shiftKey", "ctrlKey", "metaKey"]) {
    const e = key("ArrowRight", { [mod]: true });
    player.keyDown(e);
    assert.equal(document.activeElement, row[0]);
  }
  if (!module) assert.equal(requests.length, 0);
});

test("Return presses the stop an arrow focused", async () => {
  const row = [button(10, [0, 0, 100, 40]), button(11, [120, 0, 100, 40])];
  const { player } = await setup([new El({ id: 2, rect: [0, 0, 800, 600] }, row)], () => 11);
  const pressed = [];
  player.pressIn = (p) => pressed.push(["in", p.press._id]);
  player.pressOut = (p, inside) => pressed.push(["out", p.press._id, inside]);
  row[0].focus();
  press(player, "ArrowRight");
  const { e } = press(player, "Enter");
  assert.ok(e.defaultPrevented);
  assert.deepEqual(pressed, [["in", 11], ["out", 11, true]]);
});
