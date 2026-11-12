// `nsh/tizen` in a Tizen web app (a `.wgt` on a Samsung TV): the calls go
// to the TV through the page's TV shim (`globalThis.nshTizen`, which
// `nsh package --target tizen-web` loads before `nsh.js`), which uses
// `tizen.*` and `webapis.*`. On any other page the module is the runtime's
// own (`nsh_core::runtime`'s `tizen.rs`): `isAvailable()` is false and the
// other calls reject with `ERR_FEATURE_UNAVAILABLE_ON_PLATFORM`, as on every
// platform but a TV. This module runs in the page's realm; what it gives the
// app (promises, errors, objects) is made with the app realm's intrinsics.

const UNAVAILABLE = "ERR_FEATURE_UNAVAILABLE_ON_PLATFORM";

/** `keymap`'s names of the remote's keys a TV gives apps, and Tizen's names
 * of them (`tizen.tvinputdevice`). */
const KEYS = {
  red: "ColorF0Red", colorf0red: "ColorF0Red",
  green: "ColorF1Green", colorf1green: "ColorF1Green",
  yellow: "ColorF2Yellow", colorf2yellow: "ColorF2Yellow",
  blue: "ColorF3Blue", colorf3blue: "ColorF3Blue",
  mediaplaypause: "MediaPlayPause", mediaplay: "MediaPlay", mediapause: "MediaPause", mediastop: "MediaStop",
  mediarewind: "MediaRewind", mediafastforward: "MediaFastForward",
  medianext: "MediaTrackNext", mediatracknext: "MediaTrackNext",
  mediaprevious: "MediaTrackPrevious", mediatrackprevious: "MediaTrackPrevious",
  channelup: "ChannelUp", channeldown: "ChannelDown", info: "Info",
};

/**
 * The app's `nsh/tizen`: `native`, the runtime's namespace, where `tv` (the
 * shim's `nshTizen`) says the page runs on no TV; else one on the TV's APIs.
 * `realm`: the app's window. `label(name)`: `keymap.label`, which throws a
 * `TypeError` for a name of no key.
 */
export function createTizen(native, tv, realm, label) {
  if (!tv?.tv) return native;
  const fail = (Type, message, code) => {
    const error = new realm[Type](`nsh/tizen ${message}`);
    if (code) error.code = code;
    return error;
  };
  /** A promise of the app's realm for what `run` returns or throws. */
  const settle = (run) => new realm.Promise((resolve, reject) => {
    Promise.resolve().then(run).then(resolve, (error) => reject(
      error instanceof realm.Error ? error
        : error instanceof TypeError ? fail("TypeError", error.message)
        : fail("Error", String(error?.message ?? error), UNAVAILABLE),
    ));
  });
  /** Tizen's name of the key `keymap` calls `name`, checked as natively. */
  const key = (call, name) => {
    const text = String(name);
    const tizen = Object.hasOwn(KEYS, text.toLowerCase()) ? KEYS[text.toLowerCase()] : null;
    if (tizen) return tizen;
    let known = false;
    try {
      label(text);
      known = !text.includes("+");
    } catch {
      // No key at all.
    }
    if (!known) throw fail("TypeError", `${call}: ${JSON.stringify(text)} names no key`);
    throw fail("RangeError", `${call}: ${JSON.stringify(text)} is no key the TV gives apps (the remote's colors, media, channel and info keys are)`);
  };
  const tvError = (call) => (error) => {
    throw fail("Error", `${call}: ${error?.message ?? error}`, UNAVAILABLE);
  };
  return {
    isAvailable: () => true,
    productInfo: () => settle(() => {
      const info = tv.productInfo();
      const out = new realm.Object();
      for (const field of ["model", "platformVersion", "build", "manufacturer"]) out[field] = info[field] ?? null;
      return out;
    }),
    registerKey: (name) => settle(() => {
      const tizen = key("registerKey", name);
      try { tv.registerKey(tizen); } catch (error) { tvError("registerKey")(error); }
    }),
    unregisterKey: (name) => settle(() => {
      const tizen = key("unregisterKey", name);
      try { tv.unregisterKey(tizen); } catch (error) { tvError("unregisterKey")(error); }
    }),
    exitApp: () => settle(() => { tv.exit(); }),
    setPreviewData: (data) => settle(() => {
      const json = JSON.stringify(data ?? null);
      return tv.setPreviewData(json).then(() => undefined, tvError("setPreviewData"));
    }),
  };
}
