import { expect, open, test } from "nsh/test";

test("the middle, back and forward buttons reach the nearest onAuxPress", async () => {
  const app = await open("./App.tsx", { platform: "macos" });
  await app.press("id:close", { button: "middle" });
  await app.press("id:tab", { button: "back" });
  await app.keyboard.down("shift");
  await app.press("id:tab", { button: "forward" });
  await app.keyboard.up("shift");
  // The primary button presses, as without a button.
  await app.press("id:tab", { button: "left" });
  expect(app.shows("Heard: close middle|panel back 195,20|panel forward 195,20 shift|tab")).toBe(true);
});

test("the secondary button asks for a context menu, then presses", async () => {
  const app = await open("./App.tsx", { platform: "windows" });
  await app.press("id:tab", { button: "right" });
  expect(app.shows("Heard: menu|panel right 195,20")).toBe(true);
});

test("a text input takes its own buttons", async () => {
  const app = await open("./App.tsx", { platform: "linux" });
  await app.press("id:field", { button: "middle" });
  expect(app.texts()).toContain("Heard: ");
});

test("a phone has no other buttons", async () => {
  const app = await open("./App.tsx");
  await expect(() => app.press("id:close", { button: "middle" })).rejects.toThrow("a phone has no mouse");
});
