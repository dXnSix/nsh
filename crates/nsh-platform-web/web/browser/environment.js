/** Whether `a` and `b` are the same value, or objects (arrays) with the
 * same members, compared `depth` levels down. */
function same(a, b, depth) {
  if (a === b) return true;
  if (depth === 0 || !a || !b || typeof a !== "object" || typeof b !== "object") return false;
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(key => same(a[key], b[key], depth - 1));
}

/** `appInfo()`'s object from the build's JSON, its fields typed as
 * nsh-js's `AppInfo` reads them. */
function readAppInfo(json) {
  let info = {};
  try { info = JSON.parse(json) ?? {}; } catch {}
  const text = value => typeof value === "string" ? value : null;
  return {
    id: text(info.id), name: text(info.name), version: text(info.version),
    build: Number.isFinite(info.build) ? info.build : null, development: info.development === true,
  };
}

export function createEnvironment(reactive, invoke, dom) {
  const initial = { window: { width: 0, height: 0 }, safeArea: { top: 0, right: 0, bottom: 0, left: 0 },
    reservedRegions: [],
    titleBar: { height: 0, left: 0, right: 0 }, keyboard: { height: 0, duration: 0.25 },
    colorScheme: "light", fontScale: 1, pixelRatio: 1, locale: "en-US", reduceMotion: false, screenReader: false, host: false,
    lifecycle: "active", shaders: true, wakeCount: 0,
    network: { online: true, kind: "unknown", expensive: false, constrained: false, cellularData: "unknown" },
    horizontalSizeClass: "compact", verticalSizeClass: "compact",
    localeSettings: { languages: ["en-US"], timeZone: "UTC", hourCycle: "h12", firstDayOfWeek: 7,
      measurementSystem: "us", temperatureUnit: "fahrenheit" } };
  const fields = Object.fromEntries(Object.entries(initial).map(([name, value]) => [name, reactive.signal(value)]));
  // What read the tokens follows them as nsh-js's `env.rs` does: by name.
  // `token("x")` reads `tokens` (a change of most of them, a new theme) and
  // the signal of `x`, made the first time a name is read (past
  // `TOKEN_SIGNALS` names, one signal for the rest); `shape` is read by what
  // lists them (their names, kinds and modifiers); `scopes`, by what makes a
  // `ThemeScope`'s tokens over them, whatever changed.
  const tokens = reactive.signal(0), shape = reactive.signal(0), scopes = reactive.signal(0);
  const TOKEN_SIGNALS = 1024;
  const named = new Map();
  let overflow, overflowed = false;
  // A signal the environment keeps, whatever owner first read its name.
  const kept = () => reactive.runWithOwner ? reactive.runWithOwner(null, () => reactive.signal(0)) : reactive.signal(0);
  const nameSignal = name => {
    let signal = named.get(name);
    if (signal) return signal;
    if (named.size >= TOKEN_SIGNALS) { overflowed = true; return overflow ??= kept(); }
    signal = kept();
    named.set(name, signal);
    return signal;
  };
  const bump = signal => signal.update(value => value + 1);
  const insets = reactive.context(() => fields.safeArea());
  // `select` as nsh-js's `platform_select`: the platform's name, then the
  // OS's key, then `tv` on a TV, then `desktop` / `mobile`, then `default`
  // (`native` is not the web).
  const platform = { name: "web", os: "web", version: "0.0", idiom: "desktop", desktop: true, tv: false, catalyst: false,
    select(spec) {
      if (!spec || typeof spec !== "object") return undefined;
      const keys = this.name !== this.os ? [this.name, this.os] : [this.os];
      if (this.tv) keys.push("tv");
      keys.push(this.desktop ? "desktop" : "mobile");
      if (this.os !== "web") keys.push("native");
      keys.push("default");
      for (const key of keys) {
        const value = spec[key];
        if (value !== undefined) return value;
      }
      return undefined;
    } };
  let appearance = "system", systemScheme = "light";
  // What a `ThemeScope` shows, for what it renders (nsh-js's `scope_scheme`
  // and `scope_tokens`, `env.rs`): `scheme`, an accessor of "light" or
  // "dark", and `tokens`, an accessor of what names its tokens for the
  // engine (`undefined`: the surface's). No scope is looked for until one is
  // made.
  const scope = reactive.context(undefined);
  let scoped = false;
  const shownScheme = () => { const around = scoped && scope(); return around ? around.scheme() : fields.colorScheme(); };
  // `token()` (402) and `role()` (414): of the scope around what reads it
  // (tracked as one), else the surface's, tracked by name; the engine
  // gives the value in the theme format's canonical form. A role's name
  // changes when the token it resolves to does (`token_changes`).
  const readToken = (op, name) => {
    const around = scoped && scope(), held = around ? around.tokens() : undefined;
    if (held === undefined) { tokens(); if (typeof name === "string") nameSignal(name)(); }
    return invoke(op, [name, shownScheme() === "dark", held ?? null]);
  };
  // The token dictionary's reads (`tokens.*`): `get` tracked as `token()`
  // is, the lists by their shape.
  const readDictionary = (op, arg) => {
    const around = scoped && scope(), held = around ? around.tokens() : undefined;
    if (held === undefined) {
      if (op === 418) { tokens(); if (typeof arg === "string") nameSignal(arg)(); } else shape();
    }
    return JSON.parse(invoke(op, [arg, held ?? null]));
  };
  const setColorScheme = scheme => {
    if (!["light", "dark", "system"].includes(scheme)) throw new TypeError('setColorScheme: expected "light", "dark" or "system"');
    appearance = scheme;
    dom.rootProp("appearance", scheme);
    fields.colorScheme.set(scheme === "system" ? systemScheme : scheme);
  };
  // The appearance Rust set (`set_color_scheme`), which the engine's core
  // already shows (`Host::set_appearance`): `colorScheme()` follows it as
  // it follows `setColorScheme`, and nothing goes back.
  const changedAppearance = scheme => {
    if (!["light", "dark", "system"].includes(scheme) || scheme === appearance) return;
    appearance = scheme;
    fields.colorScheme.set(scheme === "system" ? systemScheme : scheme);
  };
  // The exports nsh-js's `env.rs` declares for `nsh/device` and
  // `nsh/accessibility` (`DEVICE_EXPORTS`, `ACCESSIBILITY_EXPORTS`), which
  // `nsh` serves too: one object each, so the namespaces the runtime
  // serves under those names (`modules`) are the same values as `nsh`'s.
  // `environment.test.mjs` holds both to `env.rs`.
  const device = {
    horizontalSizeClass: () => fields.horizontalSizeClass(),
    verticalSizeClass: () => fields.verticalSizeClass(),
    windowSize: () => fields.window(), colorScheme: () => shownScheme(), setColorScheme,
    // What the engine reports; a page has none to report (an empty list).
    reservedRegions: () => fields.reservedRegions(),
    fontScale: () => fields.fontScale(), pixelRatio: () => fields.pixelRatio(), locale: () => fields.locale(),
    localeSettings: () => fields.localeSettings(), lifecycle: () => fields.lifecycle(), platform,
    shadersAvailable: () => fields.shaders(),
    // The engine's count; a page has no system sleep to hear, so it stays 0.
    wakeCount: () => fields.wakeCount(),
    // The page's `navigator.onLine` and `navigator.connection`, as the
    // engine reports them.
    network: () => fields.network(),
    // A browser page has no performance monitor (its developer tools measure it),
    // so no stats either.
    fpsEnabled: () => false,
    perfStats: () => null,
  };
  const accessibility = { reduceMotion: () => fields.reduceMotion(), screenReaderEnabled: () => fields.screenReader() };
  // nsh-js's `appInfo` (`env.rs`): what the build wrote, handed over by
  // `setup` before the app's modules run.
  let appInfo = readAppInfo("{}");
  const api = {
    ...device, ...accessibility,
    appInfo: () => appInfo,
    safeArea: () => fields.safeArea(), safeAreaInsets: () => insets(), keyboardFrame: () => fields.keyboard(),
    screenReader: () => fields.screenReader(),
    token: name => readToken(402, name),
    // nsh-js's `role` (`env.rs`): the token the theme maps the role onto,
    // else the token of its name.
    role: name => readToken(414, name),
    // nsh-js's `setTheme`, `updateTheme` and `resetTheme` (`env.rs`): the
    // engine hands the write to the core with the turn's ops and returns
    // the diagnostics (`setTheme`, `updateTheme`), or throws its
    // `TypeError`; what read a token follows once the core resolved it
    // (`changedTokens`).
    setTheme: theme => invoke(406, [theme]),
    updateTheme: patch => invoke(409, [patch]),
    resetTheme(names) { invoke(410, [names]); },
    // `tokens` (`env.rs`): the token dictionary, as JSON from the engine, of
    // the scope around what reads it, else the surface's.
    tokens: {
      get: name => readDictionary(418, name) ?? undefined,
      kind: name => readDictionary(419, name) ?? undefined,
      names: filter => readDictionary(420, filter ?? null),
      entries: () => readDictionary(421, null),
    },
    // `setModifier`, `modifier` and `modifiers` (`env.rs`): the engine
    // checks the pick against the document active in this turn and hands
    // it to the core; the modifiers are read with the tokens, by shape.
    setModifier(name, context) { invoke(415, [name, context]); },
    modifier(name) { shape(); return invoke(416, [name]); },
    modifiers() { shape(); return JSON.parse(invoke(417, [])); },
  };
  // The engine's new tokens (`Host::set_tokens`, and the bundle's setup,
  // op 403), told what changed as JSON (`token_changes` in
  // `src/browser/mod.rs`): what read a token whose value changed runs
  // again, once however many it read, as nsh-js's `install` (`env.rs`)
  // runs it. A change of most of the names read, or roles mapped anew,
  // runs every reader through `tokens`.
  const changedTokens = json => {
    let changes = null;
    try { changes = typeof json === "string" ? JSON.parse(json) : null; } catch {}
    const names = Array.isArray(changes?.names) ? changes.names : [];
    // The signals of the names read that changed.
    const changed = names.map(name => named.get(name)).filter(Boolean);
    const whole = !changes || changes.whole === true || 2 * changed.length > named.size;
    reactive.batch(() => {
      if (!changes || changes.shape) bump(shape);
      if (whole) bump(tokens);
      else {
        for (const signal of changed) bump(signal);
        if (overflowed && names.length) bump(overflow);
      }
      bump(scopes);
    });
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
        const theme = props.theme, modifiers = props.modifiers, around = outer ? outer.tokens() : undefined;
        const object = value => value !== null && typeof value === "object";
        // No tokens of its own: the ones around it.
        if (!object(theme) && !object(modifiers)) { invoke(413, [slot, false]); return around; }
        if (around === undefined) scopes();
        return invoke(411, [slot, object(theme) ? theme : null, around ?? null, object(modifiers) ? modifiers : null]);
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
          // number; a reserved region's bounds, down to its origin).
          if (!same(value, previous, 4)) signal.set(value);
        }
      });
      dom.rootProp("layoutDirectionRtl", next.rtl);
    },
    // The engine's new tokens (`Host::set_tokens`): what read them follows.
    changedTokens,
    changedAppearance,
    // `setLocale`'s language, shown at once; the engine's next environment
    // carries it too.
    showLocale(tag) {
      reactive.batch(() => {
        const settings = reactive.untracked(fields.localeSettings);
        fields.localeSettings.set({ ...settings, languages: [tag, ...settings.languages.filter(l => l !== tag)] });
        fields.locale.set(tag);
      });
    },
    setup(tailwind, theme, scheme, statusBar, app) {
      if (typeof tailwind === "string") dom.rootProp("tailwindTheme", tailwind);
      // The app's theme (`{ document, overrides }`): the engine hands it to
      // the core with this turn's ops and reads it from now on (nsh-js's
      // `take_app_theme`), saying what changed for what read the tokens.
      if (typeof theme === "string") {
        const changes = invoke(403, [theme]);
        if (typeof changes === "string") changedTokens(changes);
      }
      if (["light", "dark", "system"].includes(scheme)) setColorScheme(scheme);
      if (typeof statusBar === "string") invoke(405, [statusBar]);
      if (typeof app === "string") appInfo = readAppInfo(app);
    },
  };
}
