// Scrolls ending on engines without `scrollend` (Chromium before 114, the
// Chromium 108 of 2024's Samsung TVs): `node --expose-gc --test
// crates/nsh-platform-web/web/browser/scrollend.test.mjs`. `nsh.js` loads
// in a window without `onscrollend`, and a scroll the browser drives ends
// once it is still (`SCROLL_QUIET`), with `onMomentumEnd` after a glide;
// in a window with it, only the browser's `scrollend` ends a scroll.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = await readFile(new URL("../nsh.js", import.meta.url), "utf8");
const windowListeners = new Map();
const window = {
  addEventListener: (type, fn) => windowListeners.set(type, fn),
  removeEventListener: () => {},
};
globalThis.window = window;

/** `nsh.js`'s `Player`, loaded where the window has `onscrollend` or not
 * (each load a module of its own). */
async function load(scrollend) {
  if (scrollend) window.onscrollend = null;
  else delete window.onscrollend;
  const code = `${source}\nexport { Player };\n// ${scrollend}`;
  const { Player } = await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
  return Player;
}
const Without = await load(false);
const With = await load(true);

let clock = 0;

/** A vertical scroll view 300 × 200 with content 5000 tall, on a TV, and
 * the scroll events its app hears. */
function scroller(Player) {
  const p = Object.create(Player.prototype);
  p.idiom = "tv";
  p.root = { parentNode: null };
  p.size = [1920, 1080];
  p.wasm = { nsh_scroll: () => -1, nsh_scroll_ended: () => -1 };
  p.call = (f) => f(p.wasm);
  p.showBar = () => {};
  const heard = [];
  p.event = (id, name) => heard.push(name);
  const el = {
    _id: 1, _kind: "scroll", _applied: 0, parentNode: p.root,
    _events: new Set(["onScrollDragStart", "onScrollDragEnd", "onMomentumStart", "onMomentumEnd", "onScrollEnd"]),
    clientWidth: 300, clientHeight: 200, scrollWidth: 300, scrollHeight: 5000, scrollLeft: 0, scrollTop: 0,
    style: {},
  };
  p.views = new Map([[1, el]]);
  /** The browser moved the view by `dy` (its `scroll` event). */
  const scroll = (dy) => {
    el.scrollTop += dy;
    p.scrolled(el);
  };
  return { p, el, heard, scroll };
}

function mock(t) {
  t.mock.method(performance, "now", () => clock);
  t.mock.timers.enable({ apis: ["setTimeout"] });
  return (ms) => {
    clock += ms;
    t.mock.timers.tick(ms);
  };
}

test("a scroll the browser drives ends once it is still", (t) => {
  const tick = mock(t);
  const { heard, scroll } = scroller(Without);
  for (let i = 0; i < 10; i++) {
    scroll(20);
    tick(50);
  }
  // Moving all along: no end yet.
  assert.deepEqual(heard, []);
  tick(100);
  assert.deepEqual(heard, ["onScrollEnd"]);
  tick(1000);
  assert.deepEqual(heard, ["onScrollEnd"]);
});

test("a glide after the finger lifts ends with onMomentumEnd and onScrollEnd", (t) => {
  const tick = mock(t);
  const { p, el, heard, scroll } = scroller(Without);
  p.touchScroll({ scrolls: [el], startX: 0, startY: 0 }, [0, 40]);
  scroll(40);
  // A finger that holds still is no end.
  tick(400);
  assert.deepEqual(heard, ["onScrollDragStart"]);
  windowListeners.get("touchend")({ touches: [] });
  // The browser's glide: scroll events a frame apart.
  for (let i = 0; i < 30; i++) {
    tick(16);
    scroll(10);
  }
  tick(1000);
  assert.deepEqual(heard, ["onScrollDragStart", "onScrollDragEnd", "onMomentumStart", "onMomentumEnd", "onScrollEnd"]);
  assert.equal(el._momentum, false);
});

test("a held scroll bar ends when it is let go", (t) => {
  const tick = mock(t);
  const { p, el, heard, scroll } = scroller(Without);
  el._barHeld = true;
  scroll(100);
  tick(1000);
  assert.deepEqual(heard, []);
  p.releaseBar(el);
  tick(1000);
  assert.deepEqual(heard.filter((name) => name === "onScrollEnd"), ["onScrollEnd"]);
});

test("the browser's scrollend ends a scroll where it has one, and only once", (t) => {
  const tick = mock(t);
  for (const Player of [With, Without]) {
    const { p, el, heard, scroll } = scroller(Player);
    scroll(30);
    tick(50);
    p.scrollEnded(el);
    tick(1000);
    assert.deepEqual(heard, ["onScrollEnd"]);
  }
  // Where it has one, a still scroll waits for it.
  const { heard, scroll } = scroller(With);
  scroll(30);
  tick(1000);
  assert.deepEqual(heard, []);
});
