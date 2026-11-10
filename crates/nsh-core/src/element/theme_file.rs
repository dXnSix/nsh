//! Theme files: shadcn/ui's `cssVars` (`{ "cssVars": { "theme": { … },
//! "light": { … }, "dark": { … } } }`), values as CSS writes them (colors,
//! `var(--ring)`, `color-mix()`, lengths in `rem` or `px`, font family
//! lists), read into [`Tokens`] by name. The reader gives no name a
//! meaning: a name is a token, whatever the theme calls it, and a value is
//! read by what it is (a color, a length, a font family list for a
//! `font-*` name). What it cannot read is left out and listed, never an
//! error; only text that is not JSON is.
//!
//! A UI set reads the app's theme with it (`nsh-base`'s `read_tokens`, over
//! its defaults); an embedding app's overrides are read with [`overrides`].

use rustc_hash::{FxHashMap, FxHashSet};

use super::{AdaptiveColor, AdaptiveGradient, AdaptiveShadow, Tokens};
use crate::{Color, JsonValue, color::Hsla};

/// Points per `rem`, as Tailwind's theme has them.
const REM: f32 = 16.0;

/// The tokens of theme file `json` over `base` (a UI set's defaults), with
/// what of the file it left out (a value it cannot read, a section of a
/// form it does not know). A color it names and does not set
/// (`var(--ring)`) is `base`'s. `Err`: `json` is not JSON.
pub fn parse(json: &str, base: &Tokens) -> Result<(Tokens, Vec<String>), String> {
    let mut tokens = base.clone();
    let ignored = apply_onto(&mut tokens, base, json)?;
    Ok((tokens, sentences(ignored)))
}

/// The tokens theme file `json` sets, and only those, with what of it was
/// left out: an embedding app's overrides of the app's theme, or a change
/// of some of the app's tokens (`nsh`'s `updateTheme`). A color it names
/// and does not set is `base`'s. `Err`: `json` is not JSON.
pub fn overrides(json: &str, base: &Tokens) -> Result<(Tokens, Vec<String>), String> {
    let mut tokens = Tokens::default();
    let ignored = apply_onto(&mut tokens, base, json)?;
    Ok((tokens, sentences(ignored)))
}

/// One thing a reader left out of a theme file: its sentence, as the
/// readers list it and the logs and the toolchain print it, with what the
/// core itself asks of it, so that nothing reads the sentence back.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LeftOut {
    /// The token the sentence starts with, in backquotes (`None`: it is
    /// about the file's form, or names a section's key another way).
    token: Option<Box<str>>,
    /// Whether it is a name no theme type declares, and nothing else: a
    /// build warning, and left out at run time, but no reason to refuse a
    /// file read whole (`setTheme`), as a newer theme's names are not
    /// (#1190).
    undeclared: bool,
    sentence: String,
}

impl LeftOut {
    /// `` `token` `` followed by `rest` (`": no color value"`).
    fn about(token: &str, rest: impl std::fmt::Display) -> Self {
        Self {
            token: Some(token.into()),
            undeclared: false,
            sentence: format!("`{token}`{rest}"),
        }
    }

    /// A sentence about no one token.
    pub(crate) fn said(sentence: impl Into<String>) -> Self {
        Self {
            token: None,
            undeclared: false,
            sentence: sentence.into(),
        }
    }

    /// `name` is a name no theme type declares.
    fn undeclared(name: &str) -> Self {
        Self {
            undeclared: true,
            ..Self::about(name, format_args!(": {UNDECLARED}"))
        }
    }

    /// Whether it is only a name no theme type declares.
    pub(crate) fn is_undeclared(&self) -> bool {
        self.undeclared
    }

    /// Whether its sentence starts with the token `name`.
    fn is_about(&self, name: &str) -> bool {
        self.token.as_deref() == Some(name)
    }
}

impl std::fmt::Display for LeftOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.sentence)
    }
}

/// What was left out, as the readers list it.
pub(crate) fn sentences(left_out: Vec<LeftOut>) -> Vec<String> {
    left_out.into_iter().map(|l| l.sentence).collect()
}

/// [`parse`] under a theme set: each name the file sets must be a declared
/// one (a role's name, `primary`, sets the token the role is mapped onto),
/// and holds a value of its declared kind; any other is left out and listed.
/// `var()` names `base`'s tokens (the types' defaults).
pub(crate) fn parse_declared(
    json: &str,
    base: &Tokens,
    set: &super::theme::ThemeSet,
) -> Result<(Tokens, Vec<LeftOut>), String> {
    let (tokens, left_out) = overrides_declared(json, base, set)?;
    Ok((tokens.over(base), left_out))
}

/// What a theme read under types says of a name no type declares
/// ([`LeftOut::is_undeclared`]).
const UNDECLARED: &str = "no theme type declares it";

/// [`overrides`] under a theme set, as [`parse_declared`] reads names. The
/// file is parsed once: its names become the set's in the parsed document,
/// which is then read as any theme file's is.
pub(crate) fn overrides_declared(
    json: &str,
    base: &Tokens,
    set: &super::theme::ThemeSet,
) -> Result<(Tokens, Vec<LeftOut>), String> {
    let mut value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let (kept, mut left_out) = declared_names(&mut value, set);
    let mut read = Tokens::default();
    left_out.extend(read_into(&mut read, base, &value));
    let mut out = Tokens::default();
    for (name, _, kind) in set.names() {
        if kind.holds(&read, name) {
            out.copy_named(&read, name);
        } else if kept.iter().any(|k| k == name) && !left_out.iter().any(|l| l.is_about(name)) {
            // Set, and read as no value of its kind.
            left_out.push(LeftOut::about(
                name,
                format_args!(": no {} value", kind.name()),
            ));
        }
    }
    Ok((out, left_out))
}

/// Makes the names of theme file `value` the set's: a role's name becomes
/// the token it is mapped onto (and so in `var()`s), a name no type
/// declares is left out; returns the names kept and what was left out. A
/// section that sets a token and the role mapped onto it keeps the token's
/// value; one that sets a token through two of the roles it plays, with
/// two values, keeps the first's (by name).
fn declared_names(
    value: &mut JsonValue,
    set: &super::theme::ThemeSet,
) -> (Vec<String>, Vec<LeftOut>) {
    let mut left_out = Vec::new();
    let mut kept = Vec::new();
    // The roles the types map onto tokens of other names.
    let renamed: Vec<(&str, &str)> = set.renamed_roles().collect();
    if let Some(vars) = value.get_mut("cssVars").and_then(JsonValue::as_object_mut) {
        for section in ["theme", "light", "dark"] {
            let Some(JsonValue::Object(fields)) = vars.get_mut(section) else {
                continue;
            };
            let old = std::mem::take(fields);
            // The tokens roles' names set in this section, with the role.
            let mut by_role: Vec<(&str, &str)> = Vec::new();
            // By name, whatever order the file has them in (and the map
            // keeps, with serde_json's `preserve_order`): which of two
            // roles' names sets their token must not depend on it.
            let mut keys: Vec<(&String, &JsonValue)> = old.iter().collect();
            keys.sort_by(|a, b| a.0.cmp(b.0));
            for (key, v) in keys {
                match set.target(key) {
                    Some(name) if name == key => {
                        fields.insert(key.clone(), v.clone());
                        kept.push(key.clone());
                    }
                    Some(name) => {
                        if old.contains_key(name) {
                            left_out.push(LeftOut::about(
                                key,
                                format_args!(
                                    ": the token its role is mapped onto, `{name}`, is set too"
                                ),
                            ));
                            continue;
                        }
                        // Another role the token plays set it here: the
                        // same value twice is one value, another is left
                        // out as the token set beside its role is.
                        if let Some((_, first)) = by_role.iter().find(|(token, _)| *token == name) {
                            if fields.get(name) != Some(v) {
                                left_out.push(LeftOut::about(
                                    key,
                                    format_args!(
                                        ": `{first}` sets the token its role is mapped onto, `{name}`, too"
                                    ),
                                ));
                            }
                            continue;
                        }
                        by_role.push((name, key));
                        fields.insert(name.to_owned(), v.clone());
                        kept.push(name.to_owned());
                    }
                    None => left_out.push(LeftOut::undeclared(key)),
                }
            }
        }
        // A `var()` naming a role names the token it is mapped onto.
        if !renamed.is_empty() {
            for section in ["theme", "light", "dark"] {
                let Some(JsonValue::Object(fields)) = vars.get_mut(section) else {
                    continue;
                };
                for v in fields.values_mut() {
                    if let JsonValue::String(text) = v
                        && text.contains("var(")
                    {
                        *text = rename_vars(text, &renamed);
                    }
                }
            }
        }
    }
    (kept, left_out)
}

/// `value` with each `var()` that names one of `renamed`'s roles naming
/// its token: `var(--popover)`, `var( --popover , #fff)` and one inside a
/// fallback, as the reader takes them ([`resolve_color`]). Every other
/// character stays; an unclosed `var(` is left for the reader to list.
fn rename_vars(value: &str, renamed: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start + 4]);
        let inner = &rest[start + 4..];
        let mut depth = 1;
        let end = inner
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map(|(i, _)| i);
        let Some(end) = end else {
            rest = inner;
            break;
        };
        let (var, fallback) = match inner[..end].split_once(',') {
            Some((v, f)) => (v, Some(f)),
            None => (&inner[..end], None),
        };
        let token = var
            .trim()
            .strip_prefix("--")
            .and_then(|name| renamed.iter().find(|(role, _)| *role == name))
            .map(|(_, token)| *token);
        match token {
            // The name alone changes; the space around it stays.
            Some(token) => {
                let name = var.trim();
                let at = var.find(name).unwrap_or(0);
                out.push_str(&var[..at]);
                out.push_str("--");
                out.push_str(token);
                out.push_str(&var[at + name.len()..]);
            }
            None => out.push_str(var),
        }
        if let Some(fallback) = fallback {
            out.push(',');
            out.push_str(&rename_vars(fallback, renamed));
        }
        out.push(')');
        rest = &inner[end + 1..];
    }
    out.push_str(rest);
    out
}

/// `tokens` as a theme file [`parse`] reads back: shadcn's `cssVars`, a
/// value both appearances share in `theme`, the others in `light` and
/// `dark`; colors as `#rrggbbaa`, numbers as numbers, font families and box
/// shadows as CSS writes them. Roles are no part of it.
pub fn write(tokens: &Tokens) -> String {
    let mut theme = serde_json::Map::new();
    let mut light = serde_json::Map::new();
    let mut dark = serde_json::Map::new();
    let hex = |c: Hsla| JsonValue::from(format!("#{:08x}", Color::from(c).0));
    let mut put = |name: &str, l: JsonValue, d: JsonValue| {
        if l == d {
            theme.insert(name.to_owned(), l);
        } else {
            light.insert(name.to_owned(), l);
            dark.insert(name.to_owned(), d);
        }
    };
    for (name, c) in tokens.colors() {
        put(name, hex(c.light), hex(c.dark));
    }
    for (name, n) in tokens.numbers() {
        let d = tokens.number_in(name, true).unwrap_or(n);
        put(
            name,
            JsonValue::from(n.to_string()),
            JsonValue::from(d.to_string()),
        );
    }
    for (name, f) in tokens.fonts() {
        put(
            name,
            JsonValue::from(f.to_string()),
            JsonValue::from(f.to_string()),
        );
    }
    for (name, s) in tokens.shadows() {
        put(
            name,
            JsonValue::from(s.css(false)),
            JsonValue::from(s.css(true)),
        );
    }
    for (name, g) in tokens.gradients() {
        put(
            name,
            JsonValue::from(g.css(false)),
            JsonValue::from(g.css(true)),
        );
    }
    let mut vars = serde_json::Map::new();
    for (key, section) in [("theme", theme), ("light", light), ("dark", dark)] {
        if !section.is_empty() {
            vars.insert(key.to_owned(), JsonValue::Object(section));
        }
    }
    serde_json::json!({ "cssVars": vars }).to_string()
}

/// Whether `json` can be a theme file at all: it is JSON. What a reader
/// leaves out of one is no error.
pub fn check(json: &str) -> Result<(), String> {
    serde_json::from_str::<JsonValue>(json)
        .map(|_| ())
        .map_err(|e| format!("not JSON: {e}"))
}

/// Sets the tokens of theme file `json` in `tokens`; a color it names and
/// does not set is `base`'s. Returns what it left out.
fn apply_onto(tokens: &mut Tokens, base: &Tokens, json: &str) -> Result<Vec<LeftOut>, String> {
    let value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    Ok(read_into(tokens, base, &value))
}

/// [`apply_onto`] of a theme file already parsed.
fn read_into(tokens: &mut Tokens, base: &Tokens, value: &JsonValue) -> Vec<LeftOut> {
    let mut ignored = Vec::new();
    match value.get("cssVars") {
        Some(vars) => apply_css_vars(tokens, base, vars, &mut ignored),
        None => ignored.push(LeftOut::said("no shadcn `{ \"cssVars\": { ... } }`")),
    }
    ignored
}

/// A CSS color.
fn color(value: &str) -> Option<Hsla> {
    Color::parse(value).map(Hsla::from)
}

/// Whether `value` is written as a color, which a theme means as one: a hex
/// color, a CSS color function or a `var()` alone. A value of no kind a
/// token has (a shadow, a length in `em`) is not.
fn written_as_color(value: &str) -> bool {
    let value = value.trim();
    if value.starts_with('#') {
        return true;
    }
    let Some((function, _)) = value.split_once('(') else {
        return false;
    };
    value.ends_with(')')
        && matches!(
            function.trim().to_ascii_lowercase().as_str(),
            "rgb"
                | "rgba"
                | "hsl"
                | "hsla"
                | "hwb"
                | "lab"
                | "lch"
                | "oklab"
                | "oklch"
                | "color"
                | "color-mix"
                | "light-dark"
                | "var"
        )
}

/// A CSS length in points: `0.625rem`, `10px`, `10`.
fn length(value: &str) -> Option<f32> {
    let value = value.trim();
    let (number, scale) = if let Some(n) = value.strip_suffix("rem") {
        (n, REM)
    } else if let Some(n) = value.strip_suffix("px") {
        (n, 1.0)
    } else {
        (value, 1.0)
    };
    number
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|n| n * scale)
}

/// The family of a CSS `font-family` list that the platform draws with:
/// its first, unquoted (`None`: a generic family, the system's font).
fn font_family(value: &str) -> Option<&str> {
    let first = value.split(',').next()?.trim().trim_matches(['"', '\'']);
    let generic = matches!(
        first,
        "" | "sans-serif"
            | "serif"
            | "system-ui"
            | "ui-sans-serif"
            | "ui-serif"
            | "-apple-system"
            | "BlinkMacSystemFont"
    );
    (!generic).then_some(first)
}

/// Applies shadcn's `cssVars`: `theme`'s values in both appearances,
/// `light`'s in the light one (and the dark one when `dark` leaves them
/// out), `dark`'s in the dark one. A color may name another
/// (`var(--ring)`, in `color-mix()` too) of the same appearance: the
/// file's or the one under it. A `shadow-*` value that reads as CSS
/// `box-shadow` layers is a shadow. Values of no kind a token has (lengths
/// in `em`, as shadcn's `tracking-normal: 0em`) are left out, as
/// NSH draws no utility with them; a `tracking-*` token is a length in
/// points (`px`, `rem`). A value written as a color that is none is left
/// out, and one that is no color where the other appearance has a color
/// gives way to that color; both are listed in `ignored`.
fn apply_css_vars(
    tokens: &mut Tokens,
    base: &Tokens,
    vars: &JsonValue,
    ignored: &mut Vec<LeftOut>,
) {
    let mut section = |name: &str| -> FxHashMap<String, String> {
        let Some(v) = vars.get(name) else {
            return FxHashMap::default();
        };
        let JsonValue::Object(fields) = v else {
            ignored.push(LeftOut::said(format!("cssVars.{name}: expected an object")));
            return FxHashMap::default();
        };
        let mut out = FxHashMap::default();
        for (key, value) in fields {
            match value {
                JsonValue::Null => {}
                JsonValue::String(s) => {
                    out.insert(key.clone(), s.clone());
                }
                JsonValue::Number(n) => {
                    out.insert(key.clone(), n.to_string());
                }
                v => ignored.push(LeftOut::said(format!(
                    "cssVars.{name}.{key}: expected a CSS value, got {v}"
                ))),
            }
        }
        out
    };
    let (both, light, dark) = (section("theme"), section("light"), section("dark"));
    let mut light_vars = both.clone();
    light_vars.extend(light.iter().map(|(k, v)| (k.clone(), v.clone())));
    let mut dark_vars = light_vars.clone();
    dark_vars.extend(dark.iter().map(|(k, v)| (k.clone(), v.clone())));

    let mut names: Vec<&String> = light_vars.keys().chain(dark.keys()).collect();
    names.sort();
    names.dedup();
    for name in names {
        let value = dark_vars.get(name).or_else(|| light_vars.get(name));
        let Some(value) = value else { continue };
        // A box shadow (`shadow-raised`: `0 1px 2px #0000001a`), per
        // appearance, as Tailwind's `--shadow-*`: a `shadow-*` value that
        // reads as CSS `box-shadow` layers. Any other (shadcn's
        // `shadow-color`, `shadow-opacity`) is read as before.
        if name.starts_with("shadow-")
            && let Some(dark_layers) = dark_vars.get(name).and_then(|v| crate::css::box_shadows(v))
        {
            let light_layers = match light_vars.get(name) {
                Some(v) => crate::css::box_shadows(v),
                None => Some(dark_layers.clone()),
            };
            if let Some(light_layers) = light_layers {
                tokens.set_shadow(name, AdaptiveShadow::new(light_layers, dark_layers));
                continue;
            }
        }
        // A gradient (`background-image-hero`: `linear-gradient(to right,
        // #7c3aed, #db2777)`), per appearance, as Tailwind's
        // `--background-image-*`: a `background-image-*` value that reads
        // as CSS gradients, one or several. A `var()` in a stop is the
        // theme's color in that appearance, as it is in a color. Any other
        // value is read as before.
        if name.starts_with(GRADIENTS) {
            let read = |vars: &FxHashMap<String, String>, dark: bool| {
                let css = substituted(name, vars, base, dark, &mut FxHashSet::default())?;
                Ok(css
                    .and_then(|css| crate::css::gradients(&css))
                    .filter(|layers| !layers.is_empty()))
            };
            match (read(&light_vars, false), read(&dark_vars, true)) {
                (Ok(Some(light)), Ok(Some(dark))) => {
                    tokens.set_gradient(name, AdaptiveGradient::new(light, dark));
                    continue;
                }
                (Err(e), _) | (_, Err(e)) => {
                    ignored.push(e);
                    continue;
                }
                // No gradient in an appearance: read as any other value.
                _ => {}
            }
        }
        // A weight (`font-weight-strong`: `650`) is a number, as
        // Tailwind's `--font-weight-*`; every other `font-*` a family.
        if name.starts_with("font-weight-")
            && let Some(dark_weight) = length(value)
        {
            // Per appearance, as the other numbers (below).
            let light_weight = light_vars
                .get(name)
                .and_then(|v| length(v))
                .unwrap_or(dark_weight);
            tokens.set_number(name, light_weight);
            if dark_weight != light_weight {
                tokens.set_dark_number(name, dark_weight);
            }
            continue;
        }
        if name.starts_with("font-") {
            if let Some(family) = font_family(value) {
                tokens.set_font(name, family);
            }
            continue;
        }
        let resolve = |vars: &FxHashMap<String, String>, dark: bool| {
            resolve_color(name, vars, base, dark, &mut FxHashSet::default())
        };
        let (light, dark) = match (resolve(&light_vars, false), resolve(&dark_vars, true)) {
            (Ok(light), Ok(dark)) => (light, dark),
            (Err(e), _) | (_, Err(e)) => {
                ignored.push(e);
                continue;
            }
        };
        match (light, dark) {
            (Some(l), Some(d)) => tokens.set_color(name, AdaptiveColor { light: l, dark: d }),
            (Some(c), None) | (None, Some(c)) => {
                // Set in one appearance only, the color is both's; given in
                // the other too, that value is no color.
                let (other, vars) = match light {
                    Some(_) => ("dark", &dark_vars),
                    None => ("light", &light_vars),
                };
                if let Some(v) = vars.get(name) {
                    ignored.push(LeftOut::about(
                        name,
                        format_args!(": {v} is no color in the {other} appearance"),
                    ));
                }
                tokens.set_color(name, c.into());
            }
            (None, None) => match length(value) {
                // The dark appearance's value (`dark`'s, else the light
                // one's), and the light one's: given in one appearance
                // only, a number is both's, as a color is.
                Some(dark_n) => {
                    let light_n = light_vars
                        .get(name)
                        .and_then(|v| length(v))
                        .unwrap_or(dark_n);
                    tokens.set_number(name, light_n);
                    if dark_n != light_n {
                        tokens.set_dark_number(name, dark_n);
                    }
                }
                None => {
                    let mut given: Vec<&String> = [light_vars.get(name), dark_vars.get(name)]
                        .into_iter()
                        .flatten()
                        .collect();
                    given.dedup();
                    for v in given.into_iter().filter(|v| written_as_color(v)) {
                        ignored.push(LeftOut::about(name, format_args!(": {v} is no color")));
                    }
                }
            },
        }
    }
}

/// The namespace of gradient tokens, Tailwind's `--background-image-*`:
/// `background-image-hero` is the class `bg-hero`.
pub(crate) const GRADIENTS: &str = "background-image-";

/// The color `name` is in `vars` (of one appearance), with the colors it
/// names (`var(--x)`, `var(--x, fallback)`) resolved: the file's, else
/// `base`'s. `None` when it is no color. A name that names itself is an
/// error.
fn resolve_color(
    name: &str,
    vars: &FxHashMap<String, String>,
    base: &Tokens,
    dark: bool,
    seen: &mut FxHashSet<String>,
) -> Result<Option<Hsla>, LeftOut> {
    if !vars.contains_key(name) {
        return Ok(base
            .color(name)
            .map(|c| if dark { c.dark } else { c.light }));
    }
    Ok(substituted(name, vars, base, dark, seen)?.and_then(|css| color(&css)))
}

/// The value of `name` in `vars` (of one appearance) with each `var(--x)`
/// and `var(--x, fallback)` in it written as the color it names
/// ([`resolve_color`]), or its fallback. `None` when `vars` have no
/// `name`, or a variable in it is no color and has no fallback.
fn substituted(
    name: &str,
    vars: &FxHashMap<String, String>,
    base: &Tokens,
    dark: bool,
    seen: &mut FxHashSet<String>,
) -> Result<Option<String>, LeftOut> {
    let Some(value) = vars.get(name) else {
        return Ok(None);
    };
    if !seen.insert(name.to_owned()) {
        return Err(LeftOut::about(name, " names itself"));
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value.as_str();
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let inner = &rest[start + 4..];
        let mut depth = 1;
        let end = inner
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map(|(i, _)| i)
            .ok_or_else(|| LeftOut::about(name, ": unclosed var()"))?;
        let (var, fallback) = match inner[..end].split_once(',') {
            Some((v, f)) => (v.trim(), Some(f.trim())),
            None => (inner[..end].trim(), None),
        };
        let Some(var) = var.strip_prefix("--") else {
            return Err(LeftOut::about(name, ": var() names no `--` variable"));
        };
        match resolve_color(var, vars, base, dark, seen)? {
            Some(c) => out.push_str(&format!("#{:08x}", Color::from(c).0)),
            None => match fallback {
                Some(f) => out.push_str(f),
                None => {
                    seen.remove(name);
                    return Ok(None);
                }
            },
        }
        rest = &inner[end + 1..];
    }
    out.push_str(rest);
    seen.remove(name);
    Ok(Some(out))
}

/// The reader of theme files, name-agnostic: the same tokens and the same
/// list of what it left out as `nsh-base`'s theme gave before it moved
/// here, for any names.
#[cfg(test)]
mod reads {
    use super::*;
    use crate::color::rgb;

    fn hex(c: Hsla) -> u32 {
        Color::from(c).0 >> 8
    }

    fn base() -> Tokens {
        let mut base = Tokens::default();
        base.set_color("line", AdaptiveColor::rgb(0x111111, 0x222222));
        base.set_number("radius", 10.0);
        base
    }

    #[test]
    fn sections_go_over_the_base_in_their_appearances() {
        let (tokens, ignored) = parse(
            r##"{ "cssVars": {
                "theme": { "radius": "0.5rem", "radius-card": "14px", "font-sans": "Inter, sans-serif",
                           "font-mono": "'JetBrains Mono', monospace", "gap": 3 },
                "light": { "surface": "#7c3aed", "ring": "var(--surface)",
                           "soft": "color-mix(in oklab, var(--surface) 50%, white)" },
                "dark": { "surface": "rgb(0 0 255)" }
            } }"##,
            &base(),
        )
        .unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
        let surface = tokens.color("surface").unwrap();
        assert_eq!(
            (hex(surface.light), hex(surface.dark)),
            (0x7c3aed, 0x0000ff)
        );
        // `var()` in each appearance: dark has no `ring`, so light's
        // `var(--surface)` is read there with dark's surface.
        let ring = tokens.color("ring").unwrap();
        assert_eq!((hex(ring.light), hex(ring.dark)), (0x7c3aed, 0x0000ff));
        assert!(tokens.color("soft").is_some());
        assert_eq!(tokens.number("radius"), Some(8.0));
        assert_eq!(tokens.number("radius-card"), Some(14.0));
        assert_eq!(tokens.number("gap"), Some(3.0));
        assert_eq!(tokens.font("font-sans").map(|f| &**f), Some("Inter"));
        assert_eq!(
            tokens.font("font-mono").map(|f| &**f),
            Some("JetBrains Mono")
        );
        // The base's tokens stay.
        assert_eq!(tokens.color("line"), base().color("line"));
    }

    #[test]
    fn what_it_cannot_read_is_left_out_and_listed() {
        let (tokens, mut ignored) = parse(
            r##"{ "cssVars": {
                "theme": { "shadow-sm": "0 1px 2px #0000", "letter-spacing": "0em" },
                "light": { "a": "var(--b)", "b": "var(--a)", "brand": "#e11d48", "edge": "#zzz",
                           "glow": "lab(50% 40 59)", "ink": "var(--nothing)", "odd": [1],
                           "accent": "#7c3aed" },
                "dark": { "brand": "#12", "accent": "oklch(0.7 0.1" }
            } }"##,
            &base(),
        )
        .unwrap();
        ignored.sort();
        assert_eq!(
            ignored,
            [
                "`a` names itself",
                "`accent`: oklch(0.7 0.1 is no color in the dark appearance",
                "`b` names itself",
                "`brand`: #12 is no color in the dark appearance",
                "`edge`: #zzz is no color",
                "`glow`: lab(50% 40 59) is no color",
                "`ink`: var(--nothing) is no color",
                "cssVars.light.odd: expected a CSS value, got [1]",
            ]
        );
        let brand = tokens.color("brand").unwrap();
        assert_eq!((hex(brand.light), hex(brand.dark)), (0xe11d48, 0xe11d48));
        assert_eq!(tokens.color("edge"), None);
        // A box shadow is a shadow token; a letter spacing in `em` is none.
        assert!(tokens.shadow("shadow-sm").is_some());
        assert_eq!(tokens.get("letter-spacing"), None);
        let (_, ignored) = parse(r#"{ "cssVars": { "dark": "nope" } }"#, &base()).unwrap();
        assert_eq!(ignored, ["cssVars.dark: expected an object"]);
    }

    #[test]
    fn another_form_is_left_out_and_only_text_that_is_no_json_fails() {
        let (tokens, ignored) = parse(r#"{ "colors": {} }"#, &base()).unwrap();
        assert_eq!(tokens, base());
        assert_eq!(ignored.len(), 1);
        assert!(parse("not json", &base()).is_err());
        assert!(check("not json").is_err());
        assert!(check(r#"{ "colors": {} }"#).is_ok());
    }

    #[test]
    fn a_var_falls_back_then_reads_the_base() {
        let (tokens, _) = parse(
            r##"{ "cssVars": { "light": { "edge": "var(--nothing, #ff0000)", "fence": "var(--line)" } } }"##,
            &base(),
        )
        .unwrap();
        assert_eq!(hex(tokens.color("edge").unwrap().light), 0xff0000);
        assert_eq!(tokens.color("fence"), base().color("line"));
    }

    #[test]
    fn overrides_are_the_tokens_the_file_sets() {
        let (tokens, ignored) = overrides(
            r##"{ "cssVars": { "theme": { "radius": "12px" },
                "light": { "primary": "#0000ff", "ring": "var(--primary)", "focus": "var(--line)" } } }"##,
            &base(),
        )
        .unwrap();
        assert!(ignored.is_empty());
        assert_eq!(tokens.color("primary"), Some(rgb(0x0000ff).into()));
        assert_eq!(tokens.color("ring"), tokens.color("primary"));
        assert_eq!(tokens.color("focus"), base().color("line"));
        assert_eq!(tokens.number("radius"), Some(12.0));
        assert_eq!(tokens.colors().count(), 3);
        assert_eq!(tokens.color("line"), None);
        assert!(overrides("not json", &base()).is_err());
    }
}

/// A theme file under theme types, parsed once: a `var()` naming a role
/// names its token however it is spaced, and what was left out says which
/// token it is about and whether it is only a name no type declares, in the
/// sentences it always had.
#[cfg(test)]
mod declared_once {
    use super::*;
    use crate::{
        color::rgb,
        element::{AnyRole, TokenKind, role, theme::ThemeSet},
    };

    /// A design system: `surface-overlay` plays `popover`, beside an ink
    /// and the cards' radius.
    fn set() -> ThemeSet {
        let mut defaults = Tokens::default();
        defaults.set_color("surface-overlay", rgb(0x111111).into());
        defaults.set_color("ink", rgb(0x222222).into());
        defaults.set_number("radius-card", 14.0);
        let popover = [AnyRole::from(role::POPOVER)];
        let mut set = ThemeSet::empty();
        set.try_add(
            "ds::Theme",
            [
                ("surface-overlay", TokenKind::Color, &popover[..]),
                ("ink", TokenKind::Color, &[][..]),
                ("radius-card", TokenKind::Radius, &[][..]),
            ],
            defaults,
        )
        .unwrap();
        set
    }

    fn file(json: &str) -> String {
        format!(r#"{{ "cssVars": {json} }}"#)
    }

    fn light(tokens: &Tokens, name: &str) -> Option<Color> {
        tokens.color(name).map(|c| Color::from(c.light))
    }

    #[test]
    fn a_var_naming_a_role_names_its_token_however_it_is_spaced() {
        let set = set();
        for var in [
            "var(--popover)",
            "var( --popover )",
            "var(--popover , #ffffff)",
            "var(  --popover,#ffffff )",
            "color-mix(in srgb, var( --popover ) 100%, #000000)",
        ] {
            let (tokens, left_out) = parse_declared(
                &file(&format!(
                    r##"{{ "light": {{ "surface-overlay": "#123456", "ink": "{var}" }} }}"##
                )),
                &Tokens::default(),
                &set,
            )
            .unwrap();
            assert!(left_out.is_empty(), "{var}: {left_out:?}");
            assert_eq!(
                light(&tokens, "ink"),
                Some(Color::from(rgb(0x123456))),
                "{var}"
            );
        }
    }

    #[test]
    fn only_the_roles_name_changes_in_a_var() {
        let renamed = [("popover", "surface-overlay"), ("card", "surface")];
        let renames = |value: &str| rename_vars(value, &renamed);
        assert_eq!(renames("var(--popover)"), "var(--surface-overlay)");
        assert_eq!(
            renames("var( --popover , #fff )"),
            "var( --surface-overlay , #fff )"
        );
        // In a fallback, and in a function's arguments.
        assert_eq!(
            renames("var(--ink, var(--card, var( --popover )))"),
            "var(--ink, var(--surface, var( --surface-overlay )))"
        );
        assert_eq!(
            renames("color-mix(in oklab, var(--popover) 40%, var(--ring))"),
            "color-mix(in oklab, var(--surface-overlay) 40%, var(--ring))"
        );
        // Another name that starts as a role's, no variable, an unclosed
        // one: as they were, for the reader to take or to list.
        for same in [
            "var(--popover-foreground)",
            "var(popover)",
            "var(--popover",
            "#123456",
            "",
        ] {
            assert_eq!(renames(same), same);
        }
    }

    #[test]
    fn what_was_left_out_says_its_token_and_whether_it_is_only_undeclared() {
        let (tokens, left_out) = overrides_declared(
            &file(
                r##"{ "light": { "brand": "#000000", "ink": "#zz", "popover": "#333333", "surface-overlay": "#444444" },
                      "theme": { "radius-card": "wide" } }"##,
            ),
            &Tokens::default(),
            &set(),
        )
        .unwrap();
        let mut seen: Vec<(String, bool)> = left_out
            .iter()
            .map(|l| (l.to_string(), l.is_undeclared()))
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            [
                ("`brand`: no theme type declares it".to_owned(), true),
                ("`ink`: #zz is no color".to_owned(), false),
                (
                    "`popover`: the token its role is mapped onto, `surface-overlay`, is set too"
                        .to_owned(),
                    false
                ),
                ("`radius-card`: no radius value".to_owned(), false),
            ]
        );
        // `ink` is listed once: the reader's sentence, not "no color value"
        // too. Each sentence is about the token it starts with.
        assert_eq!(left_out.iter().filter(|l| l.is_about("ink")).count(), 1);
        assert!(left_out.iter().all(|l| {
            l.token
                .as_deref()
                .is_some_and(|t| l.sentence.starts_with(&format!("`{t}`")))
        }));
        assert_eq!(
            light(&tokens, "surface-overlay"),
            Some(Color::from(rgb(0x444444)))
        );
        // The strings the readers list are the sentences.
        let listed = sentences(left_out.clone());
        assert_eq!(listed.len(), left_out.len());
        assert!(listed.contains(&"`brand`: no theme type declares it".to_owned()));
    }

    #[test]
    fn a_file_of_no_form_is_listed_as_it_was() {
        let (tokens, left_out) = overrides_declared("{}", &Tokens::default(), &set()).unwrap();
        assert!(tokens.is_empty());
        assert_eq!(
            sentences(left_out),
            ["no shadcn `{ \"cssVars\": { ... } }`"]
        );
        let (_, left_out) = overrides_declared(
            &file(r##"{ "light": { "ink": true }, "dark": 3 }"##),
            &Tokens::default(),
            &set(),
        )
        .unwrap();
        let mut listed = sentences(left_out);
        listed.sort();
        assert_eq!(
            listed,
            [
                "`ink`: no color value",
                "cssVars.dark: expected an object",
                "cssVars.light.ink: expected a CSS value, got true",
            ]
        );
        assert_eq!(
            overrides_declared("nope", &Tokens::default(), &set()).unwrap_err()[..9],
            *"not JSON:"
        );
    }
}
