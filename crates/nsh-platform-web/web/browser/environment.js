/** Whether `a` and `b` are the same value, or objects (arrays) with the
 * same members, compared `depth` levels down. */
function same(a, b, depth) {
  if (a === b) return true;
  if (depth === 0 || !a || !b || typeof a !== "object" || typeof b !== "object") return false;
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(key => same(a[key], b[key], depth - 1));
}

export function createEnvironment(reactive, invoke, dom) {
  const initial = { window: { width: 0, height: 0 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
    titleBar: { height: 0, left: 0, right: 0 }, keyboard: { height: 0, duration: 0.25 },
    colorScheme: "light", fontScale: 1, pixelRatio: 1, locale: "en-US", reduceMotion: false, screenReader: false, host: false,
    lifecycle: "active",
    localeSettings: { languages: ["en-US"], timeZone: "UTC", hourCycle: "h12", firstDayOfWeek: 7,
      measurementSystem: "us", temperatureUnit: "fahrenheit" } };
  const fields = Object.fromEntries(Object.entries(initial).map(([name, value]) => [name, reactive.signal(value)]));
  const tokens = reactive.signal(0);
  const insets = reactive.context(() => fields.safeArea());
  const platform = { os: "web", version: "0.0", idiom: "desktop", desktop: true, catalyst: false,
    select(spec) { return "web" in spec ? spec.web : this.desktop ? spec.desktop ?? spec.default : spec.mobile ?? spec.default; } };
  let appearance = "system", systemScheme = "light";
  // What a `ThemeScope` shows, for what it renders (nsh-js's `scope_scheme`
  // and `scope_tokens`, `env.rs`): `scheme`, an accessor of "light" or
  // "dark", and `tokens`, an accessor of what names its tokens for the
  // engine (`undefined`: the surface's). No scope is looked for until one is
  // made.
  const scope = reactive.context(undefined);
  let scoped = false;
  const shownScheme = () => { const around = scoped && scope(); return around ? around.scheme() : fields.colorScheme(); };
  // `token()` (402) and `role()` (414): of the scope around what reads it,
  // else the surface's, in the appearance shown there.
  const readToken = (op, name) => {
    const around = scoped && scope(), named = around ? around.tokens() : undefined;
    if (named === undefined) tokens();
    return invoke(op, [name, shownScheme() === "dark", named ?? null]);
  };
  const setColorScheme = scheme => {
    if (!["light", "dark", "system"].includes(scheme)) throw new TypeError('setColorScheme: expected "light", "dark" or "system"');
    appearance = scheme;
    dom.rootProp("appearance", scheme);
    fields.colorScheme.set(scheme === "system" ? systemScheme : scheme);
  };
  // The exports nsh-js's `env.rs` declares for `nsh/device` and
  // `nsh/accessibility` (`DEVICE_EXPORTS`, `ACCESSIBILITY_EXPORTS`), which
  // `nsh` serves too: one object each, so the namespaces the runtime
  // serves under those names (`modules`) are the same values as `nsh`'s.
  // `environment.test.mjs` holds both to `env.rs`.
  const device = {
    windowSize: () => fields.window(), colorScheme: () => shownScheme(), setColorScheme,
    fontScale: () => fields.fontScale(), pixelRatio: () => fields.pixelRatio(), locale: () => fields.locale(),
    localeSettings: () => fields.localeSettings(),
    lifecycle: () => fields.lifecycle(), platform,
    // A browser page has no performance monitor (its developer tools measure it).
    fpsEnabled: () => false,
  };
  const accessibility = { reduceMotion: () => fields.reduceMotion(), screenReaderEnabled: () => fields.screenReader() };
  const api = {
    ...device, ...accessibility,
    safeArea: () => fields.safeArea(), safeAreaInsets: () => insets(), keyboardFrame: () => fields.keyboard(),
    screenReader: () => fields.screenReader(),
    token: name => readToken(402, name),
    // nsh-js's `role` (`env.rs`): the token the theme maps the role onto,
    // else the token of its name.
    role: name => readToken(414, name),
    // nsh-js's `setTheme` (`env.rs`): the engine checks and applies it, or
    // throws its `TypeError`; what read a token follows once it took.
    setTheme(theme) { invoke(406, [theme]); tokens.update(value => value + 1); },
    // `updateTheme` and `resetTheme` (`env.rs`), the same way.
    updateTheme(theme) { invoke(409, [theme]); tokens.update(value => value + 1); },
    resetTheme(names) { invoke(410, [names]); tokens.update(value => value + 1); },
  };
  return {
    api, insets, host: () => fields.host(), titleBar: () => fields.titleBar(),
    // A `ThemeScope`'s children (`children()`), made in its scope (nsh-js's
    // `scoped_reader`, `components.rs`): its `appearance`, else what the
    // scope around it shows, else the surface; and its `theme` over the
    // tokens around it, which the engine makes and keeps in a slot.
    inScope(props, children) {
      const outer = scoped ? scope() : undefined;
      scoped = true;
      const shows = reactive.computed(() => {
        const own = props.appearance;
        return own === "light" || own === "dark" ? own : outer ? outer.scheme() : fields.colorScheme();
      });
      const slot = invoke(412, []);
      reactive.cleanup(() => invoke(413, [slot, true]));
      const draws = reactive.computed(() => {
        const theme = props.theme, around = outer ? outer.tokens() : undefined;
        // No tokens of its own: the ones around it.
        if (theme === null || typeof theme !== "object") { invoke(413, [slot, false]); return around; }
        if (around === undefined) tokens();
        return invoke(411, [slot, theme, around ?? null]);
      });
      return scope.Provider({ value: { scheme: shows, tokens: draws }, get children() { return children(); } });
    },
    modules: { "nsh/device": device, "nsh/accessibility": accessibility },
    update(next) {
      systemScheme = next.colorScheme;
      Object.assign(platform, { ...next.platform, version: "0.0", catalyst: false });
      reactive.batch(() => {
        for (const [name, signal] of Object.entries(fields)) {
          const value = name === "colorScheme" && appearance !== "system" ? appearance : next[name];
          const previous = reactive.untracked(signal);
          // A part is written when it changed: a value, or an object's
          // member (the locale settings' languages: one of them, or their
          // number).
          if (!same(value, previous, 2)) signal.set(value);
        }
      });
      dom.rootProp("layoutDirectionRtl", next.rtl);
    },
    changedTokens() { tokens.update(value => value + 1); },
    // `setLocale`'s language, shown at once; the engine's next environment
    // carries it too.
    showLocale(tag) {
      reactive.batch(() => {
        const settings = reactive.untracked(fields.localeSettings);
        fields.localeSettings.set({ ...settings, languages: [tag, ...settings.languages.filter(l => l !== tag)] });
        fields.locale.set(tag);
      });
    },
    setup(tailwind, theme, scheme, statusBar) {
      if (typeof tailwind === "string") dom.rootProp("tailwindTheme", tailwind);
      if (typeof theme === "string") { dom.rootProp("uiTheme", theme); invoke(403, [theme]); tokens.update(value => value + 1); }
      if (["light", "dark", "system"].includes(scheme)) setColorScheme(scheme);
      if (typeof statusBar === "string") invoke(405, [statusBar]);
    },
  };
}
