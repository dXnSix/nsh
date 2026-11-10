// The TV shim of Tizen web apps (`shim.js`) on fake `tizen` globals and a
// fake page: `node --test crates/nsh-cli/src/tizen_web/shim.test.mjs`.
// The app stands in for `nsh.js`, whose window `keydown` listener takes
// `BrowserBack` when it goes back and `Escape` when it closes a dialog.
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const SHIM = await readFile(new URL("./shim.js", import.meta.url), "utf8");

class FakeEvent {
  constructor(type, init = {}) {
    Object.assign(this, { type, key: "", code: "", keyCode: 0, repeat: false, bubbles: false, cancelable: false }, init);
    this.defaultPrevented = false;
    this.stopped = false;
    this.stoppedNow = false;
  }
  preventDefault() { if (this.cancelable) this.defaultPrevented = true; }
  stopPropagation() { this.stopped = true; }
  stopImmediatePropagation() { this.stopped = this.stoppedNow = true; }
}

/** An event target with a parent: capture from the top, then bubbling. */
class Target {
  constructor(parent = null) {
    this.parent = parent;
    this.listeners = [];
  }
  addEventListener(type, fn, capture = false) {
    this.listeners.push({ type, fn, capture: capture === true || capture?.capture === true });
  }
  dispatchEvent(event) {
    const path = [];
    for (let at = this; at; at = at.parent) path.push(at);
    const call = (target, capture) => {
      for (const l of target.listeners) {
        if (l.type !== event.type || (l.capture !== capture && target !== this)) continue;
        l.fn(event);
        if (event.stoppedNow) return;
      }
    };
    for (const target of path.slice(1).reverse()) {
      call(target, true);
      if (event.stopped) return !event.defaultPrevented;
    }
    call(this, true);
    if (!event.bubbles || event.stopped) return !event.defaultPrevented;
    for (const target of path.slice(1)) {
      call(target, false);
      if (event.stopped) break;
    }
    return !event.defaultPrevented;
  }
}

/** A page with the shim on it: `tizen` (none: off a TV) and the app's
 * answer to each key (`takes(key)`: whether it goes back on it). */
function page({ tizen, takes = () => false } = {}) {
  const window = new Target();
  const document = { title: "Counter", activeElement: null };
  class Element extends Target {
    constructor(tag) {
      super(null);
      this.tag = tag;
      this.style = {};
      this.children = [];
      this.attributes = {};
      this.textContent = "";
    }
    setAttribute(name, value) { this.attributes[name] = value; }
    append(...children) {
      for (const child of children) {
        child.parent = this;
        this.children.push(child);
      }
    }
    remove() {
      this.parent.children = this.parent.children.filter((c) => c !== this);
      this.parent = null;
    }
    focus() { document.activeElement = this; }
    click() { this.dispatchEvent(new FakeEvent("click", { bubbles: true })); }
  }
  document.body = new Element("body");
  document.body.parent = window;
  document.createElement = (tag) => new Element(tag);
  const closed = [];
  Object.assign(window, { document, KeyboardEvent: FakeEvent, close: () => closed.push(true) });
  if (tizen) window.tizen = tizen;
  const heard = [];
  // `nsh.js`: hears keys at the window, takes the ones it goes back on.
  window.addEventListener("keydown", (e) => {
    heard.push(e.key || e.keyCode);
    if (takes(e.key)) e.preventDefault();
  });
  const context = vm.createContext(window);
  vm.runInContext("globalThis.globalThis = globalThis;", context);
  vm.runInContext(SHIM, context, { filename: "shim.js" });
  /** The remote's key `keyCode` going down and up where the focus is. */
  const press = (keyCode, key = "") => {
    const target = document.activeElement ?? document.body;
    const down = new FakeEvent("keydown", { key, keyCode, bubbles: true, cancelable: true });
    target.dispatchEvent(down);
    target.dispatchEvent(new FakeEvent("keyup", { key, keyCode, bubbles: true, cancelable: true }));
    return down;
  };
  const dialog = () => document.body.children.find((c) => c.attributes.role === "alertdialog");
  return { window, document, heard, press, dialog, closed, shim: window.nshTizen };
}

/** A TV's `tizen`: `supported` keys, `refused` ones that throw. */
function tv({ supported, refused = [] } = {}) {
  const registered = [];
  const exits = [];
  return {
    registered,
    exits,
    tvinputdevice: {
      getSupportedKeys: () => (supported ?? []).map((name) => ({ name, code: 0 })),
      registerKey(name) {
        if (refused.includes(name)) throw new Error(`NotSupportedError: ${name}`);
        registered.push(name);
      },
    },
    application: { getCurrentApplication: () => ({ exit: () => exits.push(true) }) },
  };
}

const ALL = [
  "ColorF0Red", "ColorF1Green", "ColorF2Yellow", "ColorF3Blue",
  "MediaPlayPause", "MediaPlay", "MediaPause", "MediaStop", "MediaRewind", "MediaFastForward",
  "MediaTrackNext", "MediaTrackPrevious", "ChannelUp", "ChannelDown", "Info",
];

test("the color, media and channel keys the TV has are registered", () => {
  // This model has no Info key and refuses ChannelDown.
  const tizen = tv({ supported: [...ALL.filter((k) => k !== "Info"), "Exit", "0"], refused: ["ChannelDown"] });
  const { shim } = page({ tizen });
  const expected = ALL.filter((k) => k !== "Info" && k !== "ChannelDown");
  assert.deepEqual(tizen.registered, expected);
  assert.deepEqual([...shim.keys], expected);
  assert.equal(shim.tv, true);
  // Exit and the digits are the system's: never registered.
  assert.ok(!tizen.registered.includes("Exit"));
});

test("a TV that cannot list its keys gets every key it takes", () => {
  const tizen = tv();
  tizen.tvinputdevice.getSupportedKeys = () => { throw new Error("no list"); };
  const { shim } = page({ tizen });
  assert.deepEqual([...shim.keys], ALL);
});

test("Back goes back in the app as BrowserBack", () => {
  const tizen = tv({ supported: ALL });
  const { press, heard, dialog } = page({ tizen, takes: (key) => key === "BrowserBack" });
  const down = press(10009, "XF86Back");
  // The remote's own key never reaches the app, only the back it means.
  assert.deepEqual(heard, ["BrowserBack"]);
  assert.equal(down.defaultPrevented, true);
  assert.equal(dialog(), undefined);
  assert.deepEqual(tizen.exits, []);
});

test("Back closes what an Escape closes when BrowserBack goes nowhere", () => {
  const tizen = tv({ supported: ALL });
  const { press, heard, dialog } = page({ tizen, takes: (key) => key === "Escape" });
  press(10009);
  assert.deepEqual(heard, ["BrowserBack", "Escape"]);
  assert.equal(dialog(), undefined);
});

test("Back on the first screen asks before exiting, and exits on Exit", () => {
  const tizen = tv({ supported: ALL });
  const { press, heard, dialog, document } = page({ tizen });
  press(10009);
  assert.deepEqual(heard, ["BrowserBack", "Escape"]);
  const shown = dialog();
  assert.ok(shown, "the exit confirmation shows");
  const [exit, cancel] = shown.children[0].children[1].children;
  assert.equal(shown.children[0].children[0].textContent, "Exit Counter?");
  assert.equal(document.activeElement, exit);
  // The arrows move between the buttons; the app hears none of it.
  press(39);
  assert.equal(document.activeElement, cancel);
  press(37);
  assert.equal(document.activeElement, exit);
  press(13, "Enter");
  assert.deepEqual(heard, ["BrowserBack", "Escape"]);
  assert.equal(dialog(), undefined);
  assert.deepEqual(tizen.exits, [true]);
});

test("Back or Cancel on the confirmation stays in the app", async () => {
  const tizen = tv({ supported: ALL });
  const { press, dialog, shim } = page({ tizen });
  press(10009);
  press(10009);
  assert.equal(dialog(), undefined);
  press(10009);
  press(39);
  press(13, "Enter");
  assert.equal(dialog(), undefined);
  // The web half of `nsh/tizen` asks the same way.
  const answer = shim.confirmExit();
  dialog().children[0].children[1].children[1].click();
  assert.equal(await answer, false);
  assert.deepEqual(tizen.exits, []);
});

test("off a TV nothing registers and exiting closes the window", () => {
  const { shim, press, closed, heard } = page({ takes: (key) => key === "BrowserBack" });
  assert.equal(shim.tv, false);
  assert.deepEqual([...shim.keys], []);
  press(10009);
  assert.deepEqual(heard, ["BrowserBack"]);
  shim.exit();
  assert.deepEqual(closed, [true]);
});

test("other keys reach the app as they are", () => {
  const { press, heard } = page({ tizen: tv({ supported: ALL }) });
  press(403, "ColorF0Red");
  press(38, "ArrowUp");
  assert.deepEqual(heard, ["ColorF0Red", "ArrowUp"]);
});
