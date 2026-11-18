//! The design tokens classes resolve against: Tailwind CSS v4.1's default
//! theme (`theme.css`) in points (1rem = 16pt), extended by the app's
//! `nsh.json` (`"tailwind": { ... }`), and the app's theme, read through
//! roles by the namespaces of [`NAMESPACES`] ([`ClassTokens`]).

use std::{
    fmt::Write,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use nsh_theme::ValueType;
use rustc_hash::{FxHashMap, FxHashSet};

use super::utilities::{Decl, Part};
use crate::{
    css,
    effects::{BoxShadow, Filter},
    element::{AdaptiveShadow, Token, Typography},
    geometry::px,
    style::{Color, PropertyValue, StyleDecl, StyleKey, StyleValue, TextShadow},
};

/// Tailwind's design tokens.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Points per spacing step (`p-1`).
    pub(crate) spacing: f32,
    /// Points per `rem` (arbitrary values in `rem`).
    pub(crate) rem: f32,
    /// Flat color names: `red-500`, `black`, `brand`.
    pub(crate) colors: FxHashMap<Box<str>, Color>,
    /// Breakpoints (`sm:`), ascending.
    /// Name, width, and whether Tailwind's is in `rem` (the default
    /// theme's, which Tailwind orders after `px` ones).
    pub(crate) breakpoints: Vec<(Box<str>, f32, bool)>,
    /// `max-w-sm` and friends.
    pub(crate) containers: FxHashMap<Box<str>, f32>,
    /// `text-lg`: size and line height (a multiple of the size).
    pub(crate) text: FxHashMap<Box<str>, (f32, Option<f32>)>,
    /// `font-bold`.
    pub(crate) weights: FxHashMap<Box<str>, u16>,
    /// `font-sans`: the family (`None` is the system font).
    pub(crate) fonts: FxHashMap<Box<str>, Option<Box<str>>>,
    /// `tracking-wide`, in ems.
    pub(crate) tracking: FxHashMap<Box<str>, f32>,
    /// `leading-snug`, multiples of the font size.
    pub(crate) leading: FxHashMap<Box<str>, f32>,
    /// `rounded-lg`.
    pub(crate) radius: FxHashMap<Box<str>, f32>,
    /// `shadow-md`.
    pub(crate) shadows: FxHashMap<Box<str>, Vec<BoxShadow>>,
    /// `inset-shadow-sm`.
    pub(crate) inset_shadows: FxHashMap<Box<str>, Vec<BoxShadow>>,
    /// `drop-shadow-md` (offset x, y, blur, color per layer).
    pub(crate) drop_shadows: FxHashMap<Box<str>, Vec<BoxShadow>>,
    /// `text-shadow-md`.
    pub(crate) text_shadows: FxHashMap<Box<str>, Vec<TextShadow>>,
    /// `blur-md`.
    pub(crate) blur: FxHashMap<Box<str>, f32>,
    /// `perspective-near`.
    pub(crate) perspective: FxHashMap<Box<str>, f32>,
    /// `aspect-video`.
    pub(crate) aspect: FxHashMap<Box<str>, f32>,
    /// `ease-out`.
    pub(crate) ease: FxHashMap<Box<str>, [f32; 4]>,
    /// Default transition duration (ms) and curve.
    pub(crate) transition: (f32, [f32; 4]),
    /// The design tokens classes name (`None`: none).
    tokens: Option<Arc<ClassTokens>>,
}

/// One of Tailwind's theme namespaces (`docs/dev/theme-format.md`,
/// TW-ROLES): the utilities whose value it is, the prefix of the roles they
/// read and the value types those roles may resolve to. `<utility>-<n>`
/// reads the role `<role><n>` (`rounded-card`: `radius-card`).
#[derive(Debug)]
pub(crate) struct Namespace {
    /// The utilities (`rounded`, `rounded-t`, …).
    pub(crate) utilities: &'static [&'static str],
    /// The roles' prefix (`radius-`).
    pub(crate) role: &'static str,
    /// The value types its roles may resolve to; a role resolving to
    /// another is skipped for this namespace.
    pub(crate) accepts: &'static [ValueType],
}

const RADIUS: usize = 0;
const SPACING: usize = 1;
const TEXT: usize = 2;
const TRACKING: usize = 5;
const BLUR: usize = 7;
const SHADOW: usize = 10;
/// The bare colors' row after [`NAMESPACES`]: `<utility>-<n>[/alpha]`
/// reads the color role `<n>`, whatever the utility.
const COLOR: usize = NAMESPACES.len();
/// The rows a name can be served in: each namespace's, then the colors'.
const ROWS: usize = COLOR + 1;

/// Tailwind's theme namespaces as the theme's roles serve them (TW-ROLES).
/// The rows sharing a utility are tried in their order here (`font-<n>`: a
/// font, then a weight). The bare colors are no row of it: any utility
/// reads one ([`COLOR`]).
pub(crate) const NAMESPACES: &[Namespace] = &[
    Namespace {
        utilities: &[
            "rounded",
            "rounded-t",
            "rounded-r",
            "rounded-b",
            "rounded-l",
            "rounded-s",
            "rounded-e",
            "rounded-tl",
            "rounded-tr",
            "rounded-br",
            "rounded-bl",
            "rounded-ss",
            "rounded-se",
            "rounded-es",
            "rounded-ee",
        ],
        role: "radius-",
        accepts: &[ValueType::Number],
    },
    Namespace {
        utilities: &[
            "p",
            "px",
            "py",
            "pt",
            "pr",
            "pb",
            "pl",
            "ps",
            "pe",
            "m",
            "mx",
            "my",
            "mt",
            "mr",
            "mb",
            "ml",
            "ms",
            "me",
            "gap",
            "gap-x",
            "gap-y",
            "w",
            "h",
            "size",
            "min-w",
            "min-h",
            "max-w",
            "max-h",
            "inset",
            "inset-x",
            "inset-y",
            "inset-s",
            "inset-e",
            "top",
            "right",
            "bottom",
            "left",
            "start",
            "end",
            "space-x",
            "space-y",
            "translate-x",
            "translate-y",
            "basis",
            "indent",
            "scroll-m",
            "scroll-mx",
            "scroll-my",
            "scroll-mt",
            "scroll-mr",
            "scroll-mb",
            "scroll-ml",
            "scroll-p",
            "scroll-px",
            "scroll-py",
            "scroll-pt",
            "scroll-pr",
            "scroll-pb",
            "scroll-pl",
        ],
        role: "spacing-",
        accepts: &[ValueType::Number],
    },
    Namespace {
        utilities: &["text"],
        role: "text-",
        accepts: &[ValueType::Typography, ValueType::Number],
    },
    Namespace {
        utilities: &["font"],
        role: "font-",
        accepts: &[ValueType::Font],
    },
    Namespace {
        utilities: &["font"],
        role: "font-weight-",
        accepts: &[ValueType::Number],
    },
    Namespace {
        utilities: &["tracking"],
        role: "tracking-",
        accepts: &[ValueType::Number],
    },
    Namespace {
        utilities: &["leading"],
        role: "leading-",
        accepts: &[ValueType::Number],
    },
    Namespace {
        utilities: &["blur", "backdrop-blur"],
        role: "blur-",
        accepts: &[ValueType::Number, ValueType::Blur],
    },
    Namespace {
        utilities: &["duration", "delay"],
        role: "duration-",
        accepts: &[ValueType::Duration],
    },
    Namespace {
        utilities: &["ease"],
        role: "ease-",
        accepts: &[ValueType::Easing],
    },
    Namespace {
        utilities: &["shadow"],
        role: "shadow-",
        accepts: &[ValueType::Shadow],
    },
    Namespace {
        utilities: &["bg"],
        role: "background-image-",
        accepts: &[ValueType::Gradient],
    },
];

/// [`NAMESPACES`] as role patterns, which the core checks and resolves a
/// theme's role mappings against (`nsh_theme::check_roles`,
/// `nsh_theme::resolve_role`): a mapping such as `radius-card` is known and
/// type-checked without any set declaring it.
pub(crate) fn role_patterns() -> Vec<nsh_theme::RolePattern> {
    NAMESPACES
        .iter()
        .map(|ns| nsh_theme::RolePattern::new(ns.role, ns.accepts))
        .collect()
}

/// The rows a class of `utility` (`rounded-t`, `text`; without a minus)
/// reads, in their precedence (TW-ROLES): its namespaces, then a color,
/// except under `text-` and `bg-`, where a color comes first.
fn row_order(utility: &str) -> impl Iterator<Item = usize> + '_ {
    let color_first = matches!(utility, "text" | "bg");
    let named =
        (0..NAMESPACES.len()).filter(move |&row| NAMESPACES[row].utilities.contains(&utility));
    color_first
        .then_some(COLOR)
        .into_iter()
        .chain(named)
        .chain((!color_first).then_some(COLOR))
}

/// Whether a class of row `row` takes a `/` modifier, as Tailwind's own
/// utilities of it do: a color its opacity (`bg-brand/50`), `text-<n>` a
/// line height (`text-title/6`), `shadow-<n>` an alpha
/// (`shadow-raised/20`). The other rows refuse one.
fn takes_modifier(row: usize) -> bool {
    matches!(row, COLOR | TEXT | SHADOW)
}

/// The unit a number of row `row` is written with in its utility's
/// arbitrary value: points for a length, none for a weight or a multiple.
fn unit(row: usize) -> &'static str {
    match row {
        RADIUS | SPACING | TEXT | TRACKING => "px",
        _ => "",
    }
}

/// The app's theme as classes read it: a copy of the token map and of the
/// resolved roles (`crate::element::Tokens`), and per row of
/// [`NAMESPACES`] (and the bare colors) the names its roles serve.
///
/// A namespaced class reads the role of its namespaced name, resolved as
/// [`crate::element::Tokens::role_by_name`] resolves it: the token a theme
/// maps it onto, else the token of its own name, of a type its namespace
/// accepts (`rounded-card` reads `radius-card`; `bg-popover` the color role
/// `popover`; `shadow-raised` `shadow-raised`). A variable (`bg-(--popover)`)
/// reads the role of its name, whatever its type. The theme wins over
/// Tailwind's built-in value of a class (`shadow-md`), as Tailwind 4's
/// `@theme` does.
///
/// A class is parsed with the values of now, one rule each; when the tokens
/// change, only the classes that read one whose value or mapping changed
/// are parsed again ([`super::deps::THEME`]).
#[derive(Debug, Clone)]
pub(crate) struct ClassTokens {
    /// The tokens by name.
    values: FxHashMap<Box<str>, Token>,
    /// Each role that resolves to a token, by the role's name, with the
    /// token's: every role a layer maps and every token's own name, as
    /// [`crate::element::Tokens::role_by_name`] resolves them.
    roles: FxHashMap<Box<str>, Box<str>>,
    /// Per row (each of [`NAMESPACES`], then [`COLOR`]): each name `<n>`
    /// whose role resolves to a token of a type the row accepts, with that
    /// token's name.
    rows: Vec<FxHashMap<Box<str>, Box<str>>>,
    /// The served names by their last segment (`foreground` for
    /// `muted-foreground`), with the rows serving each (a bit per row): a
    /// class the theme cannot serve is told by one lookup of its own last
    /// segment.
    by_last: FxHashMap<Box<str>, Vec<(Box<str>, u32)>>,
}

impl Default for ClassTokens {
    fn default() -> Self {
        Self {
            values: FxHashMap::default(),
            roles: FxHashMap::default(),
            rows: vec![FxHashMap::default(); ROWS],
            by_last: FxHashMap::default(),
        }
    }
}

impl PartialEq for ClassTokens {
    /// The same tokens and roles: the rows follow from them.
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values && self.roles == other.roles
    }
}

static DEFAULT_TOKENS: LazyLock<Arc<ClassTokens>> =
    LazyLock::new(|| Arc::new(ClassTokens::new(&crate::element::Tokens::default())));

impl ClassTokens {
    /// The classes' view of `theme`: its tokens, its resolved roles and the
    /// names each namespace serves. Once per change, never per class.
    pub(crate) fn new(theme: &crate::element::Tokens) -> Self {
        let values: FxHashMap<Box<str>, Token> = theme
            .iter()
            .map(|(name, token)| (name.into(), token.clone()))
            .collect();
        // Each token's own name and each mapped role, as a role.
        let mapped = theme.roles().map(|(role, _)| role);
        let roles: FxHashMap<Box<str>, Box<str>> = theme
            .iter()
            .map(|(name, _)| name)
            .chain(mapped)
            .filter_map(|role| {
                let token = theme.role_target(role)?;
                values
                    .contains_key(token)
                    .then(|| (role.into(), token.into()))
            })
            .collect();
        let mut rows: Vec<FxHashMap<Box<str>, Box<str>>> = vec![FxHashMap::default(); ROWS];
        for (role, token) in &roles {
            let ty = values[token].value_type();
            let mut serve = |row: usize, name: &str| {
                if !name.is_empty() && !name.ends_with('-') && !name.contains('/') {
                    rows[row].insert(name.into(), token.clone());
                }
            };
            for (row, namespace) in NAMESPACES.iter().enumerate() {
                if let Some(name) = role.strip_prefix(namespace.role)
                    && namespace.accepts.contains(&ty)
                {
                    serve(row, name);
                }
            }
            if ty == ValueType::Color {
                serve(COLOR, role);
            }
        }
        let mut by_last: FxHashMap<Box<str>, Vec<(Box<str>, u32)>> = FxHashMap::default();
        for (row, names) in rows.iter().enumerate() {
            for name in names.keys() {
                let last = name.rsplit('-').next().unwrap_or(name);
                let served = by_last.entry(last.into()).or_default();
                match served.iter_mut().find(|(n, _)| n == name) {
                    Some((_, bits)) => *bits |= 1 << row,
                    None => served.push((name.clone(), 1 << row)),
                }
            }
        }
        ClassTokens {
            values,
            roles,
            rows,
            by_last,
        }
    }

    /// The names of the tokens.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(|n| &**n)
    }

    /// The token the role `role` reads.
    fn target(&self, role: &str) -> Option<&str> {
        self.roles.get(role).map(|token| &**token)
    }

    /// Whether every class that names something in `self` names the same
    /// in `other` and the other way round: the same tokens with the same
    /// types, the same variables and the same names in each row (only
    /// values, and the tokens roles resolve to, may differ).
    pub(crate) fn same_names(&self, other: &ClassTokens) -> bool {
        let same_tokens = self.values.len() == other.values.len()
            && self.values.iter().all(|(name, token)| {
                other
                    .values
                    .get(name)
                    .is_some_and(|o| o.value_type() == token.value_type())
            });
        let same_keys = |a: &FxHashMap<Box<str>, Box<str>>, b: &FxHashMap<Box<str>, Box<str>>| {
            a.len() == b.len() && a.keys().all(|name| b.contains_key(name))
        };
        same_tokens
            && same_keys(&self.roles, &other.roles)
            && self
                .rows
                .iter()
                .zip(&other.rows)
                .all(|(a, b)| same_keys(a, b))
    }

    /// What a class may read differently in `other`, which has the same
    /// names ([`ClassTokens::same_names`]): the keys ([`token_key`]) of
    /// the tokens whose value differs, and of the tokens a role resolved to
    /// before and resolves to now where those differ (a role-only patch).
    pub(crate) fn changed(&self, other: &ClassTokens) -> FxHashSet<u64> {
        let mut changed = FxHashSet::default();
        for (name, token) in &self.values {
            if other.values.get(name) != Some(token) {
                changed.insert(token_key(name));
            }
        }
        for role in self.roles.keys().chain(other.roles.keys()) {
            let (before, now) = (self.target(role), other.target(role));
            if before != now {
                changed.extend(before.into_iter().chain(now).map(token_key));
            }
        }
        changed
    }

    /// `utility` (no variants, no `!`) read through the theme's roles
    /// (TW-ROLES): of the namespaced roles and the bare colors it names,
    /// the longer utility first, and at one utility in its rows' precedence
    /// ([`row_order`]); a role of a type its row doesn't take is skipped.
    /// `None` when the theme serves none of them, which costs one hash
    /// lookup and no allocation. The token read is added to `read`
    /// ([`token_key`]).
    pub(crate) fn class(
        &self,
        theme: &Theme,
        utility: &str,
        read: &mut Vec<u64>,
    ) -> Option<Vec<Decl>> {
        if self.by_last.is_empty() {
            return None;
        }
        // A modifier: a color's opacity (`bg-primary/50`), a size's line
        // height (`text-title/6`), a shadow's alpha (`shadow-raised/[0.5]`).
        // An arbitrary value or a variable before it is no theme's name.
        let (base, modifier) = utility.split_at(utility.find('/').unwrap_or(utility.len()));
        if base.contains(['[', '(']) {
            return None;
        }
        let (_, last) = base.rsplit_once('-')?;
        let served = self.by_last.get(last)?;
        // Where each served name's utility ends (`bg` in `bg-primary`), with
        // the rows serving the name; the longer utility first.
        let mut places = [(0usize, 0u32); 8];
        let mut count = 0;
        for (name, rows) in served {
            let Some(at) = base.len().checked_sub(name.len() + 1) else {
                continue;
            };
            if at > 0
                && count < places.len()
                && base.as_bytes()[at] == b'-'
                && base[at + 1..] == **name
            {
                places[count] = (at, *rows);
                count += 1;
            }
        }
        places[..count].sort_unstable_by_key(|place| std::cmp::Reverse(place.0));
        for &(at, rows) in &places[..count] {
            let (prefix, name) = (&base[..at], &base[at + 1..]);
            let unsigned = prefix.strip_prefix('-').unwrap_or(prefix);
            for row in row_order(unsigned) {
                if rows & (1 << row) == 0 || !(modifier.is_empty() || takes_modifier(row)) {
                    continue;
                }
                if let Some(decls) = self.declarations(theme, row, prefix, name, modifier, read) {
                    return Some(decls);
                }
            }
        }
        None
    }

    /// What `<prefix>-<name><modifier>` declares when `row` serves `name`
    /// (`modifier` empty or `/…`, which only [`takes_modifier`]'s rows
    /// get): the token's value as the utility's arbitrary value for a
    /// number or a color, and as declarations for a compound (TW-COMPOUNDS)
    /// and the other types. `None` when the utility takes no such value.
    fn declarations(
        &self,
        theme: &Theme,
        row: usize,
        prefix: &str,
        name: &str,
        modifier: &str,
        read: &mut Vec<u64>,
    ) -> Option<Vec<Decl>> {
        let token = self.rows[row].get(name)?;
        let value = self.values.get(token)?;
        let negative = prefix.starts_with('-');
        let decls = match value {
            Token::Number(n) if row == BLUR => (!negative).then(|| blur(prefix, *n))?,
            Token::Number(n) => {
                super::utilities::parse(theme, &format!("{prefix}-[{n}{}]{modifier}", unit(row)))?
            }
            _ if negative => return None,
            Token::Color(c) => super::utilities::parse(
                theme,
                &format!("{prefix}-[color:#{:08x}]{modifier}", Color::from(*c).0),
            )?,
            Token::Typography(style) => {
                let mut decls = typography(style)?;
                if let Some(line) = modifier.strip_prefix('/') {
                    // The modifier is the line height, the style's own aside.
                    decls.retain(|d| !matches!(d, Decl::Default(StyleKey::LineHeight, _)));
                    decls.push(super::utilities::line_height_modifier(theme, line)?);
                }
                decls
            }
            Token::Blur(b) => blur(prefix, b.radius.0),
            Token::Font(family) => vec![Decl::Set(
                StyleKey::FontFamily,
                StyleKey::FontFamily
                    .parse_value(StyleValue::Str(family))
                    .ok()?,
            )],
            Token::Duration(d) => {
                let ms = crate::element::theme_values::millis(*d) as f32;
                vec![Decl::Part(match prefix {
                    "delay" => Part::Delay(ms),
                    _ => Part::Duration(ms),
                })]
            }
            Token::Easing(e) => vec![Decl::Part(Part::Ease(e.0))],
            Token::Shadow(layers) if row == SHADOW => match modifier.strip_prefix('/') {
                Some(alpha) => super::utilities::shadow_with_alpha(layers, alpha)?,
                None => vec![Decl::Part(Part::Shadow(layers.clone()))],
            },
            Token::Gradient(g) => vec![Decl::Style(StyleDecl::Set(
                StyleKey::BackgroundImage,
                PropertyValue::Gradients(Rc::from([g.clone()])),
            ))],
            _ => return None,
        };
        read.push(token_key(token));
        Some(decls)
    }

    /// `utility` with its variables (`(--name)` and `(hint:--name)`,
    /// Tailwind 4's shorthand of `[var(--name)]`) replaced by the values of
    /// the tokens they read, as arbitrary values: `bg-(--popover)/50` is
    /// `bg-[color:#fafafaff]/50`, `rounded-(--radius)` `rounded-[12px]`. A
    /// variable reads the role of its name: the token it is mapped onto,
    /// else the token of that name. `None` when it names none, or one its
    /// hint takes no value of. The tokens read are added to `read`.
    pub(crate) fn variables(&self, utility: &str, read: &mut Vec<u64>) -> Option<String> {
        let mut out = String::with_capacity(utility.len() + 8);
        let mut rest = utility;
        while let Some(open) = rest.find('(') {
            out.push_str(&rest[..open]);
            let close = open + rest[open..].find(')')?;
            let inner = &rest[open + 1..close];
            let (hint, var) = match inner.split_once(':') {
                Some((h, v)) => (Some(h), v),
                None => (None, inner),
            };
            let name = self.target(var.strip_prefix("--")?)?;
            let token = &self.values[name];
            out.push('[');
            // A gradient by its variable, as Tailwind reads an image
            // (`bg-(image:--background-image-hero)`): the value alone,
            // which the utility reads as gradients.
            let gradient = match token {
                Token::Gradient(g) if hint == Some("image") => Some(g),
                _ => None,
            };
            if let Some(h) = hint.filter(|_| gradient.is_none()) {
                out.push_str(h);
                out.push(':');
            }
            match token {
                Token::Color(c) if hint.is_none_or(|h| h == "color") => {
                    let _ = write!(out, "#{:08x}", Color::from(*c).0);
                }
                Token::Gradient(_) if let Some(g) = gradient => {
                    out.push_str(&g.css().replace(' ', "_"));
                }
                Token::Shadow(layers) if hint.is_none() => {
                    let css = AdaptiveShadow::from(layers.iter().copied()).css(false);
                    out.push_str(&css.replace(' ', "_"));
                }
                Token::Font(f) if hint == Some("family-name") => {
                    out.push_str(&f.replace(' ', "_"));
                }
                token => {
                    let n = number(token)?;
                    match hint {
                        Some("number") => {
                            let _ = write!(out, "{n}");
                        }
                        _ => {
                            let _ = write!(out, "{n}px");
                        }
                    }
                }
            }
            read.push(token_key(name));
            out.push(']');
            rest = &rest[close + 1..];
        }
        out.push_str(rest);
        Some(out)
    }
}

/// A token's number, as a variable gives it: a number, a text style's
/// size, a duration's milliseconds, a blur's radius.
fn number(token: &Token) -> Option<f32> {
    match token {
        Token::Number(n) => Some(*n),
        Token::Typography(style) => Some(style.size.0),
        Token::Duration(d) => Some(crate::element::theme_values::millis(*d) as f32),
        Token::Blur(b) => Some(b.radius.0),
        _ => None,
    }
}

/// What `text-<n>` declares with a text style (TW-COMPOUNDS): its size,
/// and its line height, letter spacing, weight and family as the values a
/// `leading-*`, `tracking-*` or `font-*` class of the same element replaces
/// wherever it is (Tailwind 4's `--text-*--line-height` and friends).
fn typography(style: &Typography) -> Option<Vec<Decl>> {
    use StyleKey as K;
    let mut out = vec![Decl::Set(
        K::FontSize,
        K::FontSize.number_value(style.size.0).ok()?,
    )];
    if let Some(line_height) = style.line_height {
        out.push(Decl::Default(
            K::LineHeight,
            PropertyValue::LineHeight(line_height),
        ));
    }
    if let Some(spacing) = style.letter_spacing {
        out.push(Decl::Default(
            K::LetterSpacing,
            K::LetterSpacing.number_value(spacing.0).ok()?,
        ));
    }
    if let Some(weight) = style.font_weight {
        out.push(Decl::Default(
            K::FontWeight,
            K::FontWeight.number_value(weight).ok()?,
        ));
    }
    if let Some(family) = &style.font_family {
        out.push(Decl::Default(
            K::FontFamily,
            K::FontFamily.parse_value(StyleValue::Str(family)).ok()?,
        ));
    }
    Some(out)
}

/// What `blur-<n>` (the filters) or `backdrop-blur-<n>` (the backdrop
/// filters) declares with a blur of `radius` points.
fn blur(utility: &str, radius: f32) -> Vec<Decl> {
    vec![Decl::Part(Part::Filter {
        backdrop: utility == "backdrop-blur",
        slot: 0,
        filter: Some(Filter::Blur(px(radius))),
    })]
}

/// What a class list keeps of a token it read, and a change of the tokens
/// says of one whose value changed: a hash of its name. Two names of one
/// key only make a class resolve again that did not need to.
pub(crate) fn token_key(name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    name.hash(&mut hasher);
    hasher.finish()
}

fn map<V>(entries: &[(&str, V)]) -> FxHashMap<Box<str>, V>
where
    V: Clone,
{
    entries
        .iter()
        .map(|(k, v)| ((*k).into(), v.clone()))
        .collect()
}

fn shadows(s: &str) -> Vec<BoxShadow> {
    css::box_shadows(s).unwrap_or_default()
}

impl Default for Theme {
    fn default() -> Self {
        let rem = css::REM;
        let mut colors: FxHashMap<Box<str>, Color> = FxHashMap::default();
        for (name, shades) in super::palette::PALETTE.iter() {
            for (shade, rgba) in super::palette::SHADES.iter().zip(shades) {
                colors.insert(format!("{name}-{shade}").into(), Color(*rgba));
            }
        }
        colors.insert("black".into(), Color(0x0000_00ff));
        colors.insert("white".into(), Color(0xffff_ffff));
        colors.insert("transparent".into(), Color::TRANSPARENT);
        let text_shadow = |s: &str| -> Vec<TextShadow> {
            shadows(s)
                .into_iter()
                .map(|l| TextShadow {
                    color: l.color,
                    offset_x: l.offset_x,
                    offset_y: l.offset_y,
                    radius: l.blur,
                })
                .collect()
        };
        Theme {
            spacing: 0.25 * rem,
            rem,
            colors,
            breakpoints: vec![
                ("sm".into(), 40.0 * rem, true),
                ("md".into(), 48.0 * rem, true),
                ("lg".into(), 64.0 * rem, true),
                ("xl".into(), 80.0 * rem, true),
                ("2xl".into(), 96.0 * rem, true),
            ],
            containers: map(&[
                ("3xs", 16.0 * rem),
                ("2xs", 18.0 * rem),
                ("xs", 20.0 * rem),
                ("sm", 24.0 * rem),
                ("md", 28.0 * rem),
                ("lg", 32.0 * rem),
                ("xl", 36.0 * rem),
                ("2xl", 42.0 * rem),
                ("3xl", 48.0 * rem),
                ("4xl", 56.0 * rem),
                ("5xl", 64.0 * rem),
                ("6xl", 72.0 * rem),
                ("7xl", 80.0 * rem),
            ]),
            text: map(&[
                ("xs", (0.75 * rem, Some(1.0 / 0.75))),
                ("sm", (0.875 * rem, Some(1.25 / 0.875))),
                ("base", (rem, Some(1.5))),
                ("lg", (1.125 * rem, Some(1.75 / 1.125))),
                ("xl", (1.25 * rem, Some(1.75 / 1.25))),
                ("2xl", (1.5 * rem, Some(2.0 / 1.5))),
                ("3xl", (1.875 * rem, Some(2.25 / 1.875))),
                ("4xl", (2.25 * rem, Some(2.5 / 2.25))),
                ("5xl", (3.0 * rem, Some(1.0))),
                ("6xl", (3.75 * rem, Some(1.0))),
                ("7xl", (4.5 * rem, Some(1.0))),
                ("8xl", (6.0 * rem, Some(1.0))),
                ("9xl", (8.0 * rem, Some(1.0))),
            ]),
            weights: map(&[
                ("thin", 100),
                ("extralight", 200),
                ("light", 300),
                ("normal", 400),
                ("medium", 500),
                ("semibold", 600),
                ("bold", 700),
                ("extrabold", 800),
                ("black", 900),
            ]),
            fonts: map(&[
                ("sans", None),
                ("serif", Some("serif".into())),
                ("mono", Some("monospace".into())),
            ]),
            tracking: map(&[
                ("tighter", -0.05),
                ("tight", -0.025),
                ("normal", 0.0),
                ("wide", 0.025),
                ("wider", 0.05),
                ("widest", 0.1),
            ]),
            leading: map(&[
                ("none", 1.0),
                ("tight", 1.25),
                ("snug", 1.375),
                ("normal", 1.5),
                ("relaxed", 1.625),
                ("loose", 2.0),
            ]),
            radius: map(&[
                ("xs", 0.125 * rem),
                ("sm", 0.25 * rem),
                ("md", 0.375 * rem),
                ("lg", 0.5 * rem),
                ("xl", 0.75 * rem),
                ("2xl", rem),
                ("3xl", 1.5 * rem),
                ("4xl", 2.0 * rem),
            ]),
            shadows: map(&[
                ("2xs", shadows("0 1px rgb(0 0 0 / 0.05)")),
                ("xs", shadows("0 1px 2px 0 rgb(0 0 0 / 0.05)")),
                (
                    "sm",
                    shadows("0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1)"),
                ),
                (
                    "md",
                    shadows("0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)"),
                ),
                (
                    "lg",
                    shadows("0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)"),
                ),
                (
                    "xl",
                    shadows("0 20px 25px -5px rgb(0 0 0 / 0.1), 0 8px 10px -6px rgb(0 0 0 / 0.1)"),
                ),
                ("2xl", shadows("0 25px 50px -12px rgb(0 0 0 / 0.25)")),
                // `shadow` without a size (deprecated in v4, still common).
                (
                    "",
                    shadows("0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1)"),
                ),
            ]),
            inset_shadows: map(&[
                ("2xs", shadows("inset 0 1px rgb(0 0 0 / 0.05)")),
                ("xs", shadows("inset 0 1px 1px rgb(0 0 0 / 0.05)")),
                ("sm", shadows("inset 0 2px 4px rgb(0 0 0 / 0.05)")),
            ]),
            drop_shadows: map(&[
                ("xs", shadows("0 1px 1px rgb(0 0 0 / 0.05)")),
                ("sm", shadows("0 1px 2px rgb(0 0 0 / 0.15)")),
                ("md", shadows("0 3px 3px rgb(0 0 0 / 0.12)")),
                ("lg", shadows("0 4px 4px rgb(0 0 0 / 0.15)")),
                ("xl", shadows("0 9px 7px rgb(0 0 0 / 0.1)")),
                ("2xl", shadows("0 25px 25px rgb(0 0 0 / 0.15)")),
                (
                    "",
                    shadows("0 1px 2px rgb(0 0 0 / 0.1), 0 1px 1px rgb(0 0 0 / 0.06)"),
                ),
            ]),
            text_shadows: map(&[
                ("2xs", text_shadow("0px 1px 0px rgb(0 0 0 / 0.15)")),
                ("xs", text_shadow("0px 1px 1px rgb(0 0 0 / 0.2)")),
                (
                    "sm",
                    text_shadow(
                        "0px 1px 0px rgb(0 0 0 / 0.075), 0px 1px 1px rgb(0 0 0 / 0.075), 0px 2px 2px rgb(0 0 0 / 0.075)",
                    ),
                ),
                (
                    "md",
                    text_shadow(
                        "0px 1px 1px rgb(0 0 0 / 0.1), 0px 1px 2px rgb(0 0 0 / 0.1), 0px 2px 4px rgb(0 0 0 / 0.1)",
                    ),
                ),
                (
                    "lg",
                    text_shadow(
                        "0px 1px 2px rgb(0 0 0 / 0.1), 0px 3px 2px rgb(0 0 0 / 0.1), 0px 4px 8px rgb(0 0 0 / 0.1)",
                    ),
                ),
            ]),
            blur: map(&[
                ("xs", 4.0),
                ("sm", 8.0),
                ("md", 12.0),
                ("lg", 16.0),
                ("xl", 24.0),
                ("2xl", 40.0),
                ("3xl", 64.0),
                ("", 8.0),
            ]),
            perspective: map(&[
                ("dramatic", 100.0),
                ("near", 300.0),
                ("normal", 500.0),
                ("midrange", 800.0),
                ("distant", 1200.0),
            ]),
            aspect: map(&[("square", 1.0), ("video", 16.0 / 9.0)]),
            ease: map(&[
                ("linear", [0.0, 0.0, 1.0, 1.0]),
                ("in", [0.4, 0.0, 1.0, 1.0]),
                ("out", [0.0, 0.0, 0.2, 1.0]),
                ("in-out", [0.4, 0.0, 0.2, 1.0]),
            ]),
            transition: (150.0, [0.4, 0.0, 0.2, 1.0]),
            tokens: None,
        }
    }
}

/// Why a theme configuration was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(pub String);

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeError {}

impl Theme {
    /// The default theme extended by `config`, the `"tailwind"` object of
    /// `nsh.json` (JSON). Entries with an existing name replace the default:
    ///
    /// ```json
    /// {
    ///   "colors": { "brand": { "500": "#7c3aed" }, "accent": "#f59e0b" },
    ///   "spacing": 4,
    ///   "breakpoints": { "tablet": 600 },
    ///   "fontFamily": { "display": "Inter" },
    ///   "fontSize": { "huge": [48, 1.1] },
    ///   "fontWeight": { "heavy": 850 },
    ///   "radius": { "card": 14 },
    ///   "shadow": { "card": "0 2px 8px rgb(0 0 0 / 0.12)" },
    ///   "tracking": { "label": 0.08 },
    ///   "leading": { "dense": 1.15 },
    ///   "blur": { "glass": 20 },
    ///   "aspect": { "photo": 1.5 },
    ///   "ease": { "snappy": [0.2, 0, 0, 1] },
    ///   "container": { "content": 680 }
    /// }
    /// ```
    ///
    /// Lengths are points. What it cannot read, an unknown key or a value of
    /// a form it does not know, is left out entry by entry and the rest
    /// applies: a bundle made by a newer NSH keeps its theme on an older
    /// runtime. Only text that is not JSON is an error.
    pub fn from_json(config: &str) -> Result<Theme, ThemeError> {
        Self::from_json_lenient(config).map(|(theme, _)| theme)
    }

    /// [`Theme::from_json`], with what it left out (`unknown key `colours``,
    /// `color `brand` must be a CSS color`), which the toolchain warns of
    /// (a newer NSH's, or a typo) and the runtime logs.
    pub fn from_json_lenient(config: &str) -> Result<(Theme, Vec<String>), ThemeError> {
        let value: serde_json::Value =
            serde_json::from_str(config).map_err(|e| ThemeError(e.to_string()))?;
        let mut theme = Theme::default();
        let mut ignored = Vec::new();
        theme.extend(&value, &mut ignored);
        Ok((theme, ignored))
    }

    fn extend(&mut self, value: &serde_json::Value, ignored: &mut Vec<String>) {
        use serde_json::Value;
        let Value::Object(root) = value else {
            ignored.push("`tailwind` is not an object".into());
            return;
        };
        let num = |v: &Value, what: &str| -> Result<f32, ThemeError> {
            v.as_f64()
                .map(|n| n as f32)
                .ok_or_else(|| ThemeError(format!("{what} must be a number")))
        };
        let color = |v: &Value, what: &str| -> Result<Color, ThemeError> {
            v.as_str()
                .and_then(css::color)
                .ok_or_else(|| ThemeError(format!("{what} must be a CSS color")))
        };
        // Applies `f` to each entry of section `what`, leaving out (and
        // recording) the ones it cannot read.
        fn each(
            v: &Value,
            what: &str,
            ignored: &mut Vec<String>,
            f: &mut dyn FnMut(&str, &Value) -> Result<(), ThemeError>,
        ) {
            let Value::Object(entries) = v else {
                ignored.push(format!("`{what}` must be an object"));
                return;
            };
            for (name, v) in entries {
                if let Err(e) = f(name, v) {
                    ignored.push(e.0);
                }
            }
        }
        for (key, v) in root {
            match key.as_str() {
                "spacing" | "rem" => match num(v, &format!("`{key}`")) {
                    Ok(n) if key == "spacing" => self.spacing = n,
                    Ok(n) => self.rem = n,
                    Err(e) => ignored.push(e.0),
                },
                "colors" => each(v, "colors", ignored, &mut |name, c| {
                    match c {
                        Value::Object(shades) => {
                            let mut failed = None;
                            for (shade, c) in shades {
                                let what = format!("color `{name}-{shade}`");
                                let key = if shade == "DEFAULT" {
                                    name.to_owned()
                                } else {
                                    format!("{name}-{shade}")
                                };
                                match color(c, &what) {
                                    Ok(c) => {
                                        self.colors.insert(key.into(), c);
                                    }
                                    Err(e) => failed = Some(e),
                                }
                            }
                            return failed.map_or(Ok(()), Err);
                        }
                        c => {
                            let parsed = color(c, &format!("color `{name}`"))?;
                            self.colors.insert(name.into(), parsed);
                        }
                    }
                    Ok(())
                }),
                "breakpoints" => {
                    each(v, "breakpoints", ignored, &mut |name, w| {
                        let w = num(w, &format!("breakpoint `{name}`"))?;
                        self.breakpoints.retain(|(n, ..)| **n != *name);
                        self.breakpoints.push((name.into(), w, false));
                        Ok(())
                    });
                    self.breakpoints.sort_by(|a, b| a.1.total_cmp(&b.1));
                }
                "container" => each(v, "container", ignored, &mut |name, w| {
                    let w = num(w, &format!("container `{name}`"))?;
                    self.containers.insert(name.into(), w);
                    Ok(())
                }),
                "fontSize" => each(v, "fontSize", ignored, &mut |name, size| {
                    let what = format!("font size `{name}`");
                    let entry = match size {
                        Value::Array(a) if a.len() == 2 => {
                            (num(&a[0], &what)?, Some(num(&a[1], &what)?))
                        }
                        v => (num(v, &what)?, None),
                    };
                    self.text.insert(name.into(), entry);
                    Ok(())
                }),
                "fontWeight" => each(v, "fontWeight", ignored, &mut |name, w| {
                    let w = num(w, &format!("font weight `{name}`"))?;
                    self.weights
                        .insert(name.into(), w.clamp(1.0, 1000.0) as u16);
                    Ok(())
                }),
                "fontFamily" => each(v, "fontFamily", ignored, &mut |name, f| {
                    let family = f.as_str().ok_or_else(|| {
                        ThemeError(format!("font family `{name}` must be a string"))
                    })?;
                    self.fonts.insert(name.into(), Some(family.into()));
                    Ok(())
                }),
                "tracking" | "leading" | "radius" | "blur" | "perspective" | "aspect" => {
                    let what = match key.as_str() {
                        "tracking" => "tracking",
                        "leading" => "leading",
                        "radius" => "radius",
                        "blur" => "blur",
                        "perspective" => "perspective",
                        _ => "aspect",
                    };
                    each(v, what, ignored, &mut |name, n| {
                        let n = num(n, &format!("{what} `{name}`"))?;
                        let target = match what {
                            "tracking" => &mut self.tracking,
                            "leading" => &mut self.leading,
                            "radius" => &mut self.radius,
                            "blur" => &mut self.blur,
                            "perspective" => &mut self.perspective,
                            _ => &mut self.aspect,
                        };
                        target.insert(name.into(), n);
                        Ok(())
                    })
                }
                "shadow" | "insetShadow" | "dropShadow" => each(v, key, ignored, &mut |name, s| {
                    let list = s.as_str().and_then(css::box_shadows).ok_or_else(|| {
                        ThemeError(format!("{key} `{name}` must be a CSS box-shadow"))
                    })?;
                    let target = match key.as_str() {
                        "shadow" => &mut self.shadows,
                        "insetShadow" => &mut self.inset_shadows,
                        _ => &mut self.drop_shadows,
                    };
                    target.insert(name.into(), list);
                    Ok(())
                }),
                "ease" => each(v, "ease", ignored, &mut |name, e| {
                    let what = || ThemeError(format!("ease `{name}` must be [x1, y1, x2, y2]"));
                    let Value::Array(a) = e else {
                        return Err(what());
                    };
                    let n: Vec<f32> = a
                        .iter()
                        .filter_map(|x| x.as_f64().map(|x| x as f32))
                        .collect();
                    let [x1, y1, x2, y2] = n.as_slice() else {
                        return Err(what());
                    };
                    self.ease.insert(name.into(), [*x1, *y1, *x2, *y2]);
                    Ok(())
                }),
                other => ignored.push(format!("unknown key `{other}`")),
            }
        }
    }

    /// A color by name (`red-500`, `brand`).
    pub fn color(&self, name: &str) -> Option<Color> {
        self.colors.get(name).copied()
    }

    /// The names of every scale's entries (`red-500`, `lg`, `video`,
    /// `snappy`, ...), sorted and without repeats: what utilities and
    /// breakpoint variants take by name. Tools enumerate the classes a
    /// theme offers with them (the engine's record of Tailwind classes).
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.breakpoints.iter().map(|(n, ..)| &**n).collect();
        let keys = [
            self.colors.keys().collect::<Vec<_>>(),
            self.containers.keys().collect(),
            self.text.keys().collect(),
            self.weights.keys().collect(),
            self.fonts.keys().collect(),
            self.tracking.keys().collect(),
            self.leading.keys().collect(),
            self.radius.keys().collect(),
            self.shadows.keys().collect(),
            self.inset_shadows.keys().collect(),
            self.drop_shadows.keys().collect(),
            self.text_shadows.keys().collect(),
            self.blur.keys().collect(),
            self.perspective.keys().collect(),
            self.aspect.keys().collect(),
            self.ease.keys().collect(),
        ];
        names.extend(keys.into_iter().flatten().map(|n| &**n));
        names.sort_unstable();
        names.dedup();
        names
    }

    /// The names of the component theme's tokens (`primary`,
    /// `muted-foreground`, `radius`), sorted: what `bg-primary` and
    /// `rounded-(--radius)` name.
    pub fn token_names(&self) -> Vec<&str> {
        let tokens = self.tokens();
        let mut names: Vec<&str> = tokens.names().collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// The design tokens classes name.
    pub(crate) fn tokens(&self) -> &ClassTokens {
        self.tokens.as_deref().unwrap_or(&DEFAULT_TOKENS)
    }

    /// Takes design tokens `theme`, which classes such as `bg-primary`
    /// name. Without them classes name none.
    pub fn set_tokens(&mut self, theme: &crate::element::Tokens) {
        self.tokens = Some(Arc::new(ClassTokens::new(theme)));
    }

    /// The tokens of another theme.
    pub(crate) fn share_tokens(&mut self, from: &Theme) {
        self.tokens = from.tokens.clone();
    }

    /// Replaces the tokens.
    pub(crate) fn replace_tokens(&mut self, tokens: Arc<ClassTokens>) {
        self.tokens = Some(tokens);
    }

    /// Puts `tokens` in place of the theme's and returns those (to parse
    /// a class with other values, then put them back).
    pub(crate) fn swap_tokens(
        &mut self,
        tokens: Option<Arc<ClassTokens>>,
    ) -> Option<Arc<ClassTokens>> {
        std::mem::replace(&mut self.tokens, tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_extension() {
        let t = Theme::default();
        assert_eq!(t.color("blue-500"), Some(Color(0x2b7f_ffff)));
        assert_eq!(t.text["sm"], (14.0, Some(1.25 / 0.875)));
        assert_eq!(t.shadows["md"].len(), 2);
        let t = Theme::from_json(
            r##"{ "colors": { "brand": { "500": "#7c3aed", "DEFAULT": "#000" }, "accent": "oklch(70% 0.2 50)" },
                 "breakpoints": { "tablet": 600 }, "spacing": 8 }"##,
        )
        .unwrap();
        assert_eq!(t.color("brand-500"), Some(Color(0x7c3a_edff)));
        assert_eq!(t.color("brand"), Some(Color(0x0000_00ff)));
        assert!(t.color("accent").is_some());
        assert_eq!(t.spacing, 8.0);
        assert_eq!(t.breakpoints[0].0.as_ref(), "tablet");
        assert!(Theme::from_json("not json").is_err());
    }

    #[test]
    fn what_it_cannot_read_is_left_out_entry_by_entry() {
        let (t, ignored) = Theme::from_json_lenient(
            r##"{ "colours": {}, "spacing": 2, "rem": "big",
                 "colors": { "x": "nope", "ok": "#000", "brand": { "500": "#7c3aed", "600": 3 } },
                 "radius": { "card": 14, "pill": "full" }, "ease": "linear", "aspect": 1 }"##,
        )
        .unwrap();
        assert_eq!(t.spacing, 2.0);
        assert!(t.color("ok").is_some());
        assert!(t.color("brand-500").is_some());
        assert!(t.color("x").is_none());
        assert_eq!(t.radius.get("card"), Some(&14.0));
        let mut ignored = ignored;
        ignored.sort();
        let mut expected = vec![
            "unknown key `colours`",
            "`rem` must be a number",
            "color `x` must be a CSS color",
            "color `brand-600` must be a CSS color",
            "radius `pill` must be a number",
            "`ease` must be an object",
            "`aspect` must be an object",
        ];
        expected.sort();
        assert_eq!(ignored, expected);
        let (_, ignored) = Theme::from_json_lenient("[]").unwrap();
        assert_eq!(ignored, ["`tailwind` is not an object"]);
    }
}
