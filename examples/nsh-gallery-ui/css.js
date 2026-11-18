// The HTML website consumes the same data as NSH's example components.
// This module has no NSH runtime dependency: Vite uses it while building CSS.

const prefix = '--nsh-gallery-';
const references = (value) => value.replace(/var\(\s*--([\w-]+)/g, `var(${prefix}$1`);
const kebab = (name) => name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`);
const reference = (value) => (typeof value === 'string' ? /^\{([^{}]+)\}$/.exec(value)?.[1] : undefined);

/**
 * The values of theme document `theme`'s tokens at context `context` of its
 * `colorMode` modifier (docs/dev/theme-format.md): each token's value with
 * the context's over it, references resolved.
 */
export function resolvedTokens(theme, context) {
  const values = Object.fromEntries(Object.entries(theme.tokens).map(([name, token]) => [name, token.value]));
  Object.assign(values, theme.modifiers?.colorMode?.contexts?.[context] ?? {});
  const resolve = (name, seen) => {
    const target = reference(values[name]);
    if (target === undefined) return values[name];
    if (seen.has(target) || !(target in values)) throw new Error(`theme.json: \`${name}\` refers to \`${target}\`, which does not resolve`);
    return resolve(target, new Set([...seen, target]));
  };
  return Object.fromEntries(Object.keys(values).map((name) => [name, resolve(name, new Set([name]))]));
}

/** A token's value as CSS: a color as written, a number as a length in pixels (the theme's numbers are points). */
function css(name, kind, value) {
  if (kind === 'number') return `${value}px`;
  if (kind === 'color' || kind === 'font' || kind === 'string') return String(value);
  throw new Error(`theme.json: \`${name}\` is a ${kind}, which the website's CSS has no form for`);
}

/** CSS variables for the canonical palette and surface finishes. */
export function galleryUiCss(theme, finishes) {
  const surfaces = Object.keys(finishes.dark);
  const properties = Object.fromEntries(surfaces.map((surface) => [
    surface,
    [...new Set([...Object.keys(finishes.dark[surface]), ...Object.keys(finishes.light[surface])])],
  ]));
  return [[':root', 'dark'], ['html.light', 'light']].map(([selector, mode]) => {
    const values = resolvedTokens(theme, mode);
    const declarations = Object.entries(values).map(([name, value]) => `${prefix}${name}: ${css(name, theme.tokens[name].kind, value)};`);
    for (const surface of surfaces) {
      for (const property of properties[surface]) {
        const value = finishes[mode][surface][property];
        declarations.push(`${prefix}finish-${surface}-${kebab(property)}: ${value ? references(value) : 'initial'};`);
      }
    }
    return `${selector} {\n  ${declarations.join('\n  ')}\n}`;
  }).join('\n');
}
