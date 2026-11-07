// A press on a button under a hidden title bar reaches it on the macOS host
// with the app in the background (`nsh test crates/nsh-platform-macos/tests/host
// --target macos`, also with `--foreground`): on macOS 15 a view of the
// title bar's is what AppKit hits there, and it takes no first click in a
// window that is not key (`window.rs` `pass_title_bar_press`). For the macOS
// host only, like `App.test.tsx`.
import { expect, open, test } from "nsh/test";

test("a press under a hidden title bar reaches the app's button", async () => {
  const app = await open("./TitleBar.tsx");
  await app.waitFor("Presses: 0");
  await app.press("label:Header button");
  await app.waitFor("Presses: 1");
  expect(app.shows("Presses: 1")).toBe(true);
});
