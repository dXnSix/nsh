# Upstream PR: ios: Let embedding apps set the app's location and hear its navigations

Draft of a pull request for `longbridge/nsh`, not opened: the owner decides and posts. The body is below the line.

- Branch: `pr/location/02-ios`, on `pr/location/01-core`: one commit (the fork's iOS part of `feat/location`).
- Title: `ios: Let embedding apps set the app's location and hear its navigations`.
- The fork's PR: dXnSix/nsh#111 (its iOS part).

---

**Depends on:** #<location core> (`core: Tell apps where the user is in them, and let them navigate`): the host
channel's `set_location`, `location` and `handle_navigate` this exposes.

## Description

An iOS app that embeds NSH cannot open a page of the app (a deep link or a universal link it handled) or follow where
the app went: the host channel's location has no face in the SDK.

`NshAppView.location` sets where the app starts (before a load) or moves it while it runs, and reads where it is;
`onNavigate` hears each `navigate` of the app on the main thread, with the location and whether it replaced the history
entry. The C ABI under them is `nsh_app_set_location`, `nsh_app_location` (freed with `nsh_string_free`) and
`nsh_app_set_navigate_handler`, whose handler hops to the main queue as the call handlers do.

## Public API

- C: `bool nsh_app_set_location(NshApp *app, const char *location)`; `char *nsh_app_location(NshApp *app)`;
  `void nsh_app_set_navigate_handler(NshApp *app, void (*handler)(void *context, const char *location, bool replace),
  void *context, void (*release)(void *context))`.
- Swift: `NshAppView.location: String`; `NshAppView.onNavigate: (@MainActor (String, Bool) -> Void)?`.

## How to Test

- `cargo clippy -p nsh-platform-ios -p nsh-ios-host -p nsh-ios-app --target aarch64-apple-ios-macabi --features
  nsh-platform-ios/embed,nsh-platform-ios/dev`; `swiftc -typecheck` of `NativeShell.swift` against `include/`.
- `cargo xtask ios-embed`, then an app that sets `location` before loading and logs `onNavigate`.
