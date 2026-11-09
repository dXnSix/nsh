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

use super::{AdaptiveColor, AdaptiveShadow, Tokens};
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
    Ok((tokens, ignored))
}

/// The tokens theme file `json` sets, and only those, with what of it was
/// left out: an embedding app's overrides of the app's theme, or a change
/// of some of the app's tokens (`nsh`'s `updateTheme`). A color it names
/// and does not set is `base`'s. `Err`: `json` is not JSON.
pub fn overrides(json: &str, base: &Tokens) -> Result<(Tokens, Vec<String>), String> {
    let mut tokens = Tokens::default();
    let ignored = apply_onto(&mut tokens, base, json)?;
    Ok((tokens, ignored))
}

/// [`parse`] under a theme set: each name the file sets must be a declared
/// one (a role's name, `primary`, sets the token the role is mapped onto),
/// and holds a value of its declared kind; any other is left out and listed.
/// `var()` names `base`'s tokens (the types' defaults).
pub(crate) fn parse_declared(
    json: &str,
    base: &Tokens,
    set: &super::theme::ThemeSet,
) -> Result<(Tokens, Vec<String>), String> {
    let (tokens, left_out) = overrides_declared(json, base, set)?;
    Ok((tokens.over(base), left_out))
}

/// What a theme read under types lists for a name no type declares: a
/// build warning, and left out at run time, but no reason to refuse a file
/// read whole (`setTheme`), as a newer theme's names are not (#1190).
pub(crate) const UNDECLARED: &str = "no theme type declares it";

/// Whether `what`, of what a reader left out, is a name no type declares
/// ([`UNDECLARED`]).
pub(crate) fn is_undeclared(what: &str) -> bool {
    what.ends_with(UNDECLARED)
}

/// [`overrides`] under a theme set, as [`parse_declared`] reads names.
pub(crate) fn overrides_declared(
    json: &str,
    base: &Tokens,
    set: &super::theme::ThemeSet,
) -> Result<(Tokens, Vec<String>), String> {
    let (json, kept, mut left_out) = declared_names(json, set)?;
    let (read, ignored) = overrides(&json, base)?;
    left_out.extend(ignored);
    let mut out = Tokens::default();
    for (name, _, kind) in set.names() {
        if kind.holds(&read, name) {
            out.copy_named(&read, name);
        } else if kept.iter().any(|k| k == name)
            && !left_out.iter().any(|w| w.starts_with(&format!("`{name}`")))
        {
            // Set, and read as no value of its kind.
            left_out.push(format!("`{name}`: no {} value", kind.name()));
        }
    }
    Ok((out, left_out))
}

/// Theme file `json` with its names the set's: a role's name becomes the
/// token it is mapped onto (and so in `var()`s), a name no type declares
/// is left out; the names kept and what was left out are listed. A section
/// that sets a token and the role mapped onto it keeps the token's value;
/// one that sets a token through two of the roles it plays, with two
/// values, keeps the first's (by name).
fn declared_names(
    json: &str,
    set: &super::theme::ThemeSet,
) -> Result<(String, Vec<String>, Vec<String>), String> {
    let mut value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
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
                            left_out.push(format!(
                                "`{key}`: the token its role is mapped onto, `{name}`, is set too"
                            ));
                            continue;
                        }
                        // Another role the token plays set it here: the
                        // same value twice is one value, another is left
                        // out as the token set beside its role is.
                        if let Some((_, first)) = by_role.iter().find(|(token, _)| *token == name) {
                            if fields.get(name) != Some(v) {
                                left_out.push(format!(
                                    "`{key}`: `{first}` sets the token its role is mapped onto, `{name}`, too"
                                ));
                            }
                            continue;
                        }
                        by_role.push((name, key));
                        fields.insert(name.to_owned(), v.clone());
                        kept.push(name.to_owned());
                    }
                    None => left_out.push(format!("`{key}`: {UNDECLARED}")),
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
                    if let JsonValue::String(text) = v {
                        for (role, name) in &renamed {
                            for end in [")", ","] {
                                *text = text.replace(
                                    &format!("var(--{role}{end}"),
                                    &format!("var(--{name}{end}"),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    Ok((value.to_string(), kept, left_out))
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
fn apply_onto(tokens: &mut Tokens, base: &Tokens, json: &str) -> Result<Vec<String>, String> {
    let value: JsonValue = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let mut ignored = Vec::new();
    match value.get("cssVars") {
        Some(vars) => apply_css_vars(tokens, base, vars, &mut ignored),
        None => ignored.push("no shadcn `{ \"cssVars\": { ... } }`".into()),
    }
    Ok(ignored)
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
fn apply_css_vars(tokens: &mut Tokens, base: &Tokens, vars: &JsonValue, ignored: &mut Vec<String>) {
    let mut section = |name: &str| -> FxHashMap<String, String> {
        let Some(v) = vars.get(name) else {
            return FxHashMap::default();
        };
        let JsonValue::Object(fields) = v else {
            ignored.push(format!("cssVars.{name}: expected an object"));
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
                v => ignored.push(format!(
                    "cssVars.{name}.{key}: expected a CSS value, got {v}"
                )),
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
                    ignored.push(format!(
                        "`{name}`: {v} is no color in the {other} appearance"
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
                        ignored.push(format!("`{name}`: {v} is no color"));
                    }
                }
            },
        }
    }
}

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
) -> Result<Option<Hsla>, String> {
    let Some(value) = vars.get(name) else {
        return Ok(base
            .color(name)
            .map(|c| if dark { c.dark } else { c.light }));
    };
    if !seen.insert(name.to_owned()) {
        return Err(format!("`{name}` names itself"));
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
            .ok_or_else(|| format!("`{name}`: unclosed var()"))?;
        let (var, fallback) = match inner[..end].split_once(',') {
            Some((v, f)) => (v.trim(), Some(f.trim())),
            None => (inner[..end].trim(), None),
        };
        let Some(var) = var.strip_prefix("--") else {
            return Err(format!("`{name}`: var() names no `--` variable"));
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
    Ok(color(&out))
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
