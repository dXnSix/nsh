import { expect, open, test } from "nsh/test";

test("a wheel over the page scrolls it and the listeners around hear it", async () => {
  const app = await open("./App.tsx", { platform: "macos" });
  await app.wheel("id:row", { y: 48 });
  await app.wheel("id:row", { y: -20, precise: true });
  expect(app.shows("Heard: root 0,48|root 0,-20 precise")).toBe(true);
  expect(app.shows("Offset: 28")).toBe(true);
});

test("a view whose wheelScroll is none takes the wheel: nothing scrolls, nothing around hears it", async () => {
  const app = await open("./App.tsx", { platform: "windows" });
  await app.keyboard.down("ctrl");
  await app.wheel("id:canvas", { y: -48 });
  await app.keyboard.up("ctrl");
  expect(app.shows("Heard: canvas -48 195,50 ctrl")).toBe(true);
  expect(app.shows("Offset: 0")).toBe(true);
});

test("a view whose wheelScroll is contain keeps the wheel from the page around it", async () => {
  const app = await open("./App.tsx", { platform: "web" });
  await app.wheel("id:stepper", { y: 48 });
  expect(app.shows("Heard: stepper 48")).toBe(true);
  expect(app.shows("Offset: 0")).toBe(true);
});

test("a phone has no wheel", async () => {
  const app = await open("./App.tsx");
  await expect(() => app.wheel("id:row", { y: 48 })).rejects.toThrow("a phone has no wheel");
});
