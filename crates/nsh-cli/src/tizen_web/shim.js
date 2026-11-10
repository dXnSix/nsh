// The TV shim of a Tizen web app (`nsh package --target tizen-web`), a
// classic script the page loads before `nsh.js` (`crates/nsh-cli/src/tizen_web.rs`):
//
// - the remote's Back (key code 10009) goes back in the app as a browser's
//   back key does: a `BrowserBack` key, then an `Escape` (a dialog, a
//   field), for the first the app takes (`nsh.js` `keyDown`); when nothing
//   takes it (the first screen, nothing open) the page asks whether to exit
//   and exits with `tizen.application.getCurrentApplication().exit()`, as
//   Samsung's rules ask;
// - the color, media and channel keys are registered with
//   `tizen.tvinputdevice.registerKey`, so the app hears them: `nsh.js`
//   reads them by Samsung's key codes in the TV idiom (`TV_KEY_CODES`);
// - `globalThis.nshTizen` is what the web half of `nsh/tizen` reaches
//   (`nsh-platform-web`'s `browser/tizen.js`): `tv`, `keys`, `back()`,
//   `confirmExit()`, `exit()`, and the TV's own APIs, `productInfo()`
//   (`webapis.productinfo`, `tizen.systeminfo`), `registerKey(name)` /
//   `unregisterKey(name)` (`tizen.tvinputdevice`, by Tizen's key names) and
//   `setPreviewData(json)` (a promise: the package's preview service, which
//   calls `webapis.preview`; `preview` says which preview the package has).
//
// Off a TV (`nsh run --target tizen-web` in Chrome, `nsh test --target
// tizen-web`) there is no `tizen`: keys register nothing and `exit()`
// closes the window. The page creates the player with `idiom: "tv"`.
(function (global) {
  "use strict";

  /** The remote's Back (`tizen.tvinputdevice`'s `Return`). */
  const BACK = 10009;
  const ENTER = 13;
  const ARROWS = [37, 38, 39, 40];
  /** What the shim registers, as `tizen.tvinputdevice` names them: the keys
   * `nsh.js` maps by Samsung's codes. Arrows, Enter and Back come unasked;
   * Exit is the system's. */
  const KEYS = [
    "ColorF0Red", "ColorF1Green", "ColorF2Yellow", "ColorF3Blue",
    "MediaPlayPause", "MediaPlay", "MediaPause", "MediaStop", "MediaRewind", "MediaFastForward",
    "MediaTrackNext", "MediaTrackPrevious", "ChannelUp", "ChannelDown", "Info",
  ];
  /** Smart Hub's preview of the package (`nsh.json` `tizen.smartHubPreview`),
   * which `nsh package` writes here: `{ service }`, the id of the package's
   * preview service, `{ endpoint }`, a public preview's URL, or none. */
  const PREVIEW = null;
  /** Marks the keys the shim sends, which its own listener lets through. */
  const SENT = "nshTizen";

  const tizen = global.tizen;
  const document = global.document;

  /** Registers each of `KEYS` the TV has; the names registered. A key the
   * model lacks (or refuses) is left out, not an error. */
  function registerKeys() {
    const input = tizen?.tvinputdevice;
    if (!input) return [];
    let supported = null;
    try {
      supported = new Set(input.getSupportedKeys().map((key) => key.name));
    } catch {
      // Every key is tried.
    }
    return KEYS.filter((name) => {
      if (supported && !supported.has(name)) return false;
      try {
        input.registerKey(name);
        return true;
      } catch {
        return false;
      }
    });
  }

  /** Sends the app a key down where the focus is; whether it took it
   * (`preventDefault`). */
  function send(key) {
    const target = document.activeElement || document.body || document.documentElement;
    const event = new global.KeyboardEvent("keydown", { key, code: key, bubbles: true, cancelable: true, composed: true });
    event[SENT] = true;
    return !target.dispatchEvent(event);
  }

  /** Back in the app: whether something took it. */
  function back() {
    return send("BrowserBack") || send("Escape");
  }

  /** Ends the app: the TV's application manager, or off a TV the window. */
  function exit() {
    const app = tizen?.application?.getCurrentApplication?.();
    if (app) app.exit();
    else global.close?.();
  }

  // The exit confirmation: two buttons, the remote's arrows between them,
  // Enter to choose, Back to stay.
  let confirmation = null;

  /** Asks whether to exit; resolves to the answer (true: exiting). */
  function confirmExit() {
    if (confirmation) return confirmation.answer;
    const overlay = document.createElement("div");
    overlay.id = "nsh-tizen-exit";
    overlay.setAttribute("role", "alertdialog");
    overlay.setAttribute("aria-modal", "true");
    overlay.style.cssText = "position:fixed;inset:0;z-index:2147483647;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,.6);font:32px/1.4 system-ui,sans-serif";
    const box = document.createElement("div");
    box.style.cssText = "min-width:min(640px,80vw);padding:48px;border-radius:24px;background:#1c1c1e;color:#f2f2f7;text-align:center";
    const title = document.createElement("div");
    title.textContent = document.title ? `Exit ${document.title}?` : "Exit?";
    title.style.cssText = "margin-bottom:40px";
    const row = document.createElement("div");
    row.style.cssText = "display:flex;gap:24px;justify-content:center";
    const button = (text) => {
      const el = document.createElement("button");
      el.textContent = text;
      el.style.cssText = "min-width:220px;padding:16px 32px;border:0;border-radius:16px;font:inherit;outline:none";
      row.append(el);
      return el;
    };
    const buttons = [button("Exit"), button("Cancel")];
    box.append(title, row);
    overlay.append(box);
    document.body.append(overlay);
    let resolve;
    const answer = new Promise((r) => { resolve = r; });
    const restore = document.activeElement;
    confirmation = { overlay, buttons, index: 0, answer, resolve, restore };
    buttons.forEach((el, i) => el.addEventListener("click", () => choose(i === 0)));
    highlight();
    return answer;
  }

  function highlight() {
    confirmation.buttons.forEach((el, i) => {
      const on = i === confirmation.index;
      el.style.background = on ? "#f2f2f7" : "#3a3a3c";
      el.style.color = on ? "#1c1c1e" : "#f2f2f7";
      if (on) el.focus?.();
    });
  }

  function choose(exiting) {
    const { overlay, resolve, restore } = confirmation;
    confirmation = null;
    overlay.remove();
    restore?.focus?.();
    resolve(exiting);
    if (exiting) exit();
  }

  /** A key while the confirmation shows: never the app's. */
  function confirmationKey(e) {
    if (e.keyCode === BACK || e.key === "Escape") {
      choose(false);
    } else if (e.keyCode === ENTER || e.key === "Enter") {
      choose(confirmation.index === 0);
    } else if (ARROWS.includes(e.keyCode) || e.key?.startsWith("Arrow")) {
      confirmation.index = 1 - confirmation.index;
      highlight();
    }
  }

  global.addEventListener("keydown", (e) => {
    if (e[SENT]) return;
    if (confirmation) {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (!e.repeat) confirmationKey(e);
      return;
    }
    if (e.keyCode !== BACK) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (e.repeat) return;
    if (!back()) confirmExit();
  }, true);
  // The other half of a key the confirmation took.
  global.addEventListener("keyup", (e) => {
    if (e.keyCode === BACK || (confirmation && !e[SENT])) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  }, true);

  /** What `get()` gives, `null` where it throws or gives nothing. */
  function read(get) {
    try {
      const value = get();
      return value == null || value === "" ? null : String(value);
    } catch {
      return null;
    }
  }

  /** The TV's model, Tizen version, firmware build and maker. */
  function productInfo() {
    const product = global.webapis?.productinfo;
    const capability = (key) => read(() => tizen.systeminfo.getCapability(key));
    return {
      model: read(() => product.getRealModel()) ?? read(() => product.getModel()),
      platformVersion: capability("http://tizen.org/feature/platform.version"),
      build: read(() => product.getFirmware()),
      manufacturer: capability("http://tizen.org/system/manufacturer"),
    };
  }

  /** Takes the remote's key `name` (Tizen's: `ColorF0Red`) from the TV;
   * throws the TV's error. */
  function registerKey(name) {
    tizen.tvinputdevice.registerKey(name);
    if (!keys.includes(name)) keys.push(name);
  }

  /** Gives key `name` back to the TV; throws the TV's error. */
  function unregisterKey(name) {
    tizen.tvinputdevice.unregisterKey(name);
    const at = keys.indexOf(name);
    if (at >= 0) keys.splice(at, 1);
  }

  /** Sets Smart Hub's personal preview of the app to `json` (Samsung's
   * format, as text): starts the package's preview service (`PREVIEW.service`,
   * `nsh.json` `tizen.smartHubPreview: "service"`) with it, which hands it to
   * `webapis.preview.setPreviewData`, as Samsung sets personal previews;
   * settles with the TV's answer to the launch. */
  function setPreviewData(json) {
    return new Promise((resolve, reject) => {
      if (PREVIEW?.endpoint) {
        throw new Error(`the package's preview is public: the TV reads it from ${PREVIEW.endpoint} (nsh.json tizen.smartHubPreview)`);
      }
      if (!PREVIEW?.service) {
        throw new Error("the package declares no personal preview: set nsh.json tizen.smartHubPreview to \"service\"");
      }
      const control = new tizen.ApplicationControl(
        "http://tizen.org/appcontrol/operation/service", null, null, null,
        [new tizen.ApplicationControlData("preview", [json])],
      );
      tizen.application.launchAppControl(control, PREVIEW.service, () => resolve(),
        (error) => reject(new Error(error?.message ?? String(error))));
    });
  }

  const keys = registerKeys();
  global.nshTizen = {
    /** Whether the page runs on a TV (Tizen's web runtime). */
    tv: !!tizen,
    /** The remote's keys registered for the app, by Tizen's names. */
    keys,
    /** The package's Smart Hub preview (`PREVIEW`). */
    preview: PREVIEW,
    back,
    confirmExit,
    exit,
    productInfo,
    registerKey,
    unregisterKey,
    setPreviewData,
  };
})(globalThis);
