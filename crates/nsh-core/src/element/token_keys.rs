//! Typed keys of design tokens: a token's name with the type of its value
//! ([`TokenKey`]), so code that reads or writes a token of the app's own is
//! checked by the compiler: a name that is no key does not compile, nor does
//! a value of another kind. `theme_keys!` makes them of a theme document's
//! `tokens` section; they read the token dictionary ([`Tokens`]) and hold
//! nothing of their own.
//!
//! A key reads the token of its name. Code that cannot know the app's names
//! (`nsh-base`'s components, the platforms' own controls) reads roles
//! instead ([`super::Role`]), which a theme maps onto its tokens.

use std::{marker::PhantomData, sync::Arc};

use super::{AdaptiveColor, AdaptiveGradient, AdaptiveShadow, SharedString, Tokens};
use crate::geometry::Points;

/// A design token by name, with the type of its value: a color
/// ([`AdaptiveColor`]), a number (`f32`, a length as [`Points`]), a font
/// family ([`SharedString`]), box shadows ([`AdaptiveShadow`]) or
/// gradients ([`AdaptiveGradient`]).
///
/// ```
/// use nsh_core::{
///     element::{AdaptiveColor, TokenKey, Tokens},
///     geometry::Points,
/// };
///
/// const CARD: TokenKey<Points> = TokenKey::new("radius-card");
/// const INK: TokenKey<AdaptiveColor> = TokenKey::new("ink");
///
/// let tokens = Tokens::default().with(CARD, Points(12.0));
/// assert_eq!(tokens.key(CARD), Some(Points(12.0)));
/// assert_eq!(tokens.key(INK), None);
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
    /// [`super::ThemeScope`]'s, a number its value in the appearance there,
    /// as [`super::token`] reads it. Reading it subscribes, as
    /// [`super::tokens`] does. `None`: the tokens have none of its name and
    /// kind.
    pub fn get(self) -> Option<V> {
        let tokens = super::tokens();
        let dark = tokens.has_dark_number(self.name) && super::dark();
        V::read(&tokens, self.name, dark)
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

/// What a [`TokenKey`] reads: the value of one kind of token.
pub trait TokenValue: Sized + sealed::Sealed {
    /// The kind it is, as a sentence names it (`"color"`).
    const KIND: &'static str;

    /// The token `name` of `tokens` as this, in the dark appearance
    /// (`dark`) or the light one, if they have it as this kind.
    #[doc(hidden)]
    fn read(tokens: &Tokens, name: &str, dark: bool) -> Option<Self>;

    /// Sets it as the token `name` of `tokens`.
    #[doc(hidden)]
    fn write(self, name: &str, tokens: &mut Tokens);
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::AdaptiveColor {}
    impl Sealed for f32 {}
    impl Sealed for super::Points {}
    impl Sealed for super::SharedString {}
    impl Sealed for std::sync::Arc<str> {}
    impl Sealed for super::AdaptiveShadow {}
    impl Sealed for super::AdaptiveGradient {}
}

impl TokenValue for AdaptiveColor {
    const KIND: &'static str = "color";

    fn read(tokens: &Tokens, name: &str, _: bool) -> Option<Self> {
        tokens.color(name)
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_color(name, self);
    }
}

impl TokenValue for f32 {
    const KIND: &'static str = "number";

    fn read(tokens: &Tokens, name: &str, dark: bool) -> Option<Self> {
        tokens.number_in(name, dark)
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_number(name, self);
    }
}

impl TokenValue for Points {
    const KIND: &'static str = "number";

    fn read(tokens: &Tokens, name: &str, dark: bool) -> Option<Self> {
        tokens.number_in(name, dark).map(Points)
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_number(name, self.0);
    }
}

impl TokenValue for SharedString {
    const KIND: &'static str = "font family";

    fn read(tokens: &Tokens, name: &str, _: bool) -> Option<Self> {
        tokens
            .font(name)
            .map(|family| SharedString::from(family.to_string()))
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_font(name, self.to_string());
    }
}

impl TokenValue for Arc<str> {
    const KIND: &'static str = "font family";

    fn read(tokens: &Tokens, name: &str, _: bool) -> Option<Self> {
        tokens.font(name).cloned()
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_font(name, self);
    }
}

impl TokenValue for AdaptiveShadow {
    const KIND: &'static str = "shadow";

    fn read(tokens: &Tokens, name: &str, _: bool) -> Option<Self> {
        tokens.shadow(name).cloned()
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_shadow(name, self);
    }
}

impl TokenValue for AdaptiveGradient {
    const KIND: &'static str = "gradient";

    fn read(tokens: &Tokens, name: &str, _: bool) -> Option<Self> {
        tokens.gradient(name).cloned()
    }

    fn write(self, name: &str, tokens: &mut Tokens) {
        tokens.set_gradient(name, self);
    }
}

/// Why [`Tokens::get_as`] has no value of the type asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// The tokens have no token of the name.
    Missing,
    /// The token is of another kind than the type asked for.
    WrongKind {
        /// The kind asked for (`"color"`).
        expected: &'static str,
        /// The kind the token is.
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
    /// The token of `key` as kept: a number its light value
    /// ([`TokenKey::get`] reads it in the appearance shown).
    pub fn key<V: TokenValue>(&self, key: TokenKey<V>) -> Option<V> {
        V::read(self, key.name, false)
    }

    /// The token `name` as a `V`, as kept (a number its light value):
    /// [`TokenError::Missing`] when there is none of the name,
    /// [`TokenError::WrongKind`] when it is of another kind.
    pub fn get_as<V: TokenValue>(&self, name: &str) -> Result<V, TokenError> {
        match V::read(self, name, false) {
            Some(value) => Ok(value),
            None => match self.kind(name) {
                Some(kind) => Err(TokenError::WrongKind {
                    expected: V::KIND,
                    found: kind.name(),
                }),
                None => Err(TokenError::Missing),
            },
        }
    }

    /// Sets the token of `key` to `value` (in both appearances, for a
    /// number).
    pub fn set<V: TokenValue>(&mut self, key: TokenKey<V>, value: V) {
        value.write(key.name, self);
    }

    /// These tokens with the token of `key` set to `value`
    /// ([`Tokens::set`]): a change to hand [`super::update_tokens`].
    pub fn with<V: TokenValue>(mut self, key: TokenKey<V>, value: V) -> Self {
        self.set(key, value);
        self
    }
}
