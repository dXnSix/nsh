//! Typed keys of design tokens: a token's name with the type of its value
//! ([`TokenKey`]), so code that reads or writes a token of the app's own is
//! checked by the compiler: a name that is no key does not compile, nor does
//! a value of another type. `theme_keys!` makes them of a theme document's
//! `tokens` section; they read the token store ([`Tokens`]) and hold nothing
//! of their own.
//!
//! A key reads the token of its name. Code that cannot know the app's names
//! (`nsh-base`'s components, the platforms' own controls) reads roles
//! instead ([`super::Role`]), which a theme maps onto its tokens.

use std::{marker::PhantomData, sync::Arc, time::Duration};

use nsh_theme::ValueType;

use super::{Blur, Border, CubicBezier, Token, Tokens, TransitionToken, Typography};
use crate::{BoxShadow, color::Hsla, effects::Gradient, geometry::Points};

/// A design token by name, with the type of its value: the Rust type of
/// its value type (`element/theme_values.rs`' table: [`Hsla`] for a color,
/// `f32` or [`Points`] for a number, `Arc<str>` for a font, …).
///
/// ```
/// use std::sync::Arc;
///
/// use nsh_core::element::{Patch, PatchValues, TokenKey, Tokens, Typography};
///
/// const CARD: TokenKey<f32> = TokenKey::new("radius-card");
/// const TITLE: TokenKey<Typography> = TokenKey::new("text-title");
/// const INK: TokenKey<nsh_core::color::Hsla> = TokenKey::new("ink");
///
/// let document = nsh_theme::read(
///     r##"{ "tokens": {
///         "radius-card": { "kind": "number", "value": 12 },
///         "text-title": { "kind": "typography",
///             "value": { "fontSize": 20, "lineHeight": 1.25, "fontFamily": "Inter, sans-serif" } }
///     } }"##,
/// )
/// .document;
/// let (tokens, _) = Tokens::resolve(Arc::new(document), Default::default(), Vec::new());
/// assert_eq!(tokens.key(CARD), Some(12.0));
/// assert_eq!(tokens.key(TITLE).map(|t| t.size.0), Some(20.0));
/// assert_eq!(tokens.key(INK), None);
///
/// // A change to hand `update_tokens`.
/// let patch = Patch::new().key(CARD, 14.0);
/// assert_eq!(patch.values.len(), 1);
/// ```
pub struct TokenKey<V> {
    name: &'static str,
    value: PhantomData<fn() -> V>,
}

impl<V> TokenKey<V> {
    /// The key of the token named `name`.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            value: PhantomData,
        }
    }

    /// Its token's name.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl<V: TokenValue> TokenKey<V> {
    /// Its token's value where it is read: the nearest
    /// [`super::ThemeScope`]'s, as [`super::token`] reads it. Reading it
    /// subscribes to that token alone outside a scope, to the scope's
    /// tokens inside one. `None`: the tokens have none of its name and
    /// type.
    pub fn get(self) -> Option<V> {
        super::ambient::with_token(self.name, |token| token.and_then(V::from_token))
    }
}

impl<V> Clone for TokenKey<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for TokenKey<V> {}

impl<V> PartialEq for TokenKey<V> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<V> Eq for TokenKey<V> {}

impl<V> std::hash::Hash for TokenKey<V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl<V> std::fmt::Debug for TokenKey<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TokenKey").field(&self.name).finish()
    }
}

/// What a [`TokenKey`] reads: the value of one value type, in its Rust
/// type.
pub trait TokenValue: Sized + sealed::Sealed {
    /// The value type it is.
    const TYPE: ValueType;

    /// `token` as this, if it is of this type.
    #[doc(hidden)]
    fn from_token(token: &Token) -> Option<Self>;

    /// This as a token.
    #[doc(hidden)]
    fn into_token(self) -> Token;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Hsla {}
    impl Sealed for f32 {}
    impl Sealed for super::Points {}
    impl Sealed for std::sync::Arc<str> {}
    impl Sealed for String {}
    impl Sealed for bool {}
    impl Sealed for std::time::Duration {}
    impl Sealed for super::CubicBezier {}
    impl Sealed for super::Typography {}
    impl Sealed for std::sync::Arc<[super::BoxShadow]> {}
    impl Sealed for super::Gradient {}
    impl Sealed for super::Blur {}
    impl Sealed for super::Border {}
    impl Sealed for super::TransitionToken {}
}

macro_rules! token_value {
    ($($ty:ty: $value_type:ident),* $(,)?) => {$(
        impl TokenValue for $ty {
            const TYPE: ValueType = ValueType::$value_type;

            fn from_token(token: &Token) -> Option<Self> {
                match token {
                    Token::$value_type(value) => Some(value.clone()),
                    _ => None,
                }
            }

            fn into_token(self) -> Token {
                Token::$value_type(self)
            }
        }
    )*};
}

token_value!(
    Hsla: Color,
    f32: Number,
    Arc<str>: Font,
    String: String,
    bool: Boolean,
    Duration: Duration,
    CubicBezier: Easing,
    Typography: Typography,
    Arc<[BoxShadow]>: Shadow,
    Gradient: Gradient,
    Blur: Blur,
    Border: Border,
    TransitionToken: Transition,
);

/// A number token as a length.
impl TokenValue for Points {
    const TYPE: ValueType = ValueType::Number;

    fn from_token(token: &Token) -> Option<Self> {
        match token {
            Token::Number(n) => Some(Points(*n)),
            _ => None,
        }
    }

    fn into_token(self) -> Token {
        Token::Number(self.0)
    }
}

/// Why [`Tokens::get_as`] has no value of the type asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// The tokens have no token of the name.
    Missing,
    /// The token is of another value type than the type asked for.
    WrongKind {
        /// The value type asked for (`"color"`).
        expected: &'static str,
        /// The value type the token is.
        found: &'static str,
    },
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::Missing => f.write_str("no such design token"),
            TokenError::WrongKind { expected, found } => {
                write!(f, "the design token is a {found}, not a {expected}")
            }
        }
    }
}

impl std::error::Error for TokenError {}

impl Tokens {
    /// The token of `key`.
    pub fn key<V: TokenValue>(&self, key: TokenKey<V>) -> Option<V> {
        self.value(key.name).and_then(V::from_token)
    }

    /// The token `name` as a `V`: [`TokenError::Missing`] when there is
    /// none of the name, [`TokenError::WrongKind`] when it is of another
    /// value type.
    pub fn get_as<V: TokenValue>(&self, name: &str) -> Result<V, TokenError> {
        let Some(token) = self.value(name) else {
            return Err(TokenError::Missing);
        };
        V::from_token(token).ok_or(TokenError::WrongKind {
            expected: V::TYPE.name(),
            found: token.value_type().name(),
        })
    }
}
