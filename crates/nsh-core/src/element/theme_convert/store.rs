//! The old per-appearance token store, as the `cssVars` reader wrote it: a
//! color, a shadow and a gradient with a light and a dark value, a number
//! with a dark value of its own where it has one, font families, and the
//! values of one in both appearances (strings, booleans, easings). A copy of
//! what `tokens.rs` kept before the theme format, used only by the
//! converter and its tests.
//!
//! One thing differs from the old store, for the converter: a font keeps
//! the whole family list it was read from, where the old store kept the
//! family drawn with ([`super::reader::font_family`] of it, which is the
//! old value). It is set exactly where the old store set one.

use rustc_hash::FxHashMap;

use crate::{
    color::Hsla,
    effects::{BoxShadow, Gradient},
};

/// A value in the light appearance and in the dark one.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Pair<T> {
    pub(super) light: T,
    pub(super) dark: T,
}

impl<T> Pair<T> {
    /// `light` and `dark`.
    pub(super) fn new(light: T, dark: T) -> Self {
        Self { light, dark }
    }

    /// The value of the appearance (`dark` or light).
    pub(super) fn get(&self, dark: bool) -> &T {
        if dark { &self.dark } else { &self.light }
    }
}

impl<T: Clone> Pair<T> {
    /// One value in both appearances.
    pub(super) fn both(value: T) -> Self {
        Self {
            light: value.clone(),
            dark: value,
        }
    }
}

/// A token of one value in both appearances.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Plain {
    String(Box<str>),
    Boolean(bool),
    Easing([f32; 4]),
}

impl Plain {
    fn kind(&self) -> Kind {
        match self {
            Plain::String(_) => Kind::String,
            Plain::Boolean(_) => Kind::Boolean,
            Plain::Easing(_) => Kind::Easing,
        }
    }
}

/// The kind of a stored token: a name is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Color,
    Number,
    Font,
    Shadow,
    Gradient,
    String,
    Boolean,
    Easing,
}

impl Kind {
    /// The name forms that say a kind, as a sentence lists them.
    pub(super) const FORMS: &str = "a font is `font-*`, a shadow `shadow-*`, a gradient \
                                    `background-image-*`, a weight `font-weight-*`, an easing \
                                    `ease-*`";

    /// Whether the old reader reads a token named `name` as this kind: the
    /// name says its kind, as a CSS variable's does.
    pub(super) fn names(self, name: &str) -> bool {
        let font = name.starts_with("font-");
        let weight = name.starts_with("font-weight-");
        match self {
            Kind::Font => font && !weight,
            Kind::Shadow => name.starts_with("shadow-"),
            Kind::Gradient => name.starts_with(super::reader::GRADIENTS),
            Kind::Color | Kind::String | Kind::Boolean => !font,
            Kind::Number => !font || weight,
            Kind::Easing => name.starts_with("ease-"),
        }
    }

    /// The kind as a sentence names it.
    pub(super) fn name(self) -> &'static str {
        match self {
            Kind::Color => "color",
            Kind::Number => "number",
            Kind::Font => "font family",
            Kind::Shadow => "shadow",
            Kind::Gradient => "gradient",
            Kind::String => "string",
            Kind::Boolean => "boolean",
            Kind::Easing => "easing",
        }
    }
}

/// The suffixes of a typography's parts' names after its own
/// (`text-title--line-height`), in the order line height, letter spacing,
/// font weight.
pub(super) const TYPOGRAPHY_PARTS: [&str; 3] =
    ["--line-height", "--letter-spacing", "--font-weight"];

/// The old reader's easing curves: CSS's keywords and `cubic-bezier()`.
pub(super) mod easing {
    pub(in super::super) const LINEAR: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
    pub(in super::super) const EASE: [f32; 4] = [0.25, 0.1, 0.25, 1.0];
    pub(in super::super) const EASE_IN: [f32; 4] = [0.42, 0.0, 1.0, 1.0];
    pub(in super::super) const EASE_OUT: [f32; 4] = [0.0, 0.0, 0.58, 1.0];
    pub(in super::super) const EASE_IN_OUT: [f32; 4] = [0.42, 0.0, 0.58, 1.0];

    /// A CSS easing: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`
    /// or `cubic-bezier(x1, y1, x2, y2)` (`x1` and `x2` in `0..=1`).
    pub(in super::super) fn parse(css: &str) -> Option<[f32; 4]> {
        let css = css.trim();
        Some(match css {
            "linear" => LINEAR,
            "ease" => EASE,
            "ease-in" => EASE_IN,
            "ease-out" => EASE_OUT,
            "ease-in-out" => EASE_IN_OUT,
            _ => {
                let args = css.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
                let n: Vec<f32> = args
                    .split(',')
                    .map(|a| a.trim().parse::<f32>().ok().filter(|n| n.is_finite()))
                    .collect::<Option<_>>()?;
                let [x1, y1, x2, y2] = n.as_slice() else {
                    return None;
                };
                if !(0.0..=1.0).contains(x1) || !(0.0..=1.0).contains(x2) {
                    return None;
                }
                [*x1, *y1, *x2, *y2]
            }
        })
    }
}

/// Design tokens by name, per appearance, as the old reader kept them.
#[derive(Debug, Clone, Default)]
pub(super) struct Store {
    colors: FxHashMap<Box<str>, Pair<Hsla>>,
    /// The numbers' light values, which the dark appearance has too unless
    /// `dark_numbers` has another.
    numbers: FxHashMap<Box<str>, f32>,
    dark_numbers: FxHashMap<Box<str>, f32>,
    /// Font family lists, whole.
    fonts: FxHashMap<Box<str>, Box<str>>,
    shadows: FxHashMap<Box<str>, Pair<Vec<BoxShadow>>>,
    gradients: FxHashMap<Box<str>, Pair<Vec<Gradient>>>,
    plain: FxHashMap<Box<str>, Plain>,
    /// A role's name -> the name of the token it reads.
    roles: FxHashMap<Box<str>, Box<str>>,
}

impl Store {
    pub(super) fn color(&self, name: &str) -> Option<Pair<Hsla>> {
        self.colors.get(name).cloned()
    }

    pub(super) fn set_color(&mut self, name: &str, color: Pair<Hsla>) {
        if !Kind::Color.names(name) {
            return;
        }
        self.clear_but(name, Kind::Color);
        self.colors.insert(name.into(), color);
    }

    pub(super) fn number(&self, name: &str) -> Option<f32> {
        self.numbers.get(name).copied()
    }

    /// The number in the dark appearance (`dark`) or the light one.
    pub(super) fn number_in(&self, name: &str, dark: bool) -> Option<f32> {
        dark.then(|| self.dark_numbers.get(name).copied())
            .flatten()
            .or_else(|| self.number(name))
    }

    /// Sets a number in both appearances (a dark value it had goes).
    pub(super) fn set_number(&mut self, name: &str, value: f32) {
        if !Kind::Number.names(name) {
            return;
        }
        self.clear_but(name, Kind::Number);
        self.numbers.insert(name.into(), value);
        self.dark_numbers.remove(name);
    }

    /// Gives a number a value of its own in the dark appearance.
    pub(super) fn set_dark_number(&mut self, name: &str, value: f32) {
        if !Kind::Number.names(name) {
            return;
        }
        self.dark_numbers.insert(name.into(), value);
    }

    /// The font family list, whole.
    pub(super) fn font(&self, name: &str) -> Option<&str> {
        self.fonts.get(name).map(|f| &**f)
    }

    pub(super) fn set_font(&mut self, name: &str, family: &str) {
        if !Kind::Font.names(name) {
            return;
        }
        self.clear_but(name, Kind::Font);
        self.fonts.insert(name.into(), family.into());
    }

    pub(super) fn shadow(&self, name: &str) -> Option<&Pair<Vec<BoxShadow>>> {
        self.shadows.get(name)
    }

    pub(super) fn set_shadow(&mut self, name: &str, shadow: Pair<Vec<BoxShadow>>) {
        if !Kind::Shadow.names(name) {
            return;
        }
        self.clear_but(name, Kind::Shadow);
        self.shadows.insert(name.into(), shadow);
    }

    pub(super) fn gradient(&self, name: &str) -> Option<&Pair<Vec<Gradient>>> {
        self.gradients.get(name)
    }

    pub(super) fn set_gradient(&mut self, name: &str, gradient: Pair<Vec<Gradient>>) {
        if !Kind::Gradient.names(name) {
            return;
        }
        self.clear_but(name, Kind::Gradient);
        self.gradients.insert(name.into(), gradient);
    }

    pub(super) fn string(&self, name: &str) -> Option<&str> {
        match self.plain.get(name)? {
            Plain::String(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn set_string(&mut self, name: &str, value: &str) {
        if !Kind::String.names(name) {
            return;
        }
        self.clear_but(name, Kind::String);
        self.plain.insert(name.into(), Plain::String(value.into()));
    }

    pub(super) fn boolean(&self, name: &str) -> Option<bool> {
        match self.plain.get(name)? {
            Plain::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub(super) fn set_boolean(&mut self, name: &str, value: bool) {
        if !Kind::Boolean.names(name) {
            return;
        }
        self.clear_but(name, Kind::Boolean);
        self.plain.insert(name.into(), Plain::Boolean(value));
    }

    pub(super) fn easing(&self, name: &str) -> Option<[f32; 4]> {
        match self.plain.get(name)? {
            Plain::Easing(e) => Some(*e),
            _ => None,
        }
    }

    pub(super) fn set_easing(&mut self, name: &str, value: [f32; 4]) {
        if !Kind::Easing.names(name) {
            return;
        }
        self.clear_but(name, Kind::Easing);
        self.plain.insert(name.into(), Plain::Easing(value));
    }

    /// The kind of token `name`.
    pub(super) fn kind(&self, name: &str) -> Option<Kind> {
        if self.colors.contains_key(name) {
            Some(Kind::Color)
        } else if self.numbers.contains_key(name) {
            Some(Kind::Number)
        } else if self.fonts.contains_key(name) {
            Some(Kind::Font)
        } else if self.shadows.contains_key(name) {
            Some(Kind::Shadow)
        } else if self.gradients.contains_key(name) {
            Some(Kind::Gradient)
        } else {
            self.plain.get(name).map(Plain::kind)
        }
    }

    /// Removes token `name` of any kind but `kind`: a name is one kind.
    fn clear_but(&mut self, name: &str, kind: Kind) {
        if kind != Kind::Color {
            self.colors.remove(name);
        }
        if kind != Kind::Number {
            self.numbers.remove(name);
            self.dark_numbers.remove(name);
        }
        if kind != Kind::Font {
            self.fonts.remove(name);
        }
        if kind != Kind::Shadow {
            self.shadows.remove(name);
        }
        if kind != Kind::Gradient {
            self.gradients.remove(name);
        }
        if self.plain.get(name).is_some_and(|v| v.kind() != kind) {
            self.plain.remove(name);
        }
    }

    /// These tokens with each of `roles` (role, token) mapped.
    pub(super) fn with_roles(mut self, roles: impl IntoIterator<Item = (String, String)>) -> Self {
        self.roles.extend(
            roles
                .into_iter()
                .map(|(role, token)| (role.into_boxed_str(), token.into_boxed_str())),
        );
        self
    }

    /// Each role mapped onto a token: the role's name and the token's.
    pub(super) fn roles(&self) -> impl Iterator<Item = (&str, &str)> {
        self.roles.iter().map(|(role, token)| (&**role, &**token))
    }

    /// The token the role named `role` is mapped onto, if a theme mapped it.
    pub(super) fn mapped(&self, role: &str) -> Option<&str> {
        self.roles.get(role).map(|token| &**token)
    }

    /// Each token's name, of whatever kind (in no order).
    pub(super) fn names(&self) -> impl Iterator<Item = &str> {
        self.colors
            .keys()
            .chain(self.numbers.keys())
            .chain(self.fonts.keys())
            .chain(self.shadows.keys())
            .chain(self.gradients.keys())
            .chain(self.plain.keys())
            .map(|name| &**name)
    }

    /// Removes token `name`, of whatever kind.
    pub(super) fn remove_named(&mut self, name: &str) {
        self.colors.remove(name);
        self.numbers.remove(name);
        self.dark_numbers.remove(name);
        self.fonts.remove(name);
        self.shadows.remove(name);
        self.gradients.remove(name);
        self.plain.remove(name);
    }

    /// Makes token `name` what it is in `from`, of whatever kind, or
    /// removes it when `from` has none of that name.
    pub(super) fn copy_named(&mut self, from: &Store, name: &str) {
        fn copy<V: Clone>(
            to: &mut FxHashMap<Box<str>, V>,
            from: &FxHashMap<Box<str>, V>,
            name: &str,
        ) {
            match from.get(name) {
                Some(value) => {
                    to.insert(name.into(), value.clone());
                }
                None => {
                    to.remove(name);
                }
            }
        }
        copy(&mut self.colors, &from.colors, name);
        copy(&mut self.numbers, &from.numbers, name);
        copy(&mut self.dark_numbers, &from.dark_numbers, name);
        copy(&mut self.fonts, &from.fonts, name);
        copy(&mut self.shadows, &from.shadows, name);
        copy(&mut self.gradients, &from.gradients, name);
        copy(&mut self.plain, &from.plain, name);
    }

    /// `self` with the tokens of `base` it does not have.
    pub(super) fn over(&self, base: &Store) -> Store {
        // A name of `self` is the kind `self` has it, whatever `base` has.
        let mut out = base.clone();
        for (k, c) in &self.colors {
            out.clear_but(k, Kind::Color);
            out.colors.insert(k.clone(), c.clone());
        }
        // A number of `self` is its values: a dark one of `base`'s goes
        // unless `self` has its own.
        for (k, n) in &self.numbers {
            out.clear_but(k, Kind::Number);
            out.numbers.insert(k.clone(), *n);
            out.dark_numbers.remove(k);
        }
        let darks: Vec<(Box<str>, f32)> = (self.dark_numbers.iter())
            .filter(|(k, _)| out.kind(k).is_none_or(|kind| kind == Kind::Number))
            .map(|(k, n)| (k.clone(), *n))
            .collect();
        out.dark_numbers.extend(darks);
        for (k, f) in &self.fonts {
            out.clear_but(k, Kind::Font);
            out.fonts.insert(k.clone(), f.clone());
        }
        for (k, s) in &self.shadows {
            out.clear_but(k, Kind::Shadow);
            out.shadows.insert(k.clone(), s.clone());
        }
        for (k, g) in &self.gradients {
            out.clear_but(k, Kind::Gradient);
            out.gradients.insert(k.clone(), g.clone());
        }
        for (k, v) in &self.plain {
            out.clear_but(k, v.kind());
            out.plain.insert(k.clone(), v.clone());
        }
        out.roles
            .extend(self.roles.iter().map(|(r, t)| (r.clone(), t.clone())));
        out
    }
}
