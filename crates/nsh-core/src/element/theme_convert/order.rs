//! The order a theme file writes its members in, which a parsed
//! [`crate::JsonValue`] does not keep: the old reader's modifiers and
//! contexts in file order, and the converter's tokens in the order the file
//! names them.

/// A JSON value reduced to its objects' member names, in order.
pub(super) enum Ordered {
    /// An object's members, in the order written (a name written twice
    /// twice).
    Object(Vec<(String, Ordered)>),
    /// Anything else.
    Other,
}

impl Ordered {
    /// `json`'s members in order (`Other` when it is no JSON object).
    pub(super) fn parse(json: &str) -> Ordered {
        serde_json::from_str(json).unwrap_or(Ordered::Other)
    }

    /// The member `key` (the first written).
    pub(super) fn get(&self, key: &str) -> Option<&Ordered> {
        match self {
            Ordered::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            Ordered::Other => None,
        }
    }

    /// The member at `path`.
    pub(super) fn at(&self, path: &[&str]) -> Option<&Ordered> {
        path.iter().try_fold(self, |value, key| value.get(key))
    }

    /// The member names, in order, each once.
    pub(super) fn keys(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        if let Ordered::Object(members) = self {
            for (k, _) in members {
                if !out.contains(&k.as_str()) {
                    out.push(k);
                }
            }
        }
        out
    }
}

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    out.push((key, map.next_value::<Ordered>()?));
                }
                Ok(Ordered::Object(out))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut s: A,
            ) -> Result<Ordered, A::Error> {
                while s.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(Ordered::Other)
            }
            fn visit_bool<E>(self, _: bool) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_i64<E>(self, _: i64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_u64<E>(self, _: u64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_f64<E>(self, _: f64) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_str<E>(self, _: &str) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Other)
            }
        }
        d.deserialize_any(V)
    }
}
