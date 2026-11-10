// Tab and Shift+Tab stop at pressables on the desktops, in tree order (the
// row of First and Second is laid out in reverse: Second shows first), and
// Return and Space press the pressable that has the focus:
// `nsh test tests/apps/pressable-focus --target <macos|linux|windows>` (a
// headless run without `--target` is a phone's, whose Tab moves between
// fields only).
import { open, test } from "nsh/test";

test("Tab and Shift+Tab go through the field and the pressables in tree order", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("Focused: none");
  await app.keyboard.press("tab");
  await app.waitFor("Focused: Name");
  await app.keyboard.press("tab");
  await app.waitFor("Focused: First");
  await app.keyboard.press("tab");
  await app.waitFor("Focused: Second");
  await app.keyboard.press("tab");
  await app.waitFor("Focused: Third");
  // Past the last stop Tab goes around to the first, and Shift+Tab back.
  await app.keyboard.press("tab");
  await app.waitFor("Focused: Name");
  await app.keyboard.press("shift+tab");
  await app.waitFor("Focused: Third");
  await app.keyboard.press("shift+tab");
  await app.waitFor("Focused: Second");
});

test("Return and Space press the focused pressable", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("Focused: none");
  await app.keyboard.press("shift+tab");
  await app.waitFor("Focused: Third");
  await app.keyboard.press("enter");
  await app.waitFor("Pressed: Third");
  await app.keyboard.press("shift+tab");
  await app.waitFor("Focused: Second");
  await app.keyboard.press("space");
  await app.waitFor("Pressed: Second");
});
