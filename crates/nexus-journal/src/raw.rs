//! Cheap pre-scan of journal lines.
//!
//! The game always writes `timestamp` then `event` as the first two keys, so both can be pulled out
//! with two substring searches. That lets us route every line (and skip the ones nobody cares
//! about) without running a JSON parser over it.

use crate::time::Timestamp;
use std::sync::Arc;

/// Returns the raw string value of `"key": "value"`, searching from the start of the line.
fn find_str_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(key)? + key.len();
    let rest = line[at..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Extracts `(timestamp, event name)` from a journal line without parsing the JSON.
pub fn scan_line(line: &str) -> Option<(Timestamp, &str)> {
    let ts = find_str_value(line, "\"timestamp\"")?;
    let ev = find_str_value(line, "\"event\"")?;
    Some((Timestamp::parse(ts)?, ev))
}

/// The game occasionally writes non-finite floats as bare tokens (`"Progress":-nan(ind)`,
/// `inf`, `1.#INF`, `-1.#IND`), which is invalid JSON. Returns a copy with every such token
/// outside of string literals replaced by `0`, or `None` if nothing needed fixing.
pub fn sanitize_non_finite(line: &str) -> Option<String> {
    const TOKENS: [&str; 10] =
        ["-nan(ind)", "nan(ind)", "-1.#QNAN", "1.#QNAN", "-1.#IND", "1.#IND", "-1.#INF", "1.#INF", "-nan", "-inf"];
    const SHORT: [&str; 3] = ["nan", "inf", "infinity"];
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut changed = false;
    let mut in_str = false;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c as char);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1] as char);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_str = true;
            out.push('"');
            i += 1;
            continue;
        }
        let rest = &line[i..];
        let lower = rest.get(..rest.len().min(10)).unwrap_or("").to_ascii_lowercase();
        let hit = TOKENS
            .iter()
            .find(|t| lower.starts_with(&t.to_ascii_lowercase()))
            .map(|t| t.len())
            .or_else(|| {
                SHORT.iter().rev().find(|t| lower.starts_with(*t)).map(|t| t.len()).filter(|&n| {
                    // a bare word, not part of a longer identifier
                    !rest.as_bytes().get(n).is_some_and(|ch| ch.is_ascii_alphanumeric())
                })
            });
        if let Some(n) = hit {
            out.push('0');
            changed = true;
            i += n;
            continue;
        }
        // Copy one UTF-8 char outside strings (all JSON structure is ASCII anyway).
        let ch = rest.chars().next().unwrap_or('\u{fffd}');
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

/// Parses a typed event, retrying once with [`sanitize_non_finite`] if the game wrote NaN/Inf.
pub fn parse_event_lenient(name: &str, line: &str) -> serde_json::Result<crate::events::Event> {
    match crate::events::Event::parse(name, line) {
        Ok(e) => Ok(e),
        Err(err) => match sanitize_non_finite(line) {
            Some(fixed) => crate::events::Event::parse(name, &fixed),
            None => Err(err),
        },
    }
}

/// Where a journal line came from (for the logbook and for re-reading raw JSON on demand).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineSource {
    /// Index into the journal file list held by the reader.
    pub file: u32,
    /// Byte offset of the line within the file.
    pub offset: u64,
}

/// An owned journal line with its pre-scanned header. Cheap to clone (the text is shared).
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub ts: Timestamp,
    pub name: Arc<str>,
    pub line: Arc<str>,
    pub source: LineSource,
}

impl RawEvent {
    pub fn new(line: &str, source: LineSource) -> Option<Self> {
        let (ts, name) = scan_line(line)?;
        Some(RawEvent { ts, name: Arc::from(name), line: Arc::from(line), source })
    }

    /// Parses the full JSON object (used by uploaders that forward the event with edits).
    pub fn json(&self) -> serde_json::Result<serde_json::Map<String, serde_json::Value>> {
        serde_json::from_str(&self.line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_header() {
        let l = r#"{ "timestamp":"2026-09-05T00:13:29Z", "event":"Commander", "FID":"F1", "Name":"X" }"#;
        let (t, e) = scan_line(l).unwrap();
        assert_eq!(e, "Commander");
        assert_eq!(t.to_iso(), "2026-09-05T00:13:29Z");
        assert!(scan_line("{}").is_none());
    }

    #[test]
    fn sanitizes_nan_outside_strings_only() {
        let l = r#"{ "timestamp":"2026-10-12T06:25:17Z", "event":"FSSDiscoveryScan", "Progress":-nan(ind), "BodyCount":0, "SystemName":"Nanomam inf", "X":inf, "Y":1.#IND }"#;
        let fixed = sanitize_non_finite(l).unwrap();
        assert!(fixed.contains(r#""Progress":0,"#));
        assert!(fixed.contains(r#""SystemName":"Nanomam inf""#));
        assert!(fixed.contains(r#""X":0,"#));
        assert!(fixed.contains(r#""Y":0 }"#));
        assert!(sanitize_non_finite(r#"{"a":"nan","b":1.5}"#).is_none());
        let ev = parse_event_lenient("FSSDiscoveryScan", l).unwrap();
        assert!(matches!(ev, crate::events::Event::FSSDiscoveryScan(_)));
    }
}
