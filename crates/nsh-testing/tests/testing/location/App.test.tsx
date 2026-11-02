import { expect, open, test } from "nsh/test";

test("the app starts at the root, or where open puts it", async () => {
  const app = await open("./App.tsx");
  expect(app.shows("Location: /")).toBe(true);
  expect(app.location).toBe("/");
  const deep = await open("./App.tsx", { location: "/sports?tab=live" });
  expect(deep.shows("Path: /sports Query: ?tab=live")).toBe(true);
  expect(deep.location).toBe("/sports?tab=live");
});

test("the app's navigations move its location", async () => {
  const app = await open("./App.tsx", { location: "/news" });
  await app.press("Teams");
  expect(app.shows("Location: /teams")).toBe(true);
  expect(app.location).toBe("/teams");
  await app.press("Table");
  expect(app.location).toBe("/teams?tab=table");
});

test("the platform moves the app", async () => {
  const app = await open("./App.tsx");
  await app.navigate("/news/today");
  expect(app.shows("Location: /news/today")).toBe(true);
  // Relative to where it is, as a link on its page.
  await app.navigate("tomorrow#top");
  expect(app.location).toBe("/news/tomorrow#top");
  await expect(() => app.navigate("https://example.com/")).rejects.toThrow("another site's URL");
});

test("only the browser goes forward", async () => {
  const app = await open("./App.tsx");
  await expect(() => app.forward()).rejects.toMatchObject({ code: "ERR_NSH_TEST_UNSUPPORTED" });
});

test("an invalid location fails open", async () => {
  await expect(() => open("./App.tsx", { location: "https://example.com/" })).rejects.toThrow("location");
});
