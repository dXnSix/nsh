// The TV idiom on the desktop's hosts: `platform` reports a TV on this
// machine's OS and `tv={{…}}` props apply, not `desktop={{…}}` ones:
// `nsh test tests/apps/tv-idiom --target <gpui|web> --idiom tv` (`nsh.json`
// `dev.idiom` says `tv` too; the other targets, and headless runs, take no
// idiom).
import { expect, open, test } from "nsh/test";

test("The app runs as on a TV", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("Idiom: tv");
  await app.waitFor("TV: true Desktop: false");
  await app.waitFor("Select: tv");
});

test("The tv props apply and the desktop ones do not", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("label:On a TV");
  expect(app.shows("label:On a desktop")).toBe(false);
});
