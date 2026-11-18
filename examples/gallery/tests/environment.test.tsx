// The device as the test sets it up: the system's appearance, the window.
import { expect, open, test } from "nsh/test";

test("follows the system's appearance", async () => {
  const app = await open("../App.tsx", { clock: {}, colorScheme: "dark" });
  await app.press("role:button[System]");
  await app.waitFor("Dark scheme");
  // The theme's dark values come with it: the gallery picks its theme's
  // `colorMode` with the appearance, and `foreground` is the dark one.
  await app.waitFor("text:Theme colorMode: dark");
  expect(app.spanStyle("text:Theme colorMode: dark").color).toBe("#eeeff1ff");
  await app.setColorScheme("light");
  await app.waitFor("Light scheme");
  await app.waitFor("text:Theme colorMode: light");
  expect(app.spanStyle("text:Theme colorMode: light").color).toBe("#101319ff");
});

test("shows the window's size as it rotates", async () => {
  const app = await open("../App.tsx", { clock: {} });
  await app.press("role:button[System]");
  await app.waitFor("text:/390 × 844/");
  await app.setViewport(844, 390);
  await app.waitFor("text:/844 × 390/");
});
