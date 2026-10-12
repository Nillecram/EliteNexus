//! Small shared helpers: coordinates, timestamps, website links, form encoding.

use serde::{Deserialize, Serialize};

/// Galactic coordinates in light-years, Sol at the origin.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Coords {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Coords {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Straight-line distance in ly.
    pub fn distance_to(&self, o: &Coords) -> f64 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2) + (self.z - o.z).powi(2)).sqrt()
    }

    /// `Some` only when all three components are present.
    pub fn from_parts(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Option<Self> {
        Some(Self {
            x: x?,
            y: y?,
            z: z?,
        })
    }
}

/// Parse a Spansh timestamp to unix seconds (UTC).
///
/// Accepts `2026-10-12T07:15:43Z` (records), `2026-10-12T09:17:41` (job envelopes, which are UTC
/// without a suffix), a space instead of `T`, fractional seconds and `±hh:mm` offsets.
pub fn parse_timestamp(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || !(b[10] == b'T' || b[10] == b't' || b[10] == b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let digits = |from: usize, to: usize| -> Option<i64> {
        let part = &b[from..to];
        part.iter().all(u8::is_ascii_digit).then(|| {
            part.iter()
                .fold(0i64, |acc, d| acc * 10 + i64::from(d - b'0'))
        })
    };
    let (year, month, day) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let (hour, minute, second) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    // Optional fraction, then `Z`, nothing, or an offset.
    let mut rest = &s[19..];
    if let Some(frac) = rest.strip_prefix('.') {
        let n = frac.bytes().take_while(u8::is_ascii_digit).count();
        rest = &frac[n..];
    }
    let offset = match rest {
        "" | "Z" | "z" => 0,
        _ => {
            let (sign, hm) = match rest.as_bytes()[0] {
                b'+' => (1, &rest[1..]),
                b'-' => (-1, &rest[1..]),
                _ => return None,
            };
            let hm = hm.replace(':', "");
            if hm.len() != 4 || !hm.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let h: i64 = hm[..2].parse().ok()?;
            let m: i64 = hm[2..].parse().ok()?;
            sign * (h * 3600 + m * 60)
        }
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Base of the Spansh website (for "Open in Spansh" links).
pub const WEBSITE: &str = "https://spansh.co.uk";

/// Website page of a system.
pub fn system_url(id64: u64) -> String {
    format!("{WEBSITE}/system/{id64}")
}

/// Website page of a body.
pub fn body_url(id64: u64) -> String {
    format!("{WEBSITE}/body/{id64}")
}

/// Website page of a station / carrier.
pub fn station_url(market_id: u64) -> String {
    format!("{WEBSITE}/station/{market_id}")
}

// ---------------------------------------------------------------------------------------------
// form encoding helpers (route planners)

/// Form body: one `(key, value)` pair per value, arrays as repeated keys (`via=A&via=B`).
pub(crate) type Form = Vec<(&'static str, String)>;

pub(crate) fn flag(b: bool) -> String {
    if b { "1" } else { "0" }.to_owned()
}

/// Shortest round-trip decimal (`1050`, `0.012`, `52.37`).
pub(crate) fn num(f: f64) -> String {
    format!("{f}")
}

/// A JSON number that stays an integer when it is one (`2`, not `2.0`).
pub(crate) fn json_num(f: f64) -> serde_json::Value {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9_007_199_254_740_992.0 {
        serde_json::Value::from(f as i64)
    } else {
        serde_json::Value::from(f)
    }
}

pub(crate) fn require(cond: bool, msg: &str) -> crate::Result<()> {
    if cond {
        Ok(())
    } else {
        Err(crate::Error::InvalidRequest(msg.to_owned()))
    }
}

pub(crate) fn require_name(name: &str, what: &str) -> crate::Result<()> {
    require(
        !name.trim().is_empty(),
        &format!("{what} must not be empty"),
    )
}

pub(crate) fn require_positive(v: f64, what: &str) -> crate::Result<()> {
    require(
        v.is_finite() && v > 0.0,
        &format!("{what} must be a positive number (got {v})"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(parse_timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_timestamp("2026-10-12T07:15:43Z"), Some(1_791_789_343));
        // Job envelopes have no suffix and are UTC.
        assert_eq!(parse_timestamp("2026-10-12T09:17:41"), Some(1_791_796_661));
        assert_eq!(
            parse_timestamp("2026-10-12 09:17:41.123456"),
            Some(1_791_796_661)
        );
        assert_eq!(
            parse_timestamp("2026-10-12T11:17:41+02:00"),
            Some(1_791_796_661)
        );
        assert_eq!(parse_timestamp("2000-02-29T12:00:00Z"), Some(951_825_600));
        assert_eq!(parse_timestamp("garbage"), None);
        assert_eq!(parse_timestamp("2026-13-12T09:17:41Z"), None);
        assert_eq!(parse_timestamp("2026-10-12T09:17:41Q"), None);
    }

    #[test]
    fn numbers() {
        assert_eq!(num(1050.0), "1050");
        assert_eq!(num(0.012), "0.012");
        assert_eq!(json_num(2.0), serde_json::json!(2));
        assert_eq!(json_num(2.5), serde_json::json!(2.5));
    }
}
