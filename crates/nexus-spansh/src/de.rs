//! Lenient serde helpers.
//!
//! Spansh echoes planner parameters back as strings (`"range": "50"`), uses `null` freely, and
//! spells the same concept as `["Dock"]` on one endpoint and `[{"name": "Dock"}]` on another.
//! These helpers coerce instead of failing, so one odd field never sinks a whole response.
//! Scalars of the wrong shape (arrays/objects where a number is expected) become `None`.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

// ---------------------------------------------------------------------------------------------
// numbers

#[derive(Clone, Copy)]
enum Num {
    U(u64),
    I(i64),
    F(f64),
}

impl Num {
    fn f64(self) -> f64 {
        match self {
            Num::U(u) => u as f64,
            Num::I(i) => i as f64,
            Num::F(f) => f,
        }
    }

    fn u64(self) -> Option<u64> {
        match self {
            Num::U(u) => Some(u),
            Num::I(i) => u64::try_from(i).ok(),
            // `as` saturates; reject what cannot be represented instead.
            Num::F(f) if (0.0..18_446_744_073_709_551_616.0).contains(&f) => Some(f.round() as u64),
            Num::F(_) => None,
        }
    }

    fn i64(self) -> Option<i64> {
        match self {
            Num::U(u) => i64::try_from(u).ok(),
            Num::I(i) => Some(i),
            Num::F(f) if f.is_finite() && f.abs() < 9_223_372_036_854_775_808.0 => {
                Some(f.round() as i64)
            }
            Num::F(_) => None,
        }
    }
}

fn parse_num(s: &str) -> Option<Num> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(u) = s.parse::<u64>() {
        return Some(Num::U(u));
    }
    if let Ok(i) = s.parse::<i64>() {
        return Some(Num::I(i));
    }
    s.parse::<f64>().ok().filter(|f| f.is_finite()).map(Num::F)
}

struct NumVisitor;

impl<'de> Visitor<'de> for NumVisitor {
    type Value = Option<Num>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a number, a numeric string or null")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Some(Num::U(u64::from(v))))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Some(if v >= 0 {
            Num::U(v.unsigned_abs())
        } else {
            Num::I(v)
        }))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Some(Num::U(v)))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(v.is_finite().then_some(Num::F(v)))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(parse_num(v))
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

pub(crate) fn opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(d.deserialize_any(NumVisitor)?.map(Num::f64))
}

pub(crate) fn f64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(opt_f64(d)?.unwrap_or(0.0))
}

pub(crate) fn opt_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    Ok(d.deserialize_any(NumVisitor)?.and_then(Num::u64))
}

pub(crate) fn u64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(opt_u64(d)?.unwrap_or(0))
}

pub(crate) fn opt_u32<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    Ok(opt_u64(d)?.and_then(|v| u32::try_from(v).ok()))
}

pub(crate) fn u32_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(opt_u32(d)?.unwrap_or(0))
}

pub(crate) fn opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(d.deserialize_any(NumVisitor)?.and_then(Num::i64))
}

pub(crate) fn i64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(opt_i64(d)?.unwrap_or(0))
}

// ---------------------------------------------------------------------------------------------
// booleans

struct BoolVisitor;

impl<'de> Visitor<'de> for BoolVisitor {
    type Value = Option<bool>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a boolean, 0/1, \"true\"/\"false\" or null")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Some(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Some(v != 0))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Some(v != 0))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(Some(v != 0.0))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => Some(true),
            "0" | "false" | "no" | "n" | "" => Some(false),
            _ => None,
        })
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

pub(crate) fn opt_bool<'de, D: Deserializer<'de>>(d: D) -> Result<Option<bool>, D::Error> {
    d.deserialize_any(BoolVisitor)
}

pub(crate) fn bool_or_false<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(opt_bool(d)?.unwrap_or(false))
}

// ---------------------------------------------------------------------------------------------
// strings

struct StrVisitor;

impl<'de> Visitor<'de> for StrVisitor {
    type Value = Option<String>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string, a number or null")
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Some(v.to_owned()))
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
        Ok(Some(v))
    }
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

pub(crate) fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    d.deserialize_any(StrVisitor)
}

pub(crate) fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(opt_string(d)?.unwrap_or_default())
}

/// Item of a name list: `"Dock"`, `{"name": "Dock"}`, `{"state": "Boom", "trend": 0}` or
/// `{"commodity": "Gold"}`.
fn item_name(v: Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(mut m) => {
            ["name", "state", "commodity", "value"]
                .iter()
                .find_map(|k| match m.remove(*k) {
                    Some(Value::String(s)) => Some(s),
                    _ => None,
                })
        }
        _ => None,
    }
}

/// `["A", "B"]`, `[{"name": "A"}, …]`, a single `"A"` or `null` → `vec!["A", "B"]`.
pub(crate) fn name_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(items) => items.into_iter().filter_map(item_name).collect(),
        Value::Null => Vec::new(),
        other => item_name(other).into_iter().collect(),
    })
}

/// `null` → `T::default()`. (`#[serde(default)]` only covers *missing* fields.)
pub(crate) fn nullable<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// A list whose undecodable items are skipped (with a warning) instead of failing the response.
/// Used for search results, where one odd record must not hide the other 499.
pub(crate) fn lenient_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let raw = match Value::deserialize(d)? {
        Value::Array(items) => items,
        _ => return Ok(Vec::new()),
    };
    let mut out = Vec::with_capacity(raw.len());
    for (i, item) in raw.into_iter().enumerate() {
        match serde_path_to_error::deserialize::<_, T>(item) {
            Ok(v) => out.push(v),
            Err(e) => {
                tracing::warn!(index = i, path = %e.path(), error = %e.inner(), "skipping undecodable Spansh record")
            }
        }
    }
    Ok(out)
}

/// `{"Iron": 23.5, "Nickel": 17.7}` (non-numeric values dropped, `null` → empty).
pub(crate) fn f64_map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, f64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Object(m) => m
            .into_iter()
            .filter_map(|(k, v)| match v {
                Value::Number(n) => n.as_f64().map(|f| (k, f)),
                Value::String(s) => s.trim().parse::<f64>().ok().map(|f| (k, f)),
                _ => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    })
}

/// `{"$SAA_SignalType_Biological;": 1}` → counts (non-integer values dropped, `null` → empty).
pub(crate) fn u32_map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, u32>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Object(m) => m
            .into_iter()
            .filter_map(|(k, v)| {
                let n = match &v {
                    Value::Number(n) => n
                        .as_u64()
                        .or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f.round() as u64)),
                    Value::String(s) => s.trim().parse::<u64>().ok(),
                    _ => None,
                }?;
                u32::try_from(n).ok().map(|n| (k, n))
            })
            .collect(),
        _ => BTreeMap::new(),
    })
}

/// `{"distanceToArrival": "2026-10-12T03:39:21Z"}` (non-string values dropped, `null` → empty).
pub(crate) fn string_map<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Object(m) => m
            .into_iter()
            .filter_map(|(k, v)| match v {
                Value::String(s) => Some((k, s)),
                _ => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, Deserialize)]
    #[serde(default)]
    struct T {
        #[serde(deserialize_with = "opt_f64")]
        f: Option<f64>,
        #[serde(deserialize_with = "u64_or_zero")]
        u: u64,
        #[serde(deserialize_with = "opt_i64")]
        i: Option<i64>,
        #[serde(deserialize_with = "bool_or_false")]
        b: bool,
        #[serde(deserialize_with = "opt_string")]
        s: Option<String>,
        #[serde(deserialize_with = "name_list")]
        names: Vec<String>,
        #[serde(deserialize_with = "nullable")]
        v: Vec<u32>,
    }

    #[test]
    fn coerces_scalars() {
        let t: T = serde_json::from_str(
            r#"{"f":"50","u":"128016640","i":-5.4,"b":1,"s":12,"names":[{"name":"Dock"},"Market",{"state":"Boom","trend":0}],"v":null}"#,
        )
        .expect("lenient parse");
        assert_eq!(t.f, Some(50.0));
        assert_eq!(t.u, 128_016_640);
        assert_eq!(t.i, Some(-5));
        assert!(t.b);
        assert_eq!(t.s.as_deref(), Some("12"));
        assert_eq!(t.names, ["Dock", "Market", "Boom"]);
        assert!(t.v.is_empty());
    }

    #[test]
    fn odd_shapes_become_none() {
        let t: T =
            serde_json::from_str(r#"{"f":[1,2],"u":{"a":1},"b":"maybe","s":null,"names":null}"#)
                .expect("lenient parse");
        assert_eq!(t.f, None);
        assert_eq!(t.u, 0);
        assert!(!t.b);
        assert_eq!(t.s, None);
        assert!(t.names.is_empty());
    }

    #[test]
    fn big_ids_survive() {
        let t: T = serde_json::from_str(r#"{"u":2017616565339460314}"#).expect("parse");
        assert_eq!(t.u, 2_017_616_565_339_460_314);
        let t: T = serde_json::from_str(r#"{"u":"2017616565339460314"}"#).expect("parse");
        assert_eq!(t.u, 2_017_616_565_339_460_314);
    }
}
