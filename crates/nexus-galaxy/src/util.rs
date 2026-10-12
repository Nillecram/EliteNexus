//! Tolerant accessors for untrusted JSON: a missing or mistyped field is `None`, never a panic.

use serde_json::Value;

pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn get_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

/// Non-empty, trimmed string.
pub(crate) fn get_text(v: &Value, key: &str) -> Option<String> {
    let s = get_str(v, key)?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Integer field; accepts floats (truncated) and numeric strings.
pub(crate) fn get_i64(v: &Value, key: &str) -> Option<i64> {
    as_i64(v.get(key)?)
}

pub(crate) fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|u| u as i64))
            .or_else(|| {
                let f = n.as_f64()?;
                f.is_finite().then_some(f as i64)
            }),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Unsigned id (system address, market id); rejects negatives and zero.
pub(crate) fn get_id(v: &Value, key: &str) -> Option<u64> {
    let n = get_i64(v, key)?;
    (n > 0).then_some(n as u64)
}

pub(crate) fn get_f64(v: &Value, key: &str) -> Option<f64> {
    let f = match v.get(key)? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    f.is_finite().then_some(f)
}

pub(crate) fn get_bool(v: &Value, key: &str) -> Option<bool> {
    v.get(key)?.as_bool()
}

/// `[x, y, z]` with three finite numbers.
pub(crate) fn get_pos(v: &Value, key: &str) -> Option<[f64; 3]> {
    let a = v.get(key)?.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let mut out = [0.0; 3];
    for (o, x) in out.iter_mut().zip(a) {
        let f = x.as_f64()?;
        if !f.is_finite() {
            return None;
        }
        *o = f;
    }
    Some(out)
}

/// ISO-8601 timestamp field -> unix seconds.
pub(crate) fn get_ts(v: &Value, key: &str) -> Option<i64> {
    parse_ts(get_str(v, key)?)
}

pub(crate) fn parse_ts(s: &str) -> Option<i64> {
    nexus_journal::Timestamp::parse(s.trim()).map(|t| t.0)
}

/// Array of strings (non-strings skipped).
pub(crate) fn get_strings(v: &Value, key: &str) -> Option<Vec<String>> {
    let a = v.get(key)?.as_array()?;
    Some(
        a.iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

pub(crate) fn dist2(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

pub(crate) fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    dist2(a, b).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tolerant_accessors() {
        let v = json!({"a": 5, "b": 5.9, "c": "7", "d": -1, "e": [1, 2.5, -3], "f": [1, 2], "g": "2026-10-12T09:23:11Z", "h": null});
        assert_eq!(get_i64(&v, "a"), Some(5));
        assert_eq!(get_i64(&v, "b"), Some(5));
        assert_eq!(get_i64(&v, "c"), Some(7));
        assert_eq!(get_id(&v, "d"), None);
        assert_eq!(get_pos(&v, "e"), Some([1.0, 2.5, -3.0]));
        assert_eq!(get_pos(&v, "f"), None);
        assert_eq!(get_ts(&v, "g"), Some(1_791_796_991));
        assert_eq!(get_text(&v, "h"), None);
        assert_eq!(get_f64(&v, "missing"), None);
    }
}
