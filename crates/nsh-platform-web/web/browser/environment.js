export function createEnvironment(reactive, invoke, dom) {
  const initial = { window: { width: 0, height: 0 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
    titleBar: { height: 0, left: 0, right: 0 }, keyboard: { height: 0, duration: 0.25 },
    colorScheme: "light", fontScale: 1, pixelRatio: 1, locale: "en-US", reduceMotion: false, screenReader: false, host: false,
    lifecycle: "active", timeZone: "UTC", languages: ["en-US"] };
  const fields = Object.fromEntries(Object.entries(initial).map(([name, value]) => [name, reactive.signal(value)]));
  const tokens = reactive.signal(0);
  // `location()`: the app's location as `{ url, pathname, search, hash }`
  // (the engine's `Location`, which resolves and checks it).
  const locationOf = url => {
    const hashAt = url.indexOf("#");
    const beforeHash = hashAt < 0 ? url : url.slice(0, hashAt);
    const queryAt = beforeHash.indexOf("?");
    return Object.freeze({
      url,
      pathname: queryAt < 0 ? beforeHash : beforeHash.slice(0, queryAt),
      search: queryAt < 0 ? "" : beforeHash.slice(queryAt),
      hash: hashAt < 0 ? "" : url.slice(hashAt),
    });
  };
  const location = reactive.signal(locationOf("/"));
  const setLocation = url => { if (reactive.untracked(location).url !== url) location.set(locationOf(url)); };
  const navigate = (url, options) => {
    if (typeof url !== "string") throw new TypeError("navigate: expected a URL string");
    if (options !== undefined && (options === null || typeof options !== "object")) throw new TypeError("navigate: options are an object");
    const history = options?.history;
    if (history !== undefined && history !== "push" && history !== "replace") throw new TypeError('navigate: history is "push" or "replace"');
    setLocation(invoke(406, [url, history === "replace"]));
  };
  const insets = reactive.context(() => fields.safeArea());
  const platform = { os: "web", version: "0.0", idiom: "desktop", desktop: true, catalyst: false,
    select(spec) { return "web" in spec ? spec.web : this.desktop ? spec.desktop ?? spec.default : spec.mobile ?? spec.default; } };
  let appearance = "system", systemScheme = "light";
  const setColorScheme = scheme => {
    if (!["light", "dark", "system"].includes(scheme)) throw new TypeError('setColorScheme: expected "light", "dark" or "system"');
    appearance = scheme;
    dom.rootProp("appearance", scheme);
    fields.colorScheme.set(scheme === "system" ? systemScheme : scheme);
  };
  const api = {
    windowSize: () => fields.window(), safeArea: () => fields.safeArea(), safeAreaInsets: () => insets(),
    keyboardFrame: () => fields.keyboard(), colorScheme: () => fields.colorScheme(), setColorScheme,
    fontScale: () => fields.fontScale(), pixelRatio: () => fields.pixelRatio(), locale: () => fields.locale(),
    reduceMotion: () => fields.reduceMotion(), screenReader: () => fields.screenReader(), screenReaderEnabled: () => fields.screenReader(), platform,
    lifecycle: () => fields.lifecycle(), timeZone: () => fields.timeZone(), languages: () => fields.languages(),
    location: () => location(), navigate,
    token(name) { tokens(); return invoke(402, [name, fields.colorScheme() === "dark"]); },
  };
  return {
    api, insets, host: () => fields.host(), titleBar: () => fields.titleBar(),
    update(next) {
      systemScheme = next.colorScheme;
      Object.assign(platform, { ...next.platform, version: "0.0", catalyst: false });
      reactive.batch(() => {
        for (const [name, signal] of Object.entries(fields)) {
          const value = name === "colorScheme" && appearance !== "system" ? appearance : next[name];
          const previous = reactive.untracked(signal);
          // A part is written when it changed (an object or the languages'
          // array: one of its members, or their number).
          const keys = value && typeof value === "object" ? Object.keys(value) : null;
          const same = previous && keys
            ? keys.length === Object.keys(previous).length && keys.every(key => value[key] === previous[key]) : value === previous;
          if (!same) signal.set(value);
        }
      });
      dom.rootProp("layoutDirectionRtl", next.rtl);
    },
    changedTokens() { tokens.update(value => value + 1); },
    setLocation,
    setup(tailwind, theme, scheme, statusBar) {
      if (typeof tailwind === "string") dom.rootProp("tailwindTheme", tailwind);
      if (typeof theme === "string") { dom.rootProp("uiTheme", theme); invoke(403, [theme]); tokens.update(value => value + 1); }
      if (["light", "dark", "system"].includes(scheme)) setColorScheme(scheme);
      if (typeof statusBar === "string") invoke(405, [statusBar]);
    },
  };
}
