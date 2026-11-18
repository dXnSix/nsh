//! The writer: what the old reader made of a `cssVars` theme, as a theme
//! document in the format (`nsh_theme::ThemeDocument`), by the rules of
//! `docs/dev/theme-format.md`, section 12.

use nsh_theme::{
    Color, Context, Diagnostic, Duration, Easing, EasingKeyword, Font, GradientStop, LineHeight,
    Member, Modifier, Shadow, ShadowLayer, ThemeDocument, Token, Typography, Value, ValueType,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
    code,
    reader::{Old, pointer, whole_var},
    store::{Kind, Store, TYPOGRAPHY_PARTS},
};
use crate::{
    color::{Hsla, Rgba},
    effects::{self, BoxShadow, GradientKind, GradientSize},
    geometry::DefiniteLength,
};

/// The modifier the converter adds for the appearance.
pub(super) const COLOR_MODE: &str = "colorMode";
/// `colorMode`'s contexts.
const LIGHT: &str = "light";
const DARK: &str = "dark";

/// A token's literal values in the light appearance and the dark one,
/// under no old modifier's context.
struct Literal {
    ty: ValueType,
    light: Value,
    dark: Value,
}

/// The document the old reader's `old` values make, with what the format
/// can't express added to `diagnostics`; and the names the old reader left
/// out that rule 5 made references (their left-outs are no longer true).
pub(super) fn document(
    old: &Old,
    diagnostics: &mut Vec<Diagnostic>,
) -> (ThemeDocument, Vec<String>) {
    let set = old.set.as_ref();
    let at = |name: &str| {
        old.written
            .get(name)
            .cloned()
            .unwrap_or_else(|| pointer(&["tokens", name]))
    };
    let mut inexpressible = |at: String, message: String| {
        diagnostics.push(Diagnostic::warning(code::INEXPRESSIBLE, at, message));
    };

    // Every name, in the order the files write them; any other the store
    // has (none, as the files name each) by name.
    let mut names: Vec<String> = old.order.clone();
    let known: FxHashSet<String> = names.iter().cloned().collect();
    let mut rest: Vec<&str> = old.under.names().filter(|n| !known.contains(*n)).collect();
    rest.sort_unstable();
    rest.dedup();
    names.extend(rest.into_iter().map(str::to_owned));

    // Rule 1 and 2: each token's kind, and its values in both appearances.
    let mut literals: Vec<(String, Literal)> = Vec::new();
    // Declared, and given no value; and left out for rule 8.
    let mut no_value: Vec<&str> = Vec::new();
    let mut failed: FxHashSet<&str> = FxHashSet::default();
    for name in &names {
        let declared = set.and_then(|s| s.get(name));
        let Some(ty) = value_type(old, name) else {
            continue;
        };
        if name.is_empty() || name.contains(['{', '}']) {
            inexpressible(
                at(name),
                format!("`{name}`: a token's name has no `{{` or `}}`; left out"),
            );
            continue;
        }
        match (
            value_of(&old.under, name, ty, false),
            value_of(&old.under, name, ty, true),
        ) {
            (Ok(Some(light)), Ok(Some(dark))) => {
                literals.push((name.clone(), Literal { ty, light, dark }));
            }
            (Err(e), _) | (_, Err(e)) => {
                inexpressible(at(name), format!("`{name}`: {e}; left out"));
                failed.insert(name);
            }
            _ => {
                if declared.is_some() {
                    no_value.push(name);
                }
            }
        }
    }
    let index: FxHashMap<&str, usize> = literals
        .iter()
        .enumerate()
        .map(|(i, (name, _))| (name.as_str(), i))
        .collect();

    // The tokens a context of an old modifier sets.
    let mut touched: FxHashSet<&str> = FxHashSet::default();
    if let Some(modifiers) = &old.modifiers {
        for axis in &modifiers.axes {
            for (_, part) in &axis.contexts {
                for (name, literal) in &literals {
                    if sets(part, name, literal.ty) {
                        touched.insert(name);
                    }
                }
            }
        }
    }

    // Rule 5: a whole `var(--x)` is a reference, where the value it copied
    // is the one the reference resolves to, and no context changes it.
    let mut refs = References {
        old,
        literals: &literals,
        index: &index,
        touched: &touched,
        decided: FxHashMap::default(),
        visiting: FxHashSet::default(),
    };
    let targets: Vec<Option<String>> = literals.iter().map(|(name, _)| refs.target(name)).collect();
    // A whole `var(--x)` the old reader read no value of (`var()` copied
    // only colors and numbers: a font's, a boolean's) refers to `x` too:
    // no value of the old reader's to keep, where no context changes `x`.
    let mut recovered: FxHashMap<&str, (ValueType, String)> = FxHashMap::default();
    for name in &names {
        if index.contains_key(name.as_str())
            || failed.contains(name.as_str())
            || name.is_empty()
            || name.contains(['{', '}'])
        {
            continue;
        }
        if let Some((ty, target)) = refs.recovered(name) {
            recovered.insert(name, (ty, target));
        }
    }

    // Rule 3: the dark appearance's values, where they differ from what the
    // token's own value resolves to there.
    let mut dark_entries: Vec<(String, Value)> = Vec::new();
    let mut document = ThemeDocument::new();
    document.name = old.name.clone();
    for name in &names {
        let mut token = if let Some(&i) = index.get(name.as_str()) {
            let literal = &literals[i].1;
            let (value, dark_resolved) = match &targets[i] {
                Some(target) => (
                    Value::reference(target.clone()),
                    &literals[index[target.as_str()]].1.dark,
                ),
                None => (literal.light.clone(), &literal.light),
            };
            if *dark_resolved != literal.dark {
                dark_entries.push((name.clone(), literal.dark.clone()));
            }
            Token::new(literal.ty, value)
        } else if let Some((ty, target)) = recovered.get(name.as_str()) {
            Token::new(*ty, Value::reference(target.clone()))
        } else {
            continue;
        };
        if let Some(declared) = set.and_then(|s| s.get(name)) {
            if let Some(path) = &declared.path {
                token = token.with_path(&**path);
            }
            if !declared.doc.is_empty() {
                token = token.with_doc(&*declared.doc);
            }
        }
        document.insert_token(name.clone(), token);
    }
    for name in no_value {
        if !recovered.contains_key(name) {
            inexpressible(
                at(name),
                format!("`{name}`: declared, and given no value; left out"),
            );
        }
    }

    // Rule 4: the old modifiers, each context's values as they are, or
    // through a helper token where they differ by appearance.
    let mut modifiers: Vec<(String, Modifier)> = Vec::new();
    let mut helpers: Vec<(String, Token)> = Vec::new();
    if let Some(old_modifiers) = &old.modifiers {
        for axis in &old_modifiers.axes {
            let default = &axis.contexts[axis.default].0;
            let mut modifier = Modifier::new(&**default);
            for (context_name, part) in &axis.contexts {
                let applied = part.over(&old.under);
                let mut context = Context::new();
                let at = pointer(&["modifiers", &axis.name, "contexts", context_name]);
                for (name, literal) in &literals {
                    if !sets(part, name, literal.ty) {
                        continue;
                    }
                    let (light, dark) = match (
                        value_of(&applied, name, literal.ty, false),
                        value_of(&applied, name, literal.ty, true),
                    ) {
                        (Ok(Some(light)), Ok(Some(dark))) => (light, dark),
                        (Err(e), _) | (_, Err(e)) => {
                            inexpressible(at.clone(), format!("`{name}`: {e}; left out"));
                            continue;
                        }
                        _ => continue,
                    };
                    if light == dark {
                        context.set(name.clone(), light);
                        continue;
                    }
                    let helper = format!("{name}.{}.{context_name}", axis.name);
                    if document.token(&helper).is_some() || helper.contains(['{', '}']) {
                        inexpressible(
                            at.clone(),
                            format!(
                                "`{name}`: its helper token `{helper}` is a name the theme has; left out"
                            ),
                        );
                        continue;
                    }
                    helpers.push((helper.clone(), Token::new(literal.ty, light)));
                    dark_entries.push((helper.clone(), dark));
                    context.set(name.clone(), Value::reference(helper));
                }
                modifier.insert_context(&**context_name, context);
            }
            modifiers.push((axis.name.to_string(), modifier));
        }
    }
    for (name, token) in helpers {
        document.insert_token(name, token);
    }

    // `colorMode` first, then the old modifiers in file order.
    if !dark_entries.is_empty() {
        let mut dark = Context::new();
        for (name, value) in dark_entries {
            dark.set(name, value);
        }
        let color_mode = Modifier::new(LIGHT)
            .with_context(LIGHT, Context::new())
            .with_context(DARK, dark);
        document.insert_modifier(COLOR_MODE, color_mode);
        document.resolution_order.push(COLOR_MODE.to_owned());
    }
    for (name, modifier) in modifiers {
        document.resolution_order.push(name.clone());
        document.insert_modifier(name, modifier);
    }

    // Rule 6: the roles, onto the tokens written.
    let mut roles: Vec<(&str, &str)> = old.under.roles().collect();
    roles.sort_unstable();
    for (role, token) in roles {
        if document.token(token).is_some() {
            document.set_role(role, token);
        } else {
            inexpressible(
                pointer(&["roles", role]),
                format!("the role `{role}` is mapped onto `{token}`, which is no token; left out"),
            );
        }
    }
    let mut recovered: Vec<String> = recovered.into_keys().map(str::to_owned).collect();
    recovered.sort_unstable();
    (document, recovered)
}

/// The decisions of rule 5: which token's value is a reference, and to
/// what.
struct References<'a> {
    old: &'a Old,
    literals: &'a [(String, Literal)],
    index: &'a FxHashMap<&'a str, usize>,
    touched: &'a FxHashSet<&'a str>,
    /// Each token decided: the token its value refers to, if it does.
    decided: FxHashMap<String, Option<String>>,
    visiting: FxHashSet<String>,
}

impl References<'_> {
    /// The token `name`'s value refers to: the one its whole `var(--x)`
    /// names (`x`, or the token the role `x` is mapped onto), of its type,
    /// whose light value is the one the old reader read for `name`, when no
    /// context of an old modifier sets a token on the way. `None`: its
    /// value is a literal.
    fn target(&mut self, name: &str) -> Option<String> {
        if let Some(decided) = self.decided.get(name) {
            return decided.clone();
        }
        if !self.visiting.insert(name.to_owned()) {
            return None;
        }
        let target = self.find(name);
        self.visiting.remove(name);
        self.decided.insert(name.to_owned(), target.clone());
        target
    }

    fn find(&mut self, name: &str) -> Option<String> {
        let (literals, index) = (self.literals, self.index);
        let literal = &literals[*index.get(name)?].1;
        let target = self.var_target(name)?;
        let target_literal = &literals[index[target.as_str()]].1;
        if target_literal.ty != literal.ty || target_literal.light != literal.light {
            return None;
        }
        self.chain_clear(&target, name).then_some(target)
    }

    /// The token a whole `var(--x)` of `name`'s text (the light value's,
    /// `theme` or `light`) names: `x`, or the token the role `x` is mapped
    /// onto, when it has a literal and is not `name`.
    fn var_target(&self, name: &str) -> Option<String> {
        let (old, index) = (self.old, self.index);
        let x = whole_var(old.raw.get(name)?.light.as_deref()?)?;
        let target = if index.contains_key(x) {
            x
        } else {
            old.under.mapped(x).filter(|t| index.contains_key(*t))?
        };
        (target != name).then(|| target.to_owned())
    }

    /// Whether the chain of references from `target` on is clear: no
    /// context of an old modifier sets a token on it, and it doesn't come
    /// back to `name`.
    fn chain_clear(&mut self, target: &str, name: &str) -> bool {
        let mut at = Some(target.to_owned());
        let mut seen = FxHashSet::default();
        while let Some(token) = at {
            if token == name || !seen.insert(token.clone()) || self.touched.contains(token.as_str())
            {
                return false;
            }
            at = self.target(&token);
        }
        true
    }

    /// For a name the old reader read no value of: the type and target of
    /// the reference its whole `var(--x)` makes, when its dark text is the
    /// same `var(--x)` (or none), `x`'s type is the one it is declared as
    /// (else the one its form says, else `x`'s), and the chain is clear.
    fn recovered(&mut self, name: &str) -> Option<(ValueType, String)> {
        let raw = self.old.raw.get(name)?;
        if raw.dark.is_some() && raw.dark != raw.light {
            return None;
        }
        let target = self.var_target(name)?;
        let target_ty = self.literals[self.index[target.as_str()]].1.ty;
        let declared = self.old.set.as_ref().and_then(|s| s.get(name));
        let ty = match declared {
            Some(_) => value_type(self.old, name)?,
            None => guessed_type(name).unwrap_or(target_ty),
        };
        (ty == target_ty && self.chain_clear(&target, name)).then_some((ty, target))
    }
}

/// The type the old reader's name forms give `name`, where its form says
/// one (`font-*` a font, `shadow-*` a shadow, …); `None`: a color or a
/// number, as its value says.
fn guessed_type(name: &str) -> Option<ValueType> {
    Some(if name.starts_with("shadow-") {
        ValueType::Shadow
    } else if name.starts_with(super::reader::GRADIENTS) {
        ValueType::Gradient
    } else if name.starts_with("ease-") {
        ValueType::Easing
    } else if name.starts_with("duration-") {
        ValueType::Duration
    } else if name.starts_with("font-weight-") {
        ValueType::Number
    } else if name.starts_with("font-") {
        ValueType::Font
    } else {
        return None;
    })
}

/// Whether old context `part` sets the token `name` of type `ty` (for a
/// typography, its size or one of its parts).
fn sets(part: &Store, name: &str, ty: ValueType) -> bool {
    part.kind(name).is_some()
        || ty == ValueType::Typography
            && TYPOGRAPHY_PARTS
                .iter()
                .any(|suffix| part.kind(&format!("{name}{suffix}")).is_some())
}

/// The value type of the token `name` (rule 1): its declared kind's, else
/// the kind the old reader guessed (a `duration-*` number is a duration).
/// `None`: it is a typography's part, or nothing the old reader read.
fn value_type(old: &Old, name: &str) -> Option<ValueType> {
    use super::reader::DeclaredKind as D;
    if let Some(declared) = old.set.as_ref().and_then(|s| s.get(name)) {
        if declared.part_of.is_some() {
            return None;
        }
        return Some(match declared.kind {
            D::Color => ValueType::Color,
            D::Number
            | D::Radius
            | D::Spacing
            | D::Text
            | D::Weight
            | D::Tracking
            | D::Leading
            | D::Blur => ValueType::Number,
            D::Font => ValueType::Font,
            D::Shadow => ValueType::Shadow,
            D::Gradient => ValueType::Gradient,
            D::String => ValueType::String,
            D::Boolean => ValueType::Boolean,
            D::Duration => ValueType::Duration,
            D::Easing => ValueType::Easing,
            D::Typography => ValueType::Typography,
        });
    }
    Some(match old.under.kind(name)? {
        Kind::Color => ValueType::Color,
        Kind::Number if name.starts_with("duration-") => ValueType::Duration,
        Kind::Number => ValueType::Number,
        Kind::Font => ValueType::Font,
        Kind::Shadow => ValueType::Shadow,
        Kind::Gradient => ValueType::Gradient,
        Kind::String => ValueType::String,
        Kind::Boolean => ValueType::Boolean,
        Kind::Easing => ValueType::Easing,
    })
}

/// The value of `name` as a `ty` in the format's form (rule 2), in the
/// dark appearance (`dark`) or the light one. `Ok(None)`: `store` has none;
/// `Err`: the format can't express it (rule 8).
pub(super) fn value_of(
    store: &Store,
    name: &str,
    ty: ValueType,
    dark: bool,
) -> Result<Option<Value>, String> {
    let value = match ty {
        ValueType::Color => store.color(name).map(|c| Value::Color(color(*c.get(dark)))),
        ValueType::Number => store
            .number_in(name, dark)
            .map(|n| Value::Number(number(n))),
        ValueType::Duration => store
            .number_in(name, dark)
            .map(|n| Value::Duration(Duration::from_millis(number(n)))),
        ValueType::Font => store.font(name).map(|f| Value::Font(Font::new(f))),
        ValueType::String => store.string(name).map(|s| Value::String(s.to_owned())),
        ValueType::Boolean => store.boolean(name).map(Value::Boolean),
        ValueType::Easing => store.easing(name).map(|e| Value::Easing(easing(e))),
        ValueType::Shadow => match store.shadow(name) {
            Some(shadow) => Some(shadow_value(shadow.get(dark))?),
            None => None,
        },
        ValueType::Gradient => match store.gradient(name) {
            Some(gradient) => Some(gradient_value(gradient.get(dark))?),
            None => None,
        },
        ValueType::Typography => typography(store, name, dark),
        _ => None,
    };
    if let Some(value) = &value {
        // The format's own checks (ranges, a shadow's layers).
        Value::from_json(ty, &value.to_json())?;
    }
    Ok(value)
}

/// `c` as the format's color, each channel rounded half away from zero.
pub(super) fn color(c: Hsla) -> Color {
    rgba(Rgba::from(c))
}

/// `c` as the format's color, each channel rounded half away from zero.
pub(super) fn rgba(c: Rgba) -> Color {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgba(byte(c.r), byte(c.g), byte(c.b), byte(c.a))
}

/// A packed color (`0xRRGGBBAA`) as the format's.
pub(super) fn packed(c: crate::Color) -> Color {
    let v = c.0;
    Color::rgba((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// `n` as the shortest decimal that reads back as it (`0.1`, not
/// `0.10000000149011612`).
pub(super) fn number(n: f32) -> f64 {
    n.to_string().parse().unwrap_or(f64::from(n))
}

/// An angle in degrees, to four decimals.
fn degrees(radians: f32) -> f64 {
    (f64::from(radians).to_degrees() * 1e4).round() / 1e4
}

/// An easing curve: a keyword when it is one of CSS's.
fn easing(e: [f32; 4]) -> Easing {
    let bezier = e.map(number);
    EasingKeyword::ALL
        .into_iter()
        .find(|k| k.to_bezier() == bezier)
        .map_or(Easing::Bezier(bezier), Easing::Keyword)
}

/// A typography token (`text-title`, its size) with its parts.
fn typography(store: &Store, name: &str, dark: bool) -> Option<Value> {
    let size = store.number_in(name, dark)?;
    let part = |i: usize| store.number_in(&format!("{name}{}", TYPOGRAPHY_PARTS[i]), dark);
    let mut typography = Typography::new(number(size));
    if let Some(line_height) = part(0) {
        // The old `leading-[n]`: a multiple of the size.
        typography = typography.with_line_height(LineHeight::Multiple(number(line_height)));
    }
    if let Some(spacing) = part(1) {
        typography = typography.with_letter_spacing(number(spacing));
    }
    if let Some(weight) = part(2) {
        typography = typography.with_font_weight(number(weight));
    }
    Some(Value::Typography(typography))
}

/// Box shadow layers as a shadow.
fn shadow_value(layers: &[BoxShadow]) -> Result<Value, String> {
    if layers.is_empty() {
        return Err("a shadow of no layer (`none`), and a shadow has one or more".to_owned());
    }
    let layers = layers
        .iter()
        .map(|l| {
            ShadowLayer::new(packed(l.color))
                .with_offset(number(l.offset_x.0), number(l.offset_y.0))
                .with_blur(number(l.blur.0))
                .with_spread(number(l.spread.0))
                .with_inset(l.inset)
        })
        .collect();
    Ok(Value::Shadow(Shadow::new(layers)))
}

/// Gradient layers as a gradient: one linear one, a radial one to the
/// farthest corner of an ellipse at the center, or a conic one at the
/// center, each stop at a fraction of the line or at none.
fn gradient_value(layers: &[effects::Gradient]) -> Result<Value, String> {
    let [gradient] = layers else {
        return Err(format!(
            "{} gradients in one value, and a gradient is one",
            layers.len()
        ));
    };
    if gradient.repeating {
        return Err("a repeating gradient".to_owned());
    }
    let center = |x: f32, y: f32| x == 0.5 && y == 0.5;
    let kind = match gradient.kind {
        GradientKind::Linear { angle } => nsh_theme::GradientKind::Linear {
            angle: Member::Value(degrees(angle.0)),
        },
        GradientKind::Radial {
            circle: false,
            size: GradientSize::FarthestCorner,
            x,
            y,
        } if center(x.0, y.0) => nsh_theme::GradientKind::Radial,
        GradientKind::Conic { from, x, y } if center(x.0, y.0) => nsh_theme::GradientKind::Conic {
            from: Member::Value(degrees(from.0)),
        },
        GradientKind::LinearCorner { .. } => {
            return Err("a linear gradient toward a corner (`to top right`)".to_owned());
        }
        GradientKind::Radial { .. } => {
            return Err(
                "a radial gradient other than an ellipse to the farthest corner, at the center"
                    .to_owned(),
            );
        }
        GradientKind::Conic { .. } => {
            return Err("a conic gradient whose center is not the box's".to_owned());
        }
    };
    let mut stops = Vec::with_capacity(gradient.stops.len());
    for stop in &gradient.stops {
        let position = match stop.position {
            None => None,
            Some(DefiniteLength::Fraction(p)) => Some(Member::Value(number(p.0))),
            Some(DefiniteLength::Absolute(_)) => {
                return Err("a gradient stop at a length (`10px`)".to_owned());
            }
        };
        stops.push(GradientStop::new(packed(stop.color), position));
    }
    Ok(Value::Gradient(nsh_theme::Gradient::new(kind, stops)))
}
