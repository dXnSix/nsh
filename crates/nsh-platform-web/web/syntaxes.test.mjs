// How nsh.js fetches the syntax highlighting data the module asks for
// (`want_syntaxes`), without a browser or nsh.wasm:
// `node --test crates/nsh-platform-web/web/syntaxes.test.mjs`.

import assert from "node:assert/strict";
import { mkdtemp, readFile, realpath, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";

// From a file, as in the browser: the default URL is relative to nsh.js.
const source = await readFile(new URL("nsh.js", import.meta.url), "utf8");
const copy = join(await realpath(await mkdtemp(join(tmpdir(), "nsh-syntaxes-"))), "nsh.js");
await writeFile(copy, `${source}\nexport { Player };`);
const { Player } = await import(pathToFileURL(copy).href);

/** A player on a fake engine; `answers` holds what `nsh_syntaxes` got (null: nothing). */
function fakePlayer(options) {
  const answers = [];
  const memory = { buffer: new ArrayBuffer(1 << 16) };
  let next = 8;
  const player = Object.create(Player.prototype);
  let answered;
  const done = new Promise((resolve) => (answered = resolve));
  Object.assign(player, {
    options,
    schedule() {},
    wasm: {
      memory,
      nsh_alloc(len) {
        const ptr = next;
        next += len;
        return ptr;
      },
      nsh_dealloc() {},
      nsh_syntaxes(ptr, len) {
        answers.push(len === 0 ? null : [...new Uint8Array(memory.buffer, ptr, len)]);
        answered();
        return -1;
      },
    },
  });
  return { player, answers, done };
}

const response = (status, bytes) => ({
  ok: status === 200,
  status,
  arrayBuffer: async () => new Uint8Array(bytes).buffer,
});

test("the data the page fetched reaches the module", async () => {
  const { player, answers, done } = fakePlayer({ syntaxes: Promise.resolve(response(200, [1, 2, 3])) });
  player.fetchSyntaxes();
  await done;
  assert.deepEqual(answers, [[1, 2, 3]]);
});

test("a failed fetch tells the module there is nothing, and code stays plain", async () => {
  const warn = console.warn;
  const warnings = [];
  console.warn = (message) => warnings.push(message);
  try {
    for (const syntaxes of [Promise.resolve(response(404, [])), Promise.reject(new Error("offline"))]) {
      const { player, answers, done } = fakePlayer({ syntaxes });
      player.fetchSyntaxes();
      await done;
      assert.deepEqual(answers, [null]);
    }
  } finally {
    console.warn = warn;
  }
  assert.equal(warnings.length, 2);
  assert.match(warnings[0], /HTTP 404/);
  assert.match(warnings[1], /offline/);
});

test("without the option it fetches nsh-syntaxes.bin next to nsh.js", async () => {
  const fetched = [];
  const fetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    fetched.push(String(url));
    return response(200, [7]);
  };
  try {
    const { player, answers, done } = fakePlayer({});
    player.fetchSyntaxes();
    await done;
    assert.deepEqual(answers, [[7]]);
  } finally {
    globalThis.fetch = fetch;
  }
  assert.equal(fetched.length, 1);
  assert.equal(fetched[0], new URL("nsh-syntaxes.bin", pathToFileURL(copy)).href);
});
