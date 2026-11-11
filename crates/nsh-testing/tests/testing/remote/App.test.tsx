import { expect, open, test } from "nsh/test";

const TV_KEYS = [
  "mediaplaypause",
  "mediaplay",
  "mediapause",
  "mediastop",
  "mediarewind",
  "mediafastforward",
  "medianext",
  "mediaprevious",
  "red",
  "green",
  "yellow",
  "blue",
  "channelup",
  "channeldown",
  "info",
];

test("every media and TV key fires the app's binding of its name", async () => {
  const app = await open("./App.tsx", { platform: "web" });
  for (const key of TV_KEYS) {
    await app.keyboard.remote(key);
  }
  await app.waitFor(`Pressed: ${TV_KEYS.join(" ")}`);
});

test("the arrows fire their bindings, whatever the name's case", async () => {
  const app = await open("./App.tsx", { platform: "web" });
  await app.keyboard.remote("up");
  await app.keyboard.remote("Down");
  await app.keyboard.remote("LEFT");
  await app.keyboard.remote("right");
  await app.keyboard.remote("Red");
  await app.waitFor("Pressed: up down left right red");
});

test("ok and enter press the view the focus is on", async () => {
  const app = await open("./App.tsx", { platform: "web" });
  await app.press("Focus play");
  await app.keyboard.remote("ok");
  await app.waitFor("Played: 1");
  await app.keyboard.remote("enter");
  await app.waitFor("Played: 2");
});

test("back goes back, and does nothing with nothing to go back from", async () => {
  const app = await open("./App.tsx", { platform: "web" });
  await app.press("Open");
  await app.waitFor("Detail body");
  await app.keyboard.remote("back");
  expect(app.shows("Detail body")).toBe(false);
  expect(app.shows("Played: 0")).toBe(true);
  await app.keyboard.remote("back");
  expect(app.shows("Played: 0")).toBe(true);
});

test("an unknown key fails with the names there are", async () => {
  const app = await open("./App.tsx", { platform: "web" });
  await expect(() => app.keyboard.remote("power")).rejects.toThrow(
    '"power" is no TV remote key: a remote\'s keys are up, down, left, right, enter, ok, back, ' +
      "mediaplaypause, mediaplay, mediapause, mediastop, mediarewind, mediafastforward, medianext, " +
      "mediaprevious, red, green, yellow, blue, channelup, channeldown, info",
  );
  // A keyboard's keys that a remote has not.
  await expect(() => app.keyboard.remote("escape")).rejects.toThrow("is no TV remote key");
  await expect(() => app.keyboard.remote("mod+k")).rejects.toThrow("is no TV remote key");
});
