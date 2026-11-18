//! The Rust types of a theme's value types, and their conversions from and
//! to `nsh-theme`'s literals: the one mapping that [`super::Token`],
//! [`super::TokenKey`], [`super::Tokens::get_as`] and [`super::Role`] use.
//!
//! | Value type | Rust type |
//! | --- | --- |
//! | color | [`Hsla`] |
//! | number | `f32` (points, or unitless) |
//! | font | `Arc<str>` (the whole family list) |
//! | string | `String` |
//! | boolean | `bool` |
//! | duration | [`Duration`] |
//! | easing | [`CubicBezier`] (the keywords are CSS's standard curves) |
//! | typography | [`Typography`] |
//! | shadow | `Arc<[BoxShadow]>` |
//! | gradient | [`Gradient`] |
//! | blur | [`Blur`] |
//! | border | [`Border`] |
//! | transition | [`TransitionToken`] |
//!
//! `nsh-theme` resolves a document's references; what reaches here is
//! literals. A value that still holds a reference (none does after
//! `nsh_theme::resolve`) converts to nothing.

use std::{sync::Arc, time::Duration};

use nsh_theme::{Member, Value};

use super::{CubicBezier, Token};
use crate::{
    BoxShadow, Color,
    color::Hsla,
    effects::{ColorStop, Gradient, GradientKind, GradientSize},
    geometry::{DefiniteLength, Percentage, Points, deg},
    style::{BorderStyle, LineHeight},
};

/// A text style as one token (`typography`): its font size with its font
/// family, line height, letter spacing and weight, which
/// [`super::Styled::typography`] sets together.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Typography {
    /// The font family (a CSS family list), when it sets one.
    pub font_family: Option<Arc<str>>,
    /// The font size.
    pub size: Points,
    /// The line height: a multiple of the font size
    /// ([`LineHeight::Multiple`]) or points ([`LineHeight::Points`]).
    pub line_height: Option<LineHeight>,
    /// The letter spacing.
    pub letter_spacing: Option<Points>,
    /// The font weight (`1`…`1000`).
    pub font_weight: Option<f32>,
}

impl Typography {
    /// A text style of this size and nothing else.
    pub fn new(size: impl Into<Points>) -> Self {
        Self {
            size: size.into(),
            ..Self::default()
        }
    }

    /// With this font family.
    #[must_use]
    pub fn with_font_family(mut self, family: impl Into<Arc<str>>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// With this line height.
    #[must_use]
    pub fn with_line_height(mut self, line_height: LineHeight) -> Self {
        self.line_height = Some(line_height);
        self
    }

    /// With this letter spacing.
    #[must_use]
    pub fn with_letter_spacing(mut self, spacing: impl Into<Points>) -> Self {
        self.letter_spacing = Some(spacing.into());
        self
    }

    /// With this font weight.
    #[must_use]
    pub fn with_font_weight(mut self, weight: f32) -> Self {
        self.font_weight = Some(weight);
        self
    }
}

crate::static_reactive!(Typography: Typography);

/// What a [`Blur`] blurs.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlurKind {
    /// The view itself (`layer`).
    #[default]
    Layer,
    /// What is behind the view (`background`).
    Background,
}

/// A blur token: what it blurs and its radius.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Blur {
    /// What it blurs.
    pub kind: BlurKind,
    /// The radius.
    pub radius: Points,
}

impl Blur {
    /// A blur of this kind and radius.
    pub fn new(kind: BlurKind, radius: impl Into<Points>) -> Self {
        Self {
            kind,
            radius: radius.into(),
        }
    }
}

/// A border token: its color, width and style.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Border {
    /// The color.
    pub color: Hsla,
    /// The width.
    pub width: Points,
    /// The line style.
    pub style: BorderStyle,
}

impl Border {
    /// A border of this color, width and style.
    pub fn new(color: impl Into<Hsla>, width: impl Into<Points>, style: BorderStyle) -> Self {
        Self {
            color: color.into(),
            width: width.into(),
            style,
        }
    }
}

/// A transition token: its duration, delay and easing. A theme's value,
/// not a style's transition ([`crate::effects::Transition`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionToken {
    /// How long it runs.
    pub duration: Duration,
    /// How long it waits first.
    pub delay: Duration,
    /// Its curve.
    pub easing: CubicBezier,
}

impl TransitionToken {
    /// A transition of this duration, no delay and CSS's `ease`.
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            delay: Duration::ZERO,
            easing: CubicBezier::EASE,
        }
    }

    /// With this delay.
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// With this easing.
    #[must_use]
    pub fn with_easing(mut self, easing: CubicBezier) -> Self {
        self.easing = easing;
        self
    }
}

impl Default for TransitionToken {
    fn default() -> Self {
        Self::new(Duration::ZERO)
    }
}

// From `nsh-theme`'s literals.

/// A member's literal (`None`: a reference, which resolving leaves none of).
fn member<T: Clone>(member: &Member<T>) -> Option<T> {
    member.value().cloned()
}

fn hsla(color: nsh_theme::Color) -> Hsla {
    Hsla::from(Color::rgba(color.r, color.g, color.b, color.a))
}

fn packed(color: nsh_theme::Color) -> Color {
    Color::rgba(color.r, color.g, color.b, color.a)
}

fn duration(d: nsh_theme::Duration) -> Duration {
    let nanos = (d.millis * 1e6).round();
    if nanos.is_finite() && nanos >= 0.0 {
        Duration::from_nanos(nanos.min(u64::MAX as f64) as u64)
    } else {
        Duration::ZERO
    }
}

/// `d` in milliseconds.
pub(crate) fn millis(d: Duration) -> f64 {
    d.as_nanos() as f64 / 1e6
}

fn bezier(easing: nsh_theme::Easing) -> CubicBezier {
    let [a, b, c, d] = easing.to_bezier();
    CubicBezier([a as f32, b as f32, c as f32, d as f32])
}

fn line_height(line_height: nsh_theme::LineHeight) -> Option<LineHeight> {
    Some(match line_height {
        nsh_theme::LineHeight::Multiple(m) => LineHeight::Multiple(m as f32),
        nsh_theme::LineHeight::Px(p) => LineHeight::Points(Points(p as f32)),
        _ => return None,
    })
}

fn shadow_layer(layer: &nsh_theme::ShadowLayer) -> Option<BoxShadow> {
    Some(BoxShadow {
        offset_x: Points(member(&layer.x)? as f32),
        offset_y: Points(member(&layer.y)? as f32),
        blur: Points(member(&layer.blur)? as f32),
        spread: Points(member(&layer.spread)? as f32),
        color: packed(member(&layer.color)?),
        inset: layer.inset,
    })
}

fn gradient(gradient: &nsh_theme::Gradient) -> Option<Gradient> {
    let center = Percentage(0.5);
    let kind = match &gradient.kind {
        nsh_theme::GradientKind::Linear { angle } => GradientKind::Linear {
            angle: deg(member(angle)? as f32),
        },
        nsh_theme::GradientKind::Radial => GradientKind::Radial {
            circle: false,
            size: GradientSize::FarthestCorner,
            x: center,
            y: center,
        },
        nsh_theme::GradientKind::Conic { from } => GradientKind::Conic {
            from: deg(member(from)? as f32),
            x: center,
            y: center,
        },
        _ => return None,
    };
    let stops = gradient
        .stops
        .iter()
        .map(|stop| {
            let position = match &stop.position {
                Some(p) => Some(DefiniteLength::Fraction(Percentage(member(p)? as f32))),
                None => None,
            };
            Some(ColorStop {
                color: packed(member(&stop.color)?),
                position,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Gradient {
        kind,
        stops,
        repeating: false,
    })
}

fn border_style(style: nsh_theme::BorderStyle) -> BorderStyle {
    match style {
        nsh_theme::BorderStyle::Dashed => BorderStyle::Dashed,
        nsh_theme::BorderStyle::Dotted => BorderStyle::Dotted,
        _ => BorderStyle::Solid,
    }
}

/// The token a resolved literal is, in nsh-core's types (`None`: a value
/// that still holds a reference).
pub(crate) fn token_of(value: &Value) -> Option<Token> {
    Some(match value {
        Value::Color(c) => Token::Color(hsla(*c)),
        Value::Number(n) => Token::Number(*n as f32),
        Value::Font(f) => Token::Font(Arc::from(f.as_str())),
        Value::String(s) => Token::String(s.clone()),
        Value::Boolean(b) => Token::Boolean(*b),
        Value::Duration(d) => Token::Duration(duration(*d)),
        Value::Easing(e) => Token::Easing(bezier(*e)),
        Value::Typography(t) => Token::Typography(Typography {
            font_family: match &t.font_family {
                Some(f) => Some(Arc::from(member(f)?.as_str())),
                None => None,
            },
            size: Points(member(&t.font_size)? as f32),
            line_height: match &t.line_height {
                Some(l) => Some(line_height(member(l)?)?),
                None => None,
            },
            letter_spacing: match &t.letter_spacing {
                Some(l) => Some(Points(member(l)? as f32)),
                None => None,
            },
            font_weight: match &t.font_weight {
                Some(w) => Some(member(w)? as f32),
                None => None,
            },
        }),
        Value::Shadow(shadow) => Token::Shadow(
            shadow
                .0
                .iter()
                .map(shadow_layer)
                .collect::<Option<Vec<_>>>()?
                .into(),
        ),
        Value::Gradient(g) => Token::Gradient(gradient(g)?),
        Value::Blur(b) => Token::Blur(Blur {
            kind: match b.kind {
                nsh_theme::BlurKind::Background => BlurKind::Background,
                _ => BlurKind::Layer,
            },
            radius: Points(member(&b.radius)? as f32),
        }),
        Value::Border(b) => Token::Border(Border {
            color: hsla(member(&b.color)?),
            width: Points(member(&b.width)? as f32),
            style: border_style(b.style),
        }),
        Value::Transition(t) => Token::Transition(TransitionToken {
            duration: duration(member(&t.duration)?),
            delay: duration(member(&t.delay)?),
            easing: bezier(member(&t.easing)?),
        }),
        _ => return None,
    })
}

// To `nsh-theme`'s literals: what a patch built from typed values holds.

fn theme_color(color: Hsla) -> nsh_theme::Color {
    let c = Color::from(color).0;
    nsh_theme::Color::rgba((c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8, c as u8)
}

fn theme_packed(color: Color) -> nsh_theme::Color {
    let c = color.0;
    nsh_theme::Color::rgba((c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8, c as u8)
}

fn theme_duration(d: Duration) -> nsh_theme::Duration {
    nsh_theme::Duration::from_millis(millis(d))
}

fn theme_easing(e: CubicBezier) -> nsh_theme::Easing {
    let [a, b, c, d] = e.0;
    nsh_theme::Easing::Bezier([f64::from(a), f64::from(b), f64::from(c), f64::from(d)])
}

/// `token` as `nsh-theme`'s literal: what a patch's entry for it holds.
pub(crate) fn value_of(token: &Token) -> Value {
    match token {
        Token::Color(c) => Value::Color(theme_color(*c)),
        Token::Number(n) => Value::Number(f64::from(*n)),
        Token::Font(f) => Value::Font(nsh_theme::Font::new(&**f)),
        Token::String(s) => Value::String(s.clone()),
        Token::Boolean(b) => Value::Boolean(*b),
        Token::Duration(d) => Value::Duration(theme_duration(*d)),
        Token::Easing(e) => Value::Easing(theme_easing(*e)),
        Token::Typography(t) => {
            let mut out = nsh_theme::Typography::new(f64::from(t.size.0));
            if let Some(family) = &t.font_family {
                out = out.with_font_family(nsh_theme::Font::new(&**family));
            }
            if let Some(weight) = t.font_weight {
                out = out.with_font_weight(f64::from(weight));
            }
            match t.line_height {
                Some(LineHeight::Multiple(m)) => {
                    out = out.with_line_height(nsh_theme::LineHeight::Multiple(f64::from(m)));
                }
                Some(LineHeight::Points(p)) => {
                    out = out.with_line_height(nsh_theme::LineHeight::Px(f64::from(p.0)));
                }
                _ => {}
            }
            if let Some(spacing) = t.letter_spacing {
                out = out.with_letter_spacing(f64::from(spacing.0));
            }
            Value::Typography(out)
        }
        Token::Shadow(layers) => Value::Shadow(nsh_theme::Shadow::new(
            layers
                .iter()
                .map(|l| {
                    nsh_theme::ShadowLayer::new(theme_packed(l.color))
                        .with_offset(f64::from(l.offset_x.0), f64::from(l.offset_y.0))
                        .with_blur(f64::from(l.blur.0))
                        .with_spread(f64::from(l.spread.0))
                        .with_inset(l.inset)
                })
                .collect(),
        )),
        Token::Gradient(g) => {
            let kind = match g.kind {
                GradientKind::Linear { angle } => nsh_theme::GradientKind::Linear {
                    angle: Member::Value(f64::from(angle.to_degrees())),
                },
                GradientKind::Conic { from, .. } => nsh_theme::GradientKind::Conic {
                    from: Member::Value(f64::from(from.to_degrees())),
                },
                GradientKind::Radial { .. } => nsh_theme::GradientKind::Radial,
                // A corner's angle depends on the box: downwards.
                GradientKind::LinearCorner { .. } => nsh_theme::GradientKind::Linear {
                    angle: Member::Value(180.0),
                },
            };
            let stops = g
                .stops
                .iter()
                .map(|stop| {
                    let position = match stop.position {
                        Some(DefiniteLength::Fraction(p)) => Some(Member::Value(f64::from(p.0))),
                        _ => None,
                    };
                    nsh_theme::GradientStop::new(theme_packed(stop.color), position)
                })
                .collect();
            Value::Gradient(nsh_theme::Gradient::new(kind, stops))
        }
        Token::Blur(b) => Value::Blur(nsh_theme::Blur::new(
            match b.kind {
                BlurKind::Background => nsh_theme::BlurKind::Background,
                BlurKind::Layer => nsh_theme::BlurKind::Layer,
            },
            f64::from(b.radius.0),
        )),
        Token::Border(b) => Value::Border(nsh_theme::Border::new(
            theme_color(b.color),
            f64::from(b.width.0),
            match b.style {
                BorderStyle::Solid => nsh_theme::BorderStyle::Solid,
                BorderStyle::Dashed => nsh_theme::BorderStyle::Dashed,
                BorderStyle::Dotted => nsh_theme::BorderStyle::Dotted,
            },
        )),
        Token::Transition(t) => Value::Transition(
            nsh_theme::Transition::new(theme_duration(t.duration))
                .with_delay(theme_duration(t.delay))
                .with_easing(theme_easing(t.easing)),
        ),
    }
}

/// Building a [`super::Patch`] from typed values: the app's overrides
/// ([`super::update_tokens`]) and a [`super::ThemeScope`]'s `theme`.
///
/// ```
/// use nsh_core::{
///     color::rgb,
///     element::{Patch, PatchValues, TokenKey},
/// };
///
/// const CARD: TokenKey<f32> = TokenKey::new("radius-card");
/// let patch = Patch::new()
///     .value("radius-card", 12.0)
///     .value("primary", nsh_core::color::Hsla::from(rgb(0x3e63dd)))
///     .key(CARD, 14.0)
///     .role("popover", "surface-overlay");
/// assert_eq!(patch.values.len(), 2);
/// assert_eq!(patch.roles.len(), 1);
/// ```
pub trait PatchValues: Sized {
    /// With token `name` set to `value`; a later entry for the name
    /// replaces an earlier one.
    #[must_use]
    fn value<V: super::TokenValue>(self, name: impl Into<String>, value: V) -> Self;

    /// With the token of `key` set to `value`.
    #[must_use]
    fn key<V: super::TokenValue>(self, key: super::TokenKey<V>, value: V) -> Self {
        self.value(key.name(), value)
    }

    /// With role `role` mapped onto token `token`.
    #[must_use]
    fn role(self, role: impl Into<String>, token: impl Into<String>) -> Self;
}

impl PatchValues for nsh_theme::Patch {
    fn value<V: super::TokenValue>(mut self, name: impl Into<String>, value: V) -> Self {
        self.set(name, &value_of(&value.into_token()));
        self
    }

    fn role(mut self, role: impl Into<String>, token: impl Into<String>) -> Self {
        self.set_role(role, token);
        self
    }
}
