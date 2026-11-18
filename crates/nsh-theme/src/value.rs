//! Value types and values: the closed list of types a token can have, and
//! the literal data each holds.
//!
//! Every value is a literal of its type or a reference `{name}` to another
//! token of the same type; every member of a compound is a literal or a
//! reference to a token of the member's type. Colors are hex only: no CSS
//! syntax reaches this crate.

use std::fmt;

use crate::json::{Json, number_text};

/// The type of a token: what its values are. The list is closed; a new type
/// is a change of the format.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
    Color,
    /// A JSON number: points or a unitless value, as its reader takes it.
    Number,
    /// A CSS font-family list, kept whole.
    Font,
    /// A JSON string.
    String,
    /// `true` or `false`.
    Boolean,
    /// `<n>ms` or `<n>s`.
    Duration,
    /// A cubic Bézier `[x1, y1, x2, y2]` or a keyword.
    Easing,
    /// A text style: size, family, weight, line height, letter spacing.
    Typography,
    /// One or more shadow layers.
    Shadow,
    /// A linear, radial or conic gradient.
    Gradient,
    /// A layer or background blur.
    Blur,
    /// A border's color, width and style.
    Border,
    /// A transition's duration, delay and easing.
    Transition,
}

impl ValueType {
    /// Every value type, in the spec's order.
    pub const ALL: [ValueType; 13] = [
        ValueType::Color,
        ValueType::Number,
        ValueType::Font,
        ValueType::String,
        ValueType::Boolean,
        ValueType::Duration,
        ValueType::Easing,
        ValueType::Typography,
        ValueType::Shadow,
        ValueType::Gradient,
        ValueType::Blur,
        ValueType::Border,
        ValueType::Transition,
    ];

    /// The name a document's `kind` gives it (`"color"`, `"typography"`, …).
    pub fn name(self) -> &'static str {
        match self {
            ValueType::Color => "color",
            ValueType::Number => "number",
            ValueType::Font => "font",
            ValueType::String => "string",
            ValueType::Boolean => "boolean",
            ValueType::Duration => "duration",
            ValueType::Easing => "easing",
            ValueType::Typography => "typography",
            ValueType::Shadow => "shadow",
            ValueType::Gradient => "gradient",
            ValueType::Blur => "blur",
            ValueType::Border => "border",
            ValueType::Transition => "transition",
        }
    }

    /// The type a `kind` names.
    pub fn from_name(name: &str) -> Option<Self> {
        ValueType::ALL.into_iter().find(|t| t.name() == name)
    }

    /// Whether its values are objects or arrays of members (typography,
    /// shadow, gradient, blur, border, transition).
    pub fn is_compound(self) -> bool {
        matches!(
            self,
            ValueType::Typography
                | ValueType::Shadow
                | ValueType::Gradient
                | ValueType::Blur
                | ValueType::Border
                | ValueType::Transition
        )
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A token's value: a literal of its type, or a reference to another token.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `{name}`: the value of the token `name`, which has the same type.
    Reference(String),
    /// A color.
    Color(Color),
    /// A number.
    Number(f64),
    /// A font-family list.
    Font(Font),
    /// A string.
    String(String),
    /// A boolean.
    Boolean(bool),
    /// A duration.
    Duration(Duration),
    /// An easing.
    Easing(Easing),
    /// A typography.
    Typography(Typography),
    /// A shadow.
    Shadow(Shadow),
    /// A gradient.
    Gradient(Gradient),
    /// A blur.
    Blur(Blur),
    /// A border.
    Border(Border),
    /// A transition.
    Transition(Transition),
}

impl Value {
    /// A reference to the token `name`.
    pub fn reference(name: impl Into<String>) -> Self {
        Value::Reference(name.into())
    }

    /// The type of a literal; `None` for a reference.
    pub fn value_type(&self) -> Option<ValueType> {
        Some(match self {
            Value::Reference(_) => return None,
            Value::Color(_) => ValueType::Color,
            Value::Number(_) => ValueType::Number,
            Value::Font(_) => ValueType::Font,
            Value::String(_) => ValueType::String,
            Value::Boolean(_) => ValueType::Boolean,
            Value::Duration(_) => ValueType::Duration,
            Value::Easing(_) => ValueType::Easing,
            Value::Typography(_) => ValueType::Typography,
            Value::Shadow(_) => ValueType::Shadow,
            Value::Gradient(_) => ValueType::Gradient,
            Value::Blur(_) => ValueType::Blur,
            Value::Border(_) => ValueType::Border,
            Value::Transition(_) => ValueType::Transition,
        })
    }

    /// The name a whole-value reference names.
    pub fn as_reference(&self) -> Option<&str> {
        match self {
            Value::Reference(name) => Some(name),
            _ => None,
        }
    }

    /// Whether the value or any of its members is a reference.
    pub fn has_references(&self) -> bool {
        let mut found = Vec::new();
        references(ValueType::String, self, &mut found);
        !found.is_empty()
    }

    /// Reads a value of `value_type` from its JSON form, as a document's
    /// `value` is read. Unknown compound members are ignored here; the
    /// error says why the value doesn't fit.
    pub fn from_json(value_type: ValueType, json: &serde_json::Value) -> Result<Value, String> {
        let json = Json::from_serde(json);
        let value = parse(value_type, &json, &mut Vec::new())?;
        validate(value_type, &value)?;
        Ok(value)
    }

    /// The value's JSON form, as [`write`](crate::write()) writes it.
    pub fn to_json(&self) -> serde_json::Value {
        to_json(self).to_serde()
    }
}

/// A member of a compound: a literal of the member's type, or a reference to
/// a token of that type.
#[derive(Clone, Debug, PartialEq)]
pub enum Member<T> {
    /// A literal.
    Value(T),
    /// `{name}`: the value of the token `name`.
    Reference(String),
}

impl<T> Member<T> {
    /// A reference to the token `name`.
    pub fn reference(name: impl Into<String>) -> Self {
        Member::Reference(name.into())
    }

    /// The literal, if the member is one.
    pub fn value(&self) -> Option<&T> {
        match self {
            Member::Value(value) => Some(value),
            Member::Reference(_) => None,
        }
    }

    /// The name a reference names.
    pub fn as_reference(&self) -> Option<&str> {
        match self {
            Member::Value(_) => None,
            Member::Reference(name) => Some(name),
        }
    }
}

impl<T> From<T> for Member<T> {
    fn from(value: T) -> Self {
        Member::Value(value)
    }
}

/// An sRGB color with alpha, each channel 0–255.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha: 255 is opaque.
    pub a: u8,
}

impl Color {
    /// An opaque color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// A color with alpha.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Reads `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, in any letter case.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let digit = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok();
        let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        match hex.len() {
            3 | 4 => {
                let short = |i| digit(i).map(|d| d * 17);
                let a = if hex.len() == 4 { short(3)? } else { 255 };
                Some(Self::rgba(short(0)?, short(1)?, short(2)?, a))
            }
            6 | 8 => {
                let a = if hex.len() == 8 { pair(6)? } else { 255 };
                Some(Self::rgba(pair(0)?, pair(2)?, pair(4)?, a))
            }
            _ => None,
        }
    }

    /// Lowercase `#rrggbb`, or `#rrggbbaa` below full opacity.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

/// A CSS font-family list (`"Inter, system-ui, sans-serif"`), kept whole.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Font(pub String);

impl Font {
    /// The family list `family`.
    pub fn new(family: impl Into<String>) -> Self {
        Self(family.into())
    }

    /// The family list.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A duration, held as milliseconds.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Duration {
    /// Milliseconds, ≥ 0.
    pub millis: f64,
}

impl Duration {
    /// A duration of `millis` milliseconds.
    pub const fn from_millis(millis: f64) -> Self {
        Self { millis }
    }
}

/// An easing curve: a cubic Bézier, or one of CSS's keywords.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    /// `[x1, y1, x2, y2]`, with x1 and x2 in [0, 1].
    Bezier([f64; 4]),
    /// A keyword, written back as a keyword.
    Keyword(EasingKeyword),
}

impl Easing {
    /// The curve's control points; a keyword's are CSS's standard curve.
    pub fn to_bezier(self) -> [f64; 4] {
        match self {
            Easing::Bezier(points) => points,
            Easing::Keyword(keyword) => keyword.to_bezier(),
        }
    }
}

/// CSS's easing keywords.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EasingKeyword {
    /// `linear`.
    Linear,
    /// `ease`.
    Ease,
    /// `ease-in`.
    EaseIn,
    /// `ease-out`.
    EaseOut,
    /// `ease-in-out`.
    EaseInOut,
}

impl EasingKeyword {
    /// Every keyword.
    pub const ALL: [EasingKeyword; 5] = [
        EasingKeyword::Linear,
        EasingKeyword::Ease,
        EasingKeyword::EaseIn,
        EasingKeyword::EaseOut,
        EasingKeyword::EaseInOut,
    ];

    /// The keyword as written.
    pub fn name(self) -> &'static str {
        match self {
            EasingKeyword::Linear => "linear",
            EasingKeyword::Ease => "ease",
            EasingKeyword::EaseIn => "ease-in",
            EasingKeyword::EaseOut => "ease-out",
            EasingKeyword::EaseInOut => "ease-in-out",
        }
    }

    /// The keyword `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        EasingKeyword::ALL.into_iter().find(|k| k.name() == name)
    }

    /// CSS's control points for the keyword.
    pub fn to_bezier(self) -> [f64; 4] {
        match self {
            EasingKeyword::Linear => [0.0, 0.0, 1.0, 1.0],
            EasingKeyword::Ease => [0.25, 0.1, 0.25, 1.0],
            EasingKeyword::EaseIn => [0.42, 0.0, 1.0, 1.0],
            EasingKeyword::EaseOut => [0.0, 0.0, 0.58, 1.0],
            EasingKeyword::EaseInOut => [0.42, 0.0, 0.58, 1.0],
        }
    }
}

/// A text style.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    /// `fontFamily`.
    pub font_family: Option<Member<Font>>,
    /// `fontSize`, in points (required).
    pub font_size: Member<f64>,
    /// `fontWeight`, 1–1000.
    pub font_weight: Option<Member<f64>>,
    /// `lineHeight`.
    pub line_height: Option<Member<LineHeight>>,
    /// `letterSpacing`, in points.
    pub letter_spacing: Option<Member<f64>>,
}

impl Typography {
    /// A typography of this size and nothing else.
    pub fn new(font_size: impl Into<Member<f64>>) -> Self {
        Self {
            font_family: None,
            font_size: font_size.into(),
            font_weight: None,
            line_height: None,
            letter_spacing: None,
        }
    }

    /// With this `fontFamily`.
    pub fn with_font_family(mut self, family: impl Into<Member<Font>>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// With this `fontWeight`.
    pub fn with_font_weight(mut self, weight: impl Into<Member<f64>>) -> Self {
        self.font_weight = Some(weight.into());
        self
    }

    /// With this `lineHeight`.
    pub fn with_line_height(mut self, line_height: impl Into<Member<LineHeight>>) -> Self {
        self.line_height = Some(line_height.into());
        self
    }

    /// With this `letterSpacing`.
    pub fn with_letter_spacing(mut self, spacing: impl Into<Member<f64>>) -> Self {
        self.letter_spacing = Some(spacing.into());
        self
    }
}

/// A typography's line height.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// A multiple of the font size: a literal number (`1.5`). A multiple is
    /// written only as a literal.
    Multiple(f64),
    /// Points: a literal `"<n>px"`, or a reference to a `number` token.
    Px(f64),
}

/// A shadow: one or more layers, the first on top.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow(pub Vec<ShadowLayer>);

impl Shadow {
    /// A shadow of these layers.
    pub fn new(layers: Vec<ShadowLayer>) -> Self {
        Self(layers)
    }
}

/// One layer of a shadow.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct ShadowLayer {
    /// `color` (required).
    pub color: Member<Color>,
    /// `x` offset in points, default 0.
    pub x: Member<f64>,
    /// `y` offset in points, default 0.
    pub y: Member<f64>,
    /// `blur` radius in points, default 0.
    pub blur: Member<f64>,
    /// `spread` in points, default 0.
    pub spread: Member<f64>,
    /// `inset`, default false.
    pub inset: bool,
}

impl ShadowLayer {
    /// A layer of this color, with every other member at its default.
    pub fn new(color: impl Into<Member<Color>>) -> Self {
        Self {
            color: color.into(),
            x: Member::Value(0.0),
            y: Member::Value(0.0),
            blur: Member::Value(0.0),
            spread: Member::Value(0.0),
            inset: false,
        }
    }

    /// With this offset.
    pub fn with_offset(mut self, x: impl Into<Member<f64>>, y: impl Into<Member<f64>>) -> Self {
        self.x = x.into();
        self.y = y.into();
        self
    }

    /// With this blur radius.
    pub fn with_blur(mut self, blur: impl Into<Member<f64>>) -> Self {
        self.blur = blur.into();
        self
    }

    /// With this spread.
    pub fn with_spread(mut self, spread: impl Into<Member<f64>>) -> Self {
        self.spread = spread.into();
        self
    }

    /// Inset or not.
    pub fn with_inset(mut self, inset: bool) -> Self {
        self.inset = inset;
        self
    }
}

/// A gradient.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// Its type, with the angle it takes.
    pub kind: GradientKind,
    /// Two or more stops.
    pub stops: Vec<GradientStop>,
}

impl Gradient {
    /// A gradient of this kind and these stops.
    pub fn new(kind: GradientKind, stops: Vec<GradientStop>) -> Self {
        Self { kind, stops }
    }
}

/// A gradient's type.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum GradientKind {
    /// `linear`, `angle` its direction in degrees (default 180: downwards).
    Linear {
        /// The direction in degrees.
        angle: Member<f64>,
    },
    /// `radial`: an ellipse to the farthest corner, at the center.
    Radial,
    /// `conic`, `angle` its start in degrees (default 0), at the center.
    Conic {
        /// The start in degrees.
        from: Member<f64>,
    },
}

/// A gradient's stop.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    /// `color`.
    pub color: Member<Color>,
    /// `position`, 0–1; spread evenly where none is given.
    pub position: Option<Member<f64>>,
}

impl GradientStop {
    /// A stop of this color at this position.
    pub fn new(color: impl Into<Member<Color>>, position: Option<Member<f64>>) -> Self {
        Self {
            color: color.into(),
            position,
        }
    }
}

/// A blur.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Blur {
    /// What it blurs.
    pub kind: BlurKind,
    /// The radius in points, ≥ 0.
    pub radius: Member<f64>,
}

impl Blur {
    /// A blur of this kind and radius.
    pub fn new(kind: BlurKind, radius: impl Into<Member<f64>>) -> Self {
        Self {
            kind,
            radius: radius.into(),
        }
    }
}

/// What a blur blurs.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlurKind {
    /// `layer`: the view itself.
    Layer,
    /// `background`: what is behind the view.
    Background,
}

/// A border.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Border {
    /// `color`.
    pub color: Member<Color>,
    /// `width` in points, ≥ 0.
    pub width: Member<f64>,
    /// `style`.
    pub style: BorderStyle,
}

impl Border {
    /// A border of this color, width and style.
    pub fn new(
        color: impl Into<Member<Color>>,
        width: impl Into<Member<f64>>,
        style: BorderStyle,
    ) -> Self {
        Self {
            color: color.into(),
            width: width.into(),
            style,
        }
    }
}

/// A border's style.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorderStyle {
    /// `solid`.
    Solid,
    /// `dashed`.
    Dashed,
    /// `dotted`.
    Dotted,
}

impl BorderStyle {
    fn name(self) -> &'static str {
        match self {
            BorderStyle::Solid => "solid",
            BorderStyle::Dashed => "dashed",
            BorderStyle::Dotted => "dotted",
        }
    }
}

/// A transition.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    /// `duration`.
    pub duration: Member<Duration>,
    /// `delay`, default `0ms`.
    pub delay: Member<Duration>,
    /// `easing`, default `ease`.
    pub easing: Member<Easing>,
}

impl Transition {
    /// A transition of this duration, no delay and `ease`.
    pub fn new(duration: impl Into<Member<Duration>>) -> Self {
        Self {
            duration: duration.into(),
            delay: Member::Value(Duration::from_millis(0.0)),
            easing: Member::Value(Easing::Keyword(EasingKeyword::Ease)),
        }
    }

    /// With this delay.
    pub fn with_delay(mut self, delay: impl Into<Member<Duration>>) -> Self {
        self.delay = delay.into();
        self
    }

    /// With this easing.
    pub fn with_easing(mut self, easing: impl Into<Member<Easing>>) -> Self {
        self.easing = easing.into();
        self
    }
}

// ---------------------------------------------------------------------------
// Reading

/// The name a reference string names: exactly `{name}`, with a valid name.
pub(crate) fn reference_name(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    valid_name(inner).then_some(inner)
}

/// A token name: non-empty, without `{` or `}`.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['{', '}'])
}

/// `<n>` of `<n>ms`, `<n>s` or `<n>px`: digits with an optional fraction.
fn decimal(text: &str) -> Option<f64> {
    let (whole, fraction) = match text.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (text, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || fraction.is_some_and(|f| !digits(f)) {
        return None;
    }
    text.parse().ok()
}

fn parse_duration(text: &str) -> Option<Duration> {
    if let Some(n) = text.strip_suffix("ms") {
        return decimal(n).map(Duration::from_millis);
    }
    text.strip_suffix('s')
        .and_then(decimal)
        .map(|n| Duration::from_millis(n * 1000.0))
}

/// What reading one JSON member found: its value, or why it doesn't fit.
type Parsed<T> = Result<T, String>;

fn expect_number(json: &Json, what: &str) -> Parsed<f64> {
    match json {
        Json::Number(n) => Ok(*n),
        other => Err(format!("{what} is {}, not a number", other.kind())),
    }
}

fn member<T>(
    json: &Json,
    what: &str,
    literal: impl Fn(&Json, &str) -> Parsed<T>,
) -> Parsed<Member<T>> {
    if let Json::String(text) = json
        && let Some(name) = reference_name(text)
    {
        return Ok(Member::Reference(name.to_owned()));
    }
    literal(json, what).map(Member::Value)
}

fn color_literal(json: &Json, what: &str) -> Parsed<Color> {
    match json {
        Json::String(text) => {
            Color::from_hex(text).ok_or_else(|| format!("{what} `{text}` is not a hex color"))
        }
        other => Err(format!("{what} is {}, not a hex color", other.kind())),
    }
}

fn font_literal(json: &Json, what: &str) -> Parsed<Font> {
    match json {
        Json::String(text) if !text.is_empty() => Ok(Font::new(text.clone())),
        Json::String(_) => Err(format!("{what} is empty")),
        other => Err(format!(
            "{what} is {}, not a font-family string",
            other.kind()
        )),
    }
}

fn duration_literal(json: &Json, what: &str) -> Parsed<Duration> {
    match json {
        Json::String(text) => {
            parse_duration(text).ok_or_else(|| format!("{what} `{text}` is not `<n>ms` or `<n>s`"))
        }
        other => Err(format!("{what} is {}, not a duration string", other.kind())),
    }
}

fn easing_literal(json: &Json, what: &str) -> Parsed<Easing> {
    match json {
        Json::String(text) => EasingKeyword::from_name(text)
            .map(Easing::Keyword)
            .ok_or_else(|| format!("{what} `{text}` is not an easing keyword")),
        Json::Array(items) if items.len() == 4 => {
            let mut points = [0.0; 4];
            for (i, item) in items.iter().enumerate() {
                points[i] = expect_number(item, &format!("{what}'s item {i}"))?;
            }
            Ok(Easing::Bezier(points))
        }
        Json::Array(items) => Err(format!("{what} has {} numbers, not four", items.len())),
        other => Err(format!("{what} is {}, not an easing", other.kind())),
    }
}

fn line_height_literal(json: &Json, what: &str) -> Parsed<LineHeight> {
    match json {
        Json::Number(n) => Ok(LineHeight::Multiple(*n)),
        Json::String(text) => text
            .strip_suffix("px")
            .and_then(decimal)
            .map(LineHeight::Px)
            .ok_or_else(|| format!("{what} `{text}` is not `<n>px`")),
        other => Err(format!(
            "{what} is {}, not a number or `<n>px`",
            other.kind()
        )),
    }
}

/// The members of a compound object, with the unknown ones listed.
struct Object<'a> {
    members: &'a [(String, Json)],
    known: &'static [&'static str],
}

impl<'a> Object<'a> {
    fn new(json: &'a Json, ty: ValueType, known: &'static [&'static str]) -> Parsed<Self> {
        match json {
            Json::Object(members) => Ok(Self { members, known }),
            other => Err(format!("it is {}, not a {ty} object", other.kind())),
        }
    }

    fn get(&self, name: &str) -> Option<&'a Json> {
        self.members.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    fn required(&self, name: &str) -> Parsed<&'a Json> {
        self.get(name)
            .ok_or_else(|| format!("its `{name}` is missing"))
    }

    fn unknown(&self, at: &[String], out: &mut Vec<Vec<String>>) {
        for (name, _) in self.members {
            if !self.known.contains(&name.as_str()) {
                let mut path = at.to_vec();
                path.push(name.clone());
                out.push(path);
            }
        }
    }
}

fn number_member(object: &Object<'_>, name: &str, default: f64) -> Parsed<Member<f64>> {
    match object.get(name) {
        Some(json) => member(json, &format!("its `{name}`"), expect_number),
        None => Ok(Member::Value(default)),
    }
}

/// Reads a value of type `ty` from its JSON form: a reference `{name}`, or a
/// literal of the type. Checks the JSON shape only; [`validate`] checks
/// ranges. The paths (relative to the value) of unknown compound members go
/// to `unknown`.
pub(crate) fn parse(ty: ValueType, json: &Json, unknown: &mut Vec<Vec<String>>) -> Parsed<Value> {
    if let Json::String(text) = json
        && let Some(name) = reference_name(text)
    {
        return Ok(Value::Reference(name.to_owned()));
    }
    let what = "it";
    Ok(match ty {
        ValueType::Color => Value::Color(color_literal(json, what)?),
        ValueType::Number => Value::Number(expect_number(json, what)?),
        ValueType::Font => Value::Font(font_literal(json, what)?),
        ValueType::String => match json {
            Json::String(text) => Value::String(text.clone()),
            other => return Err(format!("it is {}, not a string", other.kind())),
        },
        ValueType::Boolean => match json {
            Json::Bool(b) => Value::Boolean(*b),
            other => return Err(format!("it is {}, not `true` or `false`", other.kind())),
        },
        ValueType::Duration => Value::Duration(duration_literal(json, what)?),
        ValueType::Easing => Value::Easing(easing_literal(json, what)?),
        ValueType::Typography => {
            const KNOWN: &[&str] = &[
                "fontSize",
                "fontFamily",
                "fontWeight",
                "lineHeight",
                "letterSpacing",
            ];
            let object = Object::new(json, ty, KNOWN)?;
            let optional = |name: &str| {
                object
                    .get(name)
                    .map(|json| member(json, &format!("its `{name}`"), expect_number))
            };
            let typography = Typography {
                font_size: member(
                    object.required("fontSize")?,
                    "its `fontSize`",
                    expect_number,
                )?,
                font_family: object
                    .get("fontFamily")
                    .map(|json| member(json, "its `fontFamily`", font_literal))
                    .transpose()?,
                font_weight: optional("fontWeight").transpose()?,
                line_height: object
                    .get("lineHeight")
                    .map(|json| member(json, "its `lineHeight`", line_height_literal))
                    .transpose()?,
                letter_spacing: optional("letterSpacing").transpose()?,
            };
            object.unknown(&[], unknown);
            Value::Typography(typography)
        }
        ValueType::Shadow => {
            const KNOWN: &[&str] = &["color", "x", "y", "blur", "spread", "inset"];
            let Json::Array(items) = json else {
                return Err(format!(
                    "it is {}, not an array of shadow layers",
                    json.kind()
                ));
            };
            if items.is_empty() {
                return Err("it has no layer".to_owned());
            }
            let mut layers = Vec::with_capacity(items.len());
            let mut found = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let object = Object::new(item, ty, KNOWN).map_err(|e| format!("layer {i}: {e}"))?;
                let layer = (|| -> Parsed<ShadowLayer> {
                    Ok(ShadowLayer {
                        color: member(object.required("color")?, "its `color`", color_literal)?,
                        x: number_member(&object, "x", 0.0)?,
                        y: number_member(&object, "y", 0.0)?,
                        blur: number_member(&object, "blur", 0.0)?,
                        spread: number_member(&object, "spread", 0.0)?,
                        inset: match object.get("inset") {
                            None => false,
                            Some(Json::Bool(b)) => *b,
                            Some(other) => {
                                return Err(format!(
                                    "its `inset` is {}, not a boolean",
                                    other.kind()
                                ));
                            }
                        },
                    })
                })()
                .map_err(|e| format!("layer {i}: {e}"))?;
                object.unknown(&[i.to_string()], &mut found);
                layers.push(layer);
            }
            unknown.extend(found);
            Value::Shadow(Shadow(layers))
        }
        ValueType::Gradient => {
            const KNOWN: &[&str] = &["type", "angle", "stops"];
            const STOP: &[&str] = &["color", "position"];
            let object = Object::new(json, ty, KNOWN)?;
            let kind = match object.required("type")? {
                Json::String(t) if t == "linear" => GradientKind::Linear {
                    angle: number_member(&object, "angle", 180.0)?,
                },
                Json::String(t) if t == "radial" => GradientKind::Radial,
                Json::String(t) if t == "conic" => GradientKind::Conic {
                    from: number_member(&object, "angle", 0.0)?,
                },
                Json::String(t) => {
                    return Err(format!(
                        "its `type` `{t}` is not `linear`, `radial` or `conic`"
                    ));
                }
                other => return Err(format!("its `type` is {}, not a string", other.kind())),
            };
            let Json::Array(items) = object.required("stops")? else {
                return Err("its `stops` is not an array".to_owned());
            };
            let mut stops = Vec::with_capacity(items.len());
            let mut found = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let stop = Object::new(item, ty, STOP).map_err(|e| format!("stop {i}: {e}"))?;
                let color = member(
                    stop.required("color")
                        .map_err(|e| format!("stop {i}: {e}"))?,
                    &format!("stop {i}'s `color`"),
                    color_literal,
                )?;
                let position = stop
                    .get("position")
                    .map(|json| member(json, &format!("stop {i}'s `position`"), expect_number))
                    .transpose()?;
                stop.unknown(&["stops".to_owned(), i.to_string()], &mut found);
                stops.push(GradientStop { color, position });
            }
            if matches!(kind, GradientKind::Radial) && object.get("angle").is_some() {
                found.push(vec!["angle".to_owned()]);
            }
            object.unknown(&[], unknown);
            unknown.extend(found);
            Value::Gradient(Gradient { kind, stops })
        }
        ValueType::Blur => {
            let object = Object::new(json, ty, &["type", "radius"])?;
            let kind = match object.required("type")? {
                Json::String(t) if t == "layer" => BlurKind::Layer,
                Json::String(t) if t == "background" => BlurKind::Background,
                Json::String(t) => {
                    return Err(format!("its `type` `{t}` is not `layer` or `background`"));
                }
                other => return Err(format!("its `type` is {}, not a string", other.kind())),
            };
            let radius = member(object.required("radius")?, "its `radius`", expect_number)?;
            object.unknown(&[], unknown);
            Value::Blur(Blur { kind, radius })
        }
        ValueType::Border => {
            let object = Object::new(json, ty, &["color", "width", "style"])?;
            let color = member(object.required("color")?, "its `color`", color_literal)?;
            let width = member(object.required("width")?, "its `width`", expect_number)?;
            let style = match object.required("style")? {
                Json::String(s) if s == "solid" => BorderStyle::Solid,
                Json::String(s) if s == "dashed" => BorderStyle::Dashed,
                Json::String(s) if s == "dotted" => BorderStyle::Dotted,
                Json::String(s) => {
                    return Err(format!(
                        "its `style` `{s}` is not `solid`, `dashed` or `dotted`"
                    ));
                }
                other => return Err(format!("its `style` is {}, not a string", other.kind())),
            };
            object.unknown(&[], unknown);
            Value::Border(Border {
                color,
                width,
                style,
            })
        }
        ValueType::Transition => {
            let object = Object::new(json, ty, &["duration", "delay", "easing"])?;
            let duration = member(
                object.required("duration")?,
                "its `duration`",
                duration_literal,
            )?;
            let delay = match object.get("delay") {
                Some(json) => member(json, "its `delay`", duration_literal)?,
                None => Member::Value(Duration::from_millis(0.0)),
            };
            let easing = match object.get("easing") {
                Some(json) => member(json, "its `easing`", easing_literal)?,
                None => Member::Value(Easing::Keyword(EasingKeyword::Ease)),
            };
            object.unknown(&[], unknown);
            Value::Transition(Transition {
                duration,
                delay,
                easing,
            })
        }
    })
}

// ---------------------------------------------------------------------------
// Checking

fn check_member<T>(
    member: &Member<T>,
    what: &str,
    literal: impl Fn(&T) -> Result<(), String>,
) -> Result<(), String> {
    match member {
        Member::Value(value) => literal(value),
        Member::Reference(name) if valid_name(name) => Ok(()),
        Member::Reference(name) => Err(format!(
            "{what} refers to `{name}`, which is not a token name"
        )),
    }
}

fn finite(what: &str) -> impl Fn(&f64) -> Result<(), String> + '_ {
    move |n| {
        if n.is_finite() {
            Ok(())
        } else {
            Err(format!("{what} is not a finite number"))
        }
    }
}

fn at_least_zero(what: &str) -> impl Fn(&f64) -> Result<(), String> + '_ {
    move |n| {
        if n.is_finite() && *n >= 0.0 {
            Ok(())
        } else {
            Err(format!("{what} {} is below 0", number_text(*n)))
        }
    }
}

fn duration_ok(what: &str) -> impl Fn(&Duration) -> Result<(), String> + '_ {
    move |d| {
        if d.millis.is_finite() && d.millis >= 0.0 {
            Ok(())
        } else {
            Err(format!("{what} is below 0ms"))
        }
    }
}

fn easing_ok(easing: &Easing) -> Result<(), String> {
    if let Easing::Bezier(points) = easing {
        if points.iter().any(|p| !p.is_finite()) {
            return Err("an easing point is not a finite number".to_owned());
        }
        if !(0.0..=1.0).contains(&points[0]) || !(0.0..=1.0).contains(&points[2]) {
            return Err("the easing's x1 and x2 must be between 0 and 1".to_owned());
        }
    }
    Ok(())
}

/// Checks that a value fits `ty` beyond its JSON shape: the variant, finite
/// numbers, ranges, reference names. A whole-value reference fits any type
/// here (its target is checked by the caller).
pub(crate) fn validate(ty: ValueType, value: &Value) -> Result<(), String> {
    if let Value::Reference(name) = value {
        return if valid_name(name) {
            Ok(())
        } else {
            Err(format!("it refers to `{name}`, which is not a token name"))
        };
    }
    if value.value_type() != Some(ty) {
        let found = value.value_type().map_or("a reference", ValueType::name);
        return Err(format!("it is a {found} value, not a {ty}"));
    }
    match value {
        Value::Number(n) => finite("it")(n),
        Value::Font(font) if font.0.is_empty() => Err("it is empty".to_owned()),
        Value::Duration(d) => duration_ok("it")(d),
        Value::Easing(e) => easing_ok(e),
        Value::Typography(t) => {
            check_member(&t.font_size, "its `fontSize`", finite("its `fontSize`"))?;
            if let Some(family) = &t.font_family {
                check_member(family, "its `fontFamily`", |f| {
                    if f.0.is_empty() {
                        Err("its `fontFamily` is empty".to_owned())
                    } else {
                        Ok(())
                    }
                })?;
            }
            if let Some(weight) = &t.font_weight {
                check_member(weight, "its `fontWeight`", |w| {
                    if (1.0..=1000.0).contains(w) {
                        Ok(())
                    } else {
                        Err(format!(
                            "its `fontWeight` {} is not between 1 and 1000",
                            number_text(*w)
                        ))
                    }
                })?;
            }
            if let Some(line_height) = &t.line_height {
                check_member(line_height, "its `lineHeight`", |l| match l {
                    LineHeight::Multiple(n) | LineHeight::Px(n) => finite("its `lineHeight`")(n),
                })?;
            }
            if let Some(spacing) = &t.letter_spacing {
                check_member(
                    spacing,
                    "its `letterSpacing`",
                    finite("its `letterSpacing`"),
                )?;
            }
            Ok(())
        }
        Value::Shadow(shadow) => {
            if shadow.0.is_empty() {
                return Err("it has no layer".to_owned());
            }
            for layer in &shadow.0 {
                check_member(&layer.color, "a layer's `color`", |_| Ok(()))?;
                for (m, name) in [
                    (&layer.x, "x"),
                    (&layer.y, "y"),
                    (&layer.blur, "blur"),
                    (&layer.spread, "spread"),
                ] {
                    check_member(
                        m,
                        &format!("a layer's `{name}`"),
                        finite("a layer's member"),
                    )?;
                }
            }
            Ok(())
        }
        Value::Gradient(gradient) => {
            match &gradient.kind {
                GradientKind::Linear { angle: a } | GradientKind::Conic { from: a } => {
                    check_member(a, "its `angle`", finite("its `angle`"))?
                }
                GradientKind::Radial => {}
            }
            if gradient.stops.len() < 2 {
                return Err(format!(
                    "it has {} stops, not two or more",
                    gradient.stops.len()
                ));
            }
            for stop in &gradient.stops {
                check_member(&stop.color, "a stop's `color`", |_| Ok(()))?;
                if let Some(position) = &stop.position {
                    check_member(position, "a stop's `position`", |p| {
                        if (0.0..=1.0).contains(p) {
                            Ok(())
                        } else {
                            Err(format!(
                                "a stop's `position` {} is not between 0 and 1",
                                number_text(*p)
                            ))
                        }
                    })?;
                }
            }
            Ok(())
        }
        Value::Blur(blur) => {
            check_member(&blur.radius, "its `radius`", at_least_zero("its `radius`"))
        }
        Value::Border(border) => {
            check_member(&border.color, "its `color`", |_| Ok(()))?;
            check_member(&border.width, "its `width`", at_least_zero("its `width`"))
        }
        Value::Transition(t) => {
            check_member(&t.duration, "its `duration`", duration_ok("its `duration`"))?;
            check_member(&t.delay, "its `delay`", duration_ok("its `delay`"))?;
            check_member(&t.easing, "its `easing`", easing_ok)
        }
        _ => Ok(()),
    }
}

/// Every reference in `value` with the type its target must have: a
/// whole-value reference needs `own` (the token's type), a member the
/// member's type (a `lineHeight`, a `number`).
pub(crate) fn references<'a>(
    own: ValueType,
    value: &'a Value,
    out: &mut Vec<(&'a str, ValueType)>,
) {
    fn add<'a, T>(member: &'a Member<T>, ty: ValueType, out: &mut Vec<(&'a str, ValueType)>) {
        if let Member::Reference(name) = member {
            out.push((name, ty));
        }
    }
    use ValueType as T;
    match value {
        Value::Reference(name) => out.push((name, own)),
        Value::Typography(t) => {
            if let Some(m) = &t.font_family {
                add(m, T::Font, out);
            }
            add(&t.font_size, T::Number, out);
            for m in [&t.font_weight, &t.letter_spacing].into_iter().flatten() {
                add(m, T::Number, out);
            }
            if let Some(m) = &t.line_height {
                add(m, T::Number, out);
            }
        }
        Value::Shadow(shadow) => {
            for layer in &shadow.0 {
                add(&layer.color, T::Color, out);
                for m in [&layer.x, &layer.y, &layer.blur, &layer.spread] {
                    add(m, T::Number, out);
                }
            }
        }
        Value::Gradient(gradient) => {
            if let GradientKind::Linear { angle: m } | GradientKind::Conic { from: m } =
                &gradient.kind
            {
                add(m, T::Number, out);
            }
            for stop in &gradient.stops {
                add(&stop.color, T::Color, out);
                if let Some(m) = &stop.position {
                    add(m, T::Number, out);
                }
            }
        }
        Value::Blur(blur) => add(&blur.radius, T::Number, out),
        Value::Border(border) => {
            add(&border.color, T::Color, out);
            add(&border.width, T::Number, out);
        }
        Value::Transition(t) => {
            add(&t.duration, T::Duration, out);
            add(&t.delay, T::Duration, out);
            add(&t.easing, T::Easing, out);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Resolving

/// `value` with every reference replaced by the resolved value `lookup`
/// gives; `None` when one has none or one of another type.
pub(crate) fn substitute(value: &Value, lookup: &dyn Fn(&str) -> Option<Value>) -> Option<Value> {
    fn number(m: &Member<f64>, lookup: &dyn Fn(&str) -> Option<Value>) -> Option<Member<f64>> {
        match m {
            Member::Value(_) => Some(m.clone()),
            Member::Reference(name) => match lookup(name)? {
                Value::Number(n) => Some(Member::Value(n)),
                _ => None,
            },
        }
    }
    fn color(m: &Member<Color>, lookup: &dyn Fn(&str) -> Option<Value>) -> Option<Member<Color>> {
        match m {
            Member::Value(_) => Some(m.clone()),
            Member::Reference(name) => match lookup(name)? {
                Value::Color(c) => Some(Member::Value(c)),
                _ => None,
            },
        }
    }
    fn duration(
        m: &Member<Duration>,
        lookup: &dyn Fn(&str) -> Option<Value>,
    ) -> Option<Member<Duration>> {
        match m {
            Member::Value(_) => Some(m.clone()),
            Member::Reference(name) => match lookup(name)? {
                Value::Duration(d) => Some(Member::Value(d)),
                _ => None,
            },
        }
    }
    Some(match value {
        Value::Reference(name) => lookup(name)?,
        Value::Typography(t) => Value::Typography(Typography {
            font_family: match &t.font_family {
                Some(Member::Reference(name)) => match lookup(name)? {
                    Value::Font(f) => Some(Member::Value(f)),
                    _ => return None,
                },
                other => other.clone(),
            },
            font_size: number(&t.font_size, lookup)?,
            font_weight: t
                .font_weight
                .as_ref()
                .map(|m| number(m, lookup))
                .map_or(Some(None), |m| m.map(Some))?,
            line_height: match &t.line_height {
                Some(Member::Reference(name)) => match lookup(name)? {
                    Value::Number(n) => Some(Member::Value(LineHeight::Px(n))),
                    _ => return None,
                },
                other => other.clone(),
            },
            letter_spacing: t
                .letter_spacing
                .as_ref()
                .map(|m| number(m, lookup))
                .map_or(Some(None), |m| m.map(Some))?,
        }),
        Value::Shadow(shadow) => {
            let mut layers = Vec::with_capacity(shadow.0.len());
            for layer in &shadow.0 {
                layers.push(ShadowLayer {
                    color: color(&layer.color, lookup)?,
                    x: number(&layer.x, lookup)?,
                    y: number(&layer.y, lookup)?,
                    blur: number(&layer.blur, lookup)?,
                    spread: number(&layer.spread, lookup)?,
                    inset: layer.inset,
                });
            }
            Value::Shadow(Shadow(layers))
        }
        Value::Gradient(gradient) => {
            let kind = match &gradient.kind {
                GradientKind::Linear { angle } => GradientKind::Linear {
                    angle: number(angle, lookup)?,
                },
                GradientKind::Radial => GradientKind::Radial,
                GradientKind::Conic { from } => GradientKind::Conic {
                    from: number(from, lookup)?,
                },
            };
            let mut stops = Vec::with_capacity(gradient.stops.len());
            for stop in &gradient.stops {
                stops.push(GradientStop {
                    color: color(&stop.color, lookup)?,
                    position: match &stop.position {
                        Some(m) => Some(number(m, lookup)?),
                        None => None,
                    },
                });
            }
            Value::Gradient(Gradient { kind, stops })
        }
        Value::Blur(blur) => Value::Blur(Blur {
            kind: blur.kind,
            radius: number(&blur.radius, lookup)?,
        }),
        Value::Border(border) => Value::Border(Border {
            color: color(&border.color, lookup)?,
            width: number(&border.width, lookup)?,
            style: border.style,
        }),
        Value::Transition(t) => Value::Transition(Transition {
            duration: duration(&t.duration, lookup)?,
            delay: duration(&t.delay, lookup)?,
            easing: match &t.easing {
                Member::Reference(name) => match lookup(name)? {
                    Value::Easing(e) => Member::Value(e),
                    _ => return None,
                },
                other => other.clone(),
            },
        }),
        literal => literal.clone(),
    })
}

// ---------------------------------------------------------------------------
// Writing

fn member_json<T>(member: &Member<T>, literal: impl Fn(&T) -> Json) -> Json {
    match member {
        Member::Value(value) => literal(value),
        Member::Reference(name) => Json::String(format!("{{{name}}}")),
    }
}

fn number_json(n: &f64) -> Json {
    Json::Number(*n)
}

fn color_json(c: &Color) -> Json {
    Json::String(c.to_hex())
}

fn duration_json(d: &Duration) -> Json {
    Json::String(format!("{}ms", number_text(d.millis)))
}

fn easing_json(e: &Easing) -> Json {
    match e {
        Easing::Bezier(points) => Json::Array(points.iter().map(number_json).collect()),
        Easing::Keyword(k) => Json::String(k.name().to_owned()),
    }
}

fn is_value<T: PartialEq>(member: &Member<T>, default: T) -> bool {
    *member == Member::Value(default)
}

/// The value's JSON form, in the canonical forms: lowercase hex colors,
/// durations in `ms`, easings and `lineHeight` as read, members at their
/// default left out.
pub(crate) fn to_json(value: &Value) -> Json {
    let object = |members: Vec<(&str, Json)>| {
        Json::Object(
            members
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    };
    match value {
        Value::Reference(name) => Json::String(format!("{{{name}}}")),
        Value::Color(c) => color_json(c),
        Value::Number(n) => Json::Number(*n),
        Value::Font(f) => Json::String(f.0.clone()),
        Value::String(s) => Json::String(s.clone()),
        Value::Boolean(b) => Json::Bool(*b),
        Value::Duration(d) => duration_json(d),
        Value::Easing(e) => easing_json(e),
        Value::Typography(t) => {
            let mut members = Vec::new();
            if let Some(m) = &t.font_family {
                members.push(("fontFamily", member_json(m, |f| Json::String(f.0.clone()))));
            }
            members.push(("fontSize", member_json(&t.font_size, number_json)));
            if let Some(m) = &t.font_weight {
                members.push(("fontWeight", member_json(m, number_json)));
            }
            if let Some(m) = &t.line_height {
                members.push((
                    "lineHeight",
                    member_json(m, |l| match l {
                        LineHeight::Multiple(n) => Json::Number(*n),
                        LineHeight::Px(n) => Json::String(format!("{}px", number_text(*n))),
                    }),
                ));
            }
            if let Some(m) = &t.letter_spacing {
                members.push(("letterSpacing", member_json(m, number_json)));
            }
            object(members)
        }
        Value::Shadow(shadow) => Json::Array(
            shadow
                .0
                .iter()
                .map(|layer| {
                    let mut members = vec![("color", member_json(&layer.color, color_json))];
                    for (name, m) in [
                        ("x", &layer.x),
                        ("y", &layer.y),
                        ("blur", &layer.blur),
                        ("spread", &layer.spread),
                    ] {
                        if !is_value(m, 0.0) {
                            members.push((name, member_json(m, number_json)));
                        }
                    }
                    if layer.inset {
                        members.push(("inset", Json::Bool(true)));
                    }
                    object(members)
                })
                .collect(),
        ),
        Value::Gradient(gradient) => {
            let mut members = Vec::new();
            match &gradient.kind {
                GradientKind::Linear { angle } => {
                    members.push(("type", Json::String("linear".to_owned())));
                    if !is_value(angle, 180.0) {
                        members.push(("angle", member_json(angle, number_json)));
                    }
                }
                GradientKind::Radial => members.push(("type", Json::String("radial".to_owned()))),
                GradientKind::Conic { from } => {
                    members.push(("type", Json::String("conic".to_owned())));
                    if !is_value(from, 0.0) {
                        members.push(("angle", member_json(from, number_json)));
                    }
                }
            }
            let stops = gradient
                .stops
                .iter()
                .map(|stop| {
                    let mut members = vec![("color", member_json(&stop.color, color_json))];
                    if let Some(position) = &stop.position {
                        members.push(("position", member_json(position, number_json)));
                    }
                    object(members)
                })
                .collect();
            members.push(("stops", Json::Array(stops)));
            object(members)
        }
        Value::Blur(blur) => object(vec![
            (
                "type",
                Json::String(
                    match blur.kind {
                        BlurKind::Layer => "layer",
                        BlurKind::Background => "background",
                    }
                    .to_owned(),
                ),
            ),
            ("radius", member_json(&blur.radius, number_json)),
        ]),
        Value::Border(border) => object(vec![
            ("color", member_json(&border.color, color_json)),
            ("width", member_json(&border.width, number_json)),
            ("style", Json::String(border.style.name().to_owned())),
        ]),
        Value::Transition(t) => {
            let mut members = vec![("duration", member_json(&t.duration, duration_json))];
            if !is_value(&t.delay, Duration::from_millis(0.0)) {
                members.push(("delay", member_json(&t.delay, duration_json)));
            }
            if !is_value(&t.easing, Easing::Keyword(EasingKeyword::Ease)) {
                members.push(("easing", member_json(&t.easing, easing_json)));
            }
            object(members)
        }
    }
}
