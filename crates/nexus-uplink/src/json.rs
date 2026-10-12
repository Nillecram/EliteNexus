//! Small JSON helpers shared by the uploaders.

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Map, Value};
use std::io::Write;

/// A JSON object.
pub type Obj = Map<String, Value>;

/// Parses a journal line or companion file as a JSON object.
///
/// This is deliberately strict: the game occasionally writes non-finite floats (`-nan(ind)`),
/// which is invalid JSON. Such lines are skipped by every uploader instead of being "repaired",
/// because a repaired value would be invented data (no data is better than bad data).
pub fn parse_object(text: &str) -> Option<Obj> {
    let text = text.trim_start_matches('\u{feff}').trim();
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}

/// Removes every key ending in `_Localised`, at any depth.
pub fn strip_localised(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| !k.ends_with("_Localised"));
            for (_, x) in m.iter_mut() {
                strip_localised(x);
            }
        }
        Value::Array(a) => {
            for x in a {
                strip_localised(x);
            }
        }
        _ => {}
    }
}

/// Removes every key ending in `_Localised` from an object (recursively).
pub fn strip_localised_obj(o: &mut Obj) {
    o.retain(|k, _| !k.ends_with("_Localised"));
    for (_, x) in o.iter_mut() {
        strip_localised(x);
    }
}

/// True if any key at any depth ends in `_Localised`.
pub fn has_localised(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.iter().any(|(k, x)| k.ends_with("_Localised") || has_localised(x)),
        Value::Array(a) => a.iter().any(has_localised),
        _ => false,
    }
}

/// True if any value at any depth is `null`.
pub fn has_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Object(m) => m.values().any(has_null),
        Value::Array(a) => a.iter().any(has_null),
        _ => false,
    }
}

/// A JSON integer (not a float that happens to be whole: EDDN validates Draft 4 `integer`).
pub fn is_int(v: &Value) -> bool {
    v.is_i64() || v.is_u64()
}

pub fn get_str<'a>(o: &'a Obj, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

/// A string value that is not empty.
pub fn get_nonempty<'a>(o: &'a Obj, k: &str) -> Option<&'a str> {
    get_str(o, k).filter(|s| !s.is_empty())
}

pub fn get_u64(o: &Obj, k: &str) -> Option<u64> {
    o.get(k).and_then(Value::as_u64)
}

pub fn get_i64(o: &Obj, k: &str) -> Option<i64> {
    o.get(k).and_then(Value::as_i64)
}

pub fn get_bool(o: &Obj, k: &str) -> Option<bool> {
    o.get(k).and_then(Value::as_bool)
}

/// A finite number (integer or float).
pub fn get_f64(o: &Obj, k: &str) -> Option<f64> {
    o.get(k).and_then(Value::as_f64).filter(|f| f.is_finite())
}

/// `[x, y, z]` with three finite numbers.
pub fn as_star_pos(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let mut out = [0.0; 3];
    for (i, x) in a.iter().enumerate() {
        out[i] = x.as_f64().filter(|f| f.is_finite())?;
    }
    Some(out)
}

pub fn get_star_pos(o: &Obj, k: &str) -> Option<[f64; 3]> {
    o.get(k).and_then(as_star_pos)
}

pub fn star_pos_value(p: [f64; 3]) -> Value {
    Value::Array(p.iter().map(|&f| Value::from(f)).collect())
}

pub fn get_array<'a>(o: &'a Obj, k: &str) -> Option<&'a Vec<Value>> {
    o.get(k).and_then(Value::as_array)
}

/// Copies the listed keys (when present) from `src` to `dst`.
pub fn copy_keys(src: &Obj, dst: &mut Obj, keys: &[&str]) {
    for k in keys {
        if let Some(v) = src.get(*k) {
            dst.insert((*k).to_string(), v.clone());
        }
    }
}

/// True if the string is an RFC 3339 / ISO 8601 date-time (what the EDDN gateway's format
/// checker accepts for `"format": "date-time"`).
pub fn is_date_time(s: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
}

/// 64-bit FNV-1a. Stable across Rust versions (unlike `DefaultHasher`), used for content
/// fingerprints that are persisted in queue files.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// gzip-compresses `bytes`.
pub fn gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut e = GzEncoder::new(Vec::with_capacity(bytes.len() / 4 + 64), Compression::default());
    e.write_all(bytes)?;
    e.finish()
}

/// Decompresses a gzip stream (tests, simulated services).
pub fn gunzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut out)?;
    Ok(out)
}

/// Decodes an `application/x-www-form-urlencoded` body (tests, simulated services).
pub fn form_urldecode(body: &str) -> Vec<(String, String)> {
    fn dec(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'+' => out.push(b' '),
                b'%' if i + 2 < b.len() => {
                    let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                    match hex {
                        Some(v) => {
                            out.push(v);
                            i += 2;
                        }
                        None => out.push(b'%'),
                    }
                }
                c => out.push(c),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    body.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (dec(k), dec(v)),
            None => (dec(p), String::new()),
        })
        .collect()
}

/// `application/x-www-form-urlencoded` encoding of `pairs` (UTF-8).
pub fn form_urlencode(pairs: &[(&str, &str)]) -> String {
    fn enc(out: &mut String, s: &str) {
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => out.push(b as char),
                b' ' => out.push('+'),
                _ => {
                    const HEX: &[u8; 16] = b"0123456789ABCDEF";
                    out.push('%');
                    out.push(HEX[usize::from(b >> 4)] as char);
                    out.push(HEX[usize::from(b & 0x0f)] as char);
                }
            }
        }
    }
    let mut out = String::new();
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        enc(&mut out, k);
        out.push('=');
        enc(&mut out, v);
    }
    out
}

/// Semantic "major version" of a game version string (`"4.4.1.1"` → 4).
pub fn game_major_version(v: &str) -> Option<u32> {
    v.trim().split('.').next()?.trim().parse().ok()
}

/// EDMC's rule: a client is alpha/beta if its `gameversion` says so.
pub fn is_beta_version(v: &str) -> bool {
    let l = v.to_ascii_lowercase();
    l.contains("alpha") || l.contains("beta")
}

/// Normalises a journal symbol such as `$gold_name;` or `$MARKET_category_metals;` by removing
/// the `$`, a trailing `;`, and a trailing `_name`.
pub fn strip_symbol(s: &str) -> &str {
    let s = s.strip_prefix('$').unwrap_or(s);
    let s = s.strip_suffix(';').unwrap_or(s);
    s.strip_suffix("_name").or_else(|| s.strip_suffix("_Name")).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_localised_at_any_depth() {
        let mut v = json!({"a_Localised": 1, "b": [{"c_Localised": 2, "d": {"e_Localised": 3, "f": 4}}]});
        assert!(has_localised(&v));
        strip_localised(&mut v);
        assert_eq!(v, json!({"b": [{"d": {"f": 4}}]}));
        assert!(!has_localised(&v));
    }

    #[test]
    fn strict_parse_rejects_nan() {
        assert!(parse_object(r#"{"a":1}"#).is_some());
        assert!(parse_object("\u{feff}{\"a\":1}").is_some());
        assert!(parse_object(r#"{"Progress":-nan(ind)}"#).is_none());
        assert!(parse_object("[1]").is_none());
    }

    #[test]
    fn form_encoding() {
        assert_eq!(form_urlencode(&[("a b", "c&d=é"), ("x", "")]), "a+b=c%26d%3D%C3%A9&x=");
        assert_eq!(form_urlencode(&[("fromGameBuild", "r332841/r0 ")]), "fromGameBuild=r332841%2Fr0+");
        let pairs = [("message", "[{\"a\":\"b c%\"}]"), ("k", "é/ ")];
        let decoded = form_urldecode(&form_urlencode(&pairs));
        assert_eq!(decoded[0], ("message".to_string(), "[{\"a\":\"b c%\"}]".to_string()));
        assert_eq!(decoded[1], ("k".to_string(), "é/ ".to_string()));
        assert_eq!(gunzip(&gzip(b"hello").unwrap()).unwrap(), b"hello");
    }

    #[test]
    fn versions() {
        assert_eq!(game_major_version("4.4.1.1"), Some(4));
        assert_eq!(game_major_version("3.8.0.1400"), Some(3));
        assert_eq!(game_major_version(""), None);
        assert!(is_beta_version("4.0.0.1450 Beta"));
        assert!(!is_beta_version("4.4.1.1"));
        assert_eq!(strip_symbol("$gold_name;"), "gold");
        assert_eq!(strip_symbol("$MARKET_category_nonmarketable;"), "MARKET_category_nonmarketable");
    }

    #[test]
    fn date_time_format() {
        assert!(is_date_time("2026-10-12T09:23:11Z"));
        assert!(is_date_time("2026-10-12T09:23:11.123Z"));
        assert!(!is_date_time("2026-10-12 09:23:11"));
        assert!(!is_date_time(""));
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
