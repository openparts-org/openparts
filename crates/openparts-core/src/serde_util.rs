//! Shared serde helpers.
//!
//! Testing and Quality Specification section 4 requires that duplicate YAML
//! mapping keys are rejected rather than silently overwritten (the default
//! serde behaviour for `BTreeMap`/`HashMap` fields is last-write-wins).
//! [`deserialize_no_dup_map`] is a drop-in `deserialize_with` for any
//! `BTreeMap<String, V>` field that must reject duplicate keys.

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

pub fn deserialize_no_dup_map<'de, D, V>(deserializer: D) -> Result<BTreeMap<String, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct NoDupVisitor<V>(PhantomData<V>);

    impl<'de, V: Deserialize<'de>> Visitor<'de> for NoDupVisitor<V> {
        type Value = BTreeMap<String, V>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "a YAML mapping with unique string keys")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut out = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, V>()? {
                if out.insert(key.clone(), value).is_some() {
                    return Err(de::Error::custom(format!(
                        "duplicate key \"{key}\" in mapping"
                    )));
                }
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(NoDupVisitor(PhantomData))
}

#[cfg(test)]
mod tests {
    // Exercised via `serde::de::value`, not a real format crate, so this
    // module stays true to the "openparts-core depends on no data format"
    // rule (spec section 7.1) even in tests.
    use super::*;
    use serde::de::value::{Error as ValueError, MapDeserializer};

    fn run(pairs: Vec<(&str, i32)>) -> Result<BTreeMap<String, i32>, ValueError> {
        let owned: Vec<(String, i32)> = pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let deserializer: MapDeserializer<_, ValueError> = MapDeserializer::new(owned.into_iter());
        deserialize_no_dup_map(deserializer)
    }

    #[test]
    fn accepts_unique_keys() {
        let result = run(vec![("a", 1), ("b", 2)]).unwrap();
        assert_eq!(result.get("a"), Some(&1));
        assert_eq!(result.get("b"), Some(&2));
    }

    #[test]
    fn rejects_duplicate_keys() {
        let err = run(vec![("a", 1), ("a", 2)]).unwrap_err();
        assert!(err.to_string().contains("duplicate key"));
    }
}
