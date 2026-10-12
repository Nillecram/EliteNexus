//! Duplicate detection. The relay stopped de-duplicating in 2026-03, and several tools upload the
//! same journal event, so identical messages arrive more than once.
//!
//! Key: a 64-bit hash of `$schemaRef` + `message` with object keys sorted (uploaders differ in key
//! order) and without `message.timestamp` (the same data uploaded seconds apart). The header (and
//! with it the uploader) is not part of the key.

use rustc_hash::{FxHashMap, FxHasher};
use serde_json::Value;
use std::collections::VecDeque;
use std::hash::Hasher;
use std::time::{Duration, Instant};

pub(crate) fn message_key(envelope: &Value) -> u64 {
    let mut h = FxHasher::default();
    if let Some(s) = envelope.get("$schemaRef").and_then(Value::as_str) {
        h.write(s.as_bytes());
    }
    h.write_u8(0xff);
    match envelope.get("message") {
        Some(Value::Object(m)) => {
            let mut keys: Vec<&String> = m.keys().filter(|k| k.as_str() != "timestamp").collect();
            keys.sort_unstable();
            h.write_u8(b'{');
            for k in keys {
                h.write(k.as_bytes());
                h.write_u8(0);
                hash_value(&m[k.as_str()], &mut h);
            }
        }
        Some(other) => hash_value(other, &mut h),
        None => {}
    }
    h.finish()
}

fn hash_value(v: &Value, h: &mut FxHasher) {
    match v {
        Value::Null => h.write_u8(b'n'),
        Value::Bool(b) => {
            h.write_u8(b'b');
            h.write_u8(u8::from(*b));
        }
        Value::Number(n) => {
            h.write_u8(b'#');
            if let Some(i) = n.as_i64() {
                h.write_i64(i);
            } else if let Some(u) = n.as_u64() {
                h.write_u64(u);
            } else {
                h.write_u64(n.as_f64().unwrap_or(0.0).to_bits());
            }
        }
        Value::String(s) => {
            h.write_u8(b'"');
            h.write(s.as_bytes());
            h.write_u8(0);
        }
        Value::Array(a) => {
            h.write_u8(b'[');
            h.write_usize(a.len());
            for x in a {
                hash_value(x, h);
            }
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_unstable();
            h.write_u8(b'{');
            h.write_usize(keys.len());
            for k in keys {
                h.write(k.as_bytes());
                h.write_u8(0);
                hash_value(&m[k.as_str()], h);
            }
        }
    }
}

/// Keys seen within a sliding time window.
pub(crate) struct Dedupe {
    window: Duration,
    seen: FxHashMap<u64, Instant>,
    order: VecDeque<(Instant, u64)>,
}

impl Dedupe {
    pub fn new(window: Duration) -> Dedupe {
        Dedupe {
            window,
            seen: FxHashMap::default(),
            order: VecDeque::new(),
        }
    }

    /// True if `key` was seen within the window; otherwise remembers it.
    pub fn is_duplicate(&mut self, key: u64, now: Instant) -> bool {
        while let Some(&(t, k)) = self.order.front() {
            if now.saturating_duration_since(t) <= self.window {
                break;
            }
            self.order.pop_front();
            if self.seen.get(&k) == Some(&t) {
                self.seen.remove(&k);
            }
        }
        if self.seen.contains_key(&key) {
            return true;
        }
        self.seen.insert(key, now);
        self.order.push_back((now, key));
        false
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_ignores_order_timestamp_and_header() {
        let a = json!({"$schemaRef": "s", "header": {"uploaderID": "a"},
                       "message": {"timestamp": "2026-10-12T09:00:00Z", "x": 1, "y": [1, {"b": 2, "a": 1}]}});
        let b = json!({"message": {"y": [1, {"a": 1, "b": 2}], "x": 1, "timestamp": "2026-10-12T09:00:05Z"},
                       "header": {"uploaderID": "b"}, "$schemaRef": "s"});
        assert_eq!(message_key(&a), message_key(&b));
        let c = json!({"$schemaRef": "s", "message": {"x": 2, "y": [1, {"a": 1, "b": 2}]}});
        assert_ne!(message_key(&a), message_key(&c));
        let d = json!({"$schemaRef": "other", "message": {"x": 1, "y": [1, {"a": 1, "b": 2}]}});
        assert_ne!(message_key(&a), message_key(&d));
        // nested timestamps (e.g. fsssignaldiscovered signals) are data, not ignored
        let e = json!({"$schemaRef": "s", "message": {"x": 1, "y": [1, {"a": 1, "b": 2, "timestamp": "t"}]}});
        assert_ne!(message_key(&a), message_key(&e));
    }

    #[test]
    fn window_expires() {
        let mut d = Dedupe::new(Duration::from_secs(600));
        let t0 = Instant::now();
        assert!(!d.is_duplicate(1, t0));
        assert!(d.is_duplicate(1, t0 + Duration::from_secs(300)));
        assert!(!d.is_duplicate(2, t0 + Duration::from_secs(301)));
        assert!(
            !d.is_duplicate(1, t0 + Duration::from_secs(601)),
            "expired after the window"
        );
        assert_eq!(d.len(), 2);
        assert!(!d.is_duplicate(3, t0 + Duration::from_secs(2000)));
        assert_eq!(d.len(), 1, "old keys are evicted");
    }
}
