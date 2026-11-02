# Upstream PR: android: Let embedding apps set the app's location and hear its navigations

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/location/03-android`, on `pr/location/02-ios`: one commit (the fork's Android part of `feat/location`).
  It needs only the core PR; it stacks on the iOS one because both edit the Navigation page's sentence and
  `docs/dev/embedding.md`'s paragraph about the SDKs.
- Title: `android: Let embedding apps set the app's location and hear its navigations`.
- The fork's PR: dXnSix/nsh#111 (its Android part).

---

**Depends on:** #<location core> (the host channel's location) and #<location ios> (the same two doc sentences).

## Description

An Android app that embeds NSH cannot open a page of the app (a deep link or an App Link it handled) or follow where
the app went: the host channel's location has no face in the SDK.

`NshView.setLocation` sets where the app starts (before a load) or moves it while it runs, `getLocation` reads where it
is, and `setNavigateHandler` hears each `navigate` of the app on the UI thread, with the location and whether it
replaced the history entry. The native side registers its handler only while Java has one, and calls
`NshView.navigateMessage` on the engine thread, which posts to the UI thread.

## Public API

- Java: `boolean NshView.setLocation(String location)`; `String NshView.getLocation()`;
  `interface NshView.NavigateHandler { void navigate(String location, boolean replace); }`;
  `void NshView.setNavigateHandler(NavigateHandler handler)`.

## How to Test

- `cargo xtask android --dev`, `cargo xtask clippy-android`.
- `nsh test <router app> --target android` (an app's location through the host channel, as the embedding app sets it).
