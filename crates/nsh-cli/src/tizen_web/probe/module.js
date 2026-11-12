// A module script from the package (`nsh.js` is one): its own probe, then
// the page's import map, statically (a module importing a mapped name, as
// the app's modules import their chunks) and with `import()`.
nshProbe.done("module", true, import.meta.url);
import("./import-map.js").catch((error) => nshProbe.done("import-map", false, error.message));
import("nsh-probe-chunk").then(
  (m) => nshProbe.done("import-map-dynamic", m.chunk === "chunk", "nsh-probe-chunk: " + m.chunk),
  (error) => nshProbe.done("import-map-dynamic", false, error.message),
);
