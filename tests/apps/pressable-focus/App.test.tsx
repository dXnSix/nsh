// Tab and Shift+Tab stop at pressables on the desktops, in tree order (the
// row of First and Second is laid out in reverse: Second shows first), the
// arrows move the focus to the nearest stop in their direction, as on GPUI
// (a field keeps Left and Right), and Return and Space press the pressable
// that has the focus:
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

test("The arrows move the focus to the nearest stop in their direction", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("Focused: none");
  await app.keyboard.press("tab");
  await app.waitFor("Focused: Name");
  await app.keyboard.press("tab");
  await app.waitFor("Focused: First");
  // The row is reversed: Second shows left of First. Nothing is right of
  // First, so Right leaves the focus there.
  await app.keyboard.press("right");
  await app.keyboard.press("left");
  await app.waitFor("Focused: Second");
  await app.keyboard.press("right");
  await app.waitFor("Focused: First");
  await app.keyboard.press("down");
  await app.waitFor("Focused: Third");
  // Up goes to the nearest above, by the left edge between equals.
  await app.keyboard.press("up");
  await app.waitFor("Focused: Second");
  await app.keyboard.press("up");
  await app.waitFor("Focused: Name");
  // Down leaves a single-line field.
  await app.keyboard.press("down");
  await app.waitFor("Focused: Second");
  await app.keyboard.press("enter");
  await app.waitFor("Pressed: Second");
});

test("With nothing focused Down takes the first stop, and a field keeps Left and Right", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("Focused: none");
  await app.keyboard.press("down");
  await app.waitFor("Focused: Name");
  await app.type("id:name", "ab");
  await app.waitFor("Name: ab");
  // Left and Right move the caret, not the focus.
  await app.keyboard.press("left");
  await app.keyboard.press("x");
  await app.waitFor("Name: axb");
  await app.keyboard.press("right");
  await app.keyboard.press("y");
  await app.waitFor("Name: axby");
  await app.waitFor("Focused: Name");
  await app.keyboard.press("down");
  await app.waitFor("Focused: Second");
});
